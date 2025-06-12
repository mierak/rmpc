use std::{
    io::{Read, Write},
    process::{Child, Stdio},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use crossbeam::channel::{Receiver, RecvError, Sender, TryRecvError};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::{Colors, PrintStyledContent, Stylize},
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate},
};
use ratatui::{Frame, layout::Rect};

use super::Pane;
use crate::{
    config::{cava::Cava, theme::cava::CavaTheme},
    context::AppContext,
    mpd::commands::State,
    shared::{
        dependencies::CAVA,
        key_event::KeyEvent,
        terminal::{TERMINAL, TtyWriter},
    },
    status_warn,
    try_skip,
    ui::{UiEvent, image::clear_area},
};

#[derive(Debug)]
pub struct CavaPane {
    area: Rect,
    handle: Option<JoinHandle<Result<()>>>,
    command_channel: (Sender<CavaCommand>, Receiver<CavaCommand>),
    is_modal_open: bool,
}

#[derive(Debug)]
enum CavaCommand {
    Start { area: Rect },
    Stop,
    Pause,
    ConfigChanged { config: Cava, theme: CavaTheme },
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
    pub fn new(_context: &AppContext) -> Self {
        Self {
            area: Rect::default(),
            handle: None,
            is_modal_open: false,
            command_channel: crossbeam::channel::bounded(0),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.command(CavaCommand::Start { area: self.area })?;
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
    #[inline]
    pub fn read_cava_data(
        height: u16,
        read_buffer: &mut [u8],
        columns: &mut [f32],
        stdout: &mut impl Read,
        stderr: &mut impl Read,
    ) -> Result<()> {
        if let Err(err) = stdout.read_exact(read_buffer) {
            let mut buf = String::new();
            stderr.read_to_string(&mut buf)?;
            log::error!(err:?, stderr = buf.as_str(); "Cava failed");
            bail!("Cava failed {err}");
        }

        for x in 0..columns.len() {
            let value = u16::from_le_bytes([read_buffer[2 * x], read_buffer[2 * x + 1]]);
            columns[x] = value as f32 * height as f32 / 65535.0f32;
        }

        Ok(())
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::too_many_arguments
    )]
    #[inline]
    pub fn render_cava(
        writer: &TtyWriter,
        area: Rect,
        columns: &mut [f32],
        x_offset: u16,
        empty_bar_symbol: &str,
        bar_width: u16,
        bar_spacing: u16,
        theme: &CavaTheme,
    ) -> Result<()> {
        let height = area.height;
        let mut writer = writer.lock();

        queue!(writer, BeginSynchronizedUpdate, SavePosition)?;

        for (col_idx, column) in columns.iter().enumerate() {
            let col_idx = col_idx as u16;
            let x = area.x + x_offset + col_idx * bar_width + col_idx * bar_spacing;

            for y in 0..height {
                let h = area.y + (height - 1) - y;
                let color = theme.bar_color.get_color(y as usize, area.height);
                let fill_amount = (*column - f32::from(y)).clamp(0.0, 0.99);
                queue!(writer, MoveTo(x, h))?;
                if fill_amount < 0.01 {
                    queue!(writer, PrintStyledContent(empty_bar_symbol.on(theme.bg_color)))?;
                } else {
                    let char_index =
                        (fill_amount * theme.bar_symbols_count as f32).floor() as usize;
                    let fill_char = theme.bar_symbols[char_index].as_str();
                    queue!(writer, PrintStyledContent(fill_char.with(color).on(theme.bg_color)))?;
                }
            }
        }

        queue!(writer, RestorePosition, EndSynchronizedUpdate)?;
        writer.flush()?;

        Ok(())
    }

    fn spawn_cava(
        bars: u16,
        bar_width: u16,
        bar_height: u16,
        config: &Cava,
    ) -> Result<ProcessGuard> {
        let cfg_dir = std::env::temp_dir().join("rmpc");
        std::fs::create_dir_all(&cfg_dir)?;
        let cfg_path = cfg_dir.join(format!("cava-{}.conf", rustix::process::geteuid().as_raw()));
        let config = config.to_cava_config_file(bars, bar_width, bar_height)?;
        std::fs::write(&cfg_path, config)?;

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

    fn run_cava_loop(
        receiver: &Receiver<CavaCommand>,
        writer: &TtyWriter,
        cava_config: Cava,
        cava_theme: CavaTheme,
    ) -> Result<()> {
        let mut prev_command: Option<Result<CavaCommand, RecvError>> = None;
        let mut cava_config = cava_config;
        let mut cava_theme = cava_theme;
        let mut area: Rect;

        'outer: loop {
            log::trace!(prev_command:?; "Waiting for command");
            let command = prev_command.take().unwrap_or_else(|| receiver.recv());
            log::trace!(command:?; "Received command");
            match command {
                Ok(CavaCommand::Start { area: new_area }) => {
                    area = new_area;
                }
                Ok(CavaCommand::Pause) => {
                    continue 'outer;
                }
                Ok(CavaCommand::Stop) => {
                    break 'outer;
                }
                Ok(CavaCommand::ConfigChanged { config, theme }) => {
                    log::trace!("Cava config changed, updating");
                    cava_config = config;
                    cava_theme = theme;
                    continue 'outer;
                }
                Err(RecvError) => {
                    log::error!("Error when trying to receive CavaCommand");
                    break 'outer;
                }
            }
            let bar_width = cava_theme.bar_width;
            let bar_spacing = cava_theme.bar_spacing;
            let bars = area.width / (bar_width + bar_spacing);

            let total_bar_width = bars * bar_width;
            let total_spacing_width = (bars - 1) * bar_spacing;
            let total_width = total_bar_width + total_spacing_width;
            let empty_bar_symbol = " ".repeat(bar_width as usize);

            let x_offset = (area.width - total_width) / 2;

            log::debug!(cava_theme:?; "theme");

            let mut process = Self::spawn_cava(bars, bar_width, bar_spacing, &cava_config)?;
            let stdout =
                process.handle.stdout.as_mut().context("Failed to spawn cava. No stdout.")?;
            let stderr =
                process.handle.stderr.as_mut().context("Failed to spawn cava. No stderr.")?;

            let mut columns = vec![0_f32; bars as usize];
            let mut buf = vec![0_u8; 2 * bars as usize];

            'inner: loop {
                Self::read_cava_data(area.height, &mut buf, &mut columns, stdout, stderr)?;
                Self::render_cava(
                    writer,
                    area,
                    &mut columns,
                    x_offset,
                    &empty_bar_symbol,
                    bar_width,
                    bar_spacing,
                    &cava_theme,
                )?;

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
                    Ok(CavaCommand::ConfigChanged { config, theme }) => {
                        prev_command = Some(Ok(CavaCommand::ConfigChanged { config, theme }));
                        break 'inner;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        log::error!("CavaCommand channel disconnected. This should never happen.");
                        break 'outer;
                    }
                }
            }

            log::debug!("Cava finished outer loop iteration");
        }

        log::debug!("Cava thread finished");

        Ok(())
    }

    pub fn spawn(&mut self, cava_config: Cava, cava_theme: CavaTheme) -> Result<()> {
        if self.handle.is_some() {
            log::debug!("Cava already running, skipping spawn");
            return Ok(());
        }
        if !CAVA.installed {
            status_warn!(
                "Cava has not been found on your system. Please install it to use the visualiser."
            );
            return Ok(());
        }

        let writer = TERMINAL.writer();
        let receiver = self.command_channel.1.clone();

        self.handle = Some(
            std::thread::Builder::new()
                .name("cava".to_owned())
                .spawn(move || -> Result<_> {
                    try_skip!(
                        Self::run_cava_loop(&receiver, &writer, cava_config, cava_theme),
                        "Cava thread encountered an error"
                    );
                    Ok(())
                })
                .context("Failed to spawn cava thread")?,
        );

        Ok(())
    }

    fn pause_and_clear(&mut self, context: &AppContext) -> Result<()> {
        log::debug!("Stopping cava thread and clearing area");
        self.command(CavaCommand::Pause)?;
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

    fn command(&self, cmd: CavaCommand) -> Result<()> {
        let Some(handle) = self.handle.as_ref() else {
            log::trace!(cmd:?; "Cava thread is not running, not sending command");
            return Ok(());
        };

        if handle.is_finished() {
            log::debug!("Cava thread has finished, not sending command");
            return Ok(());
        }

        log::trace!(cmd:?; "Sending CavaCommand");
        self.command_channel
            .0
            .send_timeout(cmd, Duration::from_secs(3))
            .map_err(|err| anyhow!("Failed to send command to cava thread: {}", err))
    }
}

impl Pane for CavaPane {
    fn render(&mut self, _frame: &mut Frame, area: Rect, _ctx: &AppContext) -> anyhow::Result<()> {
        self.area = area;
        Ok(())
    }

    fn calculate_areas(&mut self, area: Rect, _context: &AppContext) -> Result<()> {
        self.area = area;
        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        self.spawn(context.config.cava.clone(), context.config.theme.cava.clone())?;

        if matches!(context.status.state, State::Play) {
            self.run()?;
        }

        Ok(())
    }

    fn handle_action(&mut self, _ev: &mut KeyEvent, _ctx: &mut AppContext) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_hide(&mut self, context: &AppContext) -> Result<()> {
        self.pause_and_clear(context)?;
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, is_visible: bool, ctx: &AppContext) -> Result<()> {
        match event {
            UiEvent::Exit => {
                self.command(CavaCommand::Stop)?;
                if let Some(handle) = self.handle.take() {
                    handle.join().expect("Failed to join cava thread")?;
                }
            }
            UiEvent::ConfigChanged => {
                self.command(CavaCommand::ConfigChanged {
                    config: ctx.config.cava.clone(),
                    theme: ctx.config.theme.cava.clone(),
                })?;

                if is_visible && !self.is_modal_open && matches!(ctx.status.state, State::Play) {
                    self.run()?;
                }
            }
            UiEvent::Displayed if is_visible => {
                if is_visible && !self.is_modal_open && matches!(ctx.status.state, State::Play) {
                    self.run()?;
                }
            }
            UiEvent::Hidden if is_visible => {
                self.pause_and_clear(ctx)?;
            }
            UiEvent::ModalOpened if is_visible => {
                self.is_modal_open = true;
                self.pause_and_clear(ctx)?;
            }
            UiEvent::ModalClosed if is_visible && matches!(ctx.status.state, State::Play) => {
                self.is_modal_open = false;
                self.run()?;
            }
            UiEvent::PlaybackStateChanged if is_visible => match ctx.status.state {
                State::Play => {
                    self.run()?;
                }
                State::Stop | State::Pause => {
                    log::debug!("CavaPane: Player event received, clearing cava area");
                    self.pause_and_clear(ctx)?;
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
