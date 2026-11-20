//! Source locations for parsed handlist values.

use std::collections::BTreeMap;

/// A one-based source position.
///
/// Columns count Unicode scalar values, matching the parser's error columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    /// One-based line number.
    pub line: usize,
    /// One-based column, counted in Unicode scalar values.
    pub column: usize,
}

impl Span {
    /// Creates a span from a one-based line and column.
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A map from JSON Pointer paths to source spans.
///
/// The root value is addressed by the empty pointer `""`. Mapping entries and
/// sequence elements extend their parent pointer, for example `/site/pages/0`.
/// Keys are escaped according to RFC 6901: `~` becomes `~0` and `/` becomes
/// `~1`.
///
/// [`SpanMap::get`] returns the start of the **value** at a pointer.
/// [`SpanMap::key_span`] returns the start of the mapping **key** that owns a
/// value, when the value is a mapping entry. Use it when reporting a problem
/// with a field rather than with its value; for example, a missing or unknown
/// field should point at the field's key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpanMap {
    values: BTreeMap<String, Span>,
    keys: BTreeMap<String, Span>,
}

impl SpanMap {
    /// Creates an empty span map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the start position of the value at `pointer`.
    pub fn get(&self, pointer: &str) -> Option<Span> {
        self.values.get(pointer).copied()
    }

    /// Returns the start position of the mapping key that owns the value at
    /// `pointer`, if the value is a mapping entry.
    ///
    /// This returns `None` for the root value and for sequence elements.
    pub fn key_span(&self, pointer: &str) -> Option<Span> {
        self.keys.get(pointer).copied()
    }

    /// Iterates over value pointers and their spans, ordered by pointer.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Span)> + '_ {
        self.values
            .iter()
            .map(|(pointer, span)| (pointer.as_str(), *span))
    }

    /// Iterates over mapping-entry pointers and their key spans, ordered by
    /// pointer.
    pub fn key_iter(&self) -> impl Iterator<Item = (&str, Span)> + '_ {
        self.keys
            .iter()
            .map(|(pointer, span)| (pointer.as_str(), *span))
    }

    /// Returns the number of value spans in the map.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if no value spans are recorded.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(crate) fn insert_value(&mut self, pointer: &str, span: Span) {
        self.values.insert(pointer.to_owned(), span);
    }

    pub(crate) fn insert_key(&mut self, pointer: &str, span: Span) {
        self.keys.insert(pointer.to_owned(), span);
    }
}

/// Extends a JSON Pointer with one RFC 6901 reference token.
pub(crate) fn pointer_child(parent: &str, token: &str) -> String {
    let mut pointer = String::with_capacity(parent.len() + token.len() + 1);
    pointer.push_str(parent);
    pointer.push('/');
    for ch in token.chars() {
        match ch {
            '~' => pointer.push_str("~0"),
            '/' => pointer.push_str("~1"),
            _ => pointer.push(ch),
        }
    }
    pointer
}
