use std::collections::HashMap;

use chumsky::prelude::*;

use super::{
    StyleFile,
    properties::{
        PropertyFile,
        PropertyKindFile,
        PropertyKindFileOrText,
        SongPropertyFile,
        StatusPropertyFile,
        WidgetPropertyFile,
    },
    style::Modifiers,
};

pub fn string_parser<'a>() -> impl Parser<'a, &'a str, String, extra::Err<Rich<'a, char>>> + Clone {
    let escape_double_quoted_str = just('\\')
        .then(choice((
            just('\\'),
            just('/'),
            just('"'),
            just('b').to('\x08'),
            just('f').to('\x0C'),
            just('n').to('\n'),
            just('r').to('\r'),
            just('t').to('\t'),
            just('u').ignore_then(text::digits(16).exactly(4).to_slice().validate(
                |digits, e, emitter| {
                    char::from_u32(
                        u32::from_str_radix(digits, 16)
                            .expect("Only valid digits should have been parsed in text::digits"),
                    )
                    .unwrap_or_else(|| {
                        emitter.emit(Rich::custom(e.span(), "invalid unicode character"));
                        '\u{FFFD}'
                    })
                },
            )),
        )))
        .ignored()
        .boxed();

    let escape_single_queoted_str =
        choice((
            just('\\').then(just('\'')).to('a').ignored(),
            just('\\')
                .then(choice((
                    just('\\'),
                    just('/'),
                    just('b').to('\x08'),
                    just('f').to('\x0C'),
                    just('n').to('\n'),
                    just('r').to('\r'),
                    just('t').to('\t'),
                    just('u').ignore_then(text::digits(16).exactly(4).to_slice().validate(
                        |digits, e, emitter| {
                            char::from_u32(u32::from_str_radix(digits, 16).expect(
                                "Only valid digits should have been parsed in text::digits",
                            ))
                            .unwrap_or_else(|| {
                                emitter.emit(Rich::custom(e.span(), "invalid unicode character"));
                                '\u{FFFD}'
                            })
                        },
                    )),
                )))
                .ignored(),
        ))
        .boxed();

    let double_quoted_string = none_of("\\\"")
        .ignored()
        .or(escape_double_quoted_str)
        .repeated()
        .to_slice()
        .try_map(|v, span| unescape(v, '"', span))
        .delimited_by(just('"'), just('"'))
        .boxed();

    let single_quoted_string = none_of("\\'")
        .ignored()
        .or(escape_single_queoted_str)
        .repeated()
        .to_slice()
        .try_map(|v, span| unescape(v, '\'', span))
        .delimited_by(just("'"), just("'"))
        .boxed();

    double_quoted_string.clone().or(single_quoted_string.clone()).labelled("string")
}

pub fn parser<'a>()
-> impl Parser<'a, &'a str, Vec<PropertyFile<PropertyKindFile>>, extra::Err<Rich<'a, char>>> {
    let ident = text::ascii::ident();

    let string = string_parser();

    let modifiers = choice((
        just("bold"),
        just("dim"),
        just("italic"),
        just("underlined"),
        just("reversed"),
        just("crossedout"),
    ))
    .separated_by(just(',').padded())
    .collect::<Vec<_>>()
    .map(|val| {
        if val.is_empty() {
            return None;
        }

        let mut res = Modifiers::empty();
        for modifier in val {
            res = match modifier {
                "bold" => res.union(Modifiers::Bold),
                "dim" => res.union(Modifiers::Dim),
                "italic" => res.union(Modifiers::Italic),
                "underlined" => res.union(Modifiers::Underlined),
                "reversed" => res.union(Modifiers::Reversed),
                "crossedout" => res.union(Modifiers::CrossedOut),
                _ => res,
            };
        }

        Some(res)
    });

    let color = choice((
        ident.map(|c: &str| StringOrModifiers::String(c.to_owned())),
        just('#').then(ident).map(|(_, hex): (_, &str)| {
            let mut hex = hex.to_owned();
            hex.insert(0, '#');
            StringOrModifiers::String(hex)
        }),
        text::int(10).map(|n: &str| StringOrModifiers::String(n.to_owned())),
    ));

    let style_file = choice((
        just("fg").then_ignore(just(':').padded()).then(color),
        just("bg").then_ignore(just(':').padded()).then(color),
        just("mods")
            .then_ignore(just(':').padded())
            .then(modifiers.map(StringOrModifiers::Modifiers)),
    ))
    .separated_by(just(',').padded())
    .collect::<HashMap<_, _>>()
    .map(|mut m| StyleFile {
        fg: m.remove("fg").get_string(),
        bg: m.remove("bg").get_string(),
        modifiers: m.remove("mods").get_modifiers(),
    })
    .delimited_by(just('{'), just('}'))
    .boxed();

    let label = string.clone().map(|v: String| StyleOrLabel::Label(v));
    let style = style_file.clone().map(StyleOrLabel::Style);

    let generic_property = ident
        .then(
            ident
                .then_ignore(just(':').padded())
                .then(label.or(style))
                .separated_by(just(',').padded())
                .collect::<HashMap<_, _>>()
                .delimited_by(just('('), just(')'))
                .or_not(),
        )
        .boxed();

    let status_property = generic_property
        .clone()
        .try_map(|(key, mut properties), span| match key {
            "volume" => Ok(PropertyKindFile::Status(StatusPropertyFile::Volume)),
            "repeat" => Ok(PropertyKindFile::Status(StatusPropertyFile::RepeatV2 {
                on_label: properties.get_label("onLabel", "On"),
                off_label: properties.get_label("offLabel", "Off"),
                on_style: properties.get_style("onStyle"),
                off_style: properties.get_style("offStyle"),
            })),
            "random" => Ok(PropertyKindFile::Status(StatusPropertyFile::RandomV2 {
                on_label: properties.get_label("onLabel", "On"),
                off_label: properties.get_label("offLabel", "Off"),
                on_style: properties.get_style("onStyle"),
                off_style: properties.get_style("offStyle"),
            })),
            "single" => Ok(PropertyKindFile::Status(StatusPropertyFile::SingleV2 {
                on_label: properties.get_label("onLabel", "On"),
                off_label: properties.get_label("offLabel", "Off"),
                oneshot_label: properties.get_label("oneshotLabel", "Oneshot"),
                on_style: properties.get_style("onStyle"),
                off_style: properties.get_style("offStyle"),
                oneshot_style: properties.get_style("oneshotStyle"),
            })),
            "consume" => Ok(PropertyKindFile::Status(StatusPropertyFile::ConsumeV2 {
                on_label: properties.get_label("onLabel", "On"),
                off_label: properties.get_label("offLabel", "Off"),
                oneshot_label: properties.get_label("oneshotLabel", "Oneshot"),
                on_style: properties.get_style("onStyle"),
                off_style: properties.get_style("offStyle"),
                oneshot_style: properties.get_style("oneshotStyle"),
            })),
            "state" => Ok(PropertyKindFile::Status(StatusPropertyFile::StateV2 {
                playing_label: properties.get_label("playingLabel", "Playing"),
                paused_label: properties.get_label("pausedLabel", "Paused"),
                stopped_label: properties.get_label("stoppedLabel", "Stopped"),
                playing_style: properties.get_style("playingStyle"),
                paused_style: properties.get_style("pausedStyle"),
                stopped_style: properties.get_style("stoppedStyle"),
            })),
            "elapsed" => Ok(PropertyKindFile::Status(StatusPropertyFile::Elapsed)),
            "duration" => Ok(PropertyKindFile::Status(StatusPropertyFile::Duration)),
            "crossfade" => Ok(PropertyKindFile::Status(StatusPropertyFile::Crossfade)),
            "bitrate" => Ok(PropertyKindFile::Status(StatusPropertyFile::Bitrate)),
            _ => Err(Rich::custom(span, "invalid status property type")),
        })
        .boxed();

    let song_property = just("s:")
        .ignore_then(generic_property.clone())
        .try_map(|(prop_name, mut properties), span| match prop_name {
            "filename" => Ok(PropertyKindFile::Song(SongPropertyFile::Filename)),
            "fileextension" => Ok(PropertyKindFile::Song(SongPropertyFile::FileExtension)),
            "file" => Ok(PropertyKindFile::Song(SongPropertyFile::File)),
            "title" => Ok(PropertyKindFile::Song(SongPropertyFile::Title)),
            "albumartist" => {
                Ok(PropertyKindFile::Song(SongPropertyFile::Other("albumartist".to_owned())))
            }
            "artist" => Ok(PropertyKindFile::Song(SongPropertyFile::Artist)),
            "album" => Ok(PropertyKindFile::Song(SongPropertyFile::Album)),
            "track" => Ok(PropertyKindFile::Song(SongPropertyFile::Track)),
            "disc" => Ok(PropertyKindFile::Song(SongPropertyFile::Disc)),
            "duration" => Ok(PropertyKindFile::Song(SongPropertyFile::Duration)),
            "tag" => {
                let Some(value) = properties.get_label_opt("value") else {
                    return Err(Rich::custom(span, "missing tag value"));
                };
                Ok(PropertyKindFile::Song(SongPropertyFile::Other(value)))
            }
            _ => Err(Rich::custom(span, "invalid song property type")),
        })
        .boxed();

    let widget_property = just("w:")
        .ignore_then(generic_property)
        .try_map(|(prop_name, mut properties), span| match prop_name {
            "volume" => Ok(PropertyKindFile::Widget(WidgetPropertyFile::Volume)),
            "states" => Ok(PropertyKindFile::Widget(WidgetPropertyFile::States {
                active_style: properties.get_style("activeStyle"),
                separator_style: properties.get_style("separatorStyle"),
            })),
            _ => Err(Rich::custom(span, "invalid widget type")),
        })
        .boxed();

    let property =
        choice((status_property, song_property, widget_property)).labelled("property").boxed();

    let sticker = just("sticker")
        .ignored()
        .then(
            just("name")
                .ignored()
                .then_ignore(just(':').padded())
                .then(string.clone())
                .delimited_by(just('('), just(')')),
        )
        .labelled("sticker");

    recursive(|prop| {
        let group = prop
            .clone()
            .padded()
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>()
            .delimited_by(just('[').padded(), just(']').padded())
            .labelled("group");

        let prop_kind_or_text = choice((
            string.map(PropertyKindFileOrText::<PropertyKindFile>::Text),
            property.clone().map(PropertyKindFileOrText::Property),
            sticker.map(|((), ((), name)): (_, (_, String))| {
                PropertyKindFileOrText::<PropertyKindFile>::Sticker(name)
            }),
            group.map(PropertyKindFileOrText::Group),
        ));

        just('$')
            .ignored()
            .then(prop_kind_or_text)
            .then(style_file.clone().or_not())
            .then(just('|').ignored().then(prop).or_not())
            .map(
                |((((), kind), style), default): (
                    (((), PropertyKindFileOrText<PropertyKindFile>), Option<StyleFile>),
                    Option<(_, PropertyFile<PropertyKindFile>)>,
                )| {
                    PropertyFile { kind, style, default: default.map(|((), def)| Box::new(def)) }
                },
            )
    })
    .padded()
    .repeated()
    .collect::<Vec<_>>()
    .boxed()
}

fn unescape(input: &str, delim: char, span: SimpleSpan) -> Result<String, Rich<char>> {
    let mut buf = String::with_capacity(input.len());
    let mut chars = input.chars().enumerate();
    while let Some((idx, c)) = chars.next() {
        if c == '\\' {
            match chars.next() {
                None => return Err(Rich::custom(span, "Invalid escape sequence at end of string")),
                Some((_, '\\')) => buf.push('\\'),
                Some((_, next)) if next == delim => buf.push(delim),
                _ => {
                    return Err(Rich::custom(
                        span,
                        format!("Invalid escape sequence at index {idx}"),
                    ));
                }
            }
        } else {
            buf.push(c);
        }
    }

    Ok(buf)
}

trait StyleOrLabelMapExt {
    fn get_label(&mut self, key: &str, default: impl Into<String>) -> String;
    fn get_label_opt(&mut self, key: &str) -> Option<String>;
    fn get_style(&mut self, key: &str) -> Option<StyleFile>;
}

impl StyleOrLabelMapExt for Option<HashMap<&str, StyleOrLabel>> {
    fn get_label(&mut self, key: &str, default: impl Into<String>) -> String {
        if let Some(m) = self {
            if let Some(StyleOrLabel::Label(val)) = m.remove(key) {
                return val;
            }
        }
        default.into()
    }

    fn get_label_opt(&mut self, key: &str) -> Option<String> {
        if let Some(m) = self {
            if let Some(StyleOrLabel::Label(val)) = m.remove(key) {
                return Some(val);
            }
        }
        None
    }

    fn get_style(&mut self, key: &str) -> Option<StyleFile> {
        if let Some(m) = self {
            if let Some(StyleOrLabel::Style(val)) = m.remove(key) {
                return Some(val);
            }
        }
        None
    }
}

#[derive(Debug)]
enum StyleOrLabel {
    Style(StyleFile),
    Label(String),
}

#[derive(Debug)]
enum StringOrModifiers {
    String(String),
    Modifiers(Option<Modifiers>),
}

trait StringOrModifiersExt {
    fn get_string(self) -> Option<String>;
    fn get_modifiers(self) -> Option<Modifiers>;
}

impl StringOrModifiersExt for Option<StringOrModifiers> {
    fn get_string(self) -> Option<String> {
        if let Some(StringOrModifiers::String(val)) = self { Some(val) } else { None }
    }

    fn get_modifiers(self) -> Option<Modifiers> {
        if let Some(StringOrModifiers::Modifiers(val)) = self { val } else { None }
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod parser2 {
    use anyhow::{Context, Result};
    use chumsky::Parser;
    use strum::IntoEnumIterator;

    use super::*;
    use crate::config::theme::properties::{
        SongPropertyFileDiscriminants,
        StatusPropertyFileDiscriminants,
        WidgetPropertyFileDiscriminants,
    };

    #[test]
    fn group() {
        let result = parser().parse(
            r#"$[ $s:filename{fg: black, bg: red, mods: bold} $" - " $s:file ]{fg: blue, bg: yellow, mods: crossedout}"#,
        );

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Group(vec![
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                            SongPropertyFile::Filename
                        )),
                        style: Some(StyleFile {
                            fg: Some("black".to_owned()),
                            bg: Some("red".to_owned()),
                            modifiers: Some(Modifiers::Bold),
                        }),
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Text(" - ".to_owned()),
                        style: None,
                        default: None,
                    },
                    PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                            SongPropertyFile::File
                        )),
                        style: None,
                        default: None,
                    }
                ]),
                style: Some(StyleFile {
                    fg: Some("blue".to_owned()),
                    bg: Some("yellow".to_owned()),
                    modifiers: Some(Modifiers::CrossedOut),
                }),
                default: None
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[test]
    fn filename_with_style_and_default() {
        let result = parser().parse(
            "$s:filename{fg: black, bg: red, mods: bold}|$s:file{fg: black, bg: red, mods: bold}|$bitrate{fg: #FF0000, bg: 1, mods: underlined}",
        );

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                    SongPropertyFile::Filename
                )),
                style: Some(StyleFile {
                    fg: Some("black".to_owned()),
                    bg: Some("red".to_owned()),
                    modifiers: Some(Modifiers::Bold),
                }),
                default: Some(Box::new(PropertyFile {
                    kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                        SongPropertyFile::File
                    )),
                    style: Some(StyleFile {
                        fg: Some("black".to_owned()),
                        bg: Some("red".to_owned()),
                        modifiers: Some(Modifiers::Bold),
                    }),
                    default: Some(Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                            StatusPropertyFile::Bitrate
                        )),
                        style: Some(StyleFile {
                            fg: Some("#FF0000".to_owned()),
                            bg: Some("1".to_owned()),
                            modifiers: Some(Modifiers::Underlined),
                        }),
                        default: None,
                    }))
                })),
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[test]
    fn sticker_with_style() {
        let result = parser().parse(r#"$sticker(name: "artist"){fg: black, bg: red, mods: bold}"#);

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Sticker("artist".to_owned()),
                style: Some(StyleFile {
                    fg: Some("black".to_owned()),
                    bg: Some("red".to_owned()),
                    modifiers: Some(Modifiers::Bold),
                }),
                default: None,
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[test]
    fn filename_with_style() {
        let result = parser().parse("$s:filename{fg: black, bg: red, mods: bold}");

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                    SongPropertyFile::Filename
                )),
                style: Some(StyleFile {
                    fg: Some("black".to_owned()),
                    bg: Some("red".to_owned()),
                    modifiers: Some(Modifiers::Bold),
                }),
                default: None,
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[test]
    fn other_with_tag() {
        let result = parser().parse(r#"$s:tag(value: "artist")"#);

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                    SongPropertyFile::Other("artist".to_owned())
                )),
                style: None,
                default: None,
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[test]
    fn filename_simple() {
        let result = parser().parse("$s:filename");

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                    SongPropertyFile::Filename
                )),
                style: None,
                default: None,
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[test]
    fn consume() {
        let result = parser().parse(
            r#"$consume(onLabel: "test", offLabel: "im off boi",offStyle: {fg: black, bg: red, mods: bold})"#,
        );

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                    StatusPropertyFile::ConsumeV2 {
                        on_label: "test".to_owned(),
                        off_label: "im off boi".to_owned(),
                        oneshot_label: "Oneshot".to_owned(),
                        on_style: None,
                        off_style: Some(StyleFile {
                            fg: Some("black".to_owned()),
                            bg: Some("red".to_owned()),
                            modifiers: Some(Modifiers::Bold),
                        }),
                        oneshot_style: None
                    }
                )),
                style: None,
                default: None,
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[test]
    fn string_with_special_chars() {
        let result = parser().parse(r#"$consume(onLabel: "test \" test ' test $#    ")"#);

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                    StatusPropertyFile::ConsumeV2 {
                        on_label: r#"test " test ' test $#    "#.to_owned(),
                        off_label: "Off".to_owned(),
                        oneshot_label: "Oneshot".to_owned(),
                        on_style: None,
                        off_style: None,
                        oneshot_style: None
                    }
                )),
                style: None,
                default: None,
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[test]
    fn single_quoted_string_with_special_chars() {
        let result = parser().parse(r#"$consume(onLabel: 'test " test \' test $#    ')"#);

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Property(PropertyKindFile::Status(
                    StatusPropertyFile::ConsumeV2 {
                        on_label: r#"test " test ' test $#    "#.to_owned(),
                        off_label: "Off".to_owned(),
                        oneshot_label: "Oneshot".to_owned(),
                        on_style: None,
                        off_style: None,
                        oneshot_style: None
                    }
                )),
                style: None,
                default: None,
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[test]
    fn mutliple_modifiers() {
        let result = parser().parse("$'sup'{mods: bold, underlined}");

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Text("sup".to_owned()),
                style: Some(StyleFile {
                    fg: None,
                    bg: None,
                    modifiers: Some(Modifiers::Bold | Modifiers::Underlined)
                }),
                default: None,
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[test]
    fn all_status_properties() -> Result<()> {
        for prop in StatusPropertyFileDiscriminants::iter() {
            let (input, expected) = match prop {
                StatusPropertyFileDiscriminants::Volume => ("$volume", StatusPropertyFile::Volume),
                StatusPropertyFileDiscriminants::Repeat => {
                    ("$repeat", StatusPropertyFile::RepeatV2 {
                        on_label: "On".to_owned(),
                        off_label: "Off".to_owned(),
                        on_style: None,
                        off_style: None,
                    })
                }
                StatusPropertyFileDiscriminants::Random => {
                    ("$random", StatusPropertyFile::RandomV2 {
                        on_label: "On".to_owned(),
                        off_label: "Off".to_owned(),
                        on_style: None,
                        off_style: None,
                    })
                }
                StatusPropertyFileDiscriminants::Single => {
                    ("$single", StatusPropertyFile::SingleV2 {
                        on_label: "On".to_owned(),
                        off_label: "Off".to_owned(),
                        oneshot_label: "Oneshot".to_owned(),
                        on_style: None,
                        off_style: None,
                        oneshot_style: None,
                    })
                }
                StatusPropertyFileDiscriminants::Consume => {
                    ("$consume", StatusPropertyFile::ConsumeV2 {
                        on_label: "On".to_owned(),
                        off_label: "Off".to_owned(),
                        oneshot_label: "Oneshot".to_owned(),
                        on_style: None,
                        off_style: None,
                        oneshot_style: None,
                    })
                }
                StatusPropertyFileDiscriminants::State => ("$state", StatusPropertyFile::StateV2 {
                    playing_label: "Playing".to_owned(),
                    paused_label: "Paused".to_owned(),
                    stopped_label: "Stopped".to_owned(),
                    playing_style: None,
                    paused_style: None,
                    stopped_style: None,
                }),
                StatusPropertyFileDiscriminants::RepeatV2 => {
                    ("$repeat", StatusPropertyFile::RepeatV2 {
                        on_label: "On".to_owned(),
                        off_label: "Off".to_owned(),
                        on_style: None,
                        off_style: None,
                    })
                }
                StatusPropertyFileDiscriminants::RandomV2 => {
                    ("$random", StatusPropertyFile::RandomV2 {
                        on_label: "On".to_owned(),
                        off_label: "Off".to_owned(),
                        on_style: None,
                        off_style: None,
                    })
                }
                StatusPropertyFileDiscriminants::SingleV2 => {
                    ("$single", StatusPropertyFile::SingleV2 {
                        on_label: "On".to_owned(),
                        off_label: "Off".to_owned(),
                        oneshot_label: "Oneshot".to_owned(),
                        on_style: None,
                        off_style: None,
                        oneshot_style: None,
                    })
                }
                StatusPropertyFileDiscriminants::ConsumeV2 => {
                    ("$consume", StatusPropertyFile::ConsumeV2 {
                        on_label: "On".to_owned(),
                        off_label: "Off".to_owned(),
                        oneshot_label: "Oneshot".to_owned(),
                        on_style: None,
                        off_style: None,
                        oneshot_style: None,
                    })
                }
                StatusPropertyFileDiscriminants::StateV2 => {
                    ("$state", StatusPropertyFile::StateV2 {
                        playing_label: "Playing".to_owned(),
                        paused_label: "Paused".to_owned(),
                        stopped_label: "Stopped".to_owned(),
                        playing_style: None,
                        paused_style: None,
                        stopped_style: None,
                    })
                }
                StatusPropertyFileDiscriminants::Elapsed => {
                    ("$elapsed", StatusPropertyFile::Elapsed)
                }
                StatusPropertyFileDiscriminants::Duration => {
                    ("$duration", StatusPropertyFile::Duration)
                }
                StatusPropertyFileDiscriminants::Crossfade => {
                    ("$crossfade", StatusPropertyFile::Crossfade)
                }
                StatusPropertyFileDiscriminants::Bitrate => {
                    ("$bitrate", StatusPropertyFile::Bitrate)
                }
                StatusPropertyFileDiscriminants::Partition => todo!(),
                StatusPropertyFileDiscriminants::QueueLength => todo!(),
                StatusPropertyFileDiscriminants::QueueTimeTotal => todo!(),
                StatusPropertyFileDiscriminants::QueueTimeRemaining => todo!(),
                StatusPropertyFileDiscriminants::ActiveTab => todo!(),
            };

            let result = parser()
                .parse(input)
                .into_output()
                .context(format!("failed to parse '{input}'"))?
                .pop()
                .unwrap()
                .kind;

            assert_eq!(
                result,
                PropertyKindFileOrText::Property(PropertyKindFile::Status(expected))
            );
        }

        Ok(())
    }

    #[test]
    fn all_widget_properties() -> Result<()> {
        for prop in WidgetPropertyFileDiscriminants::iter() {
            let (input, expected) = match &prop {
                WidgetPropertyFileDiscriminants::States => {
                    ("$w:states", WidgetPropertyFile::States {
                        active_style: None,
                        separator_style: None,
                    })
                }
                WidgetPropertyFileDiscriminants::Volume => {
                    ("$w:volume", WidgetPropertyFile::Volume)
                }
                WidgetPropertyFileDiscriminants::ScanStatus => todo!(),
            };

            let result = parser()
                .parse(input)
                .into_output()
                .context(format!("failed to parse '{input}'"))?
                .pop()
                .unwrap()
                .kind;

            assert_eq!(
                result,
                PropertyKindFileOrText::Property(PropertyKindFile::Widget(expected))
            );
        }

        Ok(())
    }

    #[test]
    fn all_song_properties() -> Result<()> {
        for prop in SongPropertyFileDiscriminants::iter() {
            let (input, expected) = match prop {
                SongPropertyFileDiscriminants::Filename => {
                    ("$s:filename", SongPropertyFile::Filename)
                }
                SongPropertyFileDiscriminants::File => ("$s:file", SongPropertyFile::File),
                SongPropertyFileDiscriminants::FileExtension => {
                    ("$s:fileextension", SongPropertyFile::FileExtension)
                }
                SongPropertyFileDiscriminants::Title => ("$s:title", SongPropertyFile::Title),
                SongPropertyFileDiscriminants::Artist => ("$s:artist", SongPropertyFile::Artist),
                SongPropertyFileDiscriminants::Album => ("$s:album", SongPropertyFile::Album),
                SongPropertyFileDiscriminants::Duration => {
                    ("$s:duration", SongPropertyFile::Duration)
                }
                SongPropertyFileDiscriminants::Track => ("$s:track", SongPropertyFile::Track),
                SongPropertyFileDiscriminants::Disc => ("$s:disc", SongPropertyFile::Disc),
                SongPropertyFileDiscriminants::Other => {
                    ("$s:tag(value: \"sometag\")", SongPropertyFile::Other("sometag".to_owned()))
                }
                SongPropertyFileDiscriminants::Position => todo!(),
            };

            let result = parser()
                .parse(input)
                .into_output()
                .context(format!("failed to parse '{input}'"))?
                .pop()
                .unwrap()
                .kind;

            assert_eq!(result, PropertyKindFileOrText::Property(PropertyKindFile::Song(expected)));
        }

        Ok(())
    }

    #[allow(clippy::needless_raw_string_hashes)]
    mod string {
        use chumsky::Parser;
        use test_case::test_case;

        #[test_case(r#""hello world""#,                  "hello world";              "simple")]
        #[test_case(r#""hello \" world""#,               r#"hello " world"#;         "quotes")]
        #[test_case(r#""hello \" \"\" \"\"\"world""#,    r#"hello " "" """world"#;   "mutliple quotes")]
        #[test_case(r#""^{()}$~#:-_;@`!+<>%/""#,         r#"^{()}$~#:-_;@`!+<>%/"#;  "random special chars")]
        #[test_case(r#""\\ \\\\ \\\\\\ \\\\\\\\""#,      r#"\ \\ \\\ \\\\"#;         "backslashes")]
        fn double_quoted_string(input: &str, expected: &str) {
            let result = super::string_parser().parse(input);

            assert_eq!(
                result.clone().into_result(),
                Ok(expected.to_owned()),
                "expected '{input}' to be '{expected}' but got {result:?}"
            );
        }

        #[test_case(r#"'hello world'"#,                 r#"hello world"#;            "simple")]
        #[test_case(r#"'hello \' world'"#,              r#"hello ' world"#;          "quotes")]
        #[test_case(r#"'hello \' \'\' \'\'\'world'"#,   r#"hello ' '' '''world"#;    "mutliple quotes")]
        #[test_case(r#"'^{()}$~#:-_;@`!+<>%/'"#,        r#"^{()}$~#:-_;@`!+<>%/"#;   "random special chars")]
        #[test_case(r#"'\\ \\\\ \\\\\\ \\\\\\\\'"#,     r#"\ \\ \\\ \\\\"#;          "backslashes")]
        fn single_quoted_string(input: &str, expected: &str) {
            let result = super::string_parser().parse(input);

            assert_eq!(
                result.clone().into_result(),
                Ok(expected.to_owned()),
                "expected '{input}' to be '{expected}' but got {result:?}"
            );
        }
    }
}
