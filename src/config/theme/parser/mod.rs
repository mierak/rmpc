use ariadne::{Label, Report, ReportKind, Source};
use attributes::Attribute;
use chumsky::prelude::*;
use property::{group_parser, property_parser, sticker_parser, transform_parser};
use string::string_parser;
use style::style_parser;

use super::{
    StyleFile,
    properties::{PropertyFile, PropertyKindFile, PropertyKindFileOrText},
};

mod attributes;
mod property;
mod string;
mod style;

static MAX_DEPTH: usize = 100;

pub fn parser<'a>()
-> impl Parser<'a, &'a str, Vec<PropertyFile<PropertyKindFile>>, extra::Err<Rich<'a, char>>> {
    let string = string_parser();
    let style_file = style_parser();

    recursive(|prop| {
        let property = property_parser(prop.clone());
        let transform = transform_parser(prop.clone());
        let group = group_parser(prop.clone());
        let sticker = sticker_parser();

        let prop_kind_or_text = choice((
            string.map(PropertyKindFileOrText::Text),
            group.map(PropertyKindFileOrText::Group),
            property.map(PropertyKindFileOrText::Property),
            transform.map(PropertyKindFileOrText::Transform),
            sticker.map(PropertyKindFileOrText::Sticker),
        ));

        prop_kind_or_text
            .then(style_file.clone().or_not())
            .then(just('|').ignore_then(prop).or_not())
            .map(|((kind, style), default)| PropertyFile {
                kind,
                style,
                default: default.map(Box::new),
            })
    })
    .padded()
    .repeated()
    .at_most(MAX_DEPTH)
    .collect::<Vec<_>>()
    .boxed()
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

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod parser2 {
    use chumsky::Parser;
    use rstest::rstest;

    use super::*;
    use crate::config::theme::{
        Modifiers,
        properties::{SongPropertyFile, StatusPropertyFile, TransformFile, WidgetPropertyFile},
    };

    #[test]
    fn group() {
        let result = parser().parse(
            r#"[ $filename{fg: black, bg: red, mods: bold} " - " $file ]{fg: blue, bg: yellow, mods: crossedout}"#,
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
        let input = "$filename{fg: black, bg: red, mods: bold}|$file{fg: black, bg: red, mods: bold}|$bitrate{fg: #FF0000, bg: 1, mods: underlined}";
        let result = parser()
            .parse(input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, input)));

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
    fn truncate() {
        let input = "%trunc(content: $artist, length: 4, from_start: false){fg: black, bg: red, mods: bold}";
        let result = parser()
            .parse(input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, input)));

        assert_eq!(
            PropertyFile {
                kind: PropertyKindFileOrText::Transform(TransformFile::Truncate {
                    content: Box::new(PropertyFile {
                        kind: PropertyKindFileOrText::Property(PropertyKindFile::Song(
                            SongPropertyFile::Artist
                        )),
                        style: None,
                        default: None
                    }),
                    length: 4,
                    from_start: false
                }),
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
        let input = r#"$tag(value: "artist")"#;
        let result =
            parser().parse(input).into_result().map_err(|errs| make_error_report(errs, input));

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
            r#"$consume(on_label: "test", off_label: "im off boi",off_style: {fg: black, bg: red, mods: bold})"#,
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
        let result = parser().parse(r#"$consume(on_label: "test \" test ' test $#    ")"#);

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
        let result = parser().parse(r#"$consume(on_label: 'test " test \' test $#    ')"#);

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
        let result = parser().parse("'sup'{mods: bold, underlined}");

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
        let input = format!("'sup'{{fg: {input}}}");
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
    #[case("$cduration", StatusPropertyFile::Duration)]
    #[case("$cdur", StatusPropertyFile::Duration)]
    #[case("$currentduration", StatusPropertyFile::Duration)]
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
        let result = parser()
            .parse(input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, input)))
            .unwrap()
            .pop()
            .unwrap()
            .kind;
        assert_eq!(result, PropertyKindFileOrText::Property(PropertyKindFile::Status(expected)));
    }

    #[rstest]
    #[case("$w:states", WidgetPropertyFile::States { active_style: None, separator_style: None })]
    #[case("$w:volume", WidgetPropertyFile::Volume)]
    #[case("$w:scanstatus", WidgetPropertyFile::ScanStatus)]
    fn all_widget_properties(#[case] input: &str, #[case] expected: WidgetPropertyFile) {
        let result = parser()
            .parse(input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, input)))
            .unwrap()
            .pop()
            .unwrap()
            .kind;
        assert_eq!(result, PropertyKindFileOrText::Property(PropertyKindFile::Widget(expected)));
    }

    #[rstest]
    #[case("$filename", SongPropertyFile::Filename)]
    #[case("$file", SongPropertyFile::File)]
    #[case("$fileextension", SongPropertyFile::FileExtension)]
    #[case("$title", SongPropertyFile::Title)]
    #[case("$artist", SongPropertyFile::Artist)]
    #[case("$album", SongPropertyFile::Album)]
    #[case("$duration", SongPropertyFile::Duration)]
    #[case("$track", SongPropertyFile::Track)]
    #[case("$disc", SongPropertyFile::Disc)]
    #[case("$position", SongPropertyFile::Position)]
    #[case("$tag(value: \"sometag\")", SongPropertyFile::Other("sometag".to_owned()))]
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
