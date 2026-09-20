use std::time::{Duration, Instant};

use anyhow::Result;

use super::samples::WaveformSamples;
use crate::config::waveform::Waveform;

const PRIMING_MARGIN_MS: u32 = 25;
const INPUT_BUFFER_MARGIN_MS: u32 = 500;
const MINIMUM_POLL_INTERVAL: Duration = Duration::from_micros(250);
const MAXIMUM_POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Playback timing and buffering policy for one active waveform session.
pub(super) struct PlaybackBuffering {
    displayed_capacity: usize,
    incoming_capacity: usize,
    target_queued_frames: usize,
    frame_duration: Duration,
    last_frame: Instant,
    frame_credit: f64,
    primed: bool,
    sample_rate: u32,
    framerate: u16,
}

impl PlaybackBuffering {
    pub(super) fn new(config: &Waveform) -> Result<Self> {
        let target_queued_frames = config
            .sample_count_for_millis(config.output_latency_ms.saturating_add(PRIMING_MARGIN_MS))?
            .max(1);

        let incoming_capacity = config
            .sample_count_for_millis(
                config.output_latency_ms.saturating_add(INPUT_BUFFER_MARGIN_MS),
            )?
            .max(target_queued_frames);

        Ok(Self {
            displayed_capacity: config.sample_count_for_duration()?,
            incoming_capacity,
            target_queued_frames,
            frame_duration: Duration::from_secs_f64(1.0 / f64::from(config.framerate)),
            last_frame: Instant::now(),
            frame_credit: 0.0,
            primed: false,
            sample_rate: config.sample_rate,
            framerate: config.framerate,
        })
    }

    pub(super) fn poll_interval(&self) -> Duration {
        (self.frame_duration / 4).clamp(MINIMUM_POLL_INTERVAL, MAXIMUM_POLL_INTERVAL)
    }

    pub(super) fn trim_incoming(&self, samples: &mut WaveformSamples) {
        samples.trim_to(self.incoming_capacity);
    }

    pub(super) fn trim_displayed(&self, samples: &mut WaveformSamples) {
        samples.trim_to(self.displayed_capacity);
    }

    /// Returns the number of frames to transfer to the display history when a
    /// frame is due. `None` means the queue is not primed or lacks enough data.
    pub(super) fn frames_to_consume(
        &mut self,
        queued_frames: usize,
        now: Instant,
    ) -> Option<usize> {
        if now.duration_since(self.last_frame) < self.frame_duration {
            return None;
        }

        if !self.primed && queued_frames >= self.target_queued_frames {
            self.primed = true;
        }

        if !self.primed {
            self.last_frame = now;
            return None;
        }

        self.frame_credit += f64::from(self.sample_rate) / f64::from(self.framerate);
        let normal_frames = self.frame_credit.floor() as usize;
        self.frame_credit -= normal_frames as f64;
        if queued_frames < normal_frames {
            self.last_frame = now;
            return None;
        }

        let catch_up_frames =
            queued_frames.saturating_sub(self.target_queued_frames).min(normal_frames);

        self.last_frame = now;
        Some(normal_frames + catch_up_frames)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::PlaybackBuffering;
    use crate::config::waveform::{Waveform, WaveformDisplayMode};

    fn config() -> Waveform {
        Waveform {
            source: String::new(),
            sample_rate: 44_100,
            framerate: 60,
            duration_ms: 100,
            output_latency_ms: 0,
            gain: 1.0,
            display_mode: WaveformDisplayMode::MonoMix,
            fill_wave: false,
            render_silence: false,
        }
    }

    #[test]
    fn paced_consumption_matches_the_configured_sample_rate() {
        let config = config();
        let mut buffering = PlaybackBuffering::new(&config).unwrap();
        let start = buffering.last_frame;
        let consumed: usize = (1..=usize::from(config.framerate))
            .filter_map(|frame| {
                buffering.frames_to_consume(
                    buffering.target_queued_frames,
                    start + buffering.frame_duration * frame as u32,
                )
            })
            .sum();

        assert_eq!(consumed, config.sample_rate as usize);
        assert!(buffering.frame_credit.abs() < f64::EPSILON);
    }

    #[test]
    fn catch_up_consumption_is_bounded_to_one_extra_frame_interval() {
        let config = config();
        let mut buffering = PlaybackBuffering::new(&config).unwrap();
        buffering.primed = true;
        buffering.target_queued_frames = 100;
        let now = buffering.last_frame + buffering.frame_duration;

        assert_eq!(buffering.frames_to_consume(1_000, now), Some(1_470));
    }
}
