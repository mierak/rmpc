use std::collections::VecDeque;

use crate::config::waveform::{CAPTURE_CHANNELS, PCM_SAMPLE_BYTES, Waveform, WaveformDisplayMode};

#[derive(Debug)]
pub(super) enum WaveformSamples {
    Mono(VecDeque<f32>),
    Stereo { left: VecDeque<f32>, right: VecDeque<f32> },
}

impl WaveformSamples {
    pub(super) fn new(display_mode: WaveformDisplayMode) -> Self {
        if matches!(display_mode, WaveformDisplayMode::Stereo | WaveformDisplayMode::StereoCentered)
        {
            Self::Stereo { left: VecDeque::new(), right: VecDeque::new() }
        } else {
            Self::Mono(VecDeque::new())
        }
    }

    pub(super) fn trim_to(&mut self, maximum: usize) {
        match self {
            Self::Mono(samples) => {
                samples.drain(..samples.len().saturating_sub(maximum));
            }

            Self::Stereo { left, right } => {
                left.drain(..left.len().saturating_sub(maximum));
                right.drain(..right.len().saturating_sub(maximum));
            }
        }
    }

    pub(super) fn has_width(&self, width: usize) -> bool {
        match self {
            Self::Mono(samples) => samples.len() >= width,
            Self::Stereo { left, right } => left.len() >= width && right.len() >= width,
        }
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Mono(samples) => samples.len(),
            Self::Stereo { left, .. } => left.len(),
        }
    }

    pub(super) fn consume_into(&mut self, maximum: usize, output: &mut Self) -> usize {
        match (self, output) {
            (Self::Mono(input), Self::Mono(output)) => {
                let count = maximum.min(input.len());
                output.extend(input.drain(..count));
                count
            }
            (
                Self::Stereo { left: input_left, right: input_right },
                Self::Stereo { left: output_left, right: output_right },
            ) => {
                let count = maximum.min(input_left.len()).min(input_right.len());
                output_left.extend(input_left.drain(..count));
                output_right.extend(input_right.drain(..count));
                count
            }
            _ => unreachable!("waveform sample layout must not change while playing"),
        }
    }

    pub(super) fn decode_pcm(
        bytes: &[u8],
        config: &Waveform,
        pending: &mut Vec<u8>,
        output: &mut Self,
    ) -> usize {
        let frame_bytes = usize::from(CAPTURE_CHANNELS) * PCM_SAMPLE_BYTES;
        pending.extend_from_slice(bytes);
        let complete_bytes = pending.len() / frame_bytes * frame_bytes;

        for frame in pending[..complete_bytes].chunks_exact(frame_bytes) {
            let sample = |channel: usize| {
                f32::from(i16::from_le_bytes([
                    frame[channel * PCM_SAMPLE_BYTES],
                    frame[channel * PCM_SAMPLE_BYTES + 1],
                ])) / f32::from(i16::MAX)
            };

            let apply_gain = |value: f32| (value * config.gain).clamp(-1.0, 1.0);
            match config.display_mode {
                WaveformDisplayMode::Left => output.push_mono(apply_gain(sample(0))),
                WaveformDisplayMode::Right => output.push_mono(apply_gain(sample(1))),
                WaveformDisplayMode::MonoMix => {
                    output.push_mono(apply_gain(f32::midpoint(sample(0), sample(1))));
                }

                WaveformDisplayMode::Stereo | WaveformDisplayMode::StereoCentered => {
                    output.push_stereo(apply_gain(sample(0)), apply_gain(sample(1)));
                }
            }
        }

        pending.drain(..complete_bytes);
        complete_bytes / frame_bytes
    }

    fn push_mono(&mut self, sample: f32) {
        let Self::Mono(samples) = self else {
            unreachable!("stereo waveform requires stereo samples");
        };

        samples.push_back(sample);
    }

    fn push_stereo(&mut self, left_sample: f32, right_sample: f32) {
        let Self::Stereo { left, right } = self else {
            unreachable!("mono waveform requires a mixed sample");
        };

        left.push_back(left_sample);
        right.push_back(right_sample);
    }
}
