//! JSON Schema validation for handlist documents.

use crate::{Error, Value};
use jsonschema::Draft;
use serde_json::Value as JsonValue;
use std::fmt;

/// The result of validating a handlist document against a JSON Schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaValidation {
    errors: Vec<SchemaError>,
}

impl SchemaValidation {
    /// Returns `true` when the document conforms to the schema.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the validation errors, in validator order.
    pub fn errors(&self) -> &[SchemaError] {
        &self.errors
    }

    /// Consumes the result and returns the validation errors.
    pub fn into_errors(self) -> Vec<SchemaError> {
        self.errors
    }
}

/// One JSON Schema validation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaError {
    instance_path: String,
    schema_path: String,
    message: String,
}

impl SchemaError {
    /// A JSON Pointer to the failing value in the converted JSON document.
    ///
    /// The root value is `""`, matching RFC 6901.
    pub fn instance_path(&self) -> &str {
        &self.instance_path
    }

    /// A JSON Pointer to the failing JSON Schema keyword.
    pub fn schema_path(&self) -> &str {
        &self.schema_path
    }

    /// The human-readable validation message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = if self.instance_path.is_empty() {
            "/"
        } else {
            &self.instance_path
        };
        write!(f, "{path}: {}", self.message)
    }
}

/// Validates a handlist document against a JSON Schema.
///
/// The document is converted to JSON using handlist's data model: mappings
/// become objects, sequences become arrays, and integers/floats become JSON
/// numbers. The schema is parsed from `schema_source`.
///
/// Supported drafts are JSON Schema draft-07 and 2020-12. A schema without
/// `$schema` is treated as 2020-12. Internal `$ref`s are supported; external
/// references are not resolved.
///
/// Syntax errors and invalid schemas are returned as [`Error`]. A successfully
/// parsed document that does not conform to the schema is represented by a
/// [`SchemaValidation`] with `is_valid() == false`, not by `Err`.
pub fn validate_json_schema(source: &str, schema_source: &str) -> Result<SchemaValidation, Error> {
    let value = crate::parse(source)?;
    let json = to_json(&value)?;
    let schema: JsonValue = serde_json::from_str(schema_source)
        .map_err(|error| Error::new(format!("invalid JSON Schema: {error}")))?;
    let draft = draft_for(&schema)?;
    let validator = jsonschema::options()
        .with_draft(draft)
        .build(&schema)
        .map_err(|error| Error::new(format!("invalid JSON Schema: {error}")))?;

    let errors = validator
        .iter_errors(&json)
        .map(|error| SchemaError {
            instance_path: error.instance_path.to_string(),
            schema_path: error.schema_path.to_string(),
            message: error.to_string(),
        })
        .collect();

    Ok(SchemaValidation { errors })
}

fn to_json(value: &Value) -> Result<JsonValue, Error> {
    serde_json::to_value(value)
        .map_err(|error| Error::new(format!("could not convert handlist value to JSON: {error}")))
}

fn draft_for(schema: &JsonValue) -> Result<Draft, Error> {
    let Some(uri) = schema.get("$schema") else {
        return Ok(Draft::Draft202012);
    };
    let Some(uri) = uri.as_str() else {
        return Err(Error::new("JSON Schema '$schema' must be a string"));
    };

    let uri = uri.strip_suffix('#').unwrap_or(uri);
    match uri {
        "http://json-schema.org/draft-07/schema" | "https://json-schema.org/draft-07/schema" => {
            Ok(Draft::Draft7)
        }
        "http://json-schema.org/draft/2020-12/schema"
        | "https://json-schema.org/draft/2020-12/schema" => Ok(Draft::Draft202012),
        _ => Err(Error::new(format!(
            "unsupported JSON Schema draft {uri:?}; only draft-07 and 2020-12 are supported"
        ))),
    }
}
