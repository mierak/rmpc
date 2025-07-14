use std::collections::HashMap;

use ariadne::{Label, Report, ReportKind, Source};
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

    let escape_single_quoted_str =
        choice((
            just('\\').then(just('\'')).ignored(),
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
        .or(escape_single_quoted_str)
        .repeated()
        .to_slice()
        .try_map(|v, span| unescape(v, '\'', span))
        .delimited_by(just("'"), just("'"))
        .boxed();

    double_quoted_string.clone().or(single_quoted_string.clone()).labelled("string value")
}

pub fn modifiers_parser<'a>()
-> impl Parser<'a, &'a str, Modifiers, extra::Err<Rich<'a, char>>> + Clone {
    choice((
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

        res
    })
    .labelled("modifiers")
}

pub fn color_str_parser<'a>() -> impl Parser<'a, &'a str, String, extra::Err<Rich<'a, char>>> + Clone
{
    let ident = text::ascii::ident();
    choice((
        // RBG color
        just("rgb(")
            .padded()
            .ignore_then(
                text::int(10)
                    .separated_by(just(',').padded())
                    .exactly(3)
                    .collect_exactly::<[_; 3]>()
                    .validate(|c: [&str; 3], e, emitter| {
                        for c in c {
                            if c.parse::<u8>().is_err() {
                                emitter.emit(Rich::custom(
                                    e.span(),
                                    format!("{c} must be between 0 and 255."),
                                ));
                            }
                        }
                        c
                    })
                    .map(|[r, g, b]| format!("rgb({r},{g},{b})")),
            )
            .then_ignore(just(')').padded()),
        // HEX color
        just('#').then(ident).map(|(_, hex): (_, &str)| {
            let mut hex = hex.to_owned();
            hex.insert(0, '#');
            hex
        }),
        // ANSI color
        text::int(10).map(|n: &str| n.to_owned()),
        // Color by name
        ident.map(|c: &str| c.to_owned()),
    ))
    .labelled("color")
}

pub fn style_parser<'a>() -> impl Parser<'a, &'a str, StyleFile, extra::Err<Rich<'a, char>>> + Clone
{
    let modifiers = modifiers_parser();
    let color = color_str_parser();

    choice((
        just("fg")
            .then_ignore(just(':').padded())
            .then(color.clone().map(StringOrModifiers::String)),
        just("bg").then_ignore(just(':').padded()).then(color.map(StringOrModifiers::String)),
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
    .labelled("style")
    .boxed()
}
pub fn property_parser<'a>()
-> impl Parser<'a, &'a str, PropertyKindFile, extra::Err<Rich<'a, char>>> + Clone {
    let ident = text::ascii::ident();
    let label = string_parser().map(StyleOrLabel::Label);
    let style = style_parser().map(StyleOrLabel::Style);

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
            "partition" => Ok(PropertyKindFile::Status(StatusPropertyFile::Partition)),
            "activetab" => Ok(PropertyKindFile::Status(StatusPropertyFile::ActiveTab)),
            "queuelength" => Ok(PropertyKindFile::Status(StatusPropertyFile::QueueLength {
                thousands_separator: properties.get_label("thousandsSeparator", ","),
            })),
            "queuetotal" => Ok(PropertyKindFile::Status(StatusPropertyFile::QueueTimeTotal {
                separator: properties.get_label_opt("separator"),
            })),
            "queueremaining" => {
                Ok(PropertyKindFile::Status(StatusPropertyFile::QueueTimeRemaining {
                    separator: properties.get_label_opt("separator"),
                }))
            }
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
            "position" => Ok(PropertyKindFile::Song(SongPropertyFile::Position)),
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
            "scanstatus" => Ok(PropertyKindFile::Widget(WidgetPropertyFile::ScanStatus)),
            _ => Err(Rich::custom(span, "invalid widget type")),
        })
        .boxed();

    choice((status_property, song_property, widget_property)).labelled("property").boxed()
}

pub fn parser<'a>()
-> impl Parser<'a, &'a str, Vec<PropertyFile<PropertyKindFile>>, extra::Err<Rich<'a, char>>> {
    let string = string_parser();
    let style_file = style_parser();

    let property = property_parser();

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
    .at_most(100)
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

pub fn make_error_report<'a>(err: Vec<Rich<'a, char>>, source: &'a str) -> String {
    let mut buf = Vec::new();
    for e in err {
        Report::build(ReportKind::Error, ((), e.span().into_range()))
            .with_config(
                ariadne::Config::new()
                    .with_color(false)
                    .with_tab_width(1)
                    .with_index_type(ariadne::IndexType::Byte),
            )
            .with_message(e.to_string())
            .with_label(
                Label::new(((), e.span().into_range())).with_message(e.reason().to_string()),
            )
            .finish()
            .write_for_stdout(Source::from(&source), &mut buf)
            .expect("Write to String buffer should always succeed");
    }

    String::from_utf8_lossy(&buf).into_owned()
}

#[derive(Debug)]
enum StyleOrLabel {
    Style(StyleFile),
    Label(String),
}

#[derive(Debug)]
enum StringOrModifiers {
    String(String),
    Modifiers(Modifiers),
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
        if let Some(StringOrModifiers::Modifiers(val)) = self { Some(val) } else { None }
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod parser2 {
    use chumsky::Parser;
    use rstest::rstest;

    use super::*;

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
    fn multiple_modifiers() {
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

    #[rstest]
    #[case("red", "red")]
    #[case("blue", "blue")]
    #[case("11", "11")]
    #[case("#ff0000", "#ff0000")]
    #[case("rgb( 255, 1 ,1 )", "rgb(255,1,1)")]
    #[case("rgb(255,255,255)", "rgb(255,255,255)")]
    fn colors(#[case] input: &str, #[case] expected: String) {
        let input = format!("$'sup'{{fg: {input}}}");
        dbg!(&input);
        let result = parser()
            .parse(&input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, &input)));

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Text("sup".to_owned()),
                style: Some(StyleFile { fg: Some(expected), bg: None, modifiers: None }),
                default: None,
            },
            result.unwrap().pop().unwrap()
        );
    }

    #[rstest]
    #[case("$activetab", StatusPropertyFile::ActiveTab)]
    #[case("$queueremaining", StatusPropertyFile::QueueTimeRemaining { separator: None })]
    #[case("$queuetotal", StatusPropertyFile::QueueTimeTotal { separator: None })]
    #[case("$queuelength", StatusPropertyFile::QueueLength { thousands_separator: ",".to_owned() })]
    #[case("$partition", StatusPropertyFile::Partition)]
    #[case("$bitrate", StatusPropertyFile::Bitrate)]
    #[case("$crossfade", StatusPropertyFile::Crossfade)]
    #[case("$duration", StatusPropertyFile::Duration)]
    #[case("$elapsed", StatusPropertyFile::Elapsed)]
    #[case("$state", StatusPropertyFile::StateV2 { playing_label: "Playing".to_owned(), paused_label: "Paused".to_owned(), stopped_label: "Stopped".to_owned(), playing_style: None, paused_style: None, stopped_style: None })]
    #[case("$consume", StatusPropertyFile::ConsumeV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), oneshot_label: "Oneshot".to_owned(), on_style: None, off_style: None, oneshot_style: None })]
    #[case("$single", StatusPropertyFile::SingleV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), oneshot_label: "Oneshot".to_owned(), on_style: None, off_style: None, oneshot_style: None })]
    #[case("$random", StatusPropertyFile::RandomV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), on_style: None, off_style: None })]
    #[case("$repeat", StatusPropertyFile::RepeatV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), on_style: None, off_style: None })]
    #[case("$state", StatusPropertyFile::StateV2 { playing_label: "Playing".to_owned(), paused_label: "Paused".to_owned(), stopped_label: "Stopped".to_owned(), playing_style: None, paused_style: None, stopped_style: None })]
    #[case("$consume", StatusPropertyFile::ConsumeV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), oneshot_label: "Oneshot".to_owned(), on_style: None, off_style: None, oneshot_style: None })]
    #[case("$single", StatusPropertyFile::SingleV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), oneshot_label: "Oneshot".to_owned(), on_style: None, off_style: None, oneshot_style: None })]
    #[case("$random", StatusPropertyFile::RandomV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), on_style: None, off_style: None })]
    #[case("$repeat", StatusPropertyFile::RepeatV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), on_style: None, off_style: None })]
    #[case("$volume", StatusPropertyFile::Volume)]
    fn all_status_properties(#[case] input: &str, #[case] expected: StatusPropertyFile) {
        let result = parser().parse(input).unwrap().pop().unwrap().kind;
        assert_eq!(result, PropertyKindFileOrText::Property(PropertyKindFile::Status(expected)));
    }

    #[rstest]
    #[case("$w:states", WidgetPropertyFile::States { active_style: None, separator_style: None })]
    #[case("$w:volume", WidgetPropertyFile::Volume)]
    #[case("$w:scanstatus", WidgetPropertyFile::ScanStatus)]
    fn all_widget_properties(#[case] input: &str, #[case] expected: WidgetPropertyFile) {
        let result = parser().parse(input).unwrap().pop().unwrap().kind;
        assert_eq!(result, PropertyKindFileOrText::Property(PropertyKindFile::Widget(expected)));
    }

    #[rstest]
    #[case("$s:filename", SongPropertyFile::Filename)]
    #[case("$s:file", SongPropertyFile::File)]
    #[case("$s:fileextension", SongPropertyFile::FileExtension)]
    #[case("$s:title", SongPropertyFile::Title)]
    #[case("$s:artist", SongPropertyFile::Artist)]
    #[case("$s:album", SongPropertyFile::Album)]
    #[case("$s:duration", SongPropertyFile::Duration)]
    #[case("$s:track", SongPropertyFile::Track)]
    #[case("$s:disc", SongPropertyFile::Disc)]
    #[case("$s:position", SongPropertyFile::Position)]
    #[case("$s:tag(value: \"sometag\")", SongPropertyFile::Other("sometag".to_owned()))]
    fn all_song_properties(#[case] input: &str, #[case] expected: SongPropertyFile) {
        let result = parser().parse(input).unwrap().pop().unwrap().kind;
        assert_eq!(result, PropertyKindFileOrText::Property(PropertyKindFile::Song(expected)));
    }

    #[allow(clippy::needless_raw_string_hashes)]
    mod string {
        use chumsky::Parser;
        use test_case::test_case;

        #[test_case(r#""hello world""#,                  "hello world";              "simple")]
        #[test_case(r#""hello \" world""#,               r#"hello " world"#;         "quotes")]
        #[test_case(r#""hello \" \"\" \"\"\"world""#,    r#"hello " "" """world"#;   "multiple quotes")]
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
        #[test_case(r#"'hello \' \'\' \'\'\'world'"#,   r#"hello ' '' '''world"#;    "multiple quotes")]
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
