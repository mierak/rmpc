use std::collections::HashMap;

use chumsky::prelude::*;
use itertools::Itertools;
use strum::IntoDiscriminant;

use super::{StyleFile, string::string_parser, style::style_parser};
use crate::config::theme::properties::{PropertyFile, PropertyKindFile};

pub fn attribute_parser<'a>(
    prop_parser: impl Parser<'a, &'a str, PropertyFile<PropertyKindFile>, extra::Err<Rich<'a, char>>>
    + Clone
    + 'a,
) -> impl Parser<'a, &'a str, (&'a str, Attribute), extra::Err<Rich<'a, char>>> + Clone {
    let ident = text::ascii::ident();

    let style = style_parser().map(Attribute::Style);
    let prop = prop_parser.map(Attribute::Prop);
    let label = string_parser().map(Attribute::String);
    let bool = just("true").or(just("false")).from_str::<bool>().unwrapped().map(Attribute::Bool);
    let decimal = text::int(10).try_map(|v: &str, span| match v.parse() {
        Ok(v) => Ok(Attribute::UInt(v)),
        Err(_) => Err(Rich::custom(span, "Invalid decimal number")),
    });

    ident
        .padded()
        .then_ignore(just(':').padded())
        .then(label.or(prop).or(decimal).or(bool).or(style).padded())
        .boxed()
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, strum::EnumDiscriminants, PartialEq)]
#[strum_discriminants(derive(strum::Display))]
pub(super) enum Attribute {
    Style(StyleFile),
    String(String),
    UInt(usize),
    Bool(bool),
    Prop(PropertyFile<PropertyKindFile>),
}

impl Attribute {
    pub(super) fn to_err<'a>(
        &self,
        expected: AttributeDiscriminants,
        span: SimpleSpan,
    ) -> chumsky::error::Rich<'a, char> {
        Rich::custom(span, format!("Expected {expected} found {}", self.discriminant()))
    }
}

#[allow(dead_code)]
pub(super) trait AttrExt {
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod test {
    use chumsky::prelude::*;
    use rstest::rstest;

    use super::attribute_parser;
    use crate::config::theme::{
        Modifiers,
        StyleFile,
        parser::Attribute,
        properties::{PropertyFile, PropertyKindFile, PropertyKindFileOrText},
    };

    fn dummy_property_parser<'a>()
    -> impl Parser<'a, &'a str, PropertyFile<PropertyKindFile>, extra::Err<Rich<'a, char>>> + Clone + 'a
    {
        just("$dummy").to(PropertyFile::<PropertyKindFile> {
            kind: PropertyKindFileOrText::Text(String::new()),
            style: None,
            default: None,
        })
    }

    #[rstest]
    // bool
    #[case("name: true", "name", Ok(Attribute::Bool(true)))]
    #[case("name: false", "name", Ok(Attribute::Bool(false)))]
    #[case("name:false", "name", Ok(Attribute::Bool(false)))]
    #[case("name : false", "name", Ok(Attribute::Bool(false)))]
    // uint
    #[case("name: 0", "name", Ok(Attribute::UInt(0)))]
    #[case("name: 1", "name", Ok(Attribute::UInt(1)))]
    #[case("name: 999", "name", Ok(Attribute::UInt(999)))]
    #[case("name : 999", "name", Ok(Attribute::UInt(999)))]
    #[case("name:999", "name", Ok(Attribute::UInt(999)))]
    #[case("name: 000", "name", Err(()))]
    #[case("name: -1", "name", Err(()))]
    // style
    #[case("st: {fg: red, bg: blue, mods: bu}", "st", Ok(Attribute::Style(StyleFile { fg: Some("red".to_string()), bg: Some("blue".to_string()), modifiers: Some(Modifiers::Bold | Modifiers::Underlined) })))]
    #[case("st: { mods: bu, bg: blue, fg: red }", "st", Ok(Attribute::Style(StyleFile { fg: Some("red".to_string()), bg: Some("blue".to_string()), modifiers: Some(Modifiers::Bold | Modifiers::Underlined) })))]
    #[case("st: {fg: red}", "st", Ok(Attribute::Style(StyleFile { fg: Some("red".to_string()), bg: None, modifiers: None })))]
    #[case("st: {bg: red}", "st", Ok(Attribute::Style(StyleFile { fg: None, bg: Some("red".to_string()), modifiers: None })))]
    #[case("st: {mods: x}", "st", Ok(Attribute::Style(StyleFile { fg: None, bg: None, modifiers: Some(Modifiers::CrossedOut) })))]
    // prop
    #[case("dummy: $dummy", "dummy", Ok(Attribute::Prop(PropertyFile { kind: PropertyKindFileOrText::Text(String::new()), style: None, default: None })))]
    #[case("dummy : $dummy", "dummy", Ok(Attribute::Prop(PropertyFile { kind: PropertyKindFileOrText::Text(String::new()), style: None, default: None })))]
    #[case("dummy:$dummy", "dummy", Ok(Attribute::Prop(PropertyFile { kind: PropertyKindFileOrText::Text(String::new()), style: None, default: None })))]
    // string
    #[case("name: \"42\"", "name", Ok(Attribute::String("42".to_string())))]
    #[case("name:\"42\"", "name", Ok(Attribute::String("42".to_string())))]
    #[case("name :\"4    2\" ", "name", Ok(Attribute::String("4    2".to_string())))]
    #[case("name: \"some string\"", "name", Ok(Attribute::String("some string".to_string())))]
    #[case("name: some string", "name", Err(()))]
    // invalid
    #[case("invalid: something else", "name", Err(()))]
    fn bool_attr(#[case] input: &str, #[case] key: &str, #[case] expected: Result<Attribute, ()>) {
        let parser = attribute_parser(dummy_property_parser());
        let result = parser.parse(input).into_result();

        match (expected, result) {
            (Ok(attr), Ok(result)) => assert_eq!(result, (key, attr)),
            (Ok(attr), Err(err)) => panic!("Expected {key}: {attr:?} but got error: {err:?}"),
            (Err(()), Ok(result)) => panic!("Expected error but got: {result:?}"),
            (Err(()), Err(_)) => {}
        }
    }
}
