use std::{
    fmt::Display,
    io::{ErrorKind, Write},
    os::unix::net::UnixStream,
    process::{Child, Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use ratatui::layout::Rect;
use rustix::path::Arg;
use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use super::Backend;
use crate::{
    config::{
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    ctx::Ctx,
    shared::{macros::try_skip, tmux},
};

#[derive(Debug)]
pub struct Ueberzug {
    daemon: UeberzugDaemon,
}

#[derive(Debug)]
struct UeberzugDaemon {
    pid: Option<Pid>,
    pid_file: String,
    ueberzug_process: Option<Child>,
    layer: Layer,
}

const IDENTIFIER: &str = "rmpc-albumart";
const PID_FILE_TIMEOUT: Duration = Duration::from_secs(5);
const UEBERZUG_ALBUM_ART_PATH: &str = "/tmp/rmpc/albumart";
const UEBERZUG_ALBUM_ART_DIR: &str = "/tmp/rmpc";

#[derive(Debug)]
pub enum Layer {
    Wayland,
    X11,
}

#[derive(derive_more::Debug)]
pub struct Data {
    area: Rect,
}

impl Layer {
    fn as_str(&self) -> &'static str {
        match self {
            Layer::Wayland => "wayland",
            Layer::X11 => "x11",
        }
    }
}

impl Backend for Ueberzug {
    type EncodedData = Data;

    fn hide(
        &mut self,
        _w: &mut impl Write,
        _: Rect,
        _bg_color: Option<crossterm::style::Color>,
    ) -> Result<()> {
        self.daemon.remove_image()?;
        Ok(())
    }

    fn display(&mut self, _w: &mut impl Write, data: Self::EncodedData, _ctx: &Ctx) -> Result<()> {
        if tmux::is_in_tmux_and_hidden()? {
            // We should not command ueberzugpp to rerender when rmpc is inside
            // TMUX session without any attached clients or the pane which rmpc
            // resides in is not visible because it might make ueberzugpp popup
            // in windows/panes/sessions that do not belong to rmpc
            return Ok(());
        }
        let Rect { x, y, width, height } = data.area;

        self.daemon.spawn_daemon_if_needed()?;
        self.daemon.show_image(UEBERZUG_ALBUM_ART_PATH, x, y, width, height)?;
        Ok(())
    }

    fn create_data(
        image_data: &[u8],
        area: Rect,
        _max_size: Size,
        _halign: HorizontalAlign,
        _valign: VerticalAlign,
    ) -> Result<Self::EncodedData> {
        std::fs::create_dir_all(UEBERZUG_ALBUM_ART_DIR)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(UEBERZUG_ALBUM_ART_PATH)?;

        file.write_all(image_data)?;

        Ok(Data { area })
    }
}

impl Ueberzug {
    pub fn new(layer: Layer) -> Self {
        let pid_file_path = std::env::temp_dir()
            .join("rmpc")
            .join(format!("ueberzug-{}.pid", std::process::id()))
            .to_string_lossy()
            .into_owned();

        let mut daemon =
            UeberzugDaemon { pid: None, pid_file: pid_file_path, ueberzug_process: None, layer };
        if let Ok(pid) = daemon.spawn_daemon_if_needed() {
            daemon.pid = Some(pid);
        }

        Self { daemon }
    }
}

impl std::ops::Drop for UeberzugDaemon {
    fn drop(&mut self) {
        try_skip!(self.remove_image(), "Failed to send remove request to ueberzugpp");

        if let Some(ref mut proc) = self.ueberzug_process {
            try_skip!(proc.kill(), "Failed to kill ueberzugpp process");
            try_skip!(proc.wait(), "Ueberzugpp process failed to die");
        }

        if let Some(pid) = self.pid
            && let Some(pid) = rustix::process::Pid::from_raw(pid.0)
        {
            try_skip!(
                rustix::process::kill_process(pid, rustix::process::Signal::TERM),
                "Failed to send SIGTERM to ueberzugpp pid file"
            );
        }

        try_skip!(std::fs::remove_file(&self.pid_file), "Failed to remove ueberzugpp's pid file");
    }
}

impl UeberzugDaemon {
    fn show_image(
        &self,
        path: &'static str,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<()> {
        let Some(pid) = self.pid else {
            return Ok(());
        };

        let mut socket = UeberzugSocket::connect(pid)?;

        socket.add_image(pid, CreateData { path, width, height, x, y })?;

        Ok(())
    }

    fn remove_image(&self) -> Result<()> {
        let Some(pid) = self.pid else {
            return Ok(());
        };

        let mut socket = UeberzugSocket::connect(pid)?;
        socket.remove_image(pid)
    }

    fn is_daemon_running(pid: Pid) -> bool {
        let mut system = System::new();
        let infopid = sysinfo::Pid::from_u32(pid.0 as u32);
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[infopid]),
            true,
            ProcessRefreshKind::everything(),
        );

        system.process(infopid).is_some()
    }

    fn spawn_daemon(&self) -> Result<(Pid, Child)> {
        let mut cmd = Command::new("ueberzugpp");
        if let Err(err) = std::fs::remove_file(&self.pid_file)
            && err.kind() != ErrorKind::NotFound
        {
            log::warn!(err:?; "Failed to delete pid file");
        }
        cmd.args(["layer", "-so", self.layer.as_str(), "--no-stdin", "--pid-file", &self.pid_file]);

        let child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;

        let pid = self.read_pid()?;

        Ok((pid, child))
    }

    fn spawn_daemon_if_needed(&mut self) -> Result<Pid> {
        let Some(pid) = self.pid else {
            let (pid, child) = self.spawn_daemon()?;
            self.ueberzug_process = Some(child);
            return Ok(pid);
        };
        if Self::is_daemon_running(pid) {
            Ok(pid)
        } else {
            let (pid, child) = self.spawn_daemon()?;
            self.ueberzug_process = Some(child);
            Ok(pid)
        }
    }

    fn read_pid(&self) -> Result<Pid> {
        let start = std::time::Instant::now();

        while let Err(err) = std::fs::read_to_string(&self.pid_file) {
            if err.kind() == ErrorKind::NotFound && start.elapsed() < PID_FILE_TIMEOUT {
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
        Ok(Self(
            UnixStream::connect(pid.as_socket_path()).context(anyhow!(
                "Cannot connect to ueberzug socket: '{}'",
                pid.as_socket_path()
            ))?,
        ))
    }

    fn remove_image(&mut self, pid: Pid) -> Result<()> {
        self.0.write_all(
            format!(r#"{{"action":"remove","identifier":"{IDENTIFIER}-{pid}"}}"#).as_bytes(),
        )?;
        self.0.write_all(b"\n")?;
        Ok(())
    }

    fn add_image(
        &mut self,
        pid: Pid,
        CreateData { x, y, width, height, path }: CreateData,
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
