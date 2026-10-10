//! Bounded, deterministic repair of JSON container punctuation.
//!
//! This module knows nothing about tools, models, schemas, or execution. It
//! never invents keys/values, closes truncated containers, changes quotes, or
//! resolves duplicate keys. Callers must authorize decoding with a schema and
//! validate the returned value before using it. `strict_json::parse` stays strict.
use crate::strict_json;
use serde::Serialize;
use serde_json::Value;

const MAX_BYTES: usize = 128 * 1024;
const MAX_EDITS: usize = 8;
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EditKind {
    TrailingComma,
    MissingComma,
    RedundantCloser,
}

/// Offsets refer to UTF-8 bytes in the ORIGINAL input, not a regenerated string.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Edit {
    pub byte_offset: usize,
    pub remove: String,
    pub insert: String,
    pub kind: EditKind,
}

#[derive(Debug, Clone)]
pub struct Repair {
    pub text: String,
    pub value: Value,
    pub edits: Vec<Edit>,
}

/// Repair only: a trailing comma, a missing separator between whitespace-
/// separated complete values/members, or a redundant repeated closing delimiter
/// that cannot close the current container. Existing valid closures always win;
/// no delimiter is moved or replaced and incomplete inputs fail closed.
///
/// Returns None for valid JSON, unsupported/ambiguous errors, or exceeded bounds.
pub fn repair_container(text: &str) -> Option<Repair> {
    if text.len() > MAX_BYTES || strict_json::parse(text).is_ok() {
        return None;
    }
    let mut parser = Parser {
        text,
        at: 0,
        edits: vec![],
        last_closer: None,
    };
    parser.space();
    if !matches!(parser.peek(), Some(b'[' | b'{')) {
        return None;
    }
    parser.value(0)?;
    parser.space();
    // A complete root may have duplicated final closers, never another value.
    while parser.at < text.len() {
        parser.redundant_closer()?;
        parser.space();
    }
    if parser.edits.is_empty() {
        return None;
    }
    let mut repaired = text.to_owned();
    for edit in parser.edits.iter().rev() {
        repaired.replace_range(
            edit.byte_offset..edit.byte_offset + edit.remove.len(),
            &edit.insert,
        );
    }
    // Includes duplicate-key rejection and full-input consumption. Parsing a
    // prefix or dropping an invalid item is never sufficient evidence.
    let value = strict_json::parse(&repaired).ok()?;
    Some(Repair {
        text: repaired,
        value,
        edits: parser.edits,
    })
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
    edits: Vec<Edit>,
    last_closer: Option<u8>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }
    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.at += 1;
        }
    }
    fn edit(&mut self, at: usize, remove: &str, insert: &str, kind: EditKind) -> Option<()> {
        if self.edits.len() >= MAX_EDITS {
            return None;
        }
        self.edits.push(Edit {
            byte_offset: at,
            remove: remove.into(),
            insert: insert.into(),
            kind,
        });
        Some(())
    }
    fn redundant_closer(&mut self) -> Option<()> {
        let byte = self.peek()?;
        if !matches!(byte, b']' | b'}') || self.last_closer != Some(byte) {
            return None;
        }
        self.edit(
            self.at,
            if byte == b']' { "]" } else { "}" },
            "",
            EditKind::RedundantCloser,
        )?;
        self.at += 1;
        Some(())
    }
    fn value(&mut self, depth: usize) -> Option<()> {
        self.space();
        match self.peek()? {
            b'[' | b'{' => self.container(depth + 1),
            b'"' | b'-' | b'0'..=b'9' | b't' | b'f' | b'n' => {
                let (value, used) = strict_json::parse_prefix(&self.text[self.at..]).ok()?;
                if value.is_array() || value.is_object() {
                    return None;
                }
                self.at += used;
                self.last_closer = None;
                Some(())
            }
            _ => None,
        }
    }
    fn container(&mut self, depth: usize) -> Option<()> {
        if depth > MAX_DEPTH {
            return None;
        }
        let object = self.peek()? == b'{';
        let close = if object { b'}' } else { b']' };
        self.at += 1;
        self.last_closer = None;
        self.space();
        if self.peek() == Some(close) {
            self.at += 1;
            self.last_closer = Some(close);
            return Some(());
        }
        loop {
            if object {
                self.space();
                let (key, used) = strict_json::parse_prefix(&self.text[self.at..]).ok()?;
                if !key.is_string() {
                    return None;
                }
                self.at += used;
                self.space();
                if self.peek() != Some(b':') {
                    return None;
                }
                self.at += 1;
            }
            self.value(depth)?;
            let value_end = self.at;
            loop {
                self.space();
                match self.peek()? {
                    byte if byte == close => {
                        self.at += 1;
                        self.last_closer = Some(close);
                        return Some(());
                    }
                    b',' => {
                        let comma = self.at;
                        self.at += 1;
                        self.last_closer = None;
                        self.space();
                        if self.peek() == Some(close) {
                            self.edit(comma, ",", "", EditKind::TrailingComma)?;
                            self.at += 1;
                            self.last_closer = Some(close);
                            return Some(());
                        }
                        break;
                    }
                    b']' | b'}' => self.redundant_closer()?,
                    byte if self.at > value_end
                        && (byte == b'"'
                            || !object
                                && matches!(
                                    byte,
                                    b'[' | b'{' | b'-' | b'0'..=b'9' | b't' | b'f' | b'n'
                                )) =>
                    {
                        // Only whitespace separates the complete tokens. An
                        // earlier removed closer must not make this look safe.
                        if !self.text[value_end..self.at]
                            .bytes()
                            .all(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
                        {
                            return None;
                        }
                        self.edit(self.at, "", ",", EditKind::MissingComma)?;
                        self.last_closer = None;
                        break;
                    }
                    _ => return None,
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/json_syntax_repair.rs"]
mod tests;
