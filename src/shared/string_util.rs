pub(crate) trait CharExt {
    fn is_regex_special_char(&self) -> bool;
}

pub(crate) trait StringExt {
    fn escape_regex_chars(&self) -> String;
}

impl StringExt for String {
    fn escape_regex_chars(&self) -> String {
        let mut buf = String::with_capacity(self.len());
        for char in self.chars() {
            if char.is_regex_special_char() {
                buf.push('\\');
            }
            buf.push(char);
        }
        buf
    }
}

impl CharExt for char {
    fn is_regex_special_char(&self) -> bool {
        matches!(
            self,
            '\\' | '.'
                | '+'
                | '*'
                | '?'
                | '('
                | ')'
                | '|'
                | '['
                | ']'
                | '{'
                | '}'
                | '^'
                | '$'
                | '#'
                | '&'
                | '-'
                | '~'
        )
    }
}
