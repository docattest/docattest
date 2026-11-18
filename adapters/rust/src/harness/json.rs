//! JSON, as far as a generated program needs it: reading what the adapter tells it and
//! a project's snapshots, manifests and the files facts read, and writing its results.

use std::fmt;
use std::ops::Index;

/// A JSON value. An object keeps its keys in the order written.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

static NULL: Json = Json::Null;

impl Json {
    /// The text of a string; `None` for anything else.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(text) => Some(text),
            _ => None,
        }
    }

    /// A number; 0 for anything else.
    pub fn number(&self) -> f64 {
        match self {
            Json::Number(n) => *n,
            _ => 0.0,
        }
    }

    /// The items of an array; none for anything else.
    pub fn iter(&self) -> std::slice::Iter<'_, Json> {
        match self {
            Json::Array(items) => items.iter(),
            _ => [].iter(),
        }
    }

    /// The keys of an object, in the order written; none for anything else.
    pub fn keys(&self) -> Vec<&str> {
        match self {
            Json::Object(entries) => entries.iter().map(|(key, _)| key.as_str()).collect(),
            _ => Vec::new(),
        }
    }

    /// The value as JSON text.
    pub fn to_json(&self) -> String {
        match self {
            Json::Null => "null".into(),
            Json::Bool(b) => b.to_string(),
            Json::Number(n) => n.to_string(),
            Json::String(text) => quote(text),
            Json::Array(items) => {
                format!("[{}]", items.iter().map(Json::to_json).collect::<Vec<_>>().join(","))
            }
            Json::Object(entries) => {
                let written: Vec<String> =
                    entries.iter().map(|(k, v)| format!("{}:{}", quote(k), v.to_json())).collect();
                format!("{{{}}}", written.join(","))
            }
        }
    }
}

/// A string as JSON text.
pub fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// How a null is shown: as Rust shows the absent value it stands for, `Option::None`, so a
/// fact over JSON data and one over a typed constant agree.
pub const NULL_SHOWN: &str = "None";

/// A value as a page states it: a string as its text, a number as Rust prints it, null as
/// [`NULL_SHOWN`], and anything with parts as JSON.
impl fmt::Display for Json {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Json::String(text) => f.write_str(text),
            Json::Null => f.write_str(NULL_SHOWN),
            other => f.write_str(&other.to_json()),
        }
    }
}

/// A member of an object by its key; null when there is none.
impl Index<&str> for Json {
    type Output = Json;
    fn index(&self, key: &str) -> &Json {
        match self {
            Json::Object(entries) => entries.iter().find(|(k, _)| k == key).map_or(&NULL, |(_, v)| v),
            _ => &NULL,
        }
    }
}

/// An item of an array by its place; null when there is none.
impl Index<usize> for Json {
    type Output = Json;
    fn index(&self, at: usize) -> &Json {
        match self {
            Json::Array(items) => items.get(at).unwrap_or(&NULL),
            _ => &NULL,
        }
    }
}

impl<'a> IntoIterator for &'a Json {
    type Item = &'a Json;
    type IntoIter = std::slice::Iter<'a, Json>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Reads JSON text.
pub fn parse(text: &str) -> Result<Json, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut reader = Reader { chars: &chars, at: 0 };
    let value = reader.value()?;
    reader.space();
    if reader.at < chars.len() {
        return Err(format!("unexpected text at character {}", reader.at));
    }
    Ok(value)
}

struct Reader<'a> {
    chars: &'a [char],
    at: usize,
}

impl Reader<'_> {
    fn space(&mut self) {
        while self.chars.get(self.at).is_some_and(|c| c.is_whitespace()) {
            self.at += 1;
        }
    }

    fn take(&mut self, wanted: char) -> bool {
        self.space();
        let found = self.chars.get(self.at) == Some(&wanted);
        self.at += usize::from(found);
        found
    }

    fn value(&mut self) -> Result<Json, String> {
        self.space();
        match self.chars.get(self.at) {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => self.string().map(Json::String),
            Some(_) => self.word(),
            None => Err("unexpected end of JSON".into()),
        }
    }

    fn word(&mut self) -> Result<Json, String> {
        let start = self.at;
        while self.chars.get(self.at).is_some_and(|c| !",]}".contains(*c) && !c.is_whitespace()) {
            self.at += 1;
        }
        let word: String = self.chars[start..self.at].iter().collect();
        match word.as_str() {
            "null" => Ok(Json::Null),
            "true" => Ok(Json::Bool(true)),
            "false" => Ok(Json::Bool(false)),
            number => number.parse().map(Json::Number).map_err(|_| format!("not JSON: {number}")),
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.at += 1;
        let mut items = Vec::new();
        while !self.take(']') {
            items.push(self.value()?);
            if !self.take(',') && self.chars.get(self.at) != Some(&']') {
                return Err(format!("expected , or ] at character {}", self.at));
            }
        }
        Ok(Json::Array(items))
    }

    fn object(&mut self) -> Result<Json, String> {
        self.at += 1;
        let mut entries = Vec::new();
        while !self.take('}') {
            self.space();
            let key = self.string()?;
            if !self.take(':') {
                return Err(format!("expected : at character {}", self.at));
            }
            entries.push((key, self.value()?));
            if !self.take(',') && self.chars.get(self.at) != Some(&'}') {
                return Err(format!("expected , or }} at character {}", self.at));
            }
        }
        Ok(Json::Object(entries))
    }

    fn string(&mut self) -> Result<String, String> {
        if self.chars.get(self.at) != Some(&'"') {
            return Err(format!("expected a string at character {}", self.at));
        }
        self.at += 1;
        let mut text = String::new();
        loop {
            let c = *self.chars.get(self.at).ok_or("a string that does not end")?;
            self.at += 1;
            match c {
                '"' => return Ok(text),
                '\\' => text.push(self.escape()?),
                c => text.push(c),
            }
        }
    }

    fn escape(&mut self) -> Result<char, String> {
        let c = *self.chars.get(self.at).ok_or("a string that does not end")?;
        self.at += 1;
        Ok(match c {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'u' => return self.unicode(),
            other => other,
        })
    }

    /// A `\uXXXX` escape, with the second half of a surrogate pair when it is one.
    fn unicode(&mut self) -> Result<char, String> {
        let unit = |reader: &mut Self| -> Result<u32, String> {
            let digits: String =
                reader.chars.get(reader.at..reader.at + 4).ok_or("a short \\u escape")?.iter().collect();
            reader.at += 4;
            u32::from_str_radix(&digits, 16).map_err(|_| format!("not a \\u escape: {digits}"))
        };
        let first = unit(self)?;
        if !(0xd800..0xdc00).contains(&first) {
            return Ok(char::from_u32(first).unwrap_or('\u{fffd}'));
        }
        self.at += 2;
        let second = unit(self)?;
        let code = 0x10000 + ((first - 0xd800) << 10) + (second.wrapping_sub(0xdc00) & 0x3ff);
        Ok(char::from_u32(code).unwrap_or('\u{fffd}'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_read_with_their_keys_in_order() {
        let read = parse(r#" { "b": [1, 2.5, -3e2], "a": {"t": true, "f": false, "n": null} } "#).unwrap();
        assert_eq!(read.keys(), vec!["b", "a"]);
        assert_eq!(read["b"][1], Json::Number(2.5));
        assert_eq!(read["b"][2].number(), -300.0);
        assert_eq!(read["a"]["t"], Json::Bool(true));
        assert_eq!(read["a"]["n"], Json::Null);
        assert_eq!(read["missing"][7], Json::Null);
        assert_eq!(parse("[]").unwrap().iter().count(), 0);
    }

    #[test]
    fn strings_are_read_and_written_with_their_escapes() {
        let read = parse(r#""a\"b\\c\n\té😀""#).unwrap();
        assert_eq!(read.as_str(), Some("a\"b\\c\n\té😀"));
        assert_eq!(quote("a\"b\\c\n\u{1}"), r#""a\"b\\c\n\u0001""#);
        assert_eq!(parse(&quote("tab\there")).unwrap().as_str(), Some("tab\there"));
    }

    #[test]
    fn values_are_written_as_json_and_shown_as_a_page_states_them() {
        let value = parse(r#"{"k":["x",1,null]}"#).unwrap();
        assert_eq!(value.to_json(), r#"{"k":["x",1,null]}"#);
        assert_eq!(Json::String("EUR".into()).to_string(), "EUR");
        assert_eq!(Json::Number(1.5).to_string(), "1.5");
        assert_eq!(Json::Null.to_string(), "None");
        assert_eq!(value["k"].to_string(), r#"["x",1,null]"#);
    }

    #[test]
    fn text_that_is_not_json_is_refused() {
        for text in ["", "[1,", "[1 2]", "{\"a\" 1}", "{a: 1}", "\"open", "nul", "1 2", r#""\u12""#] {
            assert!(parse(text).is_err(), "{text:?} was read");
        }
    }
}
