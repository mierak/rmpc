#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
use std::collections::HashMap;

use anyhow::{Context, Result, ensure};
use crossterm::style::Color as CrosstermColor;
use itertools::Itertools;
use ratatui::style::Color as RatatuiColor;
use serde::{Deserialize, Deserializer, Serialize};

use super::ConfigColor;
use crate::shared::ext::vec::VecExt;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CavaThemeFile {
    pub bar_symbol: String,
    pub bg_color: Option<String>,
    pub colors: CavaColorFileOpt,
}

impl Default for CavaThemeFile {
    fn default() -> Self {
        Self {
            bar_symbol: "█".into(),
            bg_color: Some("black".to_owned()),
            colors: CavaColorFileOpt::Single("blue".into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CavaTheme {
    pub bar_symbol: String,
    pub bg_color: CrosstermColor,
    pub colors: CavaColorOpt,
}

impl Default for CavaTheme {
    fn default() -> Self {
        Self {
            bar_symbol: "█".into(),
            bg_color: CrosstermColor::Black,
            colors: CavaColorOpt::Single(CrosstermColor::Blue),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CavaColorFileOpt {
    Single(String),
    Rows(#[serde(deserialize_with = "vec_with_min_len_1")] Vec<String>),
    Gradient(#[serde(deserialize_with = "map_with_min_2_elements")] HashMap<u8, (u8, u8, u8)>),
}

#[derive(Debug, Clone)]
pub enum CavaColorOpt {
    Single(CrosstermColor),
    Rows(Vec<CrosstermColor>),
    Gradient(Vec<(u8, u8, u8)>),
}

impl Default for CavaColorOpt {
    fn default() -> Self {
        Self::Single(CrosstermColor::Reset)
    }
}

impl Default for CavaColorFileOpt {
    fn default() -> Self {
        Self::Single("blue".into())
    }
}

fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    let result = a as f64 + (b as f64 - a as f64) * t;
    result.round().clamp(0.0, 255.0) as u8
}

impl CavaThemeFile {
    pub fn into_config(self, default_bg_color: Option<RatatuiColor>) -> Result<CavaTheme> {
        Ok(CavaTheme {
            bar_symbol: self.bar_symbol,
            bg_color: self
                .bg_color
                .map(|c| -> Result<RatatuiColor> {
                    Ok(RatatuiColor::from(ConfigColor::try_from(c.as_bytes())?))
                })
                .transpose()?
                .or(default_bg_color)
                .map_or(CrosstermColor::Reset, CrosstermColor::from),
            colors: match self.colors {
                CavaColorFileOpt::Single(c) => CavaColorOpt::Single(
                    RatatuiColor::from(ConfigColor::try_from(c.as_bytes())?).into(),
                ),
                CavaColorFileOpt::Rows(cs) => CavaColorOpt::Rows(
                    cs.into_iter()
                        .map(|c| -> Result<CrosstermColor> {
                            Ok(CrosstermColor::from(RatatuiColor::from(ConfigColor::try_from(
                                c.as_bytes(),
                            )?)))
                        })
                        .try_collect()?,
                ),
                CavaColorFileOpt::Gradient(mut cs) => {
                    let first_entry = *cs
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .next()
                        .context("at least 2 elements should be guaranteed by deserialization")?
                        .1;
                    let last_entry = *cs
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .next_back()
                        .context("at least 2 elements should be guaranteed by deserialization")?
                        .1;

                    cs.entry(0).or_insert(first_entry);
                    cs.entry(100).or_insert(last_entry);

                    let cs = cs
                        .into_iter()
                        .sorted_by_key(|x| x.0)
                        .tuple_windows()
                        .fold(HashMap::new(), |mut acc, ((a_key, a_val), (b_key, b_val))| {
                            if b_key - a_key == 0 {
                                // range only includes start and end, simply include them in the map
                                acc.insert(a_key, a_val);
                                acc.insert(b_key, b_val);
                            } else {
                                // interpolate values between start and end
                                let total = f64::from(b_key - a_key);
                                for i in a_key..=b_key {
                                    let progress = f64::from(i - a_key) / total;
                                    acc.insert(
                                        i,
                                        (
                                            lerp_u8(a_val.0, b_val.0, progress),
                                            lerp_u8(a_val.1, b_val.1, progress),
                                            lerp_u8(a_val.2, b_val.2, progress),
                                        ),
                                    );
                                }
                            }
                            acc
                        })
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|v| v.1)
                        .copied()
                        .collect_vec();

                    ensure!(
                        cs.len() >= 100,
                        "Something went wrong when precalculating gradient, expected at least 100 colors, got {}. Please report this issue along with your config.",
                        cs.len()
                    );
                    CavaColorOpt::Gradient(cs)
                }
            },
        })
    }
}

impl CavaColorOpt {
    #[inline]
    pub fn get_color(&self, y: usize, height: u16) -> CrosstermColor {
        // The invariants here are guaranteed by the deserialization process.
        match self {
            CavaColorOpt::Single(c) => *c,
            CavaColorOpt::Rows(cs) => {
                *cs.get_or_last(y).expect("Rows should have at least one element")
            }
            CavaColorOpt::Gradient(cs) => {
                let perc = (y as f64 / height as f64 * 100.0).round() as usize;
                let (r, g, b) =
                    *cs.get_or_last(perc).expect("Gradient should have at least 100 elements");

                CrosstermColor::Rgb { r, g, b }
            }
        }
    }
}

fn map_with_min_2_elements<'de, D, K, V>(deserializer: D) -> Result<HashMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
    K: std::cmp::Eq + Deserialize<'de> + std::hash::Hash,
{
    let v = HashMap::deserialize(deserializer)?;
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
