use anyhow::{Result, bail};
use clap::Parser;

use crate::config::cli::Args;

/// Split a shell-like command line into tokens (argv).
///
/// Supports single `'...'` and double `"..."` quotes and backslash escapes.
///
/// # Errors
/// Returns `Err` if the input contains an **unclosed quote** or a
/// **dangling backslash** escape at the end of the string.
///
/// # Examples
/// ```
/// let v = Args::split_command_line(r#"addyt --name "rick astley""#).unwrap();
/// assert_eq!(v, ["addyt", "--name", "rick astley"]);
/// ```
pub fn split_command_line(s: &str) -> anyhow::Result<Vec<String>> {
    let mut args = Vec::new();
    let mut token = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;
    let mut in_arg = false;

    for ch in s.chars() {
        if escaped {
            if ch == '\'' || ch == '"' || ch.is_whitespace() {
                token.push(ch);
                in_arg = true;
            } else if ch == '\\' {
                token.push('\\');
                in_arg = true;
            } else {
                token.push('\\');
                token.push(ch);
                in_arg = true;
            }
            escaped = false;
            continue;
        }

        match ch {
            '\\' if !in_single_quote => {
                escaped = true;
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                in_arg = true;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                in_arg = true;
            }
            c if c.is_whitespace() && !in_single_quote && !in_double_quote => {
                if in_arg {
                    args.push(std::mem::take(&mut token));
                    in_arg = false;
                }
            }
            c => {
                token.push(c);
                in_arg = true;
            }
        }
    }

    if escaped {
        anyhow::bail!("dangling backslash");
    }
    if in_single_quote {
        anyhow::bail!("unclosed single quote");
    }
    if in_double_quote {
        anyhow::bail!("unclosed double quote");
    }

    if in_arg {
        args.push(token);
    }

    Ok(args)
}

/// Parse a command-line string into [`Args`] as if typed in the terminal.
///
/// This function tokenizes `s` like a shell, prepends a synthetic `argv[0]`
/// (`"rmpc"`), and invokes Clap’s non-exiting parser.
///
/// # Errors
/// Returns a [`clap::Error`] when:
/// - tokenization fails (e.g., **unclosed quote**, **dangling backslash**),
/// - the input is **empty**,
/// - or Clap rejects the arguments (e.g., unknown subcommand/flag, missing
///   required args).
///
/// # Examples
/// ```
/// let a = Args::parse_cli_line(r#"addyt --name "rick astley""#).unwrap();
/// match a.command {
///     Some(Command::AddYt { name, url, .. }) => { assert!(name.is_some()); assert!(url.is_none()); }
///     _ => unreachable!(),
/// }
/// ```
pub fn parse_cli_line(s: &str) -> Result<Args, clap::Error> {
    let mut argv = split_command_line(s)
        .map_err(|e| clap::Error::raw(clap::error::ErrorKind::InvalidValue, e))?;
    if argv.is_empty() {
        return Err(clap::Error::raw(
            clap::error::ErrorKind::MissingRequiredArgument,
            "empty command",
        ));
    }
    argv.insert(0, "rmpc".to_string()); // clap expects argv[0]
    <Args as Parser>::try_parse_from(argv)
}

pub fn contains_placeholder_args(command: &str) -> bool {
    let mut iter = command.chars().peekable();

    while let Some(c) = iter.next() {
        match c {
            '{' if matches!(iter.peek(), Some('{')) => {
                iter.next();
            }
            '}' if matches!(iter.peek(), Some('}')) => {
                iter.next();
            }
            '{' if matches!(iter.peek(), Some('}')) => {
                return true;
            }
            '}' => {
                return false;
            }
            '{' => {
                return false;
            }
            _ => {}
        }
    }

    false
}

pub fn replace_arg_placeholder(command: &str, args: &[String]) -> Result<(usize, String)> {
    let mut result = String::with_capacity(command.len());
    let mut iter = command.chars().peekable();
    let mut arg_idx = 0;

    while let Some(c) = iter.next() {
        match c {
            '{' if matches!(iter.peek(), Some('{')) => {
                result.push('{');
                iter.next();
            }
            '}' if matches!(iter.peek(), Some('}')) => {
                result.push('}');
                iter.next();
            }
            '{' if matches!(iter.peek(), Some('}')) => {
                iter.next();
                if let Some(arg) = args.get(arg_idx) {
                    for c in arg.chars() {
                        match c {
                            '"' => {
                                result.push('\\');
                                result.push('"');
                            }
                            _ => result.push(c),
                        }
                    }
                    arg_idx += 1;
                } else {
                    bail!("Not enough arguments provided for command");
                }
            }
            '}' => {
                bail!("Unmatched '}}' in command");
            }
            '{' => {
                // Named placeholder, unsupported
                bail!("Named placeholders are currently not supported in external command");
            }
            c => result.push(c),
        }
    }

    Ok((arg_idx, result))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    mod split_command_line {
        use super::*;

        #[test]
        fn empty_input() {
            assert_eq!(split_command_line("").unwrap(), Vec::<String>::new());
            assert_eq!(split_command_line("   \t\n").unwrap(), Vec::<String>::new());
        }

        #[test]
        fn simple_whitespace_split() {
            assert_eq!(split_command_line("a b  c\t\nd").unwrap(), vec!["a", "b", "c", "d"]);
        }

        #[test]
        fn preserves_inner_whitespace_inside_single_quotes() {
            assert_eq!(split_command_line("cmd 'two words' end").unwrap(), vec![
                "cmd",
                "two words",
                "end"
            ]);
        }

        #[test]
        fn preserves_inner_whitespace_inside_double_quotes() {
            assert_eq!(split_command_line("cmd \"two words\" end").unwrap(), vec![
                "cmd",
                "two words",
                "end"
            ]);
        }

        #[test]
        fn quotes_can_be_empty() {
            assert_eq!(split_command_line("a '' b").unwrap(), vec!["a", "", "b"]);
            assert_eq!(split_command_line("a \"\" b").unwrap(), vec!["a", "", "b"]);
        }

        #[test]
        fn quotes_can_join_tokens_without_whitespace() {
            assert_eq!(split_command_line("ab\"cd\"ef").unwrap(), vec!["abcdef"]);
            assert_eq!(split_command_line("ab'cd'ef").unwrap(), vec!["abcdef"]);
            assert_eq!(split_command_line("a\"b c\"d").unwrap(), vec!["ab cd"]);
        }

        #[test]
        fn nested_quote_chars_are_literal_when_other_quote_is_active() {
            assert_eq!(split_command_line("\"a'b'c\"").unwrap(), vec!["a'b'c"]);
            assert_eq!(split_command_line("'a\"b\"c'").unwrap(), vec!["a\"b\"c"]);
        }

        #[test]
        fn backslash_escapes_outside_single_quotes() {
            assert_eq!(split_command_line(r"a\ b").unwrap(), vec!["a b"]);
            assert_eq!(split_command_line(r#"a\"b"#).unwrap(), vec![r#"a"b"#]);
            assert_eq!(split_command_line(r"a\\b").unwrap(), vec![r"a\b"]);
        }

        #[test]
        fn backslash_is_literal_inside_single_quotes() {
            assert_eq!(split_command_line(r"'a\b'").unwrap(), vec![r"a\b"]);
            assert_eq!(split_command_line(r"'a\ b'").unwrap(), vec![r"a\ b"]);
        }

        #[test]
        fn backslash_can_escape_whitespace_and_quotes_in_double_quotes() {
            assert_eq!(split_command_line(r#""a\ b""#).unwrap(), vec!["a b"]);
            assert_eq!(split_command_line(r#""a\"b""#).unwrap(), vec![r#"a"b"#]);
            assert_eq!(split_command_line(r#""a\z""#).unwrap(), vec!["a\\z"]);
        }

        #[test]
        fn trims_argument_boundaries_only_on_unquoted_whitespace() {
            assert_eq!(split_command_line("  a   b  ").unwrap(), vec!["a", "b"]);
            assert_eq!(split_command_line("  ' a '  ").unwrap(), vec![" a "]);
            assert_eq!(split_command_line("  \" a \"  ").unwrap(), vec![" a "]);
        }

        #[test]
        fn dangling_backslash_is_error() {
            let err = split_command_line(r"abc\").unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("dangling backslash"), "got: {msg}");
        }

        #[test]
        fn unclosed_single_quote_is_error() {
            let err = split_command_line("abc 'def").unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("unclosed single quote"), "got: {msg}");
        }

        #[test]
        fn unclosed_double_quote_is_error() {
            let err = split_command_line("abc \"def").unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("unclosed double quote"), "got: {msg}");
        }

        #[test]
        fn trailing_token_without_whitespace_is_included() {
            assert_eq!(split_command_line("a b").unwrap(), vec!["a", "b"]);
            assert_eq!(split_command_line("a").unwrap(), vec!["a"]);
            assert_eq!(split_command_line("'a'").unwrap(), vec!["a"]);
            assert_eq!(split_command_line("\"a\"").unwrap(), vec!["a"]);
        }

        #[test]
        fn unicode_is_supported() {
            assert_eq!(split_command_line("λ \"你好 世界\" 'Привет мир'").unwrap(), vec![
                "λ",
                "你好 世界",
                "Привет мир"
            ]);
        }

        #[test]
        fn whitespace_characters_split_when_unquoted() {
            assert_eq!(split_command_line("a\tb\nc\rd").unwrap(), vec!["a", "b", "c", "d"]);
        }

        #[test]
        fn quote_toggling_does_not_emit_tokens_until_whitespace_or_end() {
            assert_eq!(split_command_line("a\"b\"c d").unwrap(), vec!["abc", "d"]);
            assert_eq!(split_command_line("a'b'c d").unwrap(), vec!["abc", "d"]);
        }

        #[test]
        fn escaped_whitespace_between_args_does_not_split() {
            assert_eq!(split_command_line(r"one\ two three").unwrap(), vec!["one two", "three"]);
        }
    }

    mod replace_arg_placeholder {
        use super::*;

        #[test]
        fn replaces_positional_placeholders_in_order() {
            let cmd = "echo {} {}";
            let args = vec!["a".to_string(), "b".to_string()];
            let (used_count, out) = replace_arg_placeholder(cmd, &args).unwrap();
            assert_eq!(out, "echo a b");
            assert_eq!(used_count, 2);
        }

        #[test]
        fn errors_when_not_enough_args() {
            let cmd = "echo {} {}";
            let args = vec!["only_one".to_string()];
            let err = replace_arg_placeholder(cmd, &args).unwrap_err();
            assert!(
                err.to_string().contains("Not enough arguments provided for command"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn leaves_extra_args_unused() {
            let cmd = "echo {}";
            let args = vec!["a".to_string(), "b".to_string(), "c".to_string()];
            let (used_count, out) = replace_arg_placeholder(cmd, &args).unwrap();
            assert_eq!(out, "echo a");
            assert_eq!(used_count, 1);
        }

        #[test]
        fn escaped_left_brace_double_open_becomes_single_open() {
            let cmd = "echo {{";
            let args: Vec<String> = vec![];
            let (used_count, out) = replace_arg_placeholder(cmd, &args).unwrap();
            assert_eq!(out, "echo {");
            assert_eq!(used_count, 0);
        }

        #[test]
        fn escaped_right_brace_double_close_becomes_single_close() {
            let cmd = "echo }}";
            let args: Vec<String> = vec![];
            let (used_count, out) = replace_arg_placeholder(cmd, &args).unwrap();
            assert_eq!(out, "echo }");
            assert_eq!(used_count, 0);
        }

        #[test]
        fn escaped_braces_around_placeholder_do_not_consume_args() {
            let cmd = "echo {{}}";
            let args = vec!["SHOULD_NOT_BE_USED".to_string()];
            let (used_count, out) = replace_arg_placeholder(cmd, &args).unwrap();
            assert_eq!(out, "echo {}");
            assert_eq!(used_count, 0);
        }

        #[test]
        fn unmatched_right_brace_errors() {
            let cmd = "echo }";
            let args: Vec<String> = vec![];
            let err = replace_arg_placeholder(cmd, &args).unwrap_err();
            assert!(err.to_string().contains("Unmatched '}'"), "unexpected error: {err}");
        }

        #[test]
        fn named_placeholder_is_rejected() {
            let cmd = "echo {arg1}";
            let args = vec!["x".to_string()];
            let err = replace_arg_placeholder(cmd, &args).unwrap_err();
            assert!(
                err.to_string().contains("Named placeholders are currently not supported"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn lone_left_brace_is_rejected_as_named_placeholder_start() {
            let cmd = "echo {";
            let args: Vec<String> = vec![];
            let err = replace_arg_placeholder(cmd, &args).unwrap_err();
            assert!(
                err.to_string().contains("Named placeholders are currently not supported"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn mixed_escaping_and_placeholders() {
            let cmd = "pre{{ mid {} post}}";
            let args = vec!["X".to_string()];
            let (used_count, out) = replace_arg_placeholder(cmd, &args).unwrap();
            assert_eq!(out, "pre{ mid X post}");
            assert_eq!(used_count, 1);
        }

        #[test]
        fn adjacent_placeholders() {
            let cmd = "{}{}{}";
            let args = vec!["a".to_string(), "b".to_string(), "c".to_string()];
            let (used_count, out) = replace_arg_placeholder(cmd, &args).unwrap();
            assert_eq!(out, "abc");
            assert_eq!(used_count, 3);
        }

        #[test]
        fn placeholder_at_end() {
            let cmd = "echo {}";
            let args = vec!["tail".to_string()];
            let (used_count, out) = replace_arg_placeholder(cmd, &args).unwrap();
            assert_eq!(out, "echo tail");
            assert_eq!(used_count, 1);
        }

        #[test]
        fn escapes_quotes_in_arguments() {
            let cmd = "echo {}";
            let args = vec!["He said \"Hello\"".to_string()];
            let (used_count, out) = replace_arg_placeholder(cmd, &args).unwrap();
            assert_eq!(out, "echo He said \\\"Hello\\\"");
            assert_eq!(used_count, 1);
        }
    }
}
