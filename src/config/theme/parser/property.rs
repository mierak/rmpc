use std::collections::HashMap;

use chumsky::prelude::*;

use super::{
    Attribute,
    attributes::{AttrExt as _, attribute_parser},
    string::string_parser,
};
use crate::config::{
    defaults,
    theme::properties::{
        PropertyFile,
        PropertyKindFile,
        SongPropertyFile,
        StatusPropertyFile,
        TransformFile,
        WidgetPropertyFile,
    },
};

pub(super) fn generic_property_parser<'a>(
    prop_parser: impl Parser<'a, &'a str, PropertyFile<PropertyKindFile>, extra::Err<Rich<'a, char>>>
    + Clone
    + 'a,
) -> impl Parser<'a, &'a str, (&'a str, Option<HashMap<&'a str, Attribute>>), extra::Err<Rich<'a, char>>>
+ Clone {
    let ident = text::ascii::ident();
    let attr = attribute_parser(prop_parser);

    ident
        .padded()
        .then(
            attr.separated_by(just(',').padded())
                .collect::<HashMap<&str, Attribute>>()
                .delimited_by(just('(').padded(), just(')').padded())
                .or_not(),
        )
        .boxed()
}

pub(super) fn property_parser<'a>(
    prop_parser: impl Parser<'a, &'a str, PropertyFile<PropertyKindFile>, extra::Err<Rich<'a, char>>>
    + Clone
    + 'a,
) -> impl Parser<'a, &'a str, PropertyKindFile, extra::Err<Rich<'a, char>>> + Clone {
    let status_property = just("$")
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
                "cdur" | "cduration" | "currentduration" => {
                    PropertyKindFile::Status(StatusPropertyFile::Duration)
                }
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

    let song_property = just("$")
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

    let widget_property = just("$w:")
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

pub(super) fn transform_parser<'a>(
    prop_parser: impl Parser<'a, &'a str, PropertyFile<PropertyKindFile>, extra::Err<Rich<'a, char>>>
    + Clone
    + 'a,
) -> impl Parser<'a, &'a str, TransformFile<PropertyKindFile>, extra::Err<Rich<'a, char>>> + Clone {
    just('%')
        .ignore_then(generic_property_parser(prop_parser))
        .try_map(|(prop_name, mut properties), span| {
            let res = match prop_name {
                "trunc" | "truncate" => {
                    dbg!(&properties);
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

pub(super) fn group_parser<'a>(
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

pub(super) fn sticker_parser<'a>()
-> impl Parser<'a, &'a str, String, extra::Err<Rich<'a, char>>> + Clone {
    let string = string_parser();
    just("$sticker")
        .ignore_then(
            just("name")
                .ignored()
                .then_ignore(just(':').padded())
                .then(string)
                .delimited_by(just('('), just(')')),
        )
        .map(|((), name)| name)
        .labelled("sticker")
}
