use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use rustix::path::Arg;
use serde::Serialize;
use std::fmt::Display;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::process::Child;
use std::process::Stdio;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::time::Duration;
use std::{io::ErrorKind, process::Command};

use crate::utils::macros::try_cont;
use crate::utils::macros::try_skip;

#[derive(Debug)]
pub struct Ueberzug {
    sender: Sender<Action>,
    receiver: Option<Receiver<Action>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

struct UeberzugDaemon {
    pid: Option<Pid>,
    pid_file: String,
    ueberzug_process: Option<Child>,
    layer: Layer,
}

const IDENTIFIER: &str = "rmpc-albumart";
const PID_FILE_TIMOUT: Duration = Duration::from_secs(5);

enum Action {
    Add(&'static str, u16, u16, u16, u16),
    Remove,
    Destroy,
}

pub enum Layer {
    Wayland,
    X11,
}

impl Layer {
    fn as_str(&self) -> &'static str {
        match self {
            Layer::Wayland => "wayland",
            Layer::X11 => "x11",
        }
    }
}

impl Ueberzug {
    pub fn cleanup(&mut self) -> Result<()> {
        if let Some(handle) = self.handle.take() {
            self.sender.send(Action::Destroy)?;
            handle.join().expect("Ueberzug thread to end gracefully");
        };
        Ok(())
    }

    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            sender: tx,
            receiver: Some(rx),
            handle: None,
        }
    }

    pub fn init(mut self, layer: Layer) -> Self {
        let pid_file_path = std::env::temp_dir()
            .join("rmpc")
            .join(format!("ueberzug-{}.pid", std::process::id()))
            .to_string_lossy()
            .into_owned();

        let mut daemon = UeberzugDaemon {
            pid: None,
            pid_file: pid_file_path,
            ueberzug_process: None,
            layer,
        };

        let Some(rx) = self.receiver.take() else {
            return self;
        };

        self.handle = Some(std::thread::spawn(move || {
            while let Ok(action) = rx.recv() {
                daemon.pid = Some(try_cont!(
                    daemon.spawn_daemon_if_needed(),
                    "Failed to spawn ueberzugpp daemon"
                ));
                match action {
                    Action::Add(path, x, y, width, height) => {
                        try_cont!(
                            daemon.show_image(path, x, y, width, height),
                            "Failed to send image to ueberzugpp"
                        );
                    }
                    Action::Remove => {
                        try_cont!(daemon.remove_image(), "Failed to send remove request to ueberzugpp");
                    }
                    Action::Destroy => {
                        try_skip!(daemon.remove_image(), "Failed to send remove request to ueberzugpp");

                        if let Some(ref mut proc) = daemon.ueberzug_process {
                            try_skip!(proc.kill(), "Failed to kill ueberzugpp process");
                            try_skip!(proc.wait(), "Ueberzugpp process failed to die");
                        }

                        if let Some(pid) = daemon.pid {
                            if let Some(pid) = rustix::process::Pid::from_raw(pid.0) {
                                try_skip!(
                                    rustix::process::kill_process(pid, rustix::process::Signal::Kill),
                                    "Failed to send SIGKILL to ueberzugpp pid file"
                                );
                            }
                        };

                        try_skip!(
                            std::fs::remove_file(&daemon.pid_file),
                            "Failed to remove ueberzugpp's pid file"
                        );
                        break;
                    }
                }
            }
        }));

        self
    }

    pub fn remove_image(&self) -> Result<()> {
        Ok(self.sender.send(Action::Remove)?)
    }

    pub fn show_image(&self, path: &'static str, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
        Ok(self.sender.send(Action::Add(path, x, y, width, height))?)
    }
}

impl UeberzugDaemon {
    pub fn show_image(&self, path: &'static str, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
        let Some(pid) = self.pid else {
            return Ok(());
        };

        let mut socket = UeberzugSocket::connect(pid)?;

        socket.add_image(
            pid,
            CreateData {
                path,
                width,
                height,
                x,
                y,
            },
        )?;

        Ok(())
    }

    pub fn remove_image(&self) -> Result<()> {
        let Some(pid) = self.pid else {
            return Ok(());
        };

        let mut socket = UeberzugSocket::connect(pid)?;
        socket.remove_image(pid)
    }

    pub fn spawn_daemon_if_needed(&mut self) -> Result<Pid> {
        match self.pid {
            Some(pid) => Ok(pid),
            None => match std::fs::read_to_string(&self.pid_file) {
                Ok(pid) => {
                    let pid = Pid(pid
                        .trim()
                        .parse::<i32>()
                        .context(anyhow!("Failed to parse ueberzug's PID {pid}"))?);
                    self.pid = Some(pid);

                    Ok(pid)
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    let mut cmd = Command::new("ueberzugpp");
                    cmd.args([
                        "layer",
                        "-so",
                        self.layer.as_str(),
                        "--no-stdin",
                        "--pid-file",
                        &self.pid_file,
                    ]);

                    let child = cmd
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()?;

                    let pid = self.read_pid()?;

                    self.pid = Some(pid);
                    self.ueberzug_process = Some(child);

                    Ok(pid)
                }
                Err(err) => Err(err.into()),
            },
        }
    }

    fn read_pid(&self) -> Result<Pid> {
        let start = std::time::Instant::now();

        while let Err(err) = std::fs::read_to_string(&self.pid_file) {
            if err.kind() == ErrorKind::NotFound && start.elapsed() < PID_FILE_TIMOUT {
                std::thread::sleep(Duration::from_millis(100));
            } else {
                return Err(err.into());
            }
        }

        Ok(Pid(std::fs::read_to_string(&self.pid_file)?.parse()?))
    }
}

struct UeberzugSocket(UnixStream);
impl UeberzugSocket {
    fn connect(pid: Pid) -> Result<UeberzugSocket> {
        Ok(Self(UnixStream::connect(pid.as_socket_path()).context(anyhow!(
            "Cannot connect to ueberzug socket: '{}'",
            pid.as_socket_path()
        ))?))
    }

    fn remove_image(&mut self, pid: Pid) -> Result<()> {
        self.0
            .write_all(format!(r#"{{"action":"remove","identifier":"{IDENTIFIER}-{pid}"}}"#).as_bytes())?;
        self.0.write_all(b"\n")?;
        Ok(())
    }

    fn add_image(
        &mut self,
        pid: Pid,
        CreateData {
            x,
            y,
            width,
            height,
            path,
        }: CreateData,
    ) -> Result<()> {
        self.0.write_all(format!(r#"{{"action":"add","identifier":"{IDENTIFIER}-{pid}","max_height":{height},"max_width":{width},"path":"{path}","x":{x},"y":{y}}}"#)
            .as_bytes(),
        )?;
        self.0.write_all(b"\n")?;
        Ok(())
    }
}

#[derive(Default, Debug, Serialize, Clone, Copy)]
struct CreateData<'a> {
    path: &'a str,
    width: u16,
    height: u16,
    x: u16,
    y: u16,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Pid(i32);

impl Pid {
    fn as_socket_path(self) -> String {
        format!("/tmp/ueberzugpp-{}.socket", self.0)
    }
}

impl Display for Pid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
