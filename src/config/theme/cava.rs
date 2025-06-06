use crossterm::style::Color as CrosstermColor;
use itertools::Itertools;
use ratatui::style::Color as RatatuiColor;
use serde::{Deserialize, Deserializer, Serialize};

use super::ConfigColor;
use crate::shared::ext::vec::VecExt;

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CavaThemeFile {
    pub bar_symbol: String,
    pub colors: CavaColorFileOpt,
}

#[derive(Debug, Default, Clone)]
pub struct CavaTheme {
    pub bar_symbol: String,
    pub colors: CavaColorOpt,
}

#[derive(Debug, Clone, Copy)]
pub struct CavaColor {
    pub bar: CrosstermColor,
    pub bg: CrosstermColor,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CavaColorFile {
    bar: String,
    bg: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CavaColorFileOpt {
    Single(CavaColorFile),
    Rows(#[serde(deserialize_with = "vec_with_min_len_1")] Vec<CavaColorFile>),
    Gradient(#[serde(deserialize_with = "vec_with_min_len_2")] Vec<(u8, u8, u8)>),
}

#[derive(Debug, Clone)]
pub enum CavaColorOpt {
    Single(CavaColor),
    Rows(Vec<CavaColor>),
    Gradient(Vec<(u8, u8, u8)>),
}

impl Default for CavaColor {
    fn default() -> Self {
        CavaColor { bar: CrosstermColor::Reset, bg: CrosstermColor::Reset }
    }
}

impl Default for CavaColorOpt {
    fn default() -> Self {
        Self::Single(CavaColor { bar: CrosstermColor::Reset, bg: CrosstermColor::Reset })
    }
}

impl Default for CavaColorFileOpt {
    fn default() -> Self {
        Self::Single(CavaColorFile { bar: "red".into(), bg: "blue".into() })
    }
}

impl TryFrom<CavaColorFile> for CavaColor {
    type Error = anyhow::Error;

    fn try_from(value: CavaColorFile) -> Result<Self, Self::Error> {
        Ok(CavaColor {
            bar: RatatuiColor::from(ConfigColor::try_from(value.bar.as_bytes())?).into(),
            bg: RatatuiColor::from(ConfigColor::try_from(value.bg.as_bytes())?).into(),
        })
    }
}

impl TryFrom<CavaThemeFile> for CavaTheme {
    type Error = anyhow::Error;

    fn try_from(value: CavaThemeFile) -> Result<Self, Self::Error> {
        Ok(Self {
            bar_symbol: value.bar_symbol,
            colors: match value.colors {
                CavaColorFileOpt::Single(c) => CavaColorOpt::Single(c.try_into()?),
                CavaColorFileOpt::Rows(cs) => {
                    CavaColorOpt::Rows(cs.into_iter().map(|c| c.try_into()).try_collect()?)
                }
                CavaColorFileOpt::Gradient(cs) => CavaColorOpt::Gradient(cs),
            },
        })
    }
}

fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    let result = a as f64 + (b as f64 - a as f64) * t;
    result.round().clamp(0.0, 255.0) as u8
}

impl CavaColorOpt {
    pub fn get_color(&self, idx: usize, perc: f64) -> CavaColor {
        // Minimum vec sizes are ensured during deserialization
        match self {
            CavaColorOpt::Single(c) => *c,
            CavaColorOpt::Rows(cs) => {
                *cs.get_or_last(idx).expect("Rows should have at least one element")
            }
            CavaColorOpt::Gradient(c) => {
                let first = c.first().expect("Gradient should have at least two elements");
                let last = c.last().expect("Gradient should have at least two elements");
                let perc = (perc * 1.3).min(1.0);

                return CavaColor {
                    bar: CrosstermColor::Rgb {
                        r: lerp_u8(first.0, last.0, perc),
                        g: lerp_u8(first.1, last.1, perc),
                        b: lerp_u8(first.2, last.2, perc),
                    },
                    bg: CrosstermColor::Black,
                };
            }
        }
    }
}

fn vec_with_min_len_2<'de, D, V>(deserializer: D) -> Result<Vec<V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    let v = Vec::deserialize(deserializer)?;
    if v.len() < 2 {
        return Err(serde::de::Error::custom(format!(
            "Expected at least 2 elements, got {}",
            v.len()
        )));
    }
    Ok(v)
}

fn vec_with_min_len_1<'de, D, V>(deserializer: D) -> Result<Vec<V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    let v = Vec::deserialize(deserializer)?;
    if v.is_empty() {
        return Err(serde::de::Error::custom(format!(
            "Expected at least 1 element, got {}",
            v.len()
        )));
    }
    Ok(v)
}
