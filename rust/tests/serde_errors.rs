use handlist::Error;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct Config {
    name: String,
    count: u8,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Root {
    site: Site,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Site {
    name: String,
    count: u8,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Values {
    values: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Settings {
    action: Action,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
enum Action {
    Page,
    None,
}

fn deserialize_error<T: DeserializeOwned>(source: &str) -> Error {
    handlist::from_str::<T>(source)
        .err()
        .expect("expected a deserialization error")
}

fn assert_location(error: &Error, line: usize, column: usize) {
    assert_eq!(error.line(), Some(line), "error: {error}");
    assert_eq!(error.column(), Some(column), "error: {error}");
    assert_eq!(error.span(), Some(handlist::Span::new(line, column)));
    assert!(
        error
            .to_string()
            .starts_with(&format!("line {line}, column {column}: ")),
        "unexpected display: {error}"
    );
}

#[test]
fn wrong_field_type_points_at_the_field_key() {
    let error = deserialize_error::<Config>("name: example\ncount: nope\n");
    assert_location(&error, 2, 1);
    assert!(error.message().contains("invalid type"), "{error}");
}

#[test]
fn unknown_field_points_at_the_unknown_key() {
    let error = deserialize_error::<Config>("name: example\ncount: 1\nextra: true\n");
    assert_location(&error, 3, 1);
    assert!(error.message().contains("unknown field"), "{error}");
}

#[test]
fn missing_field_points_at_the_last_consumed_field() {
    let error = deserialize_error::<Config>("name: example\n");
    assert_location(&error, 1, 1);
    assert!(error.message().contains("missing field"), "{error}");

    // With no fields at all, the error falls back to the map's own position.
    let error = deserialize_error::<Config>("{}\n");
    assert_location(&error, 1, 1);
    assert!(error.message().contains("missing field"), "{error}");

    // Nested maps keep the inner field's position.
    let error = deserialize_error::<Root>("site:\n  name: example\n");
    assert_location(&error, 2, 3);
    assert!(error.message().contains("missing field"), "{error}");
}

#[test]
fn nested_field_error_keeps_the_inner_key_position() {
    let source = "\
site:
  name: example
  count: nope
";
    let error = deserialize_error::<Root>(source);
    assert_location(&error, 3, 3);
    assert!(error.message().contains("invalid type"), "{error}");
}

#[test]
fn sequence_element_error_points_at_the_element() {
    let source = "\
values:
  - 1
  - nope
";
    let error = deserialize_error::<Values>(source);
    assert_location(&error, 3, 5);
    assert!(error.message().contains("invalid type"), "{error}");
}

#[test]
fn root_type_error_points_at_the_root_value() {
    let error = deserialize_error::<String>("name: example\n");
    assert_location(&error, 1, 1);
}

#[test]
fn enum_and_map_key_errors_have_positions() {
    let error = deserialize_error::<Settings>("action: unknown\n");
    assert_location(&error, 1, 1);
    assert!(error.message().contains("unknown variant"), "{error}");

    let error = deserialize_error::<BTreeMap<u8, String>>("\"abc\": value\n");
    assert_location(&error, 1, 1);
    assert!(error.message().contains("mapping key"), "{error}");
}
