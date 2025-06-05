use std::{
    io::{Read, Write},
    process::{Child, Stdio},
    thread::JoinHandle,
};

use anyhow::{Context, Result, anyhow, bail};
use crossbeam::channel::{Receiver, RecvError, Sender, TryRecvError};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::{Color, Colors, PrintStyledContent, Stylize},
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate},
};
use ratatui::layout::Rect;

use super::Pane;
use crate::{
    context::AppContext,
    mpd::commands::State,
    shared::{
        dependencies::CAVA,
        terminal::{TERMINAL, TtyWriter},
    },
    status_warn,
    ui::{UiEvent, image::clear_area},
};

#[derive(Debug)]
pub struct CavaPane {
    area: Rect,
    handle: Option<JoinHandle<Result<()>>>,
    command_channel: (Sender<CavaCommand>, Receiver<CavaCommand>),
    is_modal_open: bool,
}

fn cava_config(bars: u16) -> String {
    format!(
        r"
[general]
bars = {bars}
framerate = 60

[input]
method = fifo
source = /tmp/mpd.fifo
sample_rate = 44100
sample_bits = 16
channels = 2

[output]
method = raw
channels = mono
data_format = binary
bit_format = 16bit
reverse = 0

[smoothing]
noise_reduction = 0.3
"
    )
}

#[derive(Debug)]
enum CavaCommand {
    Start { area: Rect },
    Stop,
    Pause,
}

struct ProcessGuard {
    handle: Child,
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if let Err(e) = self.handle.kill() {
            log::error!("Failed to kill cava process: {e}");
            return;
        }
        if let Err(e) = self.handle.wait() {
            log::error!("Failed to wait for cava process to die: {e}");
        }
    }
}

impl CavaPane {
    pub fn new(context: &AppContext) -> Result<Self> {
        let mut res = Self {
            area: Rect::default(),
            handle: None,
            is_modal_open: false,
            command_channel: crossbeam::channel::bounded(0),
        };

        res.spawn(
            context.config.theme.background_color.map(Color::from),
            context.config.theme.borders_style.fg.map_or_else(|| Color::White, Color::from)
        )?;

        Ok(res)
    }

    pub fn run(&mut self) -> Result<()> {
        self.command_channel.0.send(CavaCommand::Start { area: self.area })?;
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
    pub fn read_cava_data(
        height: u16,
        buf: &mut [u8],
        columns: &mut [u16],
        stdout: &mut impl Read,
        stderr: &mut impl Read,
    ) -> Result<()> {
        if let Err(err) = stdout.read_exact(buf) {
            let mut stderr_contents = String::new();
            stderr.read_to_string(&mut stderr_contents)?;
            log::error!(err:?, stderr_contents = stderr_contents.as_str(); "Cava failed");
            bail!("Cava failed {err}");
        }

        for x in 0..columns.len() {
            let value = u16::from_le_bytes([buf[2 * x], buf[2 * x + 1]]);
            columns[x] = (value as u64 * height as u64 / 65535) as u16;
        }

        Ok(())
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
    pub fn render_cava(writer: &TtyWriter, area: Rect, columns: &mut [u16], bg_color: Color, bar_color: Color) -> Result<()> {
        let height = area.height;
        let mut writer = writer.lock();

        queue!(writer, BeginSynchronizedUpdate, SavePosition)?;
        for y in 0..height {
            let h = area.y + (height - 1) - y;
            queue!(writer, MoveTo(area.x, h))?;
            for column in columns.iter() {
                if *column > y {
                    queue!(writer, PrintStyledContent(" ".on(bar_color)))?;
                } else {
                    queue!(writer, PrintStyledContent(" ".on(bg_color)))?;
                }
                queue!(writer, PrintStyledContent(" ".on(bg_color)))?;
            }
        }
        queue!(writer, RestorePosition, EndSynchronizedUpdate)?;
        writer.flush()?;

        Ok(())
    }

    fn spawn_cava(bars: u16) -> Result<ProcessGuard> {
        let cfg_path = "/tmp/rmpc/cava.conf";
        std::fs::create_dir_all("/tmp/rmpc")?;
        let config = cava_config(bars);
        std::fs::write(cfg_path, config)?;

        Ok(ProcessGuard {
            handle: std::process::Command::new("cava")
                .arg("-p")
                .arg(cfg_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .spawn()?,
        })
    }

    pub fn spawn(&mut self, bg_color: Option<Color>, bar_color: Color) -> Result<()> {
        if !CAVA.installed {
            status_warn!(
                "Cava has not been found on your system. Please install it to use the visualiser."
            );
            return Ok(());
        }
        if self.handle.is_some() {
            log::debug!("Cava already running, skipping spawn");
            return Ok(());
        }

        let writer = TERMINAL.writer();
        let receiver = self.command_channel.1.clone();

        let bg_color = bg_color.unwrap_or(Color::Reset);
        self.handle = Some(
            std::thread::Builder::new()
                .name("cava".to_owned())
                .spawn(move || -> Result<_> {
                    let mut prev_command: Option<Result<CavaCommand, RecvError>> = None;

                    'outer: loop {
                        log::debug!("Cava thread waiting for command");
                        let area = match prev_command.take().unwrap_or(receiver.recv()) {
                            Ok(CavaCommand::Start { area }) => area,
                            Ok(CavaCommand::Pause) => {
                                continue 'outer;
                            }
                            Ok(CavaCommand::Stop) => {
                                break 'outer;
                            }
                            Err(RecvError) => {
                                log::error!("Error when trying to receive CavaCommand");
                                break 'outer;
                            }
                        };
                        let bars = area.width / 2;

                        let mut process = Self::spawn_cava(bars)?;
                        let stdout = process
                            .handle
                            .stdout
                            .as_mut()
                            .context("Failed to spawn cava. No stdout.")?;
                        let stderr = process
                            .handle
                            .stderr
                            .as_mut()
                            .context("Failed to spawn cava. No stderr.")?;

                        let mut columns = vec![0_u16; bars as usize];
                        let mut buf = vec![0_u8; 2 * bars as usize];

                        'inner: loop {
                            Self::read_cava_data(area.height, &mut buf, &mut columns, stdout, stderr)?;
                            Self::render_cava(&writer, area, &mut columns, bg_color, bar_color)?;

                            match receiver.try_recv() {
                                Ok(CavaCommand::Stop) => {
                                    break 'outer;
                                }
                                Ok(CavaCommand::Pause) => {
                                    break 'inner;
                                }
                                Ok(CavaCommand::Start { area }) => {
                                    prev_command = Some(Ok(CavaCommand::Start { area }));
                                    break 'inner;
                                }
                                Err(TryRecvError::Empty) => {}
                                Err(TryRecvError::Disconnected) => {
                                    log::error!(
                                        "CavaCommand channel disconnected. This should never happen."
                                    );
                                    break 'outer;
                                }
                            }
                        }
                        
                        log::debug!("Cava finished outer loop iteration");
                    }

                    log::debug!("Cava thread finished");

                    Ok(())
                })
                .context("Failed to spawn cava thread")?,
        );

        Ok(())
    }

    fn pause_and_clear(&mut self, context: &AppContext) -> Result<()> {
        log::debug!("Stopping cava thread and clearing area");
        self.command_channel
            .0
            .send(CavaCommand::Pause)
            .map_err(|err| anyhow!("Failed to send pause command to cava thread: {}", err))?;

        log::debug!("Waiting for cava thread to finish");
        self.clear(context)?;

        Ok(())
    }

    fn clear(&self, context: &AppContext) -> Result<()> {
        let writer = TERMINAL.writer();
        let mut w = writer.lock();

        let colors = Colors {
            background: context.config.theme.background_color.map(Into::into),
            foreground: None,
        };
        clear_area(w.by_ref(), colors, self.area)?;

        Ok(())
    }
}

impl Pane for CavaPane {
    fn render(
        &mut self,
        _frame: &mut ratatui::Frame,
        area: Rect,
        _context: &crate::context::AppContext,
    ) -> anyhow::Result<()> {
        self.area = area;
        Ok(())
    }

    fn calculate_areas(&mut self, area: Rect, _context: &AppContext) -> Result<()> {
        self.area = area;
        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        if matches!(context.status.state, State::Play) {
            self.run()?;
        }

        Ok(())
    }

    fn handle_action(
        &mut self,
        _event: &mut crate::shared::key_event::KeyEvent,
        _context: &mut crate::context::AppContext,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_hide(&mut self, context: &AppContext) -> Result<()> {
        self.pause_and_clear(context)?;
        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
        match event {
            UiEvent::Exit => {
                if let Some(handle) = self.handle.take() {
                    self.command_channel.0.send(CavaCommand::Stop).map_err(|err| {
                        anyhow!("Failed to send stop command to cava thread: {}", err)
                    })?;
                    handle.join().expect("Failed to join cava thread")?;
                }
            }
            UiEvent::Displayed if is_visible => {
                if is_visible && !self.is_modal_open {
                    self.run()?;
                }
            }
            UiEvent::Hidden if is_visible => {
                self.pause_and_clear(context)?;
            }
            UiEvent::ModalOpened if is_visible => {
                self.is_modal_open = true;
                self.pause_and_clear(context)?;
            }
            UiEvent::ModalClosed if is_visible => {
                self.is_modal_open = false;
                self.run()?;
            }
            UiEvent::PlaybackStateChanged => match context.status.state {
                State::Play => {
                    self.run()?;
                }
                State::Stop | State::Pause => {
                    log::debug!("CavaPane: Player event received, clearing cava area");
                    self.pause_and_clear(context)?;
                }
            },
            _ => {}
        }
        Ok(())
    }

    fn resize(&mut self, area: Rect, context: &AppContext) -> Result<()> {
        if self.is_modal_open {
            return Ok(());
        }

        self.area = area;
        self.pause_and_clear(context)?;

        if matches!(context.status.state, State::Play) {
            self.run()?;
        }
        Ok(())
    }
}
