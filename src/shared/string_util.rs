pub(crate) trait CharExt {
    fn is_regex_special_char(&self) -> bool;
}

pub(crate) trait StringExt {
    fn escape_regex_chars(&self) -> String;
    fn from_utf8_lossy_as_owned(v: Vec<u8>) -> String;
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

    fn from_utf8_lossy_as_owned(v: Vec<u8>) -> String {
        if let std::borrow::Cow::Owned(string) = String::from_utf8_lossy(&v) {
            string
        } else {
            // SAFETY: `String::from_utf8_lossy`'s guaranteec valid utf8 when a borrowed
            // variant is returned. Owned value, meaning invalid utf8, is handled above.
            unsafe { String::from_utf8_unchecked(v) }
        }
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
