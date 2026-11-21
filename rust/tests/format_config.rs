#![cfg(feature = "format")]

use handlist::FormatConfig;

fn config(source: &str) -> FormatConfig {
    FormatConfig::parse(source).expect("valid format config")
}

#[test]
fn parses_the_documented_format_config() {
    let config = config(
        "\
version: 1
allow:
  - non-complex-one-line-seq
  - one-line-in-multi-line-text
",
    );
    assert!(config.allow_non_complex_one_line_seq());
    assert!(config.allow_one_line_in_multi_line_text());

    let from_str = "version: 1\n"
        .parse::<FormatConfig>()
        .expect("valid config");
    assert!(!from_str.allow_non_complex_one_line_seq());
    assert!(!from_str.allow_one_line_in_multi_line_text());
}

#[test]
fn rejects_invalid_format_configs() {
    let cases = [
        ("allow: []\n", "version"),
        ("version: 2\n", "version 2"),
        ("version: \"1\"\n", "integer 1"),
        ("version: 1\nextra: true\n", "unknown format config key"),
        (
            "version: 1\nallow:\n  - unknown-option\n",
            "unknown format config option",
        ),
        (
            "version: 1\nallow:\n  - non-complex-one-line-seq\n  - non-complex-one-line-seq\n",
            "duplicate format config option",
        ),
        ("version: 1\nallow: nope\n", "'allow' must be a sequence"),
        ("version: 1\nallow:\n  - 1\n", "entries must be strings"),
        ("1\n", "must be a mapping"),
    ];

    for (source, needle) in cases {
        let error = FormatConfig::parse(source).unwrap_err();
        assert!(
            error.message().contains(needle),
            "expected {needle:?} for {source:?}, got {error}"
        );
    }
}

#[test]
fn keeps_only_source_inline_non_complex_sequences() {
    let source = "\
tags: [a, b] # tags comment
block:
  - a
  - b
nested:
  - [x, y]
";
    let config = config("version: 1\nallow:\n  - non-complex-one-line-seq\n");

    let expected = "\
tags: [a, b] # tags comment
block:
  - a
  - b
nested:
  - [x, y]
";
    let formatted = handlist::format_with_config(source, &config).expect("valid source");
    assert_eq!(formatted, expected);
    assert_eq!(
        handlist::parse(&formatted).expect("valid formatted output"),
        handlist::parse(source).expect("valid source")
    );

    let default = handlist::format(source).expect("valid source");
    assert_eq!(
        default,
        "tags:\n  - a\n  - b\n  # tags comment\nblock:\n  - a\n  - b\nnested:\n  - - x\n    - y\n"
    );
}

#[test]
fn preserves_source_one_line_block_text() {
    let source = "\
text: | # text comment
  line
empty: |
inline: line
multi: |
  one
  two
";
    let config = config("version: 1\nallow:\n  - one-line-in-multi-line-text\n");

    let expected = "\
text: | # text comment
  line
empty: |
inline: line
multi: |
  one
  two
";
    assert_eq!(
        handlist::format_with_config(source, &config).expect("valid source"),
        expected
    );

    let default = handlist::format(source).expect("valid source");
    assert!(default.contains("text: line # text comment"));
    assert!(default.contains("empty: \"\""));
    assert!(default.contains("inline: line"));
    assert!(default.contains("multi: |\n  one\n  two"));
}

#[test]
fn configured_formatting_is_idempotent() {
    let source = "\
# top
inline_tags: [a, b] # inline
block:
  - x
  - y
text: |
  one line
";
    let config = config(
        "version: 1\nallow:\n  - non-complex-one-line-seq\n  - one-line-in-multi-line-text\n",
    );
    let once = handlist::format_with_config(source, &config).expect("valid source");
    let twice = handlist::format_with_config(&once, &config).expect("valid formatted source");
    assert_eq!(twice, once);
    assert_eq!(
        handlist::parse(&once).expect("valid formatted output"),
        handlist::parse(source).expect("valid source")
    );
}
