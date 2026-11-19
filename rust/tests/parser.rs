use handlist::{Map, Value};

fn map(entries: Vec<(&str, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn seq(values: Vec<Value>) -> Value {
    Value::Seq(values)
}

fn text(value: &str) -> Value {
    Value::String(value.to_owned())
}

fn parse(source: &str) -> Value {
    handlist::parse(source).unwrap_or_else(|error| panic!("{error}\n--- source ---\n{source}"))
}

fn assert_error(source: &str, needle: &str) {
    let error = handlist::parse(source)
        .map(|_| ())
        .expect_err(&format!("expected an error for:\n{source}"));
    assert!(
        error.to_string().contains(needle),
        "expected error containing {needle:?}, got: {error}\n--- source ---\n{source}"
    );
    assert!(
        error.line().is_some(),
        "error must have a line number: {error}"
    );
    assert!(
        error.column().is_some(),
        "error must have a column number: {error}"
    );
}

#[test]
fn parses_basic_mapping() {
    let source = "\
name: example
enabled: true
count: 3
ratio: 1.5
nothing: null
list: [a, 1, null, \"x\"]
empty_list: []
empty_map: {}
";
    assert_eq!(
        parse(source),
        map(vec![
            ("name", text("example")),
            ("enabled", Value::Bool(true)),
            ("count", Value::Int(3)),
            ("ratio", Value::Float(1.5)),
            ("nothing", Value::Null),
            (
                "list",
                seq(vec![text("a"), Value::Int(1), Value::Null, text("x")])
            ),
            ("empty_list", seq(vec![])),
            ("empty_map", map(vec![])),
        ])
    );
}

#[test]
fn parses_nested_structures_and_compact_forms() {
    let source = "\
root:
  - name: first
    title: One
    values:
      - 1
      - 2
  - name: second
    nested:
      key: value
  - - a
    - b
  - - name: c
    - name: d
";
    assert_eq!(
        parse(source),
        map(vec![(
            "root",
            seq(vec![
                map(vec![
                    ("name", text("first")),
                    ("title", text("One")),
                    ("values", seq(vec![Value::Int(1), Value::Int(2)])),
                ]),
                map(vec![
                    ("name", text("second")),
                    ("nested", map(vec![("key", text("value"))])),
                ]),
                seq(vec![text("a"), text("b")]),
                seq(vec![
                    map(vec![("name", text("c"))]),
                    map(vec![("name", text("d"))]),
                ]),
            ]),
        )])
    );
}

#[test]
fn parses_compact_mapping_with_child_block() {
    let source = "\
- name:
    list:
      - one
      - two
  title: after
";
    assert_eq!(
        parse(source),
        seq(vec![map(vec![
            (
                "name",
                map(vec![("list", seq(vec![text("one"), text("two")]))])
            ),
            ("title", text("after")),
        ])])
    );
}

#[test]
fn parses_root_scalars() {
    assert_eq!(parse("hello\n"), text("hello"));
    assert_eq!(parse("42\n"), Value::Int(42));
    assert_eq!(parse("-5\n"), Value::Int(-5));
    assert_eq!(parse("1.5\n"), Value::Float(1.5));
    assert_eq!(parse("null\n"), Value::Null);
    assert_eq!(parse("true\n"), Value::Bool(true));
    assert_eq!(parse("\"a\\tb\"\n"), text("a\tb"));
    assert_eq!(parse("[a, b]\n"), seq(vec![text("a"), text("b")]));
    assert_eq!(parse("{}\n"), map(vec![]));
    assert_eq!(parse("\"\"\n"), text(""));
    assert_eq!(parse("Tom's Hardware\n"), text("Tom's Hardware"));
    assert_eq!(parse("---foo\n"), text("---foo"));
    assert_eq!(parse("...foo\n"), text("...foo"));
    assert_eq!(
        parse("https://example.com/a#b\n"),
        text("https://example.com/a#b")
    );
}

#[test]
fn resolves_only_minimal_implicit_types() {
    let strings = [
        "yes",
        "no",
        "on",
        "off",
        "True",
        "FALSE",
        "~",
        "Null",
        "NULL",
        "01",
        "+1",
        "0x10",
        "0o17",
        "1_000",
        "1e3",
        ".5",
        "1.",
        "1:30",
        "2024-01-01",
        ".inf",
        ".nan",
        "NaN",
        "Infinity",
        "-01",
        "00",
    ];
    for value in strings {
        assert_eq!(parse(value), text(value), "expected string for {value:?}");
    }

    assert_eq!(parse("9007199254740991"), Value::Int(9_007_199_254_740_991));
    assert_eq!(
        parse("-9007199254740991"),
        Value::Int(-9_007_199_254_740_991)
    );
    assert_eq!(parse("-0"), Value::Int(0));
    assert_eq!(parse("-0.0"), Value::Float(-0.0));
    assert_error("9007199254740992", "safe integer range");
    assert_error("-9007199254740992", "safe integer range");
    let huge = format!("{}.0", "9".repeat(400));
    assert_error(&huge, "out of range");
}

#[test]
fn parses_comments() {
    assert_eq!(
        parse(
            "\
# leading comment
name: value # trailing comment
url: https://example.com/#fragment
csharp: C#
empty: [] # empty list
"
        ),
        map(vec![
            ("name", text("value")),
            ("url", text("https://example.com/#fragment")),
            ("csharp", text("C#")),
            ("empty", seq(vec![])),
        ])
    );

    assert_eq!(parse("name: value #\n"), map(vec![("name", text("value"))]));
    assert_eq!(
        parse("name: value # \tcomment with tab\n"),
        map(vec![("name", text("value"))])
    );
}

#[test]
fn rejects_bad_comments() {
    assert_error("name: value #comment\n", "hash followed by a space");
    assert_error("name: value #\tcomment\n", "hash followed by a space");
    assert_error("name: # comment\n", "missing before the comment");
    assert_error("[a # comment]\n", "space followed by '#'");
}

#[test]
fn parses_block_text() {
    let source = "\
text: |
  first line
  second line

  fourth line
next: 1
";
    assert_eq!(
        parse(source),
        map(vec![
            ("text", text("first line\nsecond line\n\nfourth line")),
            ("next", Value::Int(1)),
        ])
    );

    assert_eq!(
        parse("text: |\n  a\n    b\n"),
        map(vec![("text", text("a\n  b"))])
    );
    assert_eq!(
        parse("text: |\n  # not a comment\n  <p>html</p>\n"),
        map(vec![("text", text("# not a comment\n<p>html</p>"))])
    );
    assert_eq!(
        parse("text: |\nnext: 1\n"),
        map(vec![("text", text("")), ("next", Value::Int(1))])
    );
    assert_eq!(
        parse("text: | # note\n  content\n"),
        map(vec![("text", text("content"))])
    );
    assert_eq!(
        parse("- |\n  item text\n- second\n"),
        seq(vec![text("item text"), text("second")])
    );
    assert_eq!(
        parse("- text: |\n    in item\n  other: 1\n"),
        seq(vec![map(vec![
            ("text", text("in item")),
            ("other", Value::Int(1)),
        ])])
    );
    assert_eq!(
        parse("text: |\n  a\n    \n  b\n"),
        map(vec![("text", text("a\n\nb"))])
    );
    assert_eq!(parse("|\n"), text(""));
    assert_eq!(parse("text: |\n"), map(vec![("text", text(""))]));
    assert_eq!(parse("- |\n"), seq(vec![text("")]));
}

#[test]
fn block_text_handles_blank_lines_and_trailing_whitespace() {
    assert_eq!(
        parse("\ntext: |\n\n\n  a   \n  b\t\n\nnext: 1\n"),
        map(vec![("text", text("a\nb")), ("next", Value::Int(1))])
    );
}

#[test]
fn parses_flow_sequences() {
    assert_eq!(
        parse("items: [a, \"b, c\", 3, true, null]\n"),
        map(vec![(
            "items",
            seq(vec![
                text("a"),
                text("b, c"),
                Value::Int(3),
                Value::Bool(true),
                Value::Null,
            ])
        )])
    );
    assert_eq!(parse("items: [ ]\n"), map(vec![("items", seq(vec![]))]));
    assert_eq!(
        parse("items: [a b]\n"),
        map(vec![("items", seq(vec![text("a b")]))])
    );
}

#[test]
fn rejects_bad_flow_sequences() {
    assert_error("items: [a,]\n", "empty flow-sequence element");
    assert_error("items: [a,,b]\n", "empty flow-sequence element");
    assert_error("items: [,a]\n", "empty flow-sequence element");
    assert_error("items: [a\n", "flow sequence");
    assert_error("items: [a b\n", "flow sequence");
    assert_error("items: [a # comment]\n", "space followed by '#'");
    assert_error("items: [[a]]\n", "nested flow collections");
    assert_error("items: [{a: b}]\n", "nested flow collections");
    assert_error("items: [a] extra\n", "unexpected character");
    assert_error("items: {a: b}\n", "only the empty flow mapping");
}

#[test]
fn parses_quoted_strings_and_escapes() {
    assert_eq!(
        parse("value: \"quote: \\\" backslash: \\\\ newline: \\n tab: \\t\"\n"),
        map(vec![(
            "value",
            text("quote: \" backslash: \\ newline: \n tab: \t")
        )])
    );
    assert_error("value: \"\\u0041\"\n", "Unicode and hex escapes");
    assert_error("value: \"\\x41\"\n", "Unicode and hex escapes");
    assert_error("value: \"\\q\"\n", "unsupported escape");
    assert_error("value: \"raw\ttab\"\n", "raw tabs");
    assert_error("value: \"unterminated\n", "unterminated");
    assert_error("value: 'single'\n", "single-quoted strings");
    assert_error("value: \"a\" \"b\"\n", "unexpected character");
}

#[test]
fn keys_are_always_strings() {
    let source = "\
10: ten
true: boolean-looking key
\"a b\": spaced
\"10\": duplicate-looking but same key
";
    assert_error(source, "duplicate mapping key");
    assert_eq!(
        parse("10: ten\ntrue: yes\n\"a b\": spaced\n"),
        map(vec![
            ("10", text("ten")),
            ("true", text("yes")),
            ("a b", text("spaced")),
        ])
    );
    assert_error("\"\": value\n", "must not be empty");
}

#[test]
fn rejects_missing_values() {
    assert_error("key:\n", "expected a child block");
    assert_error("key:   \n", "expected a child block");
    assert_error("-\n", "must have a value");
    assert_error("- \n", "must have a value");
    assert_error("-  value\n", "exactly one space");
    assert_error("-\tvalue\n", "tabs are not allowed");
}

#[test]
fn rejects_scalar_child_blocks() {
    assert_error(
        "key:\n  plain scalar on the next line\n",
        "must be a mapping or a sequence",
    );
}

#[test]
fn rejects_bad_indentation() {
    assert_error(
        "key:\n- value\n",
        "expected a child block indented 2 spaces",
    );
    assert_error("key:\n   value: 1\n", "unexpected indentation");
    assert_error("key:\n    value: 1\n", "unexpected indentation");
    assert_error("a: 1\n  b: 2\n", "unexpected indentation");
    assert_error("a:\n  b: 1\n   c: 2\n", "unexpected indentation");
    assert_error("- a\n  - b\n", "unexpected indentation");
    assert_error("a:\n  - b\n   - c\n", "unexpected indentation");
    assert_error(" key: value\n", "root node must start at column 1");
}

#[test]
fn rejects_tabs_outside_allowed_places() {
    assert_error("\tkey: value\n", "tabs are not allowed");
    assert_error("key:\tvalue\n", "tabs are not allowed");
    assert_error("key: \tvalue\n", "tabs are not allowed");
    assert_error("key: value\t\n", "tabs are not allowed");
    assert_error(
        "items: [a,\tb]\n",
        "tabs are not allowed inside a flow sequence",
    );
}

#[test]
fn rejects_tab_only_lines_that_are_not_block_text_blank_lines() {
    assert_error("text: |\n  a\n\t\n  b\n", "expected a mapping key");
    assert_error(
        "text: |\n  a\n \t\n  b\n",
        "block text content must be indented 2 spaces",
    );
}

#[test]
fn tabs_are_allowed_in_comments_and_block_text() {
    assert_eq!(
        parse("# comment\twith tab\nkey: value\n"),
        map(vec![("key", text("value"))])
    );
    assert_eq!(
        parse("text: |\n  a\tb\n"),
        map(vec![("text", text("a\tb"))])
    );
    assert_eq!(
        parse("text: |\n  \t\n  b\n"),
        map(vec![("text", text("b"))])
    );
}

#[test]
fn rejects_block_marker_variants() {
    assert_error("text: >\n  folded\n", "folded block text");
    assert_error("text: |-\n  text\n", "block text marker must be exactly");
    assert_error("text: |+\n  text\n", "block text marker must be exactly");
    assert_error("text: |2\n  text\n", "block text marker must be exactly");
    assert_error("text: |x\n", "block text marker must be exactly");
    assert_error("text: |#comment\n  x\n", "separated from '|'");
    assert_error("text: | #comment\n  x\n", "hash followed by a space");
}

#[test]
fn rejects_bad_plain_scalars() {
    assert_error("value: a: b\n", "colon in a plain scalar");
    assert_error("value: ends:\n", "colon in a plain scalar");
    assert_error("value: - item\n", "cannot start with '- '");
    assert_error("value: ? item\n", "plain scalar cannot start with");
    assert_error("value: : item\n", "plain scalar cannot start with");
    assert_error("value: # comment\n", "missing before the comment");
    assert_error("value: [unclosed\n", "flow sequence");
    assert_error("value: ]\n", "plain scalar cannot start with");
}

#[test]
fn rejects_empty_and_multiple_documents() {
    assert_error("", "must contain a root node");
    assert_error("\n\n   \n# only comments\n", "must contain a root node");
    assert_error("---\na: 1\n", "document markers");
    assert_error("...\n", "document markers");
    assert_error("a: 1\nb: 2\nc\n", "expected a mapping key");
    assert_error("a: 1\n  \nb: 2\nscalar\n", "expected a mapping key");
}

#[test]
fn rejects_duplicate_keys_including_quoted_forms() {
    assert_error("a: 1\na: 2\n", "duplicate mapping key");
    assert_error("10: a\n\"10\": b\n", "duplicate mapping key");
    assert_error("- a: 1\n  a: 2\n", "duplicate mapping key");
}

#[test]
fn parses_crlf_and_bom() {
    let source = "\u{feff}a: 1\r\nb: |\r\n  line\r\n";
    assert_eq!(
        parse(source),
        map(vec![("a", Value::Int(1)), ("b", text("line"))])
    );
    assert_error("a: 1\rb: 2\n", "bare carriage return");
    assert_error("a: \u{feff}1\n", "byte-order mark");
    assert_error("value: \u{fffe}\n", "noncharacter");
    assert_error("value: \"\u{ffff}\"\n", "noncharacter");
}

#[test]
fn canonical_output_round_trips_special_values() {
    let values = vec![
        text("true"),
        text("null"),
        text("01"),
        text("1e3"),
        text("a: b"),
        text("a # b"),
        text("- item"),
        text("---"),
        text("line one\nline two"),
        text("line with trailing space\nnext"),
        text("tab\there"),
        text("quote \" and backslash \\"),
        text("中文与 emoji 😀"),
        seq(vec![map(vec![("a", seq(vec![text("x\ny")]))])]),
        map(vec![]),
        seq(vec![]),
        Value::Null,
        Value::Bool(false),
        Value::Int(-9_007_199_254_740_991),
        Value::Float(1.0),
        Value::Float(-0.0),
        Value::Float(0.1),
    ];

    for value in values {
        let source = handlist::to_string_value(&value)
            .unwrap_or_else(|error| panic!("failed to serialize {value:?}: {error}"));
        let reparsed = parse(&source);
        assert_eq!(value, reparsed, "round-trip failed for:\n{source}");
        assert!(source.ends_with('\n'));
        assert_eq!(source, handlist::to_string_value(&reparsed).unwrap());
    }
}

#[test]
fn canonical_output_for_extreme_floats_is_plain_decimal() {
    let values = [
        1e300_f64,
        -1e300_f64,
        1e-300_f64,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::EPSILON,
    ];
    for float in values {
        let value = Value::Float(float);
        let text = handlist::to_string_value(&value).expect("float is serializable");
        assert!(
            !text.contains('e') && !text.contains('E'),
            "exponent notation leaked into {text}"
        );
        assert!(
            text.contains('.'),
            "float output must contain a decimal point: {text}"
        );
        assert_eq!(parse(&text), value, "round-trip failed for {text}");
    }
}

#[test]
fn canonical_output_is_stable_and_readable() {
    let value = map(vec![
        ("name", text("example")),
        ("description", text("first line\nsecond line")),
        ("items", seq(vec![text("one"), text("two")])),
        ("empty_map", map(vec![])),
        ("empty_seq", seq(vec![])),
        ("numbers", seq(vec![Value::Int(1), Value::Float(1.5)])),
        ("quoted", text("true")),
        ("matrix", seq(vec![seq(vec![text("a"), text("b")])])),
    ]);
    let expected = "\
name: example
description: |
  first line
  second line
items:
  - one
  - two
empty_map: {}
empty_seq: []
numbers:
  - 1
  - 1.5
quoted: \"true\"
matrix:
  - - a
    - b
";
    assert_eq!(handlist::to_string_value(&value).unwrap(), expected);
}

#[test]
fn serializer_rejects_unrepresentable_values() {
    let mut map_with_empty_key = Map::new();
    map_with_empty_key.insert(String::new(), Value::Null);
    assert!(handlist::to_string_value(&Value::Map(map_with_empty_key)).is_err());

    let mut map_with_newline_key = Map::new();
    map_with_newline_key.insert("a\nb".to_owned(), Value::Null);
    assert!(handlist::to_string_value(&Value::Map(map_with_newline_key)).is_err());

    for float in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(handlist::to_string_value(&Value::Float(float)).is_err());
    }

    assert!(handlist::to_string_value(&Value::Int(9_007_199_254_740_992)).is_err());
    assert!(handlist::to_string_value(&text("bell \u{7}")).is_err());
    assert!(handlist::to_string_value(&text("bom \u{feff}")).is_err());
    assert!(handlist::to_string_value(&text("noncharacter \u{fffe}")).is_err());
}

#[test]
fn error_locations_are_reported() {
    let error = handlist::parse("a: 1\nb: [x,,y]\n").unwrap_err();
    assert_eq!(error.line(), Some(2));
    assert!(error.column().is_some());
    assert!(error.message().contains("empty flow-sequence element"));

    let error = handlist::parse("key: # comment\n").unwrap_err();
    assert_eq!(error.line(), Some(1));
    assert_eq!(error.column(), Some(6));
}
