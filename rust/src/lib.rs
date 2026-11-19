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
//! The parser has no dependencies. Enable the default `serde` feature for
//! `from_str`, `to_string`, and `to_value`.

#![warn(missing_docs)]

mod emit;
mod error;
mod parser;
#[cfg(feature = "serde")]
mod serde_support;
mod value;

pub use error::Error;
pub use value::{Map, Value};

/// Parses a handlist document into a [`Value`].
///
/// See [`from_str`] for deserializing directly into a Rust type.
pub fn parse(source: &str) -> Result<Value, Error> {
    parser::parse(source)
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
