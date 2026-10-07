//! Minimal JSON writer, so the app needs no external dependencies.

use std::fmt::Display;

/// Returns `s` as a quoted, escaped JSON string.
pub fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
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

/// Joins already-serialized JSON values into an array.
pub fn array(items: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(","))
}

/// Builder for a JSON object: `Object::new().str("a", "b").num("n", 1).build()`.
#[derive(Default)]
pub struct Object {
    body: String,
}

impl Object {
    pub fn new() -> Object {
        Object::default()
    }

    fn key(&mut self, key: &str) {
        if !self.body.is_empty() {
            self.body.push(',');
        }
        self.body.push_str(&string(key));
        self.body.push(':');
    }

    pub fn str(mut self, key: &str, value: &str) -> Object {
        self.key(key);
        self.body.push_str(&string(value));
        self
    }

    pub fn num(mut self, key: &str, value: impl Display) -> Object {
        self.key(key);
        self.body.push_str(&value.to_string());
        self
    }

    /// Number rounded to one decimal place, or `null`.
    pub fn opt_f64(mut self, key: &str, value: Option<f64>) -> Object {
        self.key(key);
        self.body.push_str(&value.map(|v| format!("{v:.1}")).unwrap_or_else(|| "null".into()));
        self
    }

    pub fn opt_num<T: Display>(mut self, key: &str, value: Option<T>) -> Object {
        self.key(key);
        self.body.push_str(&value.map(|v| v.to_string()).unwrap_or_else(|| "null".into()));
        self
    }

    pub fn bool(mut self, key: &str, value: bool) -> Object {
        self.key(key);
        self.body.push_str(if value { "true" } else { "false" });
        self
    }

    /// Inserts an already-serialized JSON value.
    pub fn raw(mut self, key: &str, json: &str) -> Object {
        self.key(key);
        self.body.push_str(json);
        self
    }

    pub fn build(self) -> String {
        format!("{{{}}}", self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_strings() {
        assert_eq!(string("a\"b\\c\nd"), r#""a\"b\\c\nd""#);
    }

    #[test]
    fn builds_objects() {
        let json = Object::new()
            .str("name", "Deco")
            .num("n", 3)
            .opt_f64("ms", Some(1.25))
            .opt_num::<u32>("loss", None)
            .bool("ok", true)
            .raw("list", &array(["1".to_string(), "2".to_string()]))
            .build();
        assert_eq!(json, r#"{"name":"Deco","n":3,"ms":1.2,"loss":null,"ok":true,"list":[1,2]}"#);
    }
}
