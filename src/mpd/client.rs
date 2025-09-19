use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpStream},
    os::unix::net::UnixStream,
};

use anyhow::Result;
use log::debug;

use super::{
    commands::mpd_config::MpdConfig,
    errors::MpdError,
    proto_client::SocketClient,
    version::Version,
};
use crate::{
    config::{MpdAddress, address::MpdPassword},
    mpd::{
        errors::{ErrorCode, MpdFailureResponse},
        mpd_client::MpdClient,
    },
    shared::macros::status_warn,
};

type MpdResult<T> = Result<T, MpdError>;

const MIN_SUPPORTED_VERSION: Version = Version { major: 0, minor: 23, patch: 5 };

pub struct Client<'name> {
    name: &'name str,
    rx: BufReader<TcpOrUnixStream>,
    pub stream: TcpOrUnixStream,
    addr: MpdAddress,
    password: Option<MpdPassword>,
    pub version: Version,
    pub config: Option<MpdConfig>,
    pub supported_commands: HashSet<String>,
    partition: Option<String>,
    autocreate_partition: bool,
}

impl std::fmt::Debug for Client<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client {{ name: {:?}, addr: {:?} }}", self.name, self.addr)
    }
}

pub enum TcpOrUnixStream {
    Unix(UnixStream),
    Tcp(TcpStream),
}

impl TcpOrUnixStream {
    fn set_write_timeout(&mut self, duration: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            TcpOrUnixStream::Unix(s) => {
                s.set_write_timeout(duration)?;
            }
            TcpOrUnixStream::Tcp(s) => {
                s.set_write_timeout(duration)?;
            }
        }
        Ok(())
    }

    fn set_read_timeout(&mut self, duration: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            TcpOrUnixStream::Unix(s) => {
                s.set_read_timeout(duration)?;
            }
            TcpOrUnixStream::Tcp(s) => {
                s.set_read_timeout(duration)?;
            }
        }
        Ok(())
    }

    pub fn try_clone(&self) -> std::io::Result<Self> {
        Ok(match self {
            TcpOrUnixStream::Unix(s) => TcpOrUnixStream::Unix(s.try_clone()?),
            TcpOrUnixStream::Tcp(s) => TcpOrUnixStream::Tcp(s.try_clone()?),
        })
    }

    pub fn shutdown_both(&mut self) -> std::io::Result<()> {
        match self {
            TcpOrUnixStream::Unix(s) => s.shutdown(Shutdown::Both),
            TcpOrUnixStream::Tcp(s) => s.shutdown(Shutdown::Both),
        }
    }
}

impl std::io::Read for TcpOrUnixStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            TcpOrUnixStream::Unix(s) => s.read(buf),
            TcpOrUnixStream::Tcp(s) => s.read(buf),
        }
    }
}

impl std::io::Write for TcpOrUnixStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            TcpOrUnixStream::Unix(s) => s.write(buf),
            TcpOrUnixStream::Tcp(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            TcpOrUnixStream::Unix(s) => s.flush(),
            TcpOrUnixStream::Tcp(s) => s.flush(),
        }
    }
}

#[allow(dead_code)]
impl<'name> Client<'name> {
    pub fn init(
        addr: MpdAddress,
        password: Option<MpdPassword>,
        name: &'name str,
        partition: Option<String>,
        autocreate_partition: bool,
    ) -> MpdResult<Client<'name>> {
        let mut stream = match addr {
            MpdAddress::IpAndPort(ref addr) => TcpOrUnixStream::Tcp(TcpStream::connect(addr)?),
            MpdAddress::SocketPath(ref addr) => TcpOrUnixStream::Unix(UnixStream::connect(addr)?),
        };
        stream.set_write_timeout(None)?;
        stream.set_read_timeout(None)?;
        let mut rx = BufReader::new(stream.try_clone()?);

        let mut buf = String::new();
        rx.read_line(&mut buf)?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        }
        let Some(version): Option<Version> =
            buf.strip_prefix("OK MPD ").and_then(|v| v.parse().ok())
        else {
            return Err(MpdError::Generic(format!(
                "Handshake validation failed. Cannot parse version from '{buf}'"
            )));
        };

        debug!(name, addr:?, version = version.to_string().as_str(), handshake = buf.trim(); "MPD client initialized");

        if version < MIN_SUPPORTED_VERSION {
            status_warn!(
                "MPD version '{version}' is lower than supported. Minimum supported protocol version is '{MIN_SUPPORTED_VERSION}'. Some features may work incorrectly."
            );
        }

        let mut client = Self {
            name,
            rx,
            stream,
            addr,
            password,
            version,
            partition,
            autocreate_partition,
            config: None,
            supported_commands: HashSet::new(),
        };

        if let Some(MpdPassword(ref password)) = client.password.clone() {
            debug!("Used password auth to MPD");
            client.password(password)?;
        }

        if let Some(partition) = client.partition.clone() {
            debug!(partition = partition.as_str(); "Using partition");
            match client.switch_to_partition(&partition) {
                Ok(()) => {}
                Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. }))
                    if autocreate_partition =>
                {
                    client.new_partition(&partition)?;
                    client.switch_to_partition(&partition)?;
                }
                err @ Err(_) => err?,
            }
        }

        // 2^18 seems to be max limit supported by MPD and higher values dont
        // have any effect
        client.binary_limit(2u64.pow(18))?;
        client.supported_commands = client.commands()?.0.into_iter().collect();

        Ok(client)
    }

    pub fn reconnect(&mut self) -> MpdResult<&Client<'_>> {
        debug!(name = self.name, addr:? = self.addr; "trying to reconnect");
        let mut stream = match &self.addr {
            MpdAddress::IpAndPort(addr) => TcpOrUnixStream::Tcp(TcpStream::connect(addr)?),
            MpdAddress::SocketPath(addr) => TcpOrUnixStream::Unix(UnixStream::connect(addr)?),
        };
        stream.set_write_timeout(None)?;
        stream.set_read_timeout(None)?;
        let mut rx = BufReader::new(stream.try_clone()?);

        let mut buf = String::new();
        rx.read_line(&mut buf)?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        }

        let Some(version): Option<Version> =
            buf.strip_prefix("OK MPD ").and_then(|v| v.parse().ok())
        else {
            return Err(MpdError::Generic(format!(
                "Handshake validation failed. Cannot parse version from '{buf}'"
            )));
        };

        self.rx = rx;
        self.stream = stream;
        self.version = version;
        self.config = None;

        debug!(name = self.name, addr:? = self.addr, handshake = buf.trim(), version = version.to_string().as_str(); "MPD client initialized");

        if let Some(MpdPassword(password)) = &self.password.clone() {
            debug!("Used password auth to MPD");
            self.password(password)?;
        }

        if let Some(partition) = self.partition.clone() {
            debug!(partition = partition.as_str(); "Using partition");
            match self.switch_to_partition(&partition) {
                Ok(()) => {}
                Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. }))
                    if self.autocreate_partition =>
                {
                    self.new_partition(&partition)?;
                    self.switch_to_partition(&partition)?;
                }
                err @ Err(_) => err?,
            }
        }

        self.supported_commands = self.commands()?.0.into_iter().collect();

        self.binary_limit(1024 * 1024 * 5)?;

        Ok(self)
    }

    pub fn set_read_timeout(
        &mut self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    pub fn set_write_timeout(
        &mut self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<()> {
        self.stream.set_write_timeout(timeout)
    }

    fn clear_read_buf(&mut self) -> Result<()> {
        log::trace!("Reinitialized read buffer");
        self.rx = BufReader::new(self.stream.try_clone()?);
        Ok(())
    }
}

impl SocketClient for Client<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        Write::write_all(&mut self.stream, bytes)
    }

    fn read(&mut self) -> &mut impl BufRead {
        &mut self.rx
    }

    fn clear_read_buf(&mut self) -> Result<()> {
        self.clear_read_buf()
    }

    fn version(&self) -> Version {
        self.version
    }
}
