use std::collections::{HashMap, HashSet};

use chumsky::{
    container::{Container, Seq},
    prelude::*,
};

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
    .collect::<Vec<(&str, StringOrModifiers)>>()
    .validate(|mods, e, emitter| {
        let mut seen = HashMap::new();
        for (key, val) in mods {
            if seen.insert(key, val).is_some() {
                emitter.emit(Rich::custom(e.span(), format!("Duplicate key: '{key}'")));
            }
        }

        seen
    })
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
    one_of("bdiurxBDIURX")
        .padded()
        .repeated()
        .collect::<Vec<char>>()
        .validate(|mods, e, emitter| {
            let mut set = HashSet::new();
            for modifier in &mods {
                let lowercase = modifier.to_ascii_lowercase();
                if set.contains(&lowercase) {
                    emitter
                        .emit(Rich::custom(e.span(), format!("Duplicate modifier: '{modifier}'")));
                } else if !['b', 'd', 'i', 'u', 'r', 'x'].contains(&lowercase) {
                    emitter.emit(Rich::custom(e.span(), format!("Invalid modifier: '{modifier}'")));
                } else {
                    set.push(modifier);
                }
            }
            mods
        })
        .map(|mods| {
            let mut res = Modifiers::empty();
            for modifier in mods {
                res = match modifier {
                    'b' | 'B' => res.union(Modifiers::Bold),
                    'd' | 'D' => res.union(Modifiers::Dim),
                    'i' | 'I' => res.union(Modifiers::Italic),
                    'u' | 'U' => res.union(Modifiers::Underlined),
                    'r' | 'R' => res.union(Modifiers::Reversed),
                    'x' | 'X' => res.union(Modifiers::CrossedOut),
                    _ => res,
                };
            }
            res
        })
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    use anyhow::Result;
    use rstest::rstest;

    use super::*;
    use crate::config::theme::ConfigColor;

    #[rstest]
    #[case("reset", Ok(ConfigColor::Reset))]
    #[case("default", Ok(ConfigColor::Reset))]
    #[case("black", Ok(ConfigColor::Black))]
    #[case("red", Ok(ConfigColor::Red))]
    #[case("green", Ok(ConfigColor::Green))]
    #[case("yellow", Ok(ConfigColor::Yellow))]
    #[case("blue", Ok(ConfigColor::Blue))]
    #[case("magenta", Ok(ConfigColor::Magenta))]
    #[case("cyan", Ok(ConfigColor::Cyan))]
    #[case("gray", Ok(ConfigColor::Gray))]
    #[case("dark_gray", Ok(ConfigColor::DarkGray))]
    #[case("light_red", Ok(ConfigColor::LightRed))]
    #[case("light_green", Ok(ConfigColor::LightGreen))]
    #[case("light_yellow", Ok(ConfigColor::LightYellow))]
    #[case("light_blue", Ok(ConfigColor::LightBlue))]
    #[case("light_magenta", Ok(ConfigColor::LightMagenta))]
    #[case("light_cyan", Ok(ConfigColor::LightCyan))]
    #[case("white", Ok(ConfigColor::White))]
    #[case("#ff00ff", Ok(ConfigColor::Hex(u32::from_str_radix("ff00ff", 16).unwrap())))]
    #[case("#ff00f", Err(()))]
    #[case("rgb(255,0,255)", Ok(ConfigColor::Rgb(255, 0, 255)))]
    #[case("rgb(255,0,256)", Err(()))]
    #[case("rgb( 255 , 255 , 255 )", Ok(ConfigColor::Rgb(255, 255, 255)))]
    #[case("255", Ok(ConfigColor::Indexed(255)))]
    #[case("256", Err(()))]
    #[case("notacolor", Err(()))]
    fn test_config_color_try_from(#[case] input: &str, #[case] expected: Result<ConfigColor, ()>) {
        let result = color_str_parser()
            .parse(input)
            .into_result()
            .map_err(|e| anyhow::anyhow!("{:?}", e))
            .and_then(|c| ConfigColor::try_from(c.as_bytes()));
        match expected {
            Ok(val) => assert_eq!(result.unwrap(), val),
            Err(()) => assert!(result.is_err()),
        }
    }

    #[rstest]
    #[case("", Ok(Modifiers::empty()))]
    #[case("b", Ok(Modifiers::Bold))]
    #[case("d", Ok(Modifiers::Dim))]
    #[case("i", Ok(Modifiers::Italic))]
    #[case("u", Ok(Modifiers::Underlined))]
    #[case("r", Ok(Modifiers::Reversed))]
    #[case("x", Ok(Modifiers::CrossedOut))]
    #[case("B", Ok(Modifiers::Bold))]
    #[case("D", Ok(Modifiers::Dim))]
    #[case("I", Ok(Modifiers::Italic))]
    #[case("U", Ok(Modifiers::Underlined))]
    #[case("R", Ok(Modifiers::Reversed))]
    #[case("X", Ok(Modifiers::CrossedOut))]
    #[case("bb", Err(()))]
    #[case("bB", Err(()))]
    #[case("bd", Ok(Modifiers::Bold | Modifiers::Dim))]
    #[case("bdiurx", Ok(Modifiers::Bold | Modifiers::Dim | Modifiers::Italic | Modifiers::Underlined | Modifiers::Reversed | Modifiers::CrossedOut))]
    #[case("a", Err(()))]
    #[case("ba", Err(()))]
    #[case("bx", Ok(Modifiers::Bold | Modifiers::CrossedOut))]
    #[case("db", Ok(Modifiers::Dim | Modifiers::Bold))]
    #[case("  b D i U r X  ", Ok(Modifiers::Bold | Modifiers::Dim | Modifiers::Italic | Modifiers::Underlined | Modifiers::Reversed | Modifiers::CrossedOut))]
    fn test_modifiers_parser(#[case] input: &str, #[case] expected: Result<Modifiers, ()>) {
        let result = modifiers_parser().parse(input).into_result();
        match expected {
            Ok(val) => assert_eq!(result.unwrap(), val),
            Err(()) => assert!(result.is_err()),
        }
    }

    #[rstest]
    #[case("{fg:red,bg:blue,mods:bi}", Ok(StyleFile { fg: Some("red".to_string()), bg: Some("blue".to_string()), modifiers: Some(Modifiers::Bold | Modifiers::Italic) }))]
    #[case("{fg:rgb(255,0,0),bg:#ff00ff,mods:b}", Ok(StyleFile { fg: Some("rgb(255,0,0)".to_string()), bg: Some("#ff00ff".to_string()), modifiers: Some(Modifiers::Bold) }))]
    #[case("{fg:green}", Ok(StyleFile { fg: Some("green".to_string()), bg: None, modifiers: None }))]
    #[case("{bg:blue}", Ok(StyleFile { fg: None, bg: Some("blue".to_string()), modifiers: None }))]
    #[case("{mods:b}", Ok(StyleFile { fg: None, bg: None, modifiers: Some(Modifiers::Bold) }))]
    #[case("{fg:red,bg:blue}", Ok(StyleFile { fg: Some("red".to_string()), bg: Some("blue".to_string()), modifiers: None }))]
    #[case("{fg:red,fg:blue}", Err(()))]
    #[case("{mods:b,mods:d}", Err(()))]
    #[case("{fg:invalid,bg:blue,mods:b}", Ok(StyleFile { fg: Some("invalid".to_string()), bg: Some("blue".to_string()), modifiers: Some(Modifiers::Bold) }))]
    #[case("{}", Ok(StyleFile { fg: None, bg: None, modifiers: None }))]
    #[case("{fg:red,bg:blue,mods:bdiurx}", Ok(StyleFile { fg: Some("red".to_string()), bg: Some("blue".to_string()), modifiers: Some(Modifiers::Bold | Modifiers::Dim | Modifiers::Italic | Modifiers::Underlined | Modifiers::Reversed | Modifiers::CrossedOut) }))]
    #[case("{fg:red,mods:bdiurx,bg:blue}", Ok(StyleFile { fg: Some("red".to_string()), bg: Some("blue".to_string()), modifiers: Some(Modifiers::Bold | Modifiers::Dim | Modifiers::Italic | Modifiers::Underlined | Modifiers::Reversed | Modifiers::CrossedOut) }))]
    #[case("{fg:red,bg:blue,mods:b,extra:val}", Err(()))]
    fn test_style_parser(#[case] input: &str, #[case] expected: Result<StyleFile, ()>) {
        let result = style_parser().parse(input).into_result();
        match expected {
            Ok(val) => assert_eq!(result.unwrap(), val),
            Err(()) => assert!(result.is_err()),
        }
    }
}
