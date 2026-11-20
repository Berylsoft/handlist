#![cfg(feature = "schema")]

use handlist::validate_json_schema;

const SCHEMA: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["site"],
  "additionalProperties": false,
  "properties": {
    "site": {
      "type": "object",
      "required": ["name", "count"],
      "additionalProperties": false,
      "properties": {
        "name": { "type": "string" },
        "count": { "type": "integer", "minimum": 1 }
      }
    }
  }
}"#;

#[test]
fn accepts_a_document_matching_the_schema() {
    let source = "\
site:
  name: Example
  count: 3
";
    let result = validate_json_schema(source, SCHEMA).expect("schema is valid");
    assert!(result.is_valid(), "{:?}", result.errors());
    assert!(result.errors().is_empty());
}

#[test]
fn reports_the_instance_path_and_message_for_invalid_values() {
    let source = "\
site:
  name: Example
  count: 0
";
    let result = validate_json_schema(source, SCHEMA).expect("schema is valid");
    assert!(!result.is_valid());
    let errors = result.errors();
    assert!(!errors.is_empty());
    assert_eq!(errors[0].instance_path(), "/site/count");
    assert!(errors[0].message().contains("0"), "{}", errors[0]);
    assert!(errors[0].to_string().starts_with("/site/count: "));
    assert!(!result.clone().into_errors().is_empty());
}

#[test]
fn reports_missing_and_additional_properties() {
    let missing = "\
site:
  count: 1
";
    let result = validate_json_schema(missing, SCHEMA).expect("schema is valid");
    assert!(!result.is_valid());
    assert!(
        result
            .errors()
            .iter()
            .any(|error| { error.instance_path() == "/site" && error.message().contains("name") })
    );

    let additional = "\
site:
  name: Example
  count: 1
extra: true
";
    let result = validate_json_schema(additional, SCHEMA).expect("schema is valid");
    assert!(!result.is_valid());
    assert!(
        result
            .errors()
            .iter()
            .any(|error| { error.instance_path() == "" && error.message().contains("extra") })
    );
}

#[test]
fn supports_draft_07_and_internal_refs() {
    let schema = r##"{
      "$schema": "http://json-schema.org/draft-07/schema#",
      "definitions": {
        "name": { "type": "string", "minLength": 2 }
      },
      "type": "object",
      "properties": {
        "name": { "$ref": "#/definitions/name" }
      },
      "required": ["name"],
      "additionalProperties": false
    }"##;

    let valid = validate_json_schema("name: ab\n", schema).expect("schema is valid");
    assert!(valid.is_valid());

    let invalid = validate_json_schema("name: a\n", schema).expect("schema is valid");
    assert!(!invalid.is_valid());
    assert_eq!(invalid.errors()[0].instance_path(), "/name");
}

#[test]
fn supports_2020_12_defs_and_rejects_other_drafts() {
    let schema = r##"{
      "$schema": "https://json-schema.org/draft/2020-12/schema",
      "$defs": {
        "count": { "type": "integer", "minimum": 2 }
      },
      "type": "object",
      "properties": {
        "count": { "$ref": "#/$defs/count" }
      }
    }"##;
    assert!(
        validate_json_schema("count: 2\n", schema)
            .expect("valid schema")
            .is_valid()
    );
    assert!(
        !validate_json_schema("count: 1\n", schema)
            .expect("valid schema")
            .is_valid()
    );

    let error = validate_json_schema(
        "name: x\n",
        r#"{"$schema":"http://json-schema.org/draft-04/schema#"}"#,
    )
    .unwrap_err();
    assert!(error.message().contains("only draft-07 and 2020-12"));
}

#[test]
fn converts_handlist_types_to_json() {
    let schema = r#"{
      "type": "object",
      "required": ["count", "ratio", "enabled", "nothing", "tags", "nested"],
      "properties": {
        "count": { "type": "integer" },
        "ratio": { "type": "number" },
        "enabled": { "type": "boolean" },
        "nothing": { "type": "null" },
        "tags": { "type": "array", "items": { "type": "string" } },
        "nested": { "type": "object" }
      }
    }"#;
    let source = "\
count: 3
ratio: 1.5
enabled: true
nothing: null
tags: [a, b]
nested:
  key: value
";
    let result = validate_json_schema(source, schema).expect("schema is valid");
    assert!(result.is_valid(), "{:?}", result.errors());
}

#[test]
fn handles_boolean_schemas() {
    let result = validate_json_schema("name: x\n", "true").expect("schema is valid");
    assert!(result.is_valid());

    let result = validate_json_schema("name: x\n", "false").expect("schema is valid");
    assert!(!result.is_valid());
}

#[test]
fn reports_invalid_handlist_and_schema_inputs() {
    let handlist_error = validate_json_schema("name: [\n", "{}").unwrap_err();
    assert_eq!(handlist_error.line(), Some(1));

    let schema_error = validate_json_schema("name: x\n", "{").unwrap_err();
    assert!(schema_error.message().contains("invalid JSON Schema"));

    let invalid_schema = validate_json_schema("name: x\n", r#"{"type": 42}"#).unwrap_err();
    assert!(invalid_schema.message().contains("invalid JSON Schema"));
}
