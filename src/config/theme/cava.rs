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

use super::{ConfigColor, defaults};
use crate::shared::ext::vec::VecExt;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CavaThemeFile {
    #[serde(default = "defaults::default_bar_symbol")]
    pub bar_symbol: String,
    #[serde(default)]
    pub bg_color: Option<String>,
    #[serde(default)]
    pub bar_color: CavaColorFile,
}

impl Default for CavaThemeFile {
    fn default() -> Self {
        Self {
            bar_symbol: "█".into(),
            bg_color: Some("black".to_owned()),
            bar_color: CavaColorFile::Single("blue".into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CavaTheme {
    pub bar_symbol: String,
    pub bg_color: CrosstermColor,
    pub bar_color: CavaColor,
}

impl Default for CavaTheme {
    fn default() -> Self {
        Self {
            bar_symbol: "█".into(),
            bg_color: CrosstermColor::Black,
            bar_color: CavaColor::Single(CrosstermColor::Blue),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CavaColorFile {
    Single(String),
    Rows(#[serde(deserialize_with = "vec_with_min_len_1")] Vec<String>),
    Gradient(#[serde(deserialize_with = "map_with_min_2_elements")] HashMap<u8, String>),
}

#[derive(Debug, Clone)]
pub enum CavaColor {
    Single(CrosstermColor),
    Rows(Vec<CrosstermColor>),
    Gradient(Vec<CrosstermColor>),
}

impl Default for CavaColor {
    fn default() -> Self {
        Self::Single(CrosstermColor::Reset)
    }
}

impl Default for CavaColorFile {
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
            bar_color: match self.bar_color {
                CavaColorFile::Single(c) => CavaColor::Single(
                    RatatuiColor::from(ConfigColor::try_from(c.as_bytes())?).into(),
                ),
                CavaColorFile::Rows(cs) => CavaColor::Rows(
                    cs.into_iter()
                        .map(|c| -> Result<CrosstermColor> {
                            Ok(CrosstermColor::from(RatatuiColor::from(ConfigColor::try_from(
                                c.as_bytes(),
                            )?)))
                        })
                        .try_collect()?,
                ),
                CavaColorFile::Gradient(mut cs) => {
                    let first_entry = cs
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .next()
                        .context("at least 2 elements should be guaranteed by deserialization")?
                        .1
                        .clone();
                    let last_entry = cs
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .next_back()
                        .context("at least 2 elements should be guaranteed by deserialization")?
                        .1
                        .clone();

                    cs.entry(0).or_insert(first_entry);
                    cs.entry(100).or_insert(last_entry);
                    ensure!(
                        !cs.keys().any(|k| *k > 100),
                        "Gradient keys must be in the range 0-100, got: {:?}",
                        cs.keys()
                    );

                    let cs: HashMap<u8, (u8, u8, u8)> = cs
                        .into_iter()
                        .map(|(k, v)| -> Result<_> {
                            match ConfigColor::try_from(v.as_bytes())? {
                                ConfigColor::Rgb(r, g, b) => Ok((k, (r, g, b))),
                                result => Err(anyhow::anyhow!(
                                    "Gradient colors must be RGB colors, got {:?}",
                                    result
                                )),
                            }
                        })
                        .try_collect()?;

                    let cs = cs
                        .into_iter()
                        .sorted_by_key(|x| x.0)
                        .tuple_windows()
                        .fold(HashMap::new(), |mut acc, ((a_key, a_val), (b_key, b_val))| {
                            if b_key - a_key == 0 {
                                // range only includes start and end, simply include them in the map
                                acc.insert(a_key, CrosstermColor::Rgb {
                                    r: a_val.0,
                                    g: a_val.1,
                                    b: a_val.2,
                                });
                                acc.insert(b_key, CrosstermColor::Rgb {
                                    r: b_val.0,
                                    g: b_val.1,
                                    b: b_val.2,
                                });
                            } else {
                                // interpolate values between start and end
                                let total = f64::from(b_key - a_key);
                                for i in a_key..=b_key {
                                    let progress = f64::from(i - a_key) / total;
                                    acc.insert(i, CrosstermColor::Rgb {
                                        r: lerp_u8(a_val.0, b_val.0, progress),
                                        g: lerp_u8(a_val.1, b_val.1, progress),
                                        b: lerp_u8(a_val.2, b_val.2, progress),
                                    });
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
                    CavaColor::Gradient(cs)
                }
            },
        })
    }
}

impl CavaColor {
    #[inline]
    pub fn get_color(&self, y: usize, height: u16) -> CrosstermColor {
        // The invariants here are guaranteed by the deserialization process.
        match self {
            CavaColor::Single(c) => *c,
            CavaColor::Rows(cs) => {
                *cs.get_or_last(y).expect("Rows should have at least one element")
            }
            CavaColor::Gradient(cs) => {
                let perc = (y as f64 / height as f64 * 100.0).round() as usize;
                *cs.get_or_last(perc).expect("Gradient should have at least 100 elements")
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
