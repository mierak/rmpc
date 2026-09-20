use std::{
    io::{Read, Write},
    process::{Child, Command, Stdio},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use crossbeam::channel::{Receiver, RecvError, Sender, TryRecvError};
use ratatui::{
    Frame,
    layout::Rect,
    prelude::FromCrossterm,
    style::{Color as RatatuiColor, Style},
    widgets::Block,
};
use rmpc_mpd::commands::State;

use super::Pane;
use crate::{
    config::{
        tabs::PaneType,
        theme::waveform::WaveformTheme,
        waveform::{CAPTURE_CHANNELS, Waveform},
    },
    ctx::Ctx,
    shared::{
        dependencies::PW_CAT,
        keys::ActionEvent,
        terminal::{TERMINAL, TtyWriter},
    },
    status_warn,
    ui::{UiEvent, image::clear_area},
};

const PIPEWIRE_QUEUE_CAPACITY: usize = 128;
const PIPEWIRE_READ_BUFFER_SIZE: usize = 8_192;
const READER_RETRY_INTERVAL: Duration = Duration::from_secs(1);

mod render;
mod runtime;
mod samples;

use runtime::PlaybackBuffering;
use samples::WaveformSamples;

#[derive(Debug)]
pub struct WaveformPane {
    area: Rect,
    handle: Option<JoinHandle<Result<()>>>,
    command_channel: (Sender<WaveformCommand>, Receiver<WaveformCommand>),
    is_modal_open: bool,
}

#[derive(Debug)]
enum WaveformCommand {
    Start { area: Rect },
    Pause,
    Stop,
    ConfigChanged { config: Waveform, theme: WaveformTheme },
}

#[derive(Debug, PartialEq, Eq)]
enum PlaybackStateAction {
    Start,
    Pause,
    PauseAndClear,
}

struct PipeWireReader {
    process: Child,
    receiver: Receiver<std::io::Result<Vec<u8>>>,
    handle: Option<JoinHandle<()>>,
}

impl PipeWireReader {
    /// Open a new `PipeWire` capture.
    fn open(config: &Waveform) -> Option<Self> {
        match Self::new(&config.source, config) {
            Ok(reader) => Some(reader),
            Err(err) => {
                log::error!(err:?; "Waveform unavailable");
                None
            }
        }
    }

    /// Open a new `PipeWire` capture, warning the user via the status bar the
    /// first time it becomes unavailable.
    fn open_and_warn(config: &Waveform, warned: &mut bool) -> Option<Self> {
        let reader = Self::open(config);
        if reader.is_some() {
            *warned = false;
        } else if !*warned {
            *warned = true;
            status_warn!(
                "Waveform capture unavailable. Could not open PipeWire target '{}'. Retrying.",
                config.source
            );
        }
        reader
    }

    /// Create a new `pw-cat` process to capture the target source.
    fn new(target: &str, config: &Waveform) -> Result<Self> {
        let mut process = Command::new("pw-cat")
            .args([
                "--record",
                "--raw",
                "--target",
                target,
                "--latency",
                "0ms",
                "--format",
                "s16",
                "--rate",
                &config.sample_rate.to_string(),
                "--channels",
                &CAPTURE_CHANNELS.to_string(),
                "-",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context(
                "pw-cat was not found. Please install PipeWire tools to use the waveform visualizer",
            )?;

        let mut stdout = process.stdout.take().context("pw-cat did not provide stdout")?;
        let (sender, receiver) = crossbeam::channel::bounded(PIPEWIRE_QUEUE_CAPACITY);
        let handle = std::thread::Builder::new()
            .name("waveform-pipewire-reader".to_owned())
            .spawn(move || {
                let mut buffer = vec![0; PIPEWIRE_READ_BUFFER_SIZE];
                loop {
                    match stdout.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(read) => {
                            // The waveform has its own bounded latency queue.
                            // Do not block the `PipeWire` reader if terminal
                            // rendering falls behind. `try_send` drops the
                            // newest chunk when the queue is full. Stale
                            // data is flushed by `discard_buffered_samples`
                            // before playback resumes.
                            let _ = sender.try_send(Ok(buffer[..read].to_vec()));
                        }
                        Err(err) => {
                            // Never wait for a paused worker to drain the
                            // bounded queue: `PipeWireReader::drop` must
                            // always be able to join this thread.
                            let _ = sender.try_send(Err(err));
                            break;
                        }
                    }
                }
            })
            .context("Failed to spawn PipeWire waveform reader")?;

        Ok(Self { process, receiver, handle: Some(handle) })
    }

    fn discard_buffered_samples(&mut self) -> bool {
        loop {
            match self.receiver.try_recv() {
                Ok(_) => {}
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
    }
}

/// Whether reconfiguring from `old` to `new` requires restarting the
/// `pw-cat` capture.
fn capture_target_changed(old: &Waveform, new: &Waveform) -> bool {
    old.source != new.source || old.sample_rate != new.sample_rate
}

impl Drop for PipeWireReader {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl WaveformPane {
    pub fn new(_ctx: &Ctx) -> Self {
        Self {
            area: Rect::default(),
            handle: None,
            is_modal_open: false,
            command_channel: crossbeam::channel::bounded(0),
        }
    }

    fn run(&self) -> Result<()> {
        self.command(WaveformCommand::Start { area: self.area })
    }

    fn run_loop(
        receiver: &Receiver<WaveformCommand>,
        writer: &TtyWriter,
        mut config: Waveform,
        mut theme: WaveformTheme,
    ) -> Result<()> {
        let mut pending = None;
        let mut reader: Option<PipeWireReader> = None;
        let mut reader_unavailable_warned = false;
        'outer: loop {
            let command = pending.take().unwrap_or_else(|| receiver.recv());
            let area = match command {
                Ok(WaveformCommand::Start { area }) => area,
                Ok(WaveformCommand::Pause) => continue,
                Ok(WaveformCommand::Stop) | Err(RecvError) => break,
                Ok(WaveformCommand::ConfigChanged { config: new_config, theme: new_theme }) => {
                    if capture_target_changed(&config, &new_config) {
                        reader = None;
                        reader_unavailable_warned = false;
                    }

                    config = new_config;
                    theme = new_theme;
                    continue;
                }
            };

            if area.width == 0 || area.height == 0 {
                continue;
            }

            if reader.as_mut().is_some_and(|reader| !reader.discard_buffered_samples()) {
                reader = None;
            }

            if reader.is_none() {
                reader = PipeWireReader::open_and_warn(&config, &mut reader_unavailable_warned);
            }

            let mut last_open_attempt = Instant::now();
            let mut incoming_samples = WaveformSamples::new(config.display_mode);
            let mut displayed_samples = WaveformSamples::new(config.display_mode);
            let mut pending_bytes = Vec::new();
            let mut buffering = PlaybackBuffering::new(&config)?;
            let poll_interval = buffering.poll_interval();

            'inner: loop {
                crossbeam::select! {
                    recv(receiver) -> command => match command {
                        Ok(WaveformCommand::Stop) | Err(RecvError) => break 'outer,
                        Ok(WaveformCommand::Pause) => break 'inner,
                        Ok(WaveformCommand::Start { area: new_area }) => {
                            pending = Some(Ok(WaveformCommand::Start { area: new_area }));
                            break 'inner;
                        }

                        Ok(WaveformCommand::ConfigChanged {
                            config: new_config,
                            theme: new_theme,
                        }) => {
                            pending = Some(Ok(WaveformCommand::ConfigChanged {
                                config: new_config,
                                theme: new_theme,
                            }));

                            break 'inner;
                        }
                    },
                    default(poll_interval) => {
                        if let Some(active_reader) = reader.as_mut() {
                            let mut unavailable = false;
                            loop {
                                match active_reader.receiver.try_recv() {
                                    Ok(Ok(bytes)) => {
                                        WaveformSamples::decode_pcm(
                                            &bytes,
                                            &config,
                                            &mut pending_bytes,
                                            &mut incoming_samples,
                                        );
                                    }

                                    Ok(Err(err)) => {
                                        log::error!(err:?; "Failed to read PipeWire waveform source");
                                        unavailable = true;
                                        break;
                                    }

                                    Err(TryRecvError::Empty) => break,
                                    Err(TryRecvError::Disconnected) => {
                                        log::error!("PipeWire waveform reader stopped");
                                        unavailable = true;
                                        break;
                                    }
                                }
                            }

                            if unavailable {
                                reader = None;
                                last_open_attempt = Instant::now();
                            }
                        } else if last_open_attempt.elapsed() >= READER_RETRY_INTERVAL {
                            last_open_attempt = Instant::now();
                            reader = PipeWireReader::open_and_warn(
                                &config,
                                &mut reader_unavailable_warned,
                            );
                        }
                    }
                }

                buffering.trim_incoming(&mut incoming_samples);
                if let Some(frames_to_consume) =
                    buffering.frames_to_consume(incoming_samples.len(), Instant::now())
                {
                    let consumed =
                        incoming_samples.consume_into(frames_to_consume, &mut displayed_samples);

                    buffering.trim_displayed(&mut displayed_samples);
                    if consumed > 0 && displayed_samples.has_width(area.width as usize) {
                        render::render_waveform(
                            writer,
                            area,
                            &mut displayed_samples,
                            &config,
                            &theme,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }

    fn spawn(&mut self, config: Waveform, theme: WaveformTheme) -> Result<()> {
        if self.handle.as_ref().is_some_and(|handle| !handle.is_finished()) {
            return Ok(());
        }

        if !PW_CAT.installed {
            status_warn!(
                "pw-cat was not found. Please install PipeWire tools to use the waveform visualizer."
            );

            return Ok(());
        }

        if let Some(handle) = self.handle.take() {
            match handle.join() {
                Ok(Ok(())) => log::debug!("Restarting completed waveform worker"),
                Ok(Err(err)) => log::error!(err:?; "Restarting failed waveform worker"),
                Err(_) => log::error!("Restarting panicked waveform worker"),
            }
        }

        let receiver = self.command_channel.1.clone();
        let writer = TERMINAL.writer();
        self.handle = Some(
            std::thread::Builder::new()
                .name("waveform".to_owned())
                .spawn(move || Self::run_loop(&receiver, &writer, config, theme))
                .context("Failed to spawn waveform thread")?,
        );

        Ok(())
    }

    fn clear(&self, ctx: &Ctx) -> Result<()> {
        let writer = TERMINAL.writer();
        clear_area(
            writer.lock().by_ref(),
            Some(ctx.config.theme.waveform.background_color),
            self.area,
        )?;

        Ok(())
    }

    fn pause_and_clear(&self, ctx: &Ctx) -> Result<()> {
        self.pause()?;
        self.clear(ctx)
    }

    fn pause(&self) -> Result<()> {
        self.command(WaveformCommand::Pause)
    }

    pub(crate) fn stop(&mut self) -> Result<()> {
        self.command(WaveformCommand::Stop)?;
        if let Some(handle) = self.handle.take() {
            handle.join().map_err(|_| anyhow!("Waveform thread panicked"))??;
        }

        Ok(())
    }

    fn playback_state_action(state: State) -> PlaybackStateAction {
        match state {
            State::Play => PlaybackStateAction::Start,
            State::Pause => PlaybackStateAction::Pause,
            State::Stop => PlaybackStateAction::PauseAndClear,
        }
    }

    fn close_modal(&mut self) {
        self.is_modal_open = false;
    }

    fn command(&self, command: WaveformCommand) -> Result<()> {
        let Some(handle) = self.handle.as_ref() else {
            return Ok(());
        };

        if handle.is_finished() {
            return Ok(());
        }

        self.command_channel
            .0
            .send_timeout(command, Duration::from_secs(3))
            .map_err(|err| anyhow!("Failed to send waveform command: {err}"))
    }
}

impl Pane for WaveformPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.area = area;
        frame.render_widget(
            Block::default().style(
                Style::default()
                    .bg(RatatuiColor::from_crossterm(ctx.config.theme.waveform.background_color)),
            ),
            area,
        );

        Ok(())
    }

    fn calculate_areas(&mut self, area: Rect, _ctx: &Ctx) -> Result<()> {
        self.area = area;
        Ok(())
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        self.spawn(ctx.config.waveform.clone(), ctx.config.theme.waveform.clone())?;
        if matches!(ctx.status.state, State::Play) {
            self.run()?;
        }

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut ActionEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }

    fn on_hide(&mut self, ctx: &Ctx) -> Result<()> {
        self.pause_and_clear(ctx)?;
        self.on_removed_from_config(ctx)?;
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::Exit => {
                self.stop()?;
            }

            UiEvent::ConfigChanged => {
                self.command(WaveformCommand::ConfigChanged {
                    config: ctx.config.waveform.clone(),
                    theme: ctx.config.theme.waveform.clone(),
                })?;
                if is_visible && !self.is_modal_open && matches!(ctx.status.state, State::Play) {
                    self.run()?;
                }
            }

            UiEvent::Displayed
                if is_visible && !self.is_modal_open && matches!(ctx.status.state, State::Play) =>
            {
                self.run()?;
            }

            UiEvent::SongChanged | UiEvent::Output
                if is_visible && !self.is_modal_open && matches!(ctx.status.state, State::Play) =>
            {
                self.run()?;
            }

            UiEvent::Hidden if is_visible && !self.is_modal_open => self.pause_and_clear(ctx)?,
            UiEvent::ModalOpened if is_visible => {
                if !self.is_modal_open {
                    self.pause_and_clear(ctx)?;
                }

                self.is_modal_open = true;
            }

            UiEvent::ModalClosed if is_visible => {
                self.close_modal();
                if matches!(ctx.status.state, State::Play) {
                    self.run()?;
                }
            }

            UiEvent::PlaybackStateChanged if is_visible => {
                match Self::playback_state_action(ctx.status.state) {
                    PlaybackStateAction::Start => self.run()?,
                    PlaybackStateAction::Pause => self.pause()?,
                    PlaybackStateAction::PauseAndClear => self.pause_and_clear(ctx)?,
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn resize(&mut self, area: Rect, ctx: &Ctx) -> Result<()> {
        if !self.is_modal_open {
            self.area = area;
            self.pause_and_clear(ctx)?;
            if matches!(ctx.status.state, State::Play) {
                self.run()?;
            }
        }

        Ok(())
    }

    fn on_removed_from_config(&mut self, ctx: &Ctx) -> Result<()> {
        if !ctx.config.active_panes.contains(&PaneType::Waveform) {
            self.stop()?;
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::VecDeque;

    use crossterm::style::Color;
    use ratatui::layout::Rect;
    use rmpc_mpd::commands::State;

    use super::render;
    use crate::{
        config::{
            theme::waveform::WaveformTheme,
            waveform::{Waveform, WaveformDisplayMode},
        },
        ui::panes::waveform::{
            PlaybackStateAction,
            WaveformPane,
            WaveformSamples,
            capture_target_changed,
        },
    };

    fn test_config(display_mode: WaveformDisplayMode) -> Waveform {
        Waveform {
            source: String::new(),
            sample_rate: 1,
            framerate: 1,
            duration_ms: 100,
            output_latency_ms: 0,
            gain: 1.0,
            display_mode,
            fill_wave: false,
            render_silence: false,
        }
    }

    fn test_theme() -> WaveformTheme {
        WaveformTheme {
            trace_symbol: "▌".to_owned(),
            trace_color: Color::Blue,
            show_center_line: false,
            center_line_color: Color::DarkGrey,
            background_color: Color::Reset,
        }
    }

    #[test]
    fn decodes_and_downmixes_16_bit_stereo_pcm() {
        let config = test_config(WaveformDisplayMode::MonoMix);
        let mut samples = WaveformSamples::new(config.display_mode);
        let mut pending = Vec::new();
        WaveformSamples::decode_pcm(&[0xff, 0x7f, 0x01], &config, &mut pending, &mut samples);
        assert!(!samples.has_width(1));
        WaveformSamples::decode_pcm(&[0x80], &config, &mut pending, &mut samples);
        let WaveformSamples::Mono(samples) = samples else {
            panic!("expected mono samples");
        };

        assert!(samples[0].abs() < 0.001);
    }

    #[test]
    fn decodes_stereo_pcm_into_separate_channels() {
        let config = test_config(WaveformDisplayMode::Stereo);
        let mut samples = WaveformSamples::new(config.display_mode);
        let mut pending = Vec::new();

        WaveformSamples::decode_pcm(&[0xff, 0x7f, 0x00, 0x80], &config, &mut pending, &mut samples);

        let WaveformSamples::Stereo { left, right } = samples else {
            panic!("expected stereo samples");
        };

        assert!(left[0] > 0.99);
        assert!((right[0] + 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn consuming_samples_preserves_the_unrendered_queue() {
        let mut incoming = WaveformSamples::Mono([0.1, 0.2, 0.3].into());
        let mut displayed = WaveformSamples::Mono(VecDeque::default());

        assert_eq!(incoming.consume_into(2, &mut displayed), 2);
        let WaveformSamples::Mono(displayed) = displayed else {
            panic!("expected mono samples");
        };

        let WaveformSamples::Mono(incoming) = incoming else {
            panic!("expected mono samples");
        };

        assert_eq!(displayed, VecDeque::from([0.1_f32, 0.2]));
        assert_eq!(incoming, VecDeque::from([0.3_f32]));
    }

    #[test]
    fn reconfiguring_unrelated_fields_keeps_the_capture_running() {
        let config = test_config(WaveformDisplayMode::MonoMix);
        let reconfigured = Waveform {
            gain: 2.0,
            display_mode: WaveformDisplayMode::Stereo,
            fill_wave: true,
            render_silence: true,
            framerate: 10,
            duration_ms: 200,
            output_latency_ms: 10,
            ..config.clone()
        };

        assert!(!capture_target_changed(&config, &reconfigured));
    }

    #[test]
    fn reconfiguring_the_source_or_sample_rate_restarts_the_capture() {
        let config = test_config(WaveformDisplayMode::MonoMix);
        let new_source = Waveform { source: "other".to_owned(), ..config.clone() };
        let new_rate = Waveform { sample_rate: config.sample_rate + 1, ..config.clone() };

        assert!(capture_target_changed(&config, &new_source));
        assert!(capture_target_changed(&config, &new_rate));
    }

    #[test]
    fn pausing_keeps_the_rendered_waveform() {
        assert_eq!(WaveformPane::playback_state_action(State::Pause), PlaybackStateAction::Pause);
        assert_eq!(
            WaveformPane::playback_state_action(State::Stop),
            PlaybackStateAction::PauseAndClear
        );
    }

    #[test]
    fn silent_envelopes_require_both_bounds_on_the_center_row() {
        assert!(render::is_silent_envelope(None, (10, 10), None, 10));
        assert!(render::is_silent_envelope(Some((10, 10)), (10, 10), Some((10, 10)), 10));
        assert!(!render::is_silent_envelope(None, (9, 10), None, 10));
        assert!(!render::is_silent_envelope(Some((9, 10)), (10, 10), Some((10, 10)), 10));
        assert!(!render::is_silent_envelope(Some((10, 10)), (10, 10), Some((10, 11)), 10));
    }

    #[test]
    fn column_extremes_partitions_samples_across_terminal_width() {
        let samples = [1.0, 3.0, 5.0, 7.0, 9.0];

        let (minimum, maximum) = render::column_extremes(&samples, 0, 2);
        assert!((minimum - 1.0).abs() < f32::EPSILON);
        assert!((maximum - 3.0).abs() < f32::EPSILON);

        let (minimum, maximum) = render::column_extremes(&samples, 1, 2);
        assert!((minimum - 5.0).abs() < f32::EPSILON);
        assert!((maximum - 9.0).abs() < f32::EPSILON);
    }

    #[test]
    fn column_extremes_preserve_a_peak_diluted_by_a_naive_average() {
        // A single loud transient among many quiet samples averages to
        // nearly zero, but the envelope must still surface the real peak.
        let mut samples = vec![0.0; 63];
        samples.push(0.9);
        samples.push(0.0);

        let (minimum, maximum) = render::column_extremes(&samples, 0, 1);

        assert!(minimum.abs() < f32::EPSILON);
        assert!((maximum - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn envelope_trace_without_fill_covers_only_the_recorded_range() {
        let theme = test_theme();
        let mut writer = Vec::new();

        render::draw_envelope_trace(&mut writer, 0, 2, 5, 10, false, &theme).unwrap();

        assert_eq!(count_trace_symbols(&writer, &theme), 4); // rows 2..=5
    }

    #[test]
    fn envelope_trace_with_fill_extends_to_the_center_row() {
        let theme = test_theme();
        let mut writer = Vec::new();

        render::draw_envelope_trace(&mut writer, 0, 2, 5, 10, true, &theme).unwrap();

        assert_eq!(count_trace_symbols(&writer, &theme), 9); // rows 2..=10
    }

    fn count_trace_symbols(writer: &[u8], theme: &WaveformTheme) -> usize {
        String::from_utf8_lossy(writer).matches(theme.trace_symbol.as_str()).count()
    }

    #[test]
    fn centered_channel_without_fill_covers_only_the_recorded_range() {
        let config =
            Waveform { fill_wave: false, ..test_config(WaveformDisplayMode::StereoCentered) };

        let theme = test_theme();
        let mut writer = Vec::new();
        let samples = [-0.2, 0.9]; // Near distance 0.2 and far distance 0.9 land on rows 3 and 0.

        render::render_centered_channel(
            &mut writer,
            Rect::new(0, 0, 1, 9),
            &samples,
            4,
            true,
            &config,
            &theme,
        )
        .unwrap();

        assert_eq!(count_trace_symbols(&writer, &theme), 4); // rows 0..=3
    }

    #[test]
    fn centered_channel_with_fill_still_extends_to_the_middle() {
        let config =
            Waveform { fill_wave: true, ..test_config(WaveformDisplayMode::StereoCentered) };
        let theme = test_theme();
        let mut writer = Vec::new();
        let samples = [-0.2, 0.9];

        render::render_centered_channel(
            &mut writer,
            Rect::new(0, 0, 1, 9),
            &samples,
            4,
            true,
            &config,
            &theme,
        )
        .unwrap();

        assert_eq!(count_trace_symbols(&writer, &theme), 5); // rows 0..=4
    }

    #[test]
    fn stereo_layout_can_give_the_top_channel_a_zero_height_area() {
        let (top, bottom) = render::stereo_areas(Rect::new(0, 0, 8, 1));

        assert_eq!(top, Rect::new(0, 0, 8, 0));
        assert_eq!(bottom, Rect::new(0, 0, 8, 1));
    }

    #[test]
    fn rendering_a_zero_height_channel_does_not_panic() {
        let config = test_config(WaveformDisplayMode::Stereo);
        let theme = test_theme();
        let mut writer = Vec::new();

        render::render_channel(&mut writer, Rect::new(0, 0, 8, 0), &[0.5, -0.5], &config, &theme)
            .unwrap();

        assert!(writer.is_empty());
    }

    #[test]
    fn sample_to_row_maps_amplitude_to_terminal_rows() {
        let area = Rect::new(0, 0, 1, 9);
        let middle = area.y + area.height / 2;

        assert_eq!(render::sample_to_row(0.9, area, middle), 0);
        assert_eq!(render::sample_to_row(-0.9, area, middle), 8);
        assert_eq!(render::sample_to_row(0.0, area, middle), middle);
    }

    #[test]
    fn centered_stereo_channels_extend_away_from_the_middle() {
        assert_eq!(render::centered_sample_y(-1.0, 4, 10, true), 6);
        assert_eq!(render::centered_sample_y(1.0, 4, 10, false), 14);
        assert_eq!(render::centered_sample_y(0.0, 4, 10, true), 10);
        assert_eq!(render::centered_sample_y(0.0, 4, 10, false), 10);
    }
}
