use handlist::{Map, Value};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    name: String,
    enabled: bool,
    count: u8,
    ratio: f64,
    #[serde(default)]
    optional: Option<String>,
    tags: Vec<String>,
    action: Action,
    pages: Vec<Page>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Page {
    name: String,
    title: String,
    #[serde(default)]
    keywords: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Action {
    Page,
    None,
    Custom(String),
}

#[test]
fn deserializes_structs() {
    let source = "\
name: example
enabled: true
count: 3
ratio: 1.5
optional: null
tags: [a, b]
action: page
pages:
  - name: home
    title: 首页
    keywords: [site, index]
";
    let config: Config = handlist::from_str(source).unwrap();
    assert_eq!(
        config,
        Config {
            name: "example".to_owned(),
            enabled: true,
            count: 3,
            ratio: 1.5,
            optional: None,
            tags: vec!["a".to_owned(), "b".to_owned()],
            action: Action::Page,
            pages: vec![Page {
                name: "home".to_owned(),
                title: "首页".to_owned(),
                keywords: vec!["site".to_owned(), "index".to_owned()],
            }],
        }
    );
}

#[test]
fn missing_optional_fields_are_allowed() {
    let source = "\
name: example
enabled: false
count: 0
ratio: 0.0
tags: []
action: none
pages: []
";
    let config: Config = handlist::from_str(source).unwrap();
    assert_eq!(config.optional, None);
    assert_eq!(config.action, Action::None);
}

#[test]
fn serializes_structs_to_canonical_handlist() {
    let config = Config {
        name: "example".to_owned(),
        enabled: true,
        count: 3,
        ratio: 1.5,
        optional: Some("present".to_owned()),
        tags: vec!["a".to_owned(), "b".to_owned()],
        action: Action::Custom("special".to_owned()),
        pages: vec![Page {
            name: "home".to_owned(),
            title: "首页".to_owned(),
            keywords: vec![],
        }],
    };

    let text = handlist::to_string(&config).unwrap();
    let expected = "\
name: example
enabled: true
count: 3
ratio: 1.5
optional: present
tags:
  - a
  - b
action:
  custom: special
pages:
  - name: home
    title: 首页
    keywords: []
";
    assert_eq!(text, expected);

    let reparsed: Config = handlist::from_str(&text).unwrap();
    assert_eq!(reparsed, config);
}

#[test]
fn converts_between_serde_values_and_handlist_values() {
    let source = "a: 1\nb: [x, true]\n";
    let value: Value = handlist::from_str(source).unwrap();
    assert_eq!(
        value,
        Value::Map(
            vec![
                ("a".to_owned(), Value::Int(1)),
                (
                    "b".to_owned(),
                    Value::Seq(vec![Value::String("x".to_owned()), Value::Bool(true)])
                ),
            ]
            .into_iter()
            .collect()
        )
    );

    let rendered = handlist::to_string(&value).unwrap();
    assert_eq!(rendered, "a: 1\nb:\n  - x\n  - true\n");

    let converted = handlist::to_value(&value).unwrap();
    assert_eq!(converted, value);
}

#[test]
fn supports_integer_mapping_keys_from_strings() {
    let source = "\"1\": first\n\"2\": second\n";
    let map: BTreeMap<u8, String> = handlist::from_str(source).unwrap();
    assert_eq!(map.get(&1).map(String::as_str), Some("first"));
    assert_eq!(map.get(&2).map(String::as_str), Some("second"));

    let text = handlist::to_string(&map).unwrap();
    assert_eq!(text, "1: first\n2: second\n");
}

#[test]
fn supports_all_externally_tagged_enum_variants() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum Example {
        Unit,
        Newtype(u32),
        Tuple(u32, String),
        Struct { flag: bool },
    }

    assert_eq!(handlist::to_string(&Example::Unit).unwrap(), "Unit\n");
    assert_eq!(
        handlist::to_string(&Example::Newtype(5)).unwrap(),
        "Newtype: 5\n"
    );
    assert_eq!(
        handlist::to_string(&Example::Tuple(5, "x".to_owned())).unwrap(),
        "Tuple:\n  - 5\n  - x\n"
    );
    assert_eq!(
        handlist::to_string(&Example::Struct { flag: true }).unwrap(),
        "Struct:\n  flag: true\n"
    );

    let variants = [
        ("Unit\n", Example::Unit),
        ("Newtype: 5\n", Example::Newtype(5)),
        ("Tuple:\n  - 5\n  - x\n", Example::Tuple(5, "x".to_owned())),
        ("Struct:\n  flag: true\n", Example::Struct { flag: true }),
    ];
    for (source, expected) in variants {
        let parsed: Example = handlist::from_str(source).unwrap();
        assert_eq!(parsed, expected);
    }
}

#[test]
fn serializes_value_through_serde() {
    let value = Value::Map(
        vec![
            ("name".to_owned(), Value::String("x".to_owned())),
            (
                "nested".to_owned(),
                Value::Seq(vec![Value::Int(1), Value::Float(1.5)]),
            ),
        ]
        .into_iter()
        .collect(),
    );
    let text = handlist::to_string(&value).unwrap();
    assert_eq!(text, "name: x\nnested:\n  - 1\n  - 1.5\n");
    let parsed: Value = handlist::from_str(&text).unwrap();
    assert_eq!(parsed, value);
}

#[test]
fn rejects_duplicate_keys_when_serializing() {
    struct DuplicateMap;

    impl Serialize for DuplicateMap {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("a", &1)?;
            map.serialize_entry("a", &2)?;
            map.end()
        }
    }

    assert!(handlist::to_string(&DuplicateMap).is_err());
}

#[test]
fn rejects_unsupported_serde_data() {
    struct Bytes;

    impl Serialize for Bytes {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_bytes(&[1, 2, 3])
        }
    }

    assert!(handlist::to_string(&Bytes).is_err());

    struct NotFinite;

    impl Serialize for NotFinite {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_f64(f64::NAN)
        }
    }

    assert!(handlist::to_string(&NotFinite).is_err());
}

#[test]
fn serializes_an_empty_map_value() {
    let mut map = Map::new();
    map.insert("empty".to_owned(), Value::Map(Map::new()));
    assert_eq!(
        handlist::to_string(&Value::Map(map)).unwrap(),
        "empty: {}\n"
    );
}
