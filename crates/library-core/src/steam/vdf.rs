//! A parser for Valve's text KeyValues format ("VDF"), used by Steam's
//! `libraryfolders.vdf` and `appmanifest_*.acf` files.
//!
//! ```text
//! "AppState"
//! {
//!     "appid"  "570"
//!     "name"   "Dota 2"
//! }
//! ```
//!
//! Keys are case-insensitive in practice, so lookups ignore case. Duplicate keys
//! are kept in order. Conditionals such as `[$WIN32]` are skipped.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    Obj(Vec<(String, Value)>),
}

impl Value {
    /// The first child with this key, ignoring case. `None` for strings.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(entries) => entries.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Value::Str(_) => None,
        }
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Value)] {
        match self {
            Value::Obj(entries) => entries,
            Value::Str(_) => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parses a whole document. The result is an object holding the top-level
/// entries, so `parse(text)?.get("AppState")` reaches the root block.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    let mut p = Parser { chars: text.chars().peekable(), line: 1 };
    let entries = p.entries(false)?;
    Ok(Value::Obj(entries))
}

#[derive(Debug, PartialEq)]
enum Token {
    Str(String),
    Open,
    Close,
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    line: usize,
}

impl Parser<'_> {
    fn err<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError { line: self.line, message: message.into() })
    }

    /// Reads `key value` pairs until `}` (when `nested`) or end of input.
    fn entries(&mut self, nested: bool) -> Result<Vec<(String, Value)>, ParseError> {
        let mut entries = Vec::new();
        loop {
            let key = match self.token()? {
                None if nested => return self.err("unexpected end of input, expected '}'"),
                None => return Ok(entries),
                Some(Token::Close) if nested => return Ok(entries),
                Some(Token::Close) => return self.err("unexpected '}'"),
                Some(Token::Open) => return self.err("expected a key, found '{'"),
                Some(Token::Str(key)) => key,
            };
            let value = match self.token()? {
                Some(Token::Str(s)) => Value::Str(s),
                Some(Token::Open) => Value::Obj(self.entries(true)?),
                Some(Token::Close) | None => return self.err(format!("key {key:?} has no value")),
            };
            entries.push((key, value));
        }
    }

    fn token(&mut self) -> Result<Option<Token>, ParseError> {
        loop {
            let Some(c) = self.chars.next() else { return Ok(None) };
            match c {
                '\n' => self.line += 1,
                c if c.is_whitespace() => {}
                '/' if self.chars.peek() == Some(&'/') => self.skip_line(),
                // Platform conditionals like [$WIN32] apply to the preceding pair; ignore them.
                '[' => self.skip_until(']'),
                '{' => return Ok(Some(Token::Open)),
                '}' => return Ok(Some(Token::Close)),
                '"' => return self.quoted().map(|s| Some(Token::Str(s))),
                c => return Ok(Some(Token::Str(self.bare(c)))),
            }
        }
    }

    fn quoted(&mut self) -> Result<String, ParseError> {
        let mut s = String::new();
        loop {
            match self.chars.next() {
                None => return self.err("unterminated string"),
                Some('"') => return Ok(s),
                Some('\\') => match self.chars.next() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    // Windows paths in older files are sometimes written with single backslashes.
                    Some(other) => {
                        s.push('\\');
                        s.push(other);
                    }
                    None => return self.err("unterminated string"),
                },
                Some('\n') => {
                    self.line += 1;
                    s.push('\n');
                }
                Some(c) => s.push(c),
            }
        }
    }

    fn bare(&mut self, first: char) -> String {
        let mut s = String::from(first);
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() || matches!(c, '"' | '{' | '}') {
                break;
            }
            s.push(c);
            self.chars.next();
        }
        s
    }

    fn skip_line(&mut self) {
        for c in self.chars.by_ref() {
            if c == '\n' {
                self.line += 1;
                break;
            }
        }
    }

    fn skip_until(&mut self, end: char) {
        for c in self.chars.by_ref() {
            if c == '\n' {
                self.line += 1;
            }
            if c == end {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_blocks_and_ignores_key_case() {
        let doc = parse(
            r#"
            "AppState"
            {
                "appid"     "570"
                "Name"      "Dota 2"
                "UserConfig"
                {
                    "language"  "english"
                }
            }
            "#,
        )
        .unwrap();
        let app = doc.get("appstate").unwrap();
        assert_eq!(app.get_str("APPID"), Some("570"));
        assert_eq!(app.get_str("name"), Some("Dota 2"));
        assert_eq!(app.get("userconfig").unwrap().get_str("language"), Some("english"));
    }

    #[test]
    fn handles_escapes_comments_conditionals_and_bare_tokens() {
        let doc = parse(
            "// a comment\n\
             root {\n\
               \"path\" \"C:\\\\Games\\\\Steam\"\n\
               \"quote\" \"say \\\"hi\\\"\" // trailing comment\n\
               \"win\" \"1\" [$WIN32]\n\
               bare value\n\
             }\n",
        )
        .unwrap();
        let root = doc.get("root").unwrap();
        assert_eq!(root.get_str("path"), Some(r"C:\Games\Steam"));
        assert_eq!(root.get_str("quote"), Some(r#"say "hi""#));
        assert_eq!(root.get_str("win"), Some("1"));
        assert_eq!(root.get_str("bare"), Some("value"));
    }

    #[test]
    fn keeps_duplicate_keys_in_order() {
        let doc = parse(r#""a" { "k" "1" "k" "2" }"#).unwrap();
        let values: Vec<_> =
            doc.get("a").unwrap().entries().iter().map(|(_, v)| v.as_str().unwrap()).collect();
        assert_eq!(values, ["1", "2"]);
    }

    #[test]
    fn reports_errors_with_line_numbers() {
        let err = parse("\"a\"\n{\n\"k\" \"v\"\n").unwrap_err();
        assert_eq!(err.line, 4);
        assert!(err.message.contains("expected '}'"));
        assert!(parse("\"a\" \"unterminated").is_err());
        assert!(parse("}").is_err());
    }
}
