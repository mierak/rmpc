use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};

use crate::utils::macros::status_warn;

use super::{
    errors::MpdError,
    proto_client::{ProtoClient, SocketClient},
    version::Version,
};
use anyhow::Result;
use log::debug;

type MpdResult<T> = Result<T, MpdError>;

const MAX_SUPPORTED_VERSION: Version = Version {
    major: 0,
    minor: 23,
    patch: 5,
};

pub struct Client<'name> {
    name: &'name str,
    rx: BufReader<TcpStream>,
    stream: TcpStream,
    reconnect: bool,
    addr: &'static str,
    pub version: Version,
}

impl std::fmt::Debug for Client<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Client {{ name: {:?}, recconect: {}, addr: {} }}",
            self.name, self.reconnect, self.addr
        )
    }
}

#[allow(dead_code)]
impl<'name> Client<'name> {
    pub fn init(addr: &'static str, name: &'name str, reconnect: bool) -> MpdResult<Client<'name>> {
        let stream = TcpStream::connect(addr)?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(1)))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        let mut rx = BufReader::new(stream.try_clone()?);

        let mut buf = String::new();
        rx.read_line(&mut buf)?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        };
        let Some(version): Option<Version> = buf.strip_prefix("OK MPD ").and_then(|v| v.parse().ok()) else {
            return Err(MpdError::Generic(format!(
                "Handshake validation failed. Cannot parse version from '{buf}'"
            )));
        };

        debug!(name, version = version.to_string().as_str(), handshake = buf.trim(); "MPD client initiazed");

        if version > MAX_SUPPORTED_VERSION {
            status_warn!(
                "MPD version '{version}' is higher than supported. Maximum supported protocol version is '{MAX_SUPPORTED_VERSION}'. Some features may work incorrectly."
            );
        }

        Ok(Self {
            name,
            rx,
            stream,
            reconnect,
            addr,
            version,
        })
    }

    fn reconnect(&mut self) -> MpdResult<&Client> {
        let stream = TcpStream::connect(self.addr)?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(1)))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        let mut rx = BufReader::new(stream.try_clone()?);

        let mut buf = String::new();
        rx.read_line(&mut buf)?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        };

        let Some(version): Option<Version> = buf.strip_prefix("OK MPD ").and_then(|v| v.parse().ok()) else {
            return Err(MpdError::Generic(format!(
                "Handshake validation failed. Cannot parse version from '{buf}'"
            )));
        };

        self.rx = rx;
        self.stream = stream;
        self.version = version;

        debug!(name = self.name, handshake = buf.trim(), version = version.to_string().as_str(); "MPD client initiazed");

        Ok(self)
    }

    pub fn set_read_timeout(&mut self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    pub fn set_write_timeout(&mut self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        self.stream.set_write_timeout(timeout)
    }

    pub fn send<'cmd>(&mut self, command: &'cmd str) -> Result<ProtoClient<'cmd, '_, Self>, MpdError> {
        ProtoClient::new(command, self)
    }
}

impl<'name> SocketClient for Client<'name> {
    fn reconnect(&mut self) -> MpdResult<&impl SocketClient> {
        self.reconnect()
    }

    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.stream.write_all(bytes)
    }

    fn read(&mut self) -> &mut impl BufRead {
        &mut self.rx
    }
}
