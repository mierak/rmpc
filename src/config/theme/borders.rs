use std::collections::HashMap;

use ratatui::symbols::{self, border::Set};
use serde::{Deserialize, Serialize};

use crate::config::tabs::PaneConversionError;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BorderSetLib(HashMap<String, BorderSet>);

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BorderSetLibFile(HashMap<String, BorderSetLibFileEnum>);

impl TryFrom<BorderSetLibFile> for BorderSetLib {
    type Error = PaneConversionError;

    fn try_from(value: BorderSetLibFile) -> Result<Self, Self::Error> {
        let lib = BorderSetLib::default();

        Ok(Self(
            value
                .0
                .into_iter()
                .map(|(k, v)| -> Result<(String, BorderSet), PaneConversionError> {
                    Ok((k, v.into_border_set(&lib)?))
                })
                .collect::<Result<_, _>>()?,
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum BorderSetLibFileEnum {
    Custom(BorderSet),
    Inherited(BorderSetInherited),
}

// Used at runtime as a custom border symbol set
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BorderSet {
    top_left: String,
    top_right: String,
    bottom_left: String,
    bottom_right: String,
    vertical_left: String,
    vertical_right: String,
    horizontal_top: String,
    horizontal_bottom: String,
}

impl<'a> From<&'a BorderSet> for Set<'a> {
    fn from(value: &'a BorderSet) -> Self {
        Set {
            top_left: value.top_left.as_str(),
            top_right: value.top_right.as_str(),
            bottom_left: value.bottom_left.as_str(),
            bottom_right: value.bottom_right.as_str(),
            vertical_left: value.vertical_left.as_str(),
            vertical_right: value.vertical_right.as_str(),
            horizontal_top: value.horizontal_top.as_str(),
            horizontal_bottom: value.horizontal_bottom.as_str(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct BorderSetInherited {
    pub parent: Box<BorderSymbolsFile>,
    pub top_left: Option<String>,
    pub top_right: Option<String>,
    pub bottom_left: Option<String>,
    pub bottom_right: Option<String>,
    pub vertical_left: Option<String>,
    pub vertical_right: Option<String>,
    pub horizontal_top: Option<String>,
    pub horizontal_bottom: Option<String>,
}

impl BorderSetInherited {
    fn into_border_set(self, lib: &BorderSetLib) -> Result<BorderSet, PaneConversionError> {
        let sym = self.parent.into_symbols(lib)?;
        let set: Set = (&sym).into();

        Ok(BorderSet {
            top_left: self.top_left.unwrap_or_else(|| set.top_left.to_owned()),
            top_right: self.top_right.unwrap_or_else(|| set.top_right.to_owned()),
            bottom_left: self.bottom_left.unwrap_or_else(|| set.bottom_left.to_owned()),
            bottom_right: self.bottom_right.unwrap_or_else(|| set.bottom_right.to_owned()),
            vertical_left: self.vertical_left.unwrap_or_else(|| set.vertical_left.to_owned()),
            vertical_right: self.vertical_right.unwrap_or_else(|| set.vertical_right.to_owned()),
            horizontal_top: self.horizontal_top.unwrap_or_else(|| set.horizontal_top.to_owned()),
            horizontal_bottom: self
                .horizontal_bottom
                .unwrap_or_else(|| set.horizontal_bottom.to_owned()),
        })
    }
}

impl BorderSetLibFileEnum {
    fn into_border_set(self, lib: &BorderSetLib) -> Result<BorderSet, PaneConversionError> {
        match self {
            BorderSetLibFileEnum::Custom(border_set) => Ok(border_set),
            BorderSetLibFileEnum::Inherited(symbols) => symbols.into_border_set(lib),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BorderSymbolsFile {
    #[default]
    Plain,
    Rounded,
    Double,
    Thick,
    Empty,
    Full,
    ProportionalWide,
    ProportionalTall,
    OneEighthWide,
    OneEighthTall,
    Custom(BorderSet),
    Inherited(BorderSetInherited),
    Library(String),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum BorderSymbols {
    #[default]
    Plain,
    Rounded,
    Double,
    Thick,
    Empty,
    Full,
    ProportionalWide,
    ProportionalTall,
    OneEighthWide,
    OneEighthTall,
    Custom(BorderSet),
}

impl<'a> From<&'a BorderSymbols> for Set<'a> {
    fn from(value: &'a BorderSymbols) -> Self {
        match value {
            BorderSymbols::Plain => symbols::border::PLAIN,
            BorderSymbols::Rounded => symbols::border::ROUNDED,
            BorderSymbols::Double => symbols::border::DOUBLE,
            BorderSymbols::Thick => symbols::border::THICK,
            BorderSymbols::Empty => symbols::border::EMPTY,
            BorderSymbols::Full => symbols::border::FULL,
            BorderSymbols::ProportionalWide => symbols::border::PROPORTIONAL_WIDE,
            BorderSymbols::ProportionalTall => symbols::border::PROPORTIONAL_TALL,
            BorderSymbols::OneEighthWide => symbols::border::ONE_EIGHTH_WIDE,
            BorderSymbols::OneEighthTall => symbols::border::ONE_EIGHTH_TALL,
            BorderSymbols::Custom(set) => set.into(),
        }
    }
}

impl BorderSymbolsFile {
    pub fn into_symbols(self, lib: &BorderSetLib) -> Result<BorderSymbols, PaneConversionError> {
        match self {
            BorderSymbolsFile::Plain => Ok(BorderSymbols::Plain),
            BorderSymbolsFile::Rounded => Ok(BorderSymbols::Rounded),
            BorderSymbolsFile::Double => Ok(BorderSymbols::Double),
            BorderSymbolsFile::Thick => Ok(BorderSymbols::Thick),
            BorderSymbolsFile::Empty => Ok(BorderSymbols::Empty),
            BorderSymbolsFile::Full => Ok(BorderSymbols::Full),
            BorderSymbolsFile::ProportionalWide => Ok(BorderSymbols::ProportionalWide),
            BorderSymbolsFile::ProportionalTall => Ok(BorderSymbols::ProportionalTall),
            BorderSymbolsFile::OneEighthWide => Ok(BorderSymbols::OneEighthWide),
            BorderSymbolsFile::OneEighthTall => Ok(BorderSymbols::OneEighthTall),
            BorderSymbolsFile::Custom(set) => Ok(BorderSymbols::Custom(set)),
            BorderSymbolsFile::Inherited(set) => {
                Ok(BorderSymbols::Custom(set.into_border_set(lib)?))
            }
            BorderSymbolsFile::Library(name) => lib
                .0
                .get(&name)
                .map(|s| BorderSymbols::Custom(s.clone()))
                .ok_or_else(|| PaneConversionError::MissingBorderSet(name)),
        }
    }
}
