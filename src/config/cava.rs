use std::fmt::Write;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use strum::Display;

use super::defaults;

#[derive(Debug, Default, Clone)]
pub struct Cava {
    pub framerate: u16,
    pub autosens: bool,
    pub sensitivity: u16,
    pub lower_cutoff_freq: Option<u16>,
    pub higher_cutoff_freq: Option<u32>,
    pub input: CavaInput,
    pub smoothing: CavaSmoothing,
    pub eq: Vec<f64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct CavaFile {
    #[serde(default = "defaults::u16::<60>")]
    framerate: u16,
    #[serde(default = "defaults::bool::<true>")]
    pub autosens: bool,
    #[serde(default = "defaults::u16::<100>")]
    pub sensitivity: u16,
    #[serde(default)]
    lower_cutoff_freq: Option<u16>,
    #[serde(default)]
    higher_cutoff_freq: Option<u32>,
    input: CavaInputFile,
    #[serde(default)]
    smoothing: CavaSmoothingFile,
    #[serde(default)]
    eq: Vec<f64>,
}

#[derive(Debug, Display, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum CavaInputMethod {
    Fifo,
    Alsa,
    #[default]
    Pulse,
    Portaudio,
    Pipewire,
    Sndio,
    Oss,
    Jack,
    Shmem,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CavaSmoothingFile {
    #[serde(default)]
    monstercat: bool,
    #[serde(default)]
    waves: bool,
    #[serde(default = "defaults::u8::<77>")]
    noise_reduction: u8,
}

#[derive(Debug, Default, Clone)]
pub struct CavaSmoothing {
    pub monstercat: bool,
    pub waves: bool,
    pub noise_reduction: u8,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CavaInputFile {
    method: CavaInputMethod,
    source: String,
    #[serde(default)]
    sample_rate: Option<u32>,
    #[serde(default)]
    sample_bits: Option<u32>,
    #[serde(default)]
    channels: Option<u32>,
    #[serde(default)]
    autoconnect: Option<u32>,
}

#[derive(Debug, Default, Clone)]
pub struct CavaInput {
    pub method: CavaInputMethod,
    pub source: String,
    pub sample_rate: Option<u32>,
    pub sample_bits: Option<u32>,
    pub channels: Option<u32>,
    pub autoconnect: Option<u32>,
}

impl From<CavaFile> for Cava {
    fn from(value: CavaFile) -> Self {
        Cava {
            framerate: value.framerate,
            autosens: value.autosens,
            sensitivity: value.sensitivity,
            lower_cutoff_freq: value.lower_cutoff_freq,
            higher_cutoff_freq: value.higher_cutoff_freq,
            input: CavaInput {
                method: value.input.method,
                source: value.input.source,
                sample_rate: value.input.sample_rate,
                sample_bits: value.input.sample_bits,
                channels: value.input.channels,
                autoconnect: value.input.autoconnect,
            },
            smoothing: CavaSmoothing {
                monstercat: value.smoothing.monstercat,
                waves: value.smoothing.waves,
                noise_reduction: value.smoothing.noise_reduction,
            },
            eq: value.eq,
        }
    }
}

impl Cava {
    pub fn to_cava_config_file(&self, bars: u16) -> Result<String> {
        let mut buf = String::new();

        writeln!(buf, "[general]")?;
        writeln!(buf, "framerate = {}", self.framerate)?;
        writeln!(buf, "bars = {bars}")?;
        writeln!(buf, "autosens = {}", i8::from(self.autosens))?;
        writeln!(buf, "sensitivity = {}", self.sensitivity)?;
        if let Some(val) = self.lower_cutoff_freq {
            writeln!(buf, "lower_cutoff_freq = {val}")?;
        }
        if let Some(val) = self.higher_cutoff_freq {
            writeln!(buf, "higher_cutoff_freq = {val}")?;
        }

        writeln!(buf, "[input]")?;
        writeln!(buf, "method = {}", self.input.method)?;
        writeln!(buf, "source = {}", self.input.source)?;
        if let Some(val) = self.input.sample_rate {
            writeln!(buf, "sample_rate = {val}")?;
        }
        if let Some(val) = self.input.sample_bits {
            writeln!(buf, "sample_bits = {val}")?;
        }
        if let Some(val) = self.input.channels {
            writeln!(buf, "channels = {val}")?;
        }
        if let Some(val) = self.input.autoconnect {
            writeln!(buf, "autoconnect = {val}")?;
        }

        writeln!(buf, "[output]")?;
        writeln!(buf, "method = raw")?;
        writeln!(buf, "channels = mono")?;
        writeln!(buf, "data_format = binary")?;
        writeln!(buf, "bit_format = 16bit")?;
        writeln!(buf, "reverse = 0")?;

        writeln!(buf, "[smoothing]")?;
        writeln!(buf, "noise_reduction = {}", self.smoothing.noise_reduction)?;
        writeln!(buf, "monstercat = {}", i8::from(self.smoothing.monstercat))?;
        writeln!(buf, "waves = {}", i8::from(self.smoothing.waves))?;

        writeln!(buf, "[eq]")?;
        for (i, val) in self.eq.iter().enumerate() {
            writeln!(buf, "{i} = {val}")?;
        }

        Ok(buf)
    }
}
