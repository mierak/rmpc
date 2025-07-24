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
                    oneshot_style: properties.optional_style("oneshot_style", span)?,
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
                    oneshot_style: properties.optional_style("oneshot_style", span)?,
                }),
                "st" | "state" => PropertyKindFile::Status(StatusPropertyFile::StateV2 {
                    playing_label: properties.optional_string_default(
                        "playing_label",
                        "Playing",
                        span,
                    )?,
                    paused_label: properties.optional_string_default(
                        "paused_label",
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
                .delimited_by(just('(').padded(), just(')').padded()),
        )
        .map(|((), name)| name)
        .labelled("sticker")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    use rstest::rstest;

    use super::*;
    use crate::config::theme::{
        StyleFile,
        parser::make_error_report,
        properties::PropertyKindFileOrText,
    };

    #[rstest]
    #[case("$vol", Ok(StatusPropertyFile::Volume))]
    #[case("$volume", Ok(StatusPropertyFile::Volume))]
    #[case("$el", Ok(StatusPropertyFile::Elapsed))]
    #[case("$elapsed", Ok(StatusPropertyFile::Elapsed))]
    #[case("$cdur", Ok(StatusPropertyFile::Duration))]
    #[case("$cduration", Ok(StatusPropertyFile::Duration))]
    #[case("$currentduration", Ok(StatusPropertyFile::Duration))]
    #[case("$xf", Ok(StatusPropertyFile::Crossfade))]
    #[case("$xfade", Ok(StatusPropertyFile::Crossfade))]
    #[case("$crossfade", Ok(StatusPropertyFile::Crossfade))]
    #[case("$br", Ok(StatusPropertyFile::Bitrate))]
    #[case("$bitrate", Ok(StatusPropertyFile::Bitrate))]
    #[case("$part", Ok(StatusPropertyFile::Partition))]
    #[case("$partition", Ok(StatusPropertyFile::Partition))]
    #[case("$tab", Ok(StatusPropertyFile::ActiveTab))]
    #[case("$activetab", Ok(StatusPropertyFile::ActiveTab))]
    #[case("$rep", Ok(StatusPropertyFile::RepeatV2 { on_label: "On".to_string(), off_label: "Off".to_string(), on_style: None, off_style: None }))]
    #[case("$rep()", Ok(StatusPropertyFile::RepeatV2 { on_label: "On".to_string(), off_label: "Off".to_string(), on_style: None, off_style: None }))]
    #[case("$rep(on_label:'ON',off_label:'OFF', on_style: { fg: red }, off_style: { fg: black })", Ok(StatusPropertyFile::RepeatV2 { on_label: "ON".to_string(), off_label: "OFF".to_string(), on_style: Some(StyleFile { fg: Some("red".to_string()), bg: None, modifiers: None }), off_style: Some(StyleFile { fg: Some("black".to_string()), bg: None, modifiers: None }) }))]
    #[case("$rep(on_label:'ON',off_label:'OFF', on_style: { fg: red }, off_style: { fg: black }, extra: true)", Err(()))]
    #[case("$rand", Ok(StatusPropertyFile::RandomV2 { on_label: "On".to_string(), off_label: "Off".to_string(), on_style: None, off_style: None }))]
    #[case("$rand()", Ok(StatusPropertyFile::RandomV2 { on_label: "On".to_string(), off_label: "Off".to_string(), on_style: None, off_style: None }))]
    #[case("$random(on_label:'Enabled')", Ok(StatusPropertyFile::RandomV2 { on_label: "Enabled".to_string(), off_label: "Off".to_string(), on_style: None, off_style: None }))]
    #[case("$random(on_label:'Enabled',off_label:'Disabled', on_style: { fg: red }, off_style: { fg: black })", Ok(StatusPropertyFile::RandomV2 { on_label: "Enabled".to_string(), off_label: "Disabled".to_string(), on_style: Some(StyleFile { fg: Some("red".to_string()), bg: None, modifiers: None }), off_style: Some(StyleFile { fg: Some("black".to_string()), bg: None, modifiers: None }) }))]
    #[case("$random(on_label:'Enabled',off_label:'Disabled', on_style: { fg: red }, off_style: { fg: black }, extra: true)", Err(()))]
    #[case("$sin", Ok(StatusPropertyFile::SingleV2 { on_label: "On".to_string(), off_label: "Off".to_string(), oneshot_label: "Oneshot".to_string(), on_style: None, off_style: None, oneshot_style: None }))]
    #[case("$sin()", Ok(StatusPropertyFile::SingleV2 { on_label: "On".to_string(), off_label: "Off".to_string(), oneshot_label: "Oneshot".to_string(), on_style: None, off_style: None, oneshot_style: None }))]
    #[case("$single(oneshot_label:'Once')", Ok(StatusPropertyFile::SingleV2 { on_label: "On".to_string(), off_label: "Off".to_string(), oneshot_label: "Once".to_string(), on_style: None, off_style: None, oneshot_style: None }))]
    #[case("$single(on_label:'On',off_label:'Off')", Ok(StatusPropertyFile::SingleV2 { on_label: "On".to_string(), off_label: "Off".to_string(), oneshot_label: "Oneshot".to_string(), on_style: None, off_style: None, oneshot_style: None }))]
    #[case("$single(on_label:'On',off_label:'Off', oneshot_label:'Once', on_style: { fg: red }, off_style: { fg: black }, oneshot_style: { fg: blue })", Ok(StatusPropertyFile::SingleV2 { on_label: "On".to_string(), off_label: "Off".to_string(), oneshot_label: "Once".to_string(), on_style: Some(StyleFile { fg: Some("red".to_string()), bg: None, modifiers: None }), off_style: Some(StyleFile { fg: Some("black".to_string()), bg: None, modifiers: None }), oneshot_style: Some(StyleFile { fg: Some("blue".to_string()), bg: None, modifiers: None }) }))]
    #[case("$cons", Ok(StatusPropertyFile::ConsumeV2 { on_label: "On".to_string(), off_label: "Off".to_string(), oneshot_label: "Oneshot".to_string(), on_style: None, off_style: None, oneshot_style: None }))]
    #[case("$cons()", Ok(StatusPropertyFile::ConsumeV2 { on_label: "On".to_string(), off_label: "Off".to_string(), oneshot_label: "Oneshot".to_string(), on_style: None, off_style: None, oneshot_style: None }))]
    #[case("$consume(oneshot_label:'Once')", Ok(StatusPropertyFile::ConsumeV2 { on_label: "On".to_string(), off_label: "Off".to_string(), oneshot_label: "Once".to_string(), on_style: None, off_style: None, oneshot_style: None }))]
    #[case("$consume(on_label:'On',off_label:'Off')", Ok(StatusPropertyFile::ConsumeV2 { on_label: "On".to_string(), off_label: "Off".to_string(), oneshot_label: "Oneshot".to_string(), on_style: None, off_style: None, oneshot_style: None }))]
    #[case("$consume(on_label:'On',off_label:'Off', oneshot_label:'Once', on_style: { fg: red }, off_style: { fg: black }, oneshot_style: { fg: blue })", Ok(StatusPropertyFile::ConsumeV2 { on_label: "On".to_string(), off_label: "Off".to_string(), oneshot_label: "Once".to_string(), on_style: Some(StyleFile { fg: Some("red".to_string()), bg: None, modifiers: None }), off_style: Some(StyleFile { fg: Some("black".to_string()), bg: None, modifiers: None }), oneshot_style: Some(StyleFile { fg: Some("blue".to_string()), bg: None, modifiers: None }) }))]
    #[case("$st", Ok(StatusPropertyFile::StateV2 { playing_label: "Playing".to_string(), paused_label: "Paused".to_string(), stopped_label: "Stopped".to_string(), playing_style: None, paused_style: None, stopped_style: None }))]
    #[case("$st()", Ok(StatusPropertyFile::StateV2 { playing_label: "Playing".to_string(), paused_label: "Paused".to_string(), stopped_label: "Stopped".to_string(), playing_style: None, paused_style: None, stopped_style: None }))]
    #[case("$state(playing_label:'Play',paused_label:'Pause',stopped_label:'Stop')", Ok(StatusPropertyFile::StateV2 { playing_label: "Play".to_string(), paused_label: "Pause".to_string(), stopped_label: "Stop".to_string(), playing_style: None, paused_style: None, stopped_style: None }))]
    #[case("$state(playing_label:'Play',paused_label:'Pause',stopped_label:'Stop', playing_style: { fg: red }, paused_style: { fg: green }, stopped_style: { fg: blue })", Ok(StatusPropertyFile::StateV2 { playing_label: "Play".to_string(), paused_label: "Pause".to_string(), stopped_label: "Stop".to_string(), playing_style: Some(StyleFile { fg: Some("red".to_string()), bg: None, modifiers: None }), paused_style: Some(StyleFile { fg: Some("green".to_string()), bg: None, modifiers: None }), stopped_style: Some(StyleFile { fg: Some("blue".to_string()), bg: None, modifiers: None }) }))]
    #[case("$qlen", Ok(StatusPropertyFile::QueueLength { thousands_separator: ",".to_string() }))]
    #[case("$qlen()", Ok(StatusPropertyFile::QueueLength { thousands_separator: ",".to_string() }))]
    #[case("$queuelength(thousands_separator:'_')", Ok(StatusPropertyFile::QueueLength { thousands_separator: "_".to_string() }))]
    #[case("$qtot", Ok(StatusPropertyFile::QueueTimeTotal { separator: None }))]
    #[case("$qtot()", Ok(StatusPropertyFile::QueueTimeTotal { separator: None }))]
    #[case("$queuetotal(separator:':')", Ok(StatusPropertyFile::QueueTimeTotal { separator: Some(":".to_string()) }))]
    #[case("$qrem", Ok(StatusPropertyFile::QueueTimeRemaining { separator: None }))]
    #[case("$qrem()", Ok(StatusPropertyFile::QueueTimeRemaining { separator: None }))]
    #[case("$queueremaining(separator:':')", Ok(StatusPropertyFile::QueueTimeRemaining { separator: Some(":".to_string()) }))]
    fn status_properties(#[case] input: &str, #[case] expected: Result<StatusPropertyFile, ()>) {
        let result = property_parser(dummy_property_parser())
            .parse(input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, input)));

        match expected {
            Ok(v) => assert_eq!(result.unwrap(), PropertyKindFile::Status(v)),
            Err(()) => assert!(result.is_err()),
        }
    }

    #[rstest]
    #[case("$f", Ok(SongPropertyFile::File))]
    #[case("$file", Ok(SongPropertyFile::File))]
    #[case("$fn", Ok(SongPropertyFile::Filename))]
    #[case("$filename", Ok(SongPropertyFile::Filename))]
    #[case("$ext", Ok(SongPropertyFile::FileExtension))]
    #[case("$fileextension", Ok(SongPropertyFile::FileExtension))]
    #[case("$t", Ok(SongPropertyFile::Title))]
    #[case("$title", Ok(SongPropertyFile::Title))]
    #[case("$ar", Ok(SongPropertyFile::Artist))]
    #[case("$artist", Ok(SongPropertyFile::Artist))]
    #[case("$aar", Ok(SongPropertyFile::Other("albumartist".into())))]
    #[case("$albumartist", Ok(SongPropertyFile::Other("albumartist".into())))]
    #[case("$al", Ok(SongPropertyFile::Album))]
    #[case("$album", Ok(SongPropertyFile::Album))]
    #[case("$tr", Ok(SongPropertyFile::Track))]
    #[case("$track", Ok(SongPropertyFile::Track))]
    #[case("$disc", Ok(SongPropertyFile::Disc))]
    #[case("$dur", Ok(SongPropertyFile::Duration))]
    #[case("$duration", Ok(SongPropertyFile::Duration))]
    #[case("$pos", Ok(SongPropertyFile::Position))]
    #[case("$position", Ok(SongPropertyFile::Position))]
    #[case("$tag(value: \"customtag\")", Ok(SongPropertyFile::Other("customtag".into())))]
    #[case("$tag(name: \"customtag\")", Err(()))]
    #[case("$tag(value: \"customtag\", extra: true)", Err(()))]
    #[case("$nonsense", Err(()))]
    fn song_properties(#[case] input: &str, #[case] expected: Result<SongPropertyFile, ()>) {
        let result = property_parser(dummy_property_parser())
            .parse(input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, input)));
        match expected {
            Ok(v) => assert_eq!(result.unwrap(), PropertyKindFile::Song(v)),
            Err(()) => assert!(result.is_err()),
        }
    }

    #[rstest]
    #[case("$w:volume", Ok(WidgetPropertyFile::Volume))]
    #[case("$w:scanstatus", Ok(WidgetPropertyFile::ScanStatus))]
    #[case("$w:states", Ok(WidgetPropertyFile::States { active_style: None, separator_style: None }))]
    #[case("$w:states(active_style: { fg: red }, separator_style: { fg: blue })", Ok(WidgetPropertyFile::States { active_style: Some(StyleFile { fg: Some("red".to_string()), bg: None, modifiers: None }), separator_style: Some(StyleFile { fg: Some("blue".to_string()), bg: None, modifiers: None }) }))]
    fn widget_properties(#[case] input: &str, #[case] expected: Result<WidgetPropertyFile, ()>) {
        let result = property_parser(dummy_property_parser())
            .parse(input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, input)));
        match expected {
            Ok(v) => assert_eq!(result.unwrap(), PropertyKindFile::Widget(v)),
            Err(()) => assert!(result.is_err()),
        }
    }

    #[rstest]
    #[case("[$dummy $dummy]", Ok(vec![PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_string()), style: None, default: None }, PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_string()), style: None, default: None }]))]
    #[case("[$dummy$dummy]", Ok(vec![PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_string()), style: None, default: None }, PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_string()), style: None, default: None }]))]
    #[case("[ $dummy$dummy $dummy      $dummy ]", Ok(vec![PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_string()), style: None, default: None }, PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_string()), style: None, default: None }, PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_string()), style: None, default: None }, PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_string()), style: None, default: None }]))]
    #[case("[$dummy]", Ok(vec![PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_string()), style: None, default: None }]))]
    fn groups(
        #[case] input: &str,
        #[case] expected: Result<Vec<PropertyFile<PropertyKindFile>>, ()>,
    ) {
        let result = group_parser(dummy_property_parser())
            .parse(input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, input)));
        match expected {
            Ok(v) => assert_eq!(result.unwrap(), v),
            Err(()) => assert!(result.is_err()),
        }
    }

    #[rstest]
    #[case("%trunc(content : $dummy, length: 3, from_start: false)", Ok(TransformFile::Truncate { content: Box::new(PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_owned()), style: None, default: None }), length: 3, from_start: false }))]
    #[case("%truncate(content: $dummy, length: 3)", Ok(TransformFile::Truncate { content: Box::new(PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_owned()), style: None, default: None }), length: 3, from_start: false }))]
    #[case("%truncate(content:$dummy, length: 6, from_start: true)", Ok(TransformFile::Truncate { content: Box::new(PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_owned()), style: None, default: None }), length: 6, from_start: true }))]
    #[case("%truncate( content : $dummy , length:6 , from_start : true  )", Ok(TransformFile::Truncate { content: Box::new(PropertyFile { kind: PropertyKindFileOrText::Text("dummy".to_owned()), style: None, default: None }), length: 6, from_start: true }))]
    #[case("%truncate(content : $dummy, from_start: true)", Err(()))]
    #[case("%truncate(content : $dummy, length: 5, from_start: true, extra: 'prop')", Err(()))]
    fn transforms(
        #[case] input: &str,
        #[case] expected: Result<TransformFile<PropertyKindFile>, ()>,
    ) {
        let result = transform_parser(dummy_property_parser())
            .parse(input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, input)));
        match expected {
            Ok(v) => assert_eq!(result.unwrap(), v),
            Err(()) => assert!(result.is_err()),
        }
    }

    #[rstest]
    #[case("$sticker(name: \"test\")", Ok("test"))]
    #[case("$sticker( name : \"test\" )", Ok("test"))]
    #[case("$sticker(name:\"test\")", Ok("test"))]
    #[case("$sticker(name:'test')", Ok("test"))]
    #[case("$sticker(value:'test')", Err(()))]
    #[case("$sticker(name:'test', extra: 0)", Err(()))]
    fn sticker_parser_test(#[case] input: &str, #[case] expected: Result<&str, ()>) {
        let result = sticker_parser()
            .parse(input)
            .into_result()
            .map_err(|errs| anyhow::anyhow!(make_error_report(errs, input)));

        match expected {
            Ok(v) => assert_eq!(result.unwrap(), v),
            Err(()) => assert!(result.is_err()),
        }
    }

    fn dummy_property_parser<'a>()
    -> impl Parser<'a, &'a str, PropertyFile<PropertyKindFile>, extra::Err<Rich<'a, char>>> + Clone + 'a
    {
        just("$dummy").to(PropertyFile::<PropertyKindFile> {
            kind: PropertyKindFileOrText::Text("dummy".to_string()),
            style: None,
            default: None,
        })
    }
}
