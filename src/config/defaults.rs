#![allow(dead_code)]
#![allow(clippy::unnecessary_wraps)]

use std::collections::HashMap;

use super::theme::{Modifiers, ScrollbarConfigFile, StyleFile, properties::SongPropertyFile};
use crate::config::{
    tabs::{
        BorderTitlePosition,
        BordersFile,
        DirectionFile,
        PaneOrSplitFile,
        PaneTypeFile,
        SubPaneFile,
        VolumeTypeFile,
    },
    theme::{
        borders::BorderSymbolsFile,
        properties::{
            Alignment,
            PropertyFile,
            PropertyKindFile,
            PropertyKindFileOrText,
            StatusPropertyFile,
        },
        volume_slider::VolumeSliderConfigFile,
    },
};
pub fn default_column_widths() -> Vec<u16> {
    vec![20, 38, 42]
}

pub fn default_header_column_widths() -> Vec<u16> {
    vec![30, 40, 30]
}

pub fn bool<const V: bool>() -> bool {
    V
}

pub fn u8<const V: u8>() -> u8 {
    V
}

pub fn u16<const V: u16>() -> u16 {
    V
}

pub fn u32<const V: u32>() -> u32 {
    V
}

pub fn u64<const V: u64>() -> u64 {
    V
}

pub fn i32<const V: i32>() -> i32 {
    V
}

pub fn i64<const V: i64>() -> i64 {
    V
}

pub fn usize<const V: usize>() -> usize {
    V
}

pub fn default_bar_symbols() -> Vec<char> {
    "▁▂▃▄▅▆▇█".chars().collect()
}

pub fn default_inverted_bar_symbols() -> Vec<char> {
    "▔🮂🮃▀🮄🮅🮆█".chars().collect()
}

pub fn default_progress_update_interval_ms() -> Option<u64> {
    Some(1000)
}

pub fn mpd_address() -> String {
    "127.0.0.1:6600".to_string()
}

pub fn mpd_host() -> String {
    "127.0.0.1".to_string()
}

pub fn mpd_port() -> String {
    "6600".to_string()
}

pub fn disabled_album_art_protos() -> Vec<String> {
    ["http://", "https://"].into_iter().map(|p| p.to_owned()).collect()
}

pub fn default_playing_label() -> String {
    "Playing".to_string()
}

pub fn default_paused_label() -> String {
    "Paused".to_string()
}

pub fn default_stopped_label() -> String {
    "Stopped".to_string()
}

pub fn default_on_label() -> String {
    "On".to_string()
}

pub fn default_off_label() -> String {
    "Off".to_string()
}

pub fn default_oneshot_label() -> String {
    "OS".to_string()
}

pub fn default_song_sort() -> Vec<SongPropertyFile> {
    vec![
        SongPropertyFile::Disc,
        SongPropertyFile::Track,
        SongPropertyFile::Artist,
        SongPropertyFile::Title,
    ]
}

pub fn playlist_symbol() -> String {
    "P".to_owned()
}

pub fn default_tag_separator() -> String {
    " | ".to_string()
}

pub fn current_item_style() -> Option<StyleFile> {
    Some(StyleFile {
        fg: Some("black".to_string()),
        bg: Some("blue".to_string()),
        modifiers: Some(Modifiers::Bold),
    })
}

pub fn highlighted_item_style() -> Option<StyleFile> {
    Some(StyleFile { fg: Some("blue".to_string()), bg: None, modifiers: Some(Modifiers::Bold) })
}

pub fn default_preview_label_style() -> StyleFile {
    StyleFile { fg: Some("yellow".to_string()), bg: None, modifiers: None }
}

pub fn default_preview_metaga_group_heading_style() -> StyleFile {
    StyleFile { fg: Some("yellow".to_string()), bg: None, modifiers: Some(Modifiers::Bold) }
}

pub fn default_thousands_separator() -> String {
    ",".to_string()
}

pub fn default_time_unit_separator() -> String {
    ", ".to_string()
}

pub fn default_optional_time_unit_separator() -> Option<String> {
    None
}

pub fn default_scrollbar() -> Option<ScrollbarConfigFile> {
    Some(ScrollbarConfigFile::default())
}

pub fn default_trace_color() -> StyleFile {
    StyleFile { fg: Some("magenta".to_string()), bg: Some("black".to_string()), modifiers: None }
}

pub fn default_debug_color() -> StyleFile {
    StyleFile {
        fg: Some("light_green".to_string()),
        bg: Some("black".to_string()),
        modifiers: None,
    }
}

pub fn default_info_color() -> StyleFile {
    StyleFile { fg: Some("blue".to_string()), bg: Some("black".to_string()), modifiers: None }
}

pub fn default_warn_color() -> StyleFile {
    StyleFile { fg: Some("yellow".to_string()), bg: Some("black".to_string()), modifiers: None }
}

pub fn default_error_color() -> StyleFile {
    StyleFile { fg: Some("red".to_string()), bg: Some("black".to_string()), modifiers: None }
}

pub fn default_status_bar_background_color() -> StyleFile {
    StyleFile { fg: Some("black".to_string()), bg: Some("black".to_string()), modifiers: None }
}

pub fn rating_options() -> Vec<i32> {
    vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
}

pub fn components() -> HashMap<String, PaneOrSplitFile> {
    HashMap::from([
        (
            "state".to_string(),
            PaneOrSplitFile::Pane(PaneTypeFile::Property {
                align: Alignment::Left,
                scroll_speed: 0,
                content: vec![
                    PropertyFile {
                        kind: PropertyKindFileOrText::Text("[".to_string()),
                        style: Some(StyleFile {
                            fg: Some("yellow".to_string()),
                            bg: None,
                            modifiers: Some(Modifiers::Bold),
                        }),
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                            StatusPropertyFile::StateV2 {
                                playing_label: default_playing_label(),
                                paused_label: default_paused_label(),
                                stopped_label: default_stopped_label(),
                                playing_style: None,
                                paused_style: None,
                                stopped_style: None,
                            },
                        )),
                        style: Some(StyleFile {
                            fg: Some("yellow".to_string()),
                            bg: None,
                            modifiers: Some(Modifiers::Bold),
                        }),
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Text("]".to_string()),
                        style: Some(StyleFile {
                            fg: Some("yellow".to_string()),
                            bg: None,
                            modifiers: Some(Modifiers::Bold),
                        }),
                        default: None,
                    },
                ],
            }),
        ),
        (
            "elapsed_and_bitrate".to_string(),
            PaneOrSplitFile::Pane(PaneTypeFile::Property {
                align: Alignment::Left,
                scroll_speed: 0,
                content: vec![
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                            StatusPropertyFile::Elapsed,
                        )),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Text(" / ".to_string()),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                            StatusPropertyFile::Duration,
                        )),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Group(vec![
                            PropertyFile {
                                kind: PropertyKindFileOrText::Text(" (".to_string()),
                                style: None,
                                default: None,
                            },
                            PropertyFile {
                                kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                                    StatusPropertyFile::Bitrate,
                                )),
                                style: None,
                                default: None,
                            },
                            PropertyFile {
                                kind: PropertyKindFileOrText::Text(" kbps)".to_string()),
                                style: None,
                                default: None,
                            },
                        ]),
                        style: None,
                        default: None,
                    },
                ],
            }),
        ),
        (
            "title".to_string(),
            PaneOrSplitFile::Pane(PaneTypeFile::Property {
                align: Alignment::Center,
                scroll_speed: 1,
                content: vec![PropertyFile {
                    kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                        SongPropertyFile::Title,
                    )),
                    style: Some(StyleFile { fg: None, bg: None, modifiers: Some(Modifiers::Bold) }),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Text("No Song".to_string()),
                        style: Some(StyleFile {
                            fg: None,
                            bg: None,
                            modifiers: Some(Modifiers::Bold),
                        }),
                        default: None,
                    })),
                }],
            }),
        ),
        (
            "artist_and_album".to_string(),
            PaneOrSplitFile::Pane(PaneTypeFile::Property {
                align: Alignment::Center,
                scroll_speed: 1,
                content: vec![
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                            SongPropertyFile::Artist,
                        )),
                        style: Some(StyleFile {
                            fg: Some("yellow".to_string()),
                            bg: None,
                            modifiers: Some(Modifiers::Bold),
                        }),
                        default: Some(Box::new(PropertyFile {
                            kind: PropertyKindFileOrText::Text("Unknown".to_string()),
                            style: Some(StyleFile {
                                fg: Some("yellow".to_string()),
                                bg: None,
                                modifiers: Some(Modifiers::Bold),
                            }),
                            default: None,
                        })),
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Text(" - ".to_string()),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                            SongPropertyFile::Album,
                        )),
                        style: None,
                        default: Some(Box::new(PropertyFile {
                            kind: PropertyKindFileOrText::Text("Unknown Album".to_string()),
                            style: None,
                            default: None,
                        })),
                    },
                ],
            }),
        ),
        ("volume".to_string(), PaneOrSplitFile::Split {
            direction: DirectionFile::Horizontal,
            borders: BordersFile::NONE,
            panes: vec![
                SubPaneFile {
                    size: "1".to_string(),
                    borders: BordersFile::NONE,
                    border_title: Vec::new(),
                    border_title_position: BorderTitlePosition::Top,
                    border_title_alignment: Alignment::Left,
                    border_symbols: BorderSymbolsFile::default(),
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::Property {
                        align: Alignment::Left,
                        scroll_speed: 0,
                        content: vec![PropertyFile {
                            kind: PropertyKindFileOrText::Text(String::new()),
                            style: None,
                            default: None,
                        }],
                    }),
                },
                SubPaneFile {
                    size: "100%".to_string(),
                    borders: BordersFile::NONE,
                    border_title: Vec::new(),
                    border_title_position: BorderTitlePosition::Top,
                    border_title_alignment: Alignment::Left,
                    border_symbols: BorderSymbolsFile::default(),
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::Volume {
                        kind: VolumeTypeFile::Slider(VolumeSliderConfigFile {
                            symbols: crate::config::theme::volume_slider::Symbols {
                                start: None,
                                filled: "─".to_string(),
                                thumb: "●".to_string(),
                                track: "─".to_string(),
                                end: None,
                            },
                            track_style: None,
                            filled_style: None,
                            thumb_style: None,
                        }),
                    }),
                },
                SubPaneFile {
                    size: "3".to_string(),
                    borders: BordersFile::NONE,
                    border_title: Vec::new(),
                    border_title_position: BorderTitlePosition::Top,
                    border_title_alignment: Alignment::Left,
                    border_symbols: BorderSymbolsFile::default(),
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::Property {
                        align: Alignment::Right,
                        scroll_speed: 0,
                        content: vec![PropertyFile {
                            kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                                StatusPropertyFile::Volume,
                            )),
                            style: Some(StyleFile {
                                fg: Some("blue".to_string()),
                                bg: None,
                                modifiers: None,
                            }),
                            default: None,
                        }],
                    }),
                },
                SubPaneFile {
                    size: "2".to_string(),
                    borders: BordersFile::NONE,
                    border_title: Vec::new(),
                    border_title_position: BorderTitlePosition::Top,
                    border_title_alignment: Alignment::Left,
                    border_symbols: BorderSymbolsFile::default(),
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::Property {
                        align: Alignment::Left,
                        scroll_speed: 0,
                        content: vec![PropertyFile {
                            kind: PropertyKindFileOrText::Text("%".to_string()),
                            style: Some(StyleFile {
                                fg: Some("blue".to_string()),
                                bg: None,
                                modifiers: None,
                            }),
                            default: None,
                        }],
                    }),
                },
            ],
        }),
        (
            "states".to_string(),
            PaneOrSplitFile::Pane(PaneTypeFile::Property {
                align: Alignment::Right,
                scroll_speed: 0,
                content: vec![
                    PropertyFile {
                        kind: PropertyKindFileOrText::Text("[".to_string()),
                        style: Some(StyleFile {
                            fg: Some("blue".to_string()),
                            bg: None,
                            modifiers: Some(Modifiers::Bold),
                        }),
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                            StatusPropertyFile::RepeatV2 {
                                on_label: "z".to_string(),
                                off_label: "z".to_string(),
                                on_style: Some(StyleFile {
                                    fg: Some("yellow".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Bold),
                                }),
                                off_style: Some(StyleFile {
                                    fg: Some("blue".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Dim),
                                }),
                            },
                        )),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                            StatusPropertyFile::RandomV2 {
                                on_label: "x".to_string(),
                                off_label: "x".to_string(),
                                on_style: Some(StyleFile {
                                    fg: Some("yellow".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Bold),
                                }),
                                off_style: Some(StyleFile {
                                    fg: Some("blue".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Dim),
                                }),
                            },
                        )),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                            StatusPropertyFile::ConsumeV2 {
                                on_label: "c".to_string(),
                                off_label: "c".to_string(),
                                oneshot_label: "c".to_string(),
                                on_style: Some(StyleFile {
                                    fg: Some("yellow".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Bold),
                                }),
                                off_style: Some(StyleFile {
                                    fg: Some("blue".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Dim),
                                }),
                                oneshot_style: Some(StyleFile {
                                    fg: Some("red".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Dim),
                                }),
                            },
                        )),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                            StatusPropertyFile::SingleV2 {
                                on_label: "v".to_string(),
                                off_label: "v".to_string(),
                                oneshot_label: "v".to_string(),
                                on_style: Some(StyleFile {
                                    fg: Some("yellow".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Bold),
                                }),
                                off_style: Some(StyleFile {
                                    fg: Some("blue".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Dim),
                                }),
                                oneshot_style: Some(StyleFile {
                                    fg: Some("red".to_string()),
                                    bg: None,
                                    modifiers: Some(Modifiers::Bold),
                                }),
                            },
                        )),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Text("]".to_string()),
                        style: Some(StyleFile {
                            fg: Some("blue".to_string()),
                            bg: None,
                            modifiers: Some(Modifiers::Bold),
                        }),
                        default: None,
                    },
                ],
            }),
        ),
        ("header_left".to_string(), PaneOrSplitFile::Split {
            direction: DirectionFile::Vertical,
            borders: BordersFile::NONE,
            panes: vec![
                SubPaneFile {
                    size: "1".to_string(),
                    borders: BordersFile::NONE,
                    border_title: Vec::new(),
                    border_title_position: BorderTitlePosition::Top,
                    border_title_alignment: Alignment::Left,
                    border_symbols: BorderSymbolsFile::default(),
                    pane: PaneOrSplitFile::Component("state".to_string()),
                },
                SubPaneFile {
                    size: "1".to_string(),
                    borders: BordersFile::NONE,
                    border_title: Vec::new(),
                    border_title_position: BorderTitlePosition::Top,
                    border_title_alignment: Alignment::Left,
                    border_symbols: BorderSymbolsFile::default(),
                    pane: PaneOrSplitFile::Component("elapsed_and_bitrate".to_string()),
                },
            ],
        }),
        ("header_center".to_string(), PaneOrSplitFile::Split {
            direction: DirectionFile::Vertical,
            borders: BordersFile::NONE,
            panes: vec![
                SubPaneFile {
                    size: "1".to_string(),
                    borders: BordersFile::NONE,
                    border_title: Vec::new(),
                    border_title_position: BorderTitlePosition::Top,
                    border_title_alignment: Alignment::Left,
                    border_symbols: BorderSymbolsFile::default(),
                    pane: PaneOrSplitFile::Component("title".to_string()),
                },
                SubPaneFile {
                    size: "1".to_string(),
                    borders: BordersFile::NONE,
                    border_title: Vec::new(),
                    border_title_position: BorderTitlePosition::Top,
                    border_title_alignment: Alignment::Left,
                    border_symbols: BorderSymbolsFile::default(),
                    pane: PaneOrSplitFile::Component("artist_and_album".to_string()),
                },
            ],
        }),
        ("header_right".to_string(), PaneOrSplitFile::Split {
            direction: DirectionFile::Vertical,
            borders: BordersFile::NONE,
            panes: vec![
                SubPaneFile {
                    size: "1".to_string(),
                    borders: BordersFile::NONE,
                    border_title: Vec::new(),
                    border_title_position: BorderTitlePosition::Top,
                    border_title_alignment: Alignment::Left,
                    border_symbols: BorderSymbolsFile::default(),
                    pane: PaneOrSplitFile::Component("volume".to_string()),
                },
                SubPaneFile {
                    size: "1".to_string(),
                    borders: BordersFile::NONE,
                    border_title: Vec::new(),
                    border_title_position: BorderTitlePosition::Top,
                    border_title_alignment: Alignment::Left,
                    border_symbols: BorderSymbolsFile::default(),
                    pane: PaneOrSplitFile::Component("states".to_string()),
                },
            ],
        }),
    ])
}

pub fn duration_format() -> String {
    "%m:%S".to_string()
}
