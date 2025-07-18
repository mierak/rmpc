use std::collections::HashMap;

use chumsky::prelude::*;

use super::super::{StyleFile, style::Modifiers};

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
