use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

pub(crate) const PCM_SAMPLE_BYTES: usize = 2;
pub(crate) const CAPTURE_CHANNELS: u8 = 2;
const MAX_OUTPUT_LATENCY_MS: u32 = 1_000;
const MIN_FRAMERATE: u16 = 30;
const MIN_DURATION_MS: u32 = 10;
const MAX_DURATION_MS: u32 = 250;

#[derive(Debug, Clone)]
pub struct Waveform {
    pub source: String,
    pub sample_rate: u32,
    pub framerate: u16,
    pub duration_ms: u32,
    pub output_latency_ms: u32,
    pub gain: f32,
    pub display_mode: WaveformDisplayMode,
    pub fill_wave: bool,
    pub render_silence: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WaveformFile {
    pub source: String,
    pub sample_rate: u32,
    pub framerate: u16,
    pub duration_ms: u32,
    pub output_latency_ms: u32,
    pub gain: f32,
    pub display_mode: WaveformDisplayMode,
    pub fill_wave: bool,
    pub render_silence: bool,
}

impl Default for WaveformFile {
    fn default() -> Self {
        Self {
            source: "mpd.PipeWire".to_owned(),
            sample_rate: 44_100,
            framerate: 60,
            duration_ms: 50,
            output_latency_ms: 0,
            gain: 1.0,
            display_mode: WaveformDisplayMode::MonoMix,
            fill_wave: true,
            render_silence: false,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WaveformDisplayMode {
    Left,
    Right,
    Stereo,
    StereoCentered,
    #[default]
    MonoMix,
}

impl TryFrom<WaveformFile> for Waveform {
    type Error = anyhow::Error;

    fn try_from(value: WaveformFile) -> Result<Self> {
        ensure!(!value.source.is_empty(), "waveform.source must not be empty");

        ensure!(value.sample_rate > 0, "waveform.sample_rate must be greater than zero");

        ensure!(
            value.framerate >= MIN_FRAMERATE,
            "waveform.framerate must be at least {MIN_FRAMERATE}"
        );

        ensure!(
            (MIN_DURATION_MS..=MAX_DURATION_MS).contains(&value.duration_ms),
            "waveform.duration_ms must be between {MIN_DURATION_MS} and {MAX_DURATION_MS}"
        );

        ensure!(
            value.output_latency_ms <= MAX_OUTPUT_LATENCY_MS,
            "waveform.output_latency_ms must not exceed {MAX_OUTPUT_LATENCY_MS}"
        );

        ensure!(value.gain.is_finite() && value.gain > 0.0, "waveform.gain must be positive");

        let waveform = Self {
            source: value.source,
            sample_rate: value.sample_rate,
            framerate: value.framerate,
            duration_ms: value.duration_ms,
            output_latency_ms: value.output_latency_ms,
            gain: value.gain,
            display_mode: value.display_mode,
            fill_wave: value.fill_wave,
            render_silence: value.render_silence,
        };

        waveform.sample_count_for_duration()?;
        Ok(waveform)
    }
}

impl Waveform {
    /// PCM frame count to cover the waveform duration.
    pub fn sample_count_for_duration(&self) -> Result<usize> {
        self.sample_count_for_millis(self.duration_ms)
    }

    pub fn sample_count_for_millis(&self, millis: u32) -> Result<usize> {
        usize::try_from(u64::from(self.sample_rate) * u64::from(millis) / 1_000)
            .context("waveform sample rate and duration are too large")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{Waveform, WaveformFile};

    #[test]
    fn duration_retains_the_same_audio_time_at_different_sample_rates() {
        let standard_rate = Waveform::try_from(WaveformFile {
            sample_rate: 44_100,
            duration_ms: 100,
            ..Default::default()
        })
        .unwrap();

        let high_rate = Waveform::try_from(WaveformFile {
            sample_rate: 96_000,
            duration_ms: 100,
            ..Default::default()
        })
        .unwrap();

        assert_eq!(standard_rate.sample_count_for_duration().unwrap(), 4_410);
        assert_eq!(high_rate.sample_count_for_duration().unwrap(), 9_600);
    }

    #[test]
    fn output_latency_converts_to_source_frames() {
        let waveform = Waveform::try_from(WaveformFile {
            sample_rate: 44_100,
            output_latency_ms: 500,
            ..Default::default()
        })
        .unwrap();

        assert_eq!(waveform.sample_count_for_millis(waveform.output_latency_ms).unwrap(), 22_050);
    }
}
