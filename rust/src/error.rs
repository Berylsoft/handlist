//! Error type shared by the parser, the emitter, and the serde integration.

use std::fmt;

/// An error produced while parsing or serializing handlist.
///
/// Parse errors carry a one-based line and column. Serialization errors do
/// not have a source location, so [`Error::line`] and [`Error::column`] return
/// `None` for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
    location: Option<(usize, usize)>,
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
            location: Some((line, column)),
        }
    }

    /// The human-readable error message, without location information.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The one-based line number, if this is a parse error.
    pub fn line(&self) -> Option<usize> {
        self.location.map(|(line, _)| line)
    }

    /// The one-based column number (in Unicode scalar values), if this is a
    /// parse error.
    pub fn column(&self) -> Option<usize> {
        self.location.map(|(_, column)| column)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.location {
            Some((line, column)) => write!(f, "line {line}, column {column}: {}", self.message),
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
