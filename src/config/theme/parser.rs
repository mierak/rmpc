use std::collections::HashMap;

use ariadne::{Label, Report, ReportKind, Source};
use chumsky::prelude::*;
use itertools::Itertools;
use strum::IntoDiscriminant;

use super::{
    StyleFile,
    properties::{
        PropertyFile,
        PropertyKindFile,
        PropertyKindFileOrText,
        SongPropertyFile,
        StatusPropertyFile,
        TransformFile,
        WidgetPropertyFile,
    },
    style::Modifiers,
};
use crate::config::defaults;

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
                                    "RGB values must be between 0 and 255",
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

fn generic_property_parser<'a>(
    prop_parser: impl Parser<'a, &'a str, PropertyFile<PropertyKindFile>, extra::Err<Rich<'a, char>>>
    + Clone
    + 'a,
) -> impl Parser<'a, &'a str, (&'a str, Option<HashMap<&'a str, Attribute>>), extra::Err<Rich<'a, char>>>
+ Clone {
    let ident = text::ascii::ident();
    let label = string_parser().map(Attribute::String);
    let style = style_parser().map(Attribute::Style);
    let prop = prop_parser.map(Attribute::Prop);
    let decimal = text::int(10).try_map(|v: &str, span| match v.parse() {
        Ok(v) => Ok(Attribute::UInt(v)),
        Err(_) => Err(Rich::custom(span, "Invalid decimal number")),
    });

    let bool = just("true").or(just("false")).from_str::<bool>().unwrapped().map(Attribute::Bool);

    ident
        .then(
            ident
                .then_ignore(just(':').padded())
                .then(label.or(decimal).or(bool).or(style).or(prop))
                .separated_by(just(',').padded())
                .collect::<HashMap<_, _>>()
                .delimited_by(just('('), just(')'))
                .or_not(),
        )
        .boxed()
}

pub fn property_parser<'a>(
    prop_parser: impl Parser<'a, &'a str, PropertyFile<PropertyKindFile>, extra::Err<Rich<'a, char>>>
    + Clone
    + 'a,
) -> impl Parser<'a, &'a str, PropertyKindFile, extra::Err<Rich<'a, char>>> + Clone {
    let status_property = just("st:")
        .ignore_then(generic_property_parser(prop_parser.clone()))
        .try_map(|(key, mut properties), span| {
            let res = match key {
                "vol" | "volume" => PropertyKindFile::Status(StatusPropertyFile::Volume),
                "rep" | "repeat" => PropertyKindFile::Status(StatusPropertyFile::RepeatV2 {
                    on_label: properties.optional_string_default("on_label", "On", span)?,
                    off_label: properties.optional_string_default("off_label", "Off", span)?,
                    on_style: properties.optional_style("on_style", span)?,
                    off_style: properties.optional_style("off_style", span)?,
                }),
                "rand" | "random" => PropertyKindFile::Status(StatusPropertyFile::RandomV2 {
                    on_label: properties.optional_string_default("on_label", "On", span)?,
                    off_label: properties.optional_string_default("off_label", "Off", span)?,
                    on_style: properties.optional_style("on_style", span)?,
                    off_style: properties.optional_style("off_style", span)?,
                }),
                "sin" | "single" => PropertyKindFile::Status(StatusPropertyFile::SingleV2 {
                    on_label: properties.optional_string_default("on_label", "On", span)?,
                    off_label: properties.optional_string_default("off_label", "Off", span)?,
                    oneshot_label: properties.optional_string_default(
                        "oneshot_label",
                        "Oneshot",
                        span,
                    )?,
                    on_style: properties.optional_style("on_style", span)?,
                    off_style: properties.optional_style("off_style", span)?,
                    oneshot_style: properties.optional_style("os_style", span)?,
                }),
                "cons" | "consume" => PropertyKindFile::Status(StatusPropertyFile::ConsumeV2 {
                    on_label: properties.optional_string_default("on_label", "On", span)?,
                    off_label: properties.optional_string_default("off_label", "Off", span)?,
                    oneshot_label: properties.optional_string_default(
                        "oneshot_label",
                        "Oneshot",
                        span,
                    )?,
                    on_style: properties.optional_style("on_style", span)?,
                    off_style: properties.optional_style("off_style", span)?,
                    oneshot_style: properties.optional_style("os_style", span)?,
                }),
                "st" | "state" => PropertyKindFile::Status(StatusPropertyFile::StateV2 {
                    playing_label: properties.optional_string_default(
                        "playing_label",
                        "Playing",
                        span,
                    )?,
                    paused_label: properties.optional_string_default(
                        "pausedLabel",
                        "Paused",
                        span,
                    )?,
                    stopped_label: properties.optional_string_default(
                        "stopped_label",
                        "Stopped",
                        span,
                    )?,
                    playing_style: properties.optional_style("playing_style", span)?,
                    paused_style: properties.optional_style("paused_style", span)?,
                    stopped_style: properties.optional_style("stopped_style", span)?,
                }),
                "el" | "elapsed" => PropertyKindFile::Status(StatusPropertyFile::Elapsed),
                "dur" | "duration" => PropertyKindFile::Status(StatusPropertyFile::Duration),
                "xf" | "xfade" | "crossfade" => {
                    PropertyKindFile::Status(StatusPropertyFile::Crossfade)
                }
                "br" | "bitrate" => PropertyKindFile::Status(StatusPropertyFile::Bitrate),
                "part" | "partition" => PropertyKindFile::Status(StatusPropertyFile::Partition),
                "tab" | "activetab" => PropertyKindFile::Status(StatusPropertyFile::ActiveTab),
                "qlen" | "queuelength" => {
                    PropertyKindFile::Status(StatusPropertyFile::QueueLength {
                        thousands_separator: properties.optional_string_default(
                            "thousands_separator",
                            defaults::default_thousands_separator(),
                            span,
                        )?,
                    })
                }
                "qtot" | "queuetotal" => {
                    PropertyKindFile::Status(StatusPropertyFile::QueueTimeTotal {
                        separator: properties.optional_string("separator", span)?,
                    })
                }
                "qrem" | "queueremaining" => {
                    PropertyKindFile::Status(StatusPropertyFile::QueueTimeRemaining {
                        separator: properties.optional_string("separator", span)?,
                    })
                }
                _ => return Err(Rich::custom(span, "invalid status property type")),
            };
            properties.validate_empty(span)?;

            Ok(res)
        })
        .boxed();

    let song_property = just("s:")
        .ignore_then(generic_property_parser(prop_parser.clone()))
        .try_map(|(prop_name, mut properties), span| {
            let res = match prop_name {
                "fn" | "filename" => PropertyKindFile::Song(SongPropertyFile::Filename),
                "ext" | "fileextension" => PropertyKindFile::Song(SongPropertyFile::FileExtension),
                "f" | "file" => PropertyKindFile::Song(SongPropertyFile::File),
                "t" | "title" => PropertyKindFile::Song(SongPropertyFile::Title),
                "aar" | "albumartist" => {
                    PropertyKindFile::Song(SongPropertyFile::Other("albumartist".to_owned()))
                }
                "ar" | "artist" => PropertyKindFile::Song(SongPropertyFile::Artist),
                "al" | "album" => PropertyKindFile::Song(SongPropertyFile::Album),
                "tr" | "track" => PropertyKindFile::Song(SongPropertyFile::Track),
                "disc" => PropertyKindFile::Song(SongPropertyFile::Disc),
                "dur" | "duration" => PropertyKindFile::Song(SongPropertyFile::Duration),
                "tag" => {
                    let value = properties.required_string("value", span);
                    PropertyKindFile::Song(SongPropertyFile::Other(value?))
                }
                "pos" | "position" => PropertyKindFile::Song(SongPropertyFile::Position),
                _ => return Err(Rich::custom(span, "invalid song property type")),
            };

            properties.validate_empty(span)?;
            Ok(res)
        })
        .boxed();

    let widget_property = just("w:")
        .ignore_then(generic_property_parser(prop_parser))
        .try_map(|(prop_name, mut properties), span| {
            let res = match prop_name {
                "vol" | "volume" => PropertyKindFile::Widget(WidgetPropertyFile::Volume),
                "st" | "states" => PropertyKindFile::Widget(WidgetPropertyFile::States {
                    active_style: properties.optional_style("active_style", span)?,
                    separator_style: properties.optional_style("separator_style", span)?,
                }),
                "scan" | "scanstatus" => PropertyKindFile::Widget(WidgetPropertyFile::ScanStatus),
                _ => return Err(Rich::custom(span, "invalid widget type")),
            };

            properties.validate_empty(span)?;
            Ok(res)
        })
        .boxed();

    choice((status_property, song_property, widget_property))
        .labelled("status, song, or widget property")
        .boxed()
}

fn transform_parser<'a>(
    prop_parser: impl Parser<'a, &'a str, PropertyFile<PropertyKindFile>, extra::Err<Rich<'a, char>>>
    + Clone
    + 'a,
) -> impl Parser<'a, &'a str, TransformFile<PropertyKindFile>, extra::Err<Rich<'a, char>>> + Clone {
    generic_property_parser(prop_parser)
        .try_map(|(prop_name, mut properties), span| {
            let res = match prop_name {
                "trunc" | "truncate" => {
                    let content = properties.required_prop("content", span)?;
                    let length = properties.required_uint("length", span)?;
                    let from_start = properties.optional_bool_default("from_start", false, span)?;
                    TransformFile::Truncate { content: Box::new(content), length, from_start }
                }
                _ => return Err(Rich::custom(span, "Invalid transform type")),
            };

            properties.validate_empty(span)?;

            Ok(res)
        })
        .labelled("transform")
        .boxed()
}

fn group_parser<'a>(
    prop_parser: impl Parser<'a, &'a str, PropertyFile<PropertyKindFile>, extra::Err<Rich<'a, char>>>
    + Clone
    + 'a,
) -> impl Parser<'a, &'a str, Vec<PropertyFile<PropertyKindFile>>, extra::Err<Rich<'a, char>>> + Clone
{
    prop_parser
        .clone()
        .padded()
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .delimited_by(just('[').padded(), just(']').padded())
        .labelled("group")
}

static MAX_DEPTH: usize = 100;

pub fn parser<'a>()
-> impl Parser<'a, &'a str, Vec<PropertyFile<PropertyKindFile>>, extra::Err<Rich<'a, char>>> {
    let string = string_parser();
    let style_file = style_parser();

    let sticker = just("sticker")
        .ignore_then(
            just("name")
                .ignored()
                .then_ignore(just(':').padded())
                .then(string.clone())
                .delimited_by(just('('), just(')')),
        )
        .labelled("sticker");

    recursive(|prop| {
        let property = property_parser(prop.clone());
        let transform = transform_parser(prop.clone());
        let group = group_parser(prop.clone());

        let prop_kind_or_text = choice((
            string.map(PropertyKindFileOrText::<PropertyKindFile>::Text),
            group.map(PropertyKindFileOrText::Group),
            just('$').ignore_then(property.map(PropertyKindFileOrText::Property)),
            just('%').ignore_then(transform.map(PropertyKindFileOrText::Transform)),
            just('$').ignore_then(
                sticker.map(|((), name)| PropertyKindFileOrText::<PropertyKindFile>::Sticker(name)),
            ),
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

#[allow(clippy::large_enum_variant)]
#[derive(Debug, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::Display))]
enum Attribute {
    Style(StyleFile),
    String(String),
    UInt(usize),
    Bool(bool),
    Prop(PropertyFile<PropertyKindFile>),
}

impl Attribute {
    fn to_err<'a>(
        &self,
        expected: AttributeDiscriminants,
        span: SimpleSpan,
    ) -> chumsky::error::Rich<'a, char> {
        Rich::custom(span, format!("Expected {expected} found {}", self.discriminant()))
    }
}

#[allow(dead_code)]
trait AttrExt {
    fn required_attribute<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<Attribute, chumsky::error::Rich<'a, char>>;

    fn optional_attribute(&mut self, key: &str) -> Option<Attribute>;

    fn validate_empty<'a>(&self, span: SimpleSpan) -> Result<(), chumsky::error::Rich<'a, char>>;

    fn required_string<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<String, chumsky::error::Rich<'a, char>> {
        match self.required_attribute(key, span)? {
            Attribute::String(v) => Ok(v),
            attr => Err(attr.to_err(AttributeDiscriminants::String, span)),
        }
    }

    fn required_style<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<StyleFile, chumsky::error::Rich<'a, char>> {
        match self.required_attribute(key, span)? {
            Attribute::Style(v) => Ok(v),
            attr => Err(attr.to_err(AttributeDiscriminants::Style, span)),
        }
    }

    fn required_prop<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<PropertyFile<PropertyKindFile>, chumsky::error::Rich<'a, char>> {
        match self.required_attribute(key, span)? {
            Attribute::Prop(v) => Ok(v),
            attr => Err(attr.to_err(AttributeDiscriminants::Prop, span)),
        }
    }

    fn required_uint<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<usize, chumsky::error::Rich<'a, char>> {
        match self.required_attribute(key, span)? {
            Attribute::UInt(v) => Ok(v),
            attr => Err(attr.to_err(AttributeDiscriminants::UInt, span)),
        }
    }

    fn required_bool<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<bool, chumsky::error::Rich<'a, char>> {
        match self.required_attribute(key, span)? {
            Attribute::Bool(v) => Ok(v),
            attr => Err(attr.to_err(AttributeDiscriminants::Bool, span)),
        }
    }

    fn optional_string<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<Option<String>, chumsky::error::Rich<'a, char>> {
        match self.optional_attribute(key) {
            Some(Attribute::String(v)) => Ok(Some(v)),
            Some(attr) => Err(attr.to_err(AttributeDiscriminants::String, span)),
            None => Ok(None),
        }
    }

    fn optional_style<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<Option<StyleFile>, chumsky::error::Rich<'a, char>> {
        match self.optional_attribute(key) {
            Some(Attribute::Style(v)) => Ok(Some(v)),
            Some(attr) => Err(attr.to_err(AttributeDiscriminants::Style, span)),
            None => Ok(None),
        }
    }

    fn optional_prop<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<Option<PropertyFile<PropertyKindFile>>, chumsky::error::Rich<'a, char>> {
        match self.optional_attribute(key) {
            Some(Attribute::Prop(v)) => Ok(Some(v)),
            Some(attr) => Err(attr.to_err(AttributeDiscriminants::Prop, span)),
            None => Ok(None),
        }
    }

    fn optional_uint<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<Option<usize>, chumsky::error::Rich<'a, char>> {
        match self.optional_attribute(key) {
            Some(Attribute::UInt(v)) => Ok(Some(v)),
            Some(attr) => Err(attr.to_err(AttributeDiscriminants::UInt, span)),
            None => Ok(None),
        }
    }

    fn optional_bool<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<Option<bool>, chumsky::error::Rich<'a, char>> {
        match self.optional_attribute(key) {
            Some(Attribute::Bool(v)) => Ok(Some(v)),
            Some(attr) => Err(attr.to_err(AttributeDiscriminants::Bool, span)),
            None => Ok(None),
        }
    }

    fn optional_string_default<'a>(
        &mut self,
        key: &str,
        default: impl Into<String>,
        span: SimpleSpan,
    ) -> Result<String, chumsky::error::Rich<'a, char>> {
        match self.optional_string(key, span)? {
            Some(val) => Ok(val),
            None => Ok(default.into()),
        }
    }

    fn optional_style_default<'a>(
        &mut self,
        key: &str,
        default: StyleFile,
        span: SimpleSpan,
    ) -> Result<StyleFile, chumsky::error::Rich<'a, char>> {
        match self.optional_style(key, span)? {
            Some(val) => Ok(val),
            None => Ok(default),
        }
    }

    fn optional_prop_default<'a>(
        &mut self,
        key: &str,
        default: PropertyFile<PropertyKindFile>,
        span: SimpleSpan,
    ) -> Result<PropertyFile<PropertyKindFile>, chumsky::error::Rich<'a, char>> {
        match self.optional_prop(key, span)? {
            Some(val) => Ok(val),
            None => Ok(default),
        }
    }

    fn optional_uint_default<'a>(
        &mut self,
        key: &str,
        default: usize,
        span: SimpleSpan,
    ) -> Result<usize, chumsky::error::Rich<'a, char>> {
        match self.optional_uint(key, span)? {
            Some(val) => Ok(val),
            None => Ok(default),
        }
    }

    fn optional_bool_default<'a>(
        &mut self,
        key: &str,
        default: bool,
        span: SimpleSpan,
    ) -> Result<bool, chumsky::error::Rich<'a, char>> {
        match self.optional_bool(key, span)? {
            Some(val) => Ok(val),
            None => Ok(default),
        }
    }
}

impl AttrExt for Option<HashMap<&str, Attribute>> {
    fn required_attribute<'a>(
        &mut self,
        key: &str,
        span: SimpleSpan,
    ) -> Result<Attribute, chumsky::error::Rich<'a, char>> {
        match self {
            Some(m) => m
                .remove(key)
                .ok_or_else(|| Rich::custom(span, format!("'{key}' missing property attribute"))),
            None => Err(Rich::custom(
                span,
                format!("Trying to find '{key}' but attributes are either missing or invalid"),
            )),
        }
    }

    fn optional_attribute(&mut self, key: &str) -> Option<Attribute> {
        match self {
            Some(m) => m.remove(key),
            None => None,
        }
    }

    fn validate_empty<'a>(&self, span: SimpleSpan) -> Result<(), chumsky::error::Rich<'a, char>> {
        match self {
            Some(v) if v.is_empty() => Ok(()),
            Some(v) => Err(Rich::custom(
                span,
                format!("Unknown attributes found: [{}]", v.keys().join(", ")),
            )),
            None => Ok(()),
        }
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
            r#"[ $s:filename{fg: black, bg: red, mods: bold} " - " $s:file ]{fg: blue, bg: yellow, mods: crossedout}"#,
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
        let input = "$s:filename{fg: black, bg: red, mods: bold}|$s:file{fg: black, bg: red, mods: bold}|$st:bitrate{fg: #FF0000, bg: 1, mods: underlined}";
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
        let input = "%trunc(content: $s:artist, length: 4, from_start: false){fg: black, bg: red, mods: bold}";
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
        let input = r#"$s:tag(value: "artist")"#;
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
            r#"$st:consume(on_label: "test", off_label: "im off boi",off_style: {fg: black, bg: red, mods: bold})"#,
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
        let result = parser().parse(r#"$st:consume(on_label: "test \" test ' test $#    ")"#);

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
        let result = parser().parse(r#"$st:consume(on_label: 'test " test \' test $#    ')"#);

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
    #[case("$st:activetab", StatusPropertyFile::ActiveTab)]
    #[case("$st:queueremaining", StatusPropertyFile::QueueTimeRemaining { separator: None })]
    #[case("$st:queuetotal", StatusPropertyFile::QueueTimeTotal { separator: None })]
    #[case("$st:queuelength", StatusPropertyFile::QueueLength { thousands_separator: ",".to_owned() })]
    #[case("$st:partition", StatusPropertyFile::Partition)]
    #[case("$st:bitrate", StatusPropertyFile::Bitrate)]
    #[case("$st:crossfade", StatusPropertyFile::Crossfade)]
    #[case("$st:duration", StatusPropertyFile::Duration)]
    #[case("$st:elapsed", StatusPropertyFile::Elapsed)]
    #[case("$st:state", StatusPropertyFile::StateV2 { playing_label: "Playing".to_owned(), paused_label: "Paused".to_owned(), stopped_label: "Stopped".to_owned(), playing_style: None, paused_style: None, stopped_style: None })]
    #[case("$st:consume", StatusPropertyFile::ConsumeV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), oneshot_label: "Oneshot".to_owned(), on_style: None, off_style: None, oneshot_style: None })]
    #[case("$st:single", StatusPropertyFile::SingleV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), oneshot_label: "Oneshot".to_owned(), on_style: None, off_style: None, oneshot_style: None })]
    #[case("$st:random", StatusPropertyFile::RandomV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), on_style: None, off_style: None })]
    #[case("$st:repeat", StatusPropertyFile::RepeatV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), on_style: None, off_style: None })]
    #[case("$st:state", StatusPropertyFile::StateV2 { playing_label: "Playing".to_owned(), paused_label: "Paused".to_owned(), stopped_label: "Stopped".to_owned(), playing_style: None, paused_style: None, stopped_style: None })]
    #[case("$st:consume", StatusPropertyFile::ConsumeV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), oneshot_label: "Oneshot".to_owned(), on_style: None, off_style: None, oneshot_style: None })]
    #[case("$st:single", StatusPropertyFile::SingleV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), oneshot_label: "Oneshot".to_owned(), on_style: None, off_style: None, oneshot_style: None })]
    #[case("$st:random", StatusPropertyFile::RandomV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), on_style: None, off_style: None })]
    #[case("$st:repeat", StatusPropertyFile::RepeatV2 { on_label: "On".to_owned(), off_label: "Off".to_owned(), on_style: None, off_style: None })]
    #[case("$st:volume", StatusPropertyFile::Volume)]
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
