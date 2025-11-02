use chumsky::prelude::*;

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
