//! Error type shared by the parser, the emitter, and the serde integration.

use crate::span::Span;
use std::fmt;

/// An error produced while parsing or serializing handlist.
///
/// Parse errors carry a one-based line and column. Deserialization errors from
/// [`crate::from_str`] also carry the position of the field currently being
/// decoded when that position is available. Serialization errors do not have a
/// source location, so [`Error::line`] and [`Error::column`] return `None` for
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
    location: Option<Span>,
}

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            location: None,
        }
    }

    pub(crate) fn at(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            location: Some(Span::new(line, column)),
        }
    }

    /// The human-readable error message, without location information.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The source span, if this error has one.
    pub fn span(&self) -> Option<Span> {
        self.location
    }

    /// The one-based line number, if this error has a location.
    pub fn line(&self) -> Option<usize> {
        self.location.map(|span| span.line)
    }

    /// The one-based column number (in Unicode scalar values), if this error
    /// has a location.
    pub fn column(&self) -> Option<usize> {
        self.location.map(|span| span.column)
    }

    /// Attaches a span unless a more specific location is already present.
    #[cfg(feature = "serde")]
    pub(crate) fn with_span_if_missing(mut self, span: Option<Span>) -> Self {
        if self.location.is_none() {
            self.location = span;
        }
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.location {
            Some(span) => write!(
                f,
                "line {}, column {}: {}",
                span.line, span.column, self.message
            ),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(feature = "serde")]
impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::new(msg.to_string())
    }
}

#[cfg(feature = "serde")]
impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::new(msg.to_string())
    }
}
