//! # handlist
//!
//! A strict, human-friendly subset of YAML for hand-edited configuration
//! files. This crate provides the reference parser, canonical serializer, and
//! `serde` integration for [handlist 1.0](../spec/handlist-spec.md).
//!
//! ```
//! let value = handlist::parse("name: example\nitems:\n  - one\n  - two\n").unwrap();
//! let text = handlist::to_string(&value).unwrap();
//! assert_eq!(text, "name: example\nitems:\n  - one\n  - two\n");
//! ```
//!
//! The core parser has no dependencies. The default `serde` feature enables
//! `from_str`, `to_string`, and `to_value`; `format`, `schema`, and `cli` are
//! opt-in features.
//!
//! Use [`parse_with_spans`] when the source positions of values and mapping
//! keys are needed. [`from_str`] uses the same span information internally, so
//! serde errors such as a wrong field type, a missing field, or an unknown
//! field carry the position of the field currently being decoded.
//!
//! With the `format` feature, `format()` canonicalizes source text while
//! preserving comments, and `format_with_config()` applies the optional rules
//! from a `.fmt.handlist` configuration parsed as `FormatConfig`. With the
//! `schema` feature, `validate_json_schema()` validates the JSON projection of
//! a document against a draft-07 or 2020-12 JSON Schema.
//!
//! The crate also ships a `handlist` CLI with `check` and `fmt` subcommands
//! when the `cli` feature is enabled:
//!
//! ```text
//! cargo run --features cli --bin handlist -- check path/to/config
//! cargo run --features cli --bin handlist -- fmt path/to/configs
//! ```

#![warn(missing_docs)]

mod emit;
mod error;
#[cfg(feature = "format")]
mod format;
mod parser;
#[cfg(feature = "schema")]
mod schema;
#[cfg(feature = "serde")]
mod serde_support;
mod span;
mod value;

pub use error::Error;
#[cfg(feature = "format")]
pub use format::FormatConfig;
#[cfg(feature = "schema")]
pub use schema::{SchemaError, SchemaValidation, validate_json_schema};
pub use span::{Span, SpanMap};
pub use value::{Map, Value};

/// Parses a handlist document into a [`Value`].
///
/// See [`from_str`] for deserializing directly into a Rust type.
pub fn parse(source: &str) -> Result<Value, Error> {
    parser::parse(source)
}

/// Parses a handlist document and returns the values' source spans.
///
/// The returned [`SpanMap`] maps JSON Pointer paths to [`Span`]s. The root is
/// `""`, mapping entries and sequence elements extend their parent pointer (for
/// example `/site/pages/0`), and keys are escaped according to RFC 6901.
///
/// [`SpanMap::get`] returns the start of a value; [`SpanMap::key_span`] returns
/// the start of the mapping key that owns it.
pub fn parse_with_spans(source: &str) -> Result<(Value, SpanMap), Error> {
    parser::parse_with_spans(source)
}

/// Formats a handlist document into canonical form while preserving comments.
///
/// Full-line comments keep their text and are re-indented to the node they
/// belong to. Inline comments keep their text and stay on the same value line.
/// Blank lines adjacent to comments and separators between top-level nodes are
/// normalized to at most one. [`to_string_value`] remains the comment-free
/// serializer for [`Value`]s.
#[cfg(feature = "format")]
pub fn format(source: &str) -> Result<String, Error> {
    format::to_string(source)
}

/// Formats a handlist document using a `.fmt.handlist` configuration.
///
/// This function is available with the `format` feature. See
/// [`FormatConfig::parse`] and [`format()`].
#[cfg(feature = "format")]
pub fn format_with_config(source: &str, config: &FormatConfig) -> Result<String, Error> {
    format::to_string_with_config(source, config)
}

/// Serializes a [`Value`] into canonical handlist text.
///
/// Names containing a `Value` are always accepted, regardless of whether the
/// `serde` feature is enabled. When `serde` is enabled, use [`to_string`] for
/// arbitrary `Serialize` types.
pub fn to_string_value(value: &Value) -> Result<String, Error> {
    emit::to_string(value)
}

/// Serializes a [`Value`] into canonical handlist text.
///
/// This function is available when the `serde` feature is disabled.
#[cfg(not(feature = "serde"))]
pub fn to_string(value: &Value) -> Result<String, Error> {
    emit::to_string(value)
}

/// Serializes any [`serde::Serialize`] value into canonical handlist text.
///
/// This function is available when the (default) `serde` feature is enabled.
#[cfg(feature = "serde")]
pub fn to_string<T>(value: &T) -> Result<String, Error>
where
    T: ?Sized + serde::Serialize,
{
    serde_support::to_string(value)
}

/// Converts any [`serde::Serialize`] value into a handlist [`Value`].
#[cfg(feature = "serde")]
pub fn to_value<T>(value: &T) -> Result<Value, Error>
where
    T: ?Sized + serde::Serialize,
{
    serde_support::to_value(value)
}

/// Deserializes a handlist document into any [`serde::de::DeserializeOwned`]
/// type.
///
/// Errors produced by the serde data model (wrong field type, missing field,
/// unknown field, and so on) carry the source position of the field currently
/// being decoded when one is available.
#[cfg(feature = "serde")]
pub fn from_str<T>(source: &str) -> Result<T, Error>
where
    T: serde::de::DeserializeOwned,
{
    serde_support::from_str(source)
}

impl std::str::FromStr for Value {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        parse(source)
    }
}
