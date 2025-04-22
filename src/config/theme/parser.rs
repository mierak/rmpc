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

pub fn parser<'a>()
-> impl Parser<'a, &'a str, Vec<PropertyFile<PropertyKindFile>>, extra::Err<Rich<'a, char>>> {
    let ident = text::ascii::ident();

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
        .map(ToString::to_string)
        .delimited_by(just('"'), just('"'))
        .boxed();

    let single_quoted_string = none_of("\\'")
        .ignored()
        .or(escape_single_queoted_str)
        .repeated()
        .to_slice()
        .map(ToString::to_string)
        .delimited_by(just("'"), just("'"))
        .boxed();

    let string = double_quoted_string.clone().or(single_quoted_string.clone()).labelled("string");

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
    .delimited_by(just('{'), just('}'));

    let label = string.clone().map(|v: String| StyleOrLabel::Label(v));
    let style = style_file.map(StyleOrLabel::Style);

    let status_property = choice((
        just("volume").map(|_| PropertyKindFile::Status(StatusPropertyFile::Volume)),
        just("repeat")
            .ignored()
            .then(
                choice((
                    just("onStyle").then_ignore(just(':').padded()).then(style),
                    just("offStyle").then_ignore(just(':').padded()).then(style),
                    just("onLabel").then_ignore(just(':').padded()).then(label.clone()),
                    just("offLabel").then_ignore(just(':').padded()).then(label.clone()),
                ))
                .separated_by(just(',').padded())
                .collect::<HashMap<_, _>>()
                .delimited_by(just('('), just(')')),
            )
            .map(|((), mut val)| {
                PropertyKindFile::Status(StatusPropertyFile::RepeatV2 {
                    on_label: val.remove("onLabel").get_label("On"),
                    off_label: val.remove("offLabel").get_label("Off"),
                    on_style: val.remove("onStyle").get_style(),
                    off_style: val.remove("offStyle").get_style(),
                })
            }),
        just("random")
            .ignored()
            .then(
                choice((
                    just("onStyle").then_ignore(just(':').padded()).then(style),
                    just("offStyle").then_ignore(just(':').padded()).then(style),
                    just("onLabel").then_ignore(just(':').padded()).then(label.clone()),
                    just("offLabel").then_ignore(just(':').padded()).then(label.clone()),
                ))
                .separated_by(just(',').padded())
                .collect::<HashMap<_, _>>()
                .delimited_by(just('('), just(')')),
            )
            .map(|((), mut val)| {
                PropertyKindFile::Status(StatusPropertyFile::RandomV2 {
                    on_label: val.remove("onLabel").get_label("On"),
                    off_label: val.remove("offLabel").get_label("Off"),
                    on_style: val.remove("onStyle").get_style(),
                    off_style: val.remove("offStyle").get_style(),
                })
            }),
        just("single")
            .ignored()
            .then(
                choice((
                    just("onStyle").then_ignore(just(':').padded()).then(style),
                    just("offStyle").then_ignore(just(':').padded()).then(style),
                    just("oneshotStyle").then_ignore(just(':').padded()).then(style),
                    just("onLabel").then_ignore(just(':').padded()).then(label.clone()),
                    just("offLabel").then_ignore(just(':').padded()).then(label.clone()),
                    just("oneshotLabel").then_ignore(just(':').padded()).then(label.clone()),
                ))
                .separated_by(just(',').padded())
                .collect::<HashMap<_, _>>()
                .delimited_by(just('('), just(')')),
            )
            .map(|((), mut val)| {
                PropertyKindFile::Status(StatusPropertyFile::SingleV2 {
                    on_label: val.remove("onLabel").get_label("On"),
                    off_label: val.remove("offLabel").get_label("Off"),
                    oneshot_label: val.remove("oneshotLabel").get_label("Oneshot"),
                    on_style: val.remove("onStyle").get_style(),
                    off_style: val.remove("offStyle").get_style(),
                    oneshot_style: val.remove("oneshotStyle").get_style(),
                })
            }),
        just("consume")
            .ignored()
            .then(
                choice((
                    just("onStyle").then_ignore(just(':').padded()).then(style),
                    just("offStyle").then_ignore(just(':').padded()).then(style),
                    just("oneshotStyle").then_ignore(just(':').padded()).then(style),
                    just("onLabel").then_ignore(just(':').padded()).then(label.clone()),
                    just("offLabel").then_ignore(just(':').padded()).then(label.clone()),
                    just("oneshotLabel").then_ignore(just(':').padded()).then(label.clone()),
                ))
                .separated_by(just(',').padded())
                .collect::<HashMap<_, _>>()
                .delimited_by(just('('), just(')')),
            )
            .map(|((), mut val)| {
                PropertyKindFile::Status(StatusPropertyFile::ConsumeV2 {
                    on_label: val.remove("onLabel").get_label("On"),
                    off_label: val.remove("offLabel").get_label("Off"),
                    oneshot_label: val.remove("oneshotLabel").get_label("Oneshot"),
                    on_style: val.remove("onStyle").get_style(),
                    off_style: val.remove("offStyle").get_style(),
                    oneshot_style: val.remove("oneshotStyle").get_style(),
                })
            }),
        just("state")
            .ignored()
            .then(
                choice((
                    just("playingStyle").then_ignore(just(':').padded()).then(style),
                    just("pausedStyle").then_ignore(just(':').padded()).then(style),
                    just("stoppedStyle").then_ignore(just(':').padded()).then(style),
                    just("playingLabel").then_ignore(just(':').padded()).then(label.clone()),
                    just("pausedLabel").then_ignore(just(':').padded()).then(label.clone()),
                    just("stoppedLabel").then_ignore(just(':').padded()).then(label.clone()),
                ))
                .separated_by(just(',').padded())
                .collect::<HashMap<_, _>>()
                .delimited_by(just('('), just(')')),
            )
            .map(|((), mut val)| {
                PropertyKindFile::Status(StatusPropertyFile::StateV2 {
                    playing_label: val.remove("playingLabel").get_label("Playing"),
                    paused_label: val.remove("pausedLabel").get_label("Paused"),
                    stopped_label: val.remove("stoppedLabel").get_label("Stopped"),
                    playing_style: val.remove("playingStyle").get_style(),
                    paused_style: val.remove("pausedStyle").get_style(),
                    stopped_style: val.remove("stoppedStyle").get_style(),
                })
            }),
        just("elapsed").map(|_| PropertyKindFile::Status(StatusPropertyFile::Elapsed)),
        just("duration").map(|_| PropertyKindFile::Status(StatusPropertyFile::Duration)),
        just("crossfade").map(|_| PropertyKindFile::Status(StatusPropertyFile::Crossfade)),
        just("bitrate").map(|_| PropertyKindFile::Status(StatusPropertyFile::Bitrate)),
    ))
    .boxed();

    let song_property = choice((
        just("filename").map(|_| PropertyKindFile::Song(SongPropertyFile::Filename)),
        just("file").map(|_| PropertyKindFile::Song(SongPropertyFile::File)),
        just("fileextension").map(|_| PropertyKindFile::Song(SongPropertyFile::FileExtension)),
        just("title").map(|_| PropertyKindFile::Song(SongPropertyFile::Title)),
        just("artist").map(|_| PropertyKindFile::Song(SongPropertyFile::Artist)),
        just("album").map(|_| PropertyKindFile::Song(SongPropertyFile::Album)),
        just("track").map(|_| PropertyKindFile::Song(SongPropertyFile::Track)),
        just("disc").map(|_| PropertyKindFile::Song(SongPropertyFile::Disc)),
        just("tag")
            .ignored()
            .then(
                just("value")
                    .ignored()
                    .then_ignore(just(':').padded())
                    .then(ident.delimited_by(just("\""), just("\"")))
                    .delimited_by(just('('), just(')')),
            )
            .map(|((), ((), v)): (_, (_, &str))| {
                PropertyKindFile::Song(SongPropertyFile::Other(v.to_owned()))
            }),
        // just("duration").map(|_| PropertyKind::Song(SongProperty::Duration)),
    ))
    .boxed();

    let widget_property = choice((
        just("volume").map(|_| PropertyKindFile::Widget(WidgetPropertyFile::Volume)),
        just("states")
            .ignored()
            .then(
                choice((
                    just("activeStyle").then_ignore(just(':').padded()).then(style),
                    just("separatorStyle").then_ignore(just(':').padded()).then(style),
                ))
                .separated_by(just(',').padded())
                .collect::<HashMap<_, _>>()
                .delimited_by(just('('), just(')')),
            )
            .map(|((), mut val)| {
                PropertyKindFile::Widget(WidgetPropertyFile::States {
                    active_style: val.remove("activeStyle").get_style(),
                    separator_style: val.remove("separatorStyle").get_style(),
                })
            }),
    ))
    .boxed();

    let property =
        choice((status_property, song_property, widget_property)).labelled("property").boxed();

    let sticker = just("sticker")
        .ignored()
        .then(
            just("name")
                .ignored()
                .then_ignore(just(':').padded())
                .then(double_quoted_string.clone())
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
            .then(style_file.or_not())
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

#[derive(Debug)]
enum StyleOrLabel {
    Style(StyleFile),
    Label(String),
}

trait StyleOrLabelExt {
    fn get_label(self, default: impl Into<String>) -> String;
    fn get_style(self) -> Option<StyleFile>;
}

impl StyleOrLabelExt for Option<StyleOrLabel> {
    fn get_label(self, default: impl Into<String>) -> String {
        if let Some(StyleOrLabel::Label(val)) = self { val } else { default.into() }
    }

    fn get_style(self) -> Option<StyleFile> {
        if let Some(StyleOrLabel::Style(val)) = self { Some(val) } else { None }
    }
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
    use chumsky::Parser;

    use super::*;

    #[test]
    fn group() {
        let result = parser().parse(
            r#"$[ $filename{fg: black, bg: red, mods: bold} $" - " $file ]{fg: blue, bg: yellow, mods: crossedout}"#,
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
            "$filename{fg: black, bg: red, mods: bold}|$file{fg: black, bg: red, mods: bold}|$bitrate{fg: #FF0000, bg: 1, mods: underlined}",
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
        let result = parser().parse("$filename{fg: black, bg: red, mods: bold}");

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
        let result = parser().parse(r#"$tag(value: "artist")"#);

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
        let result = parser().parse("$filename");

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
                        on_label: r#"test \" test ' test $#    "#.to_owned(),
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
                        on_label: r#"test " test \' test $#    "#.to_owned(),
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
}
