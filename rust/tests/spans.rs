use handlist::{Span, SpanMap, Value};

fn span(line: usize, column: usize) -> Span {
    Span::new(line, column)
}

fn value(pointer: &str, expected: Span, spans: &SpanMap) {
    assert_eq!(
        spans.get(pointer),
        Some(expected),
        "value span for {pointer:?}"
    );
}

fn key(pointer: &str, expected: Span, spans: &SpanMap) {
    assert_eq!(
        spans.key_span(pointer),
        Some(expected),
        "key span for {pointer:?}"
    );
}

#[test]
fn maps_values_and_keys_to_spans() {
    let source = "\
# comment
site:
  name: Example
  pages:
    - name: home
      title: Home
    - name: tools
      title: Tools
";
    let (parsed, spans) = handlist::parse_with_spans(source).expect("valid document");

    assert_eq!(parsed, handlist::parse(source).expect("valid document"));
    value("", span(2, 1), &spans);
    assert_eq!(spans.key_span(""), None);

    value("/site", span(3, 3), &spans);
    key("/site", span(2, 1), &spans);
    value("/site/name", span(3, 9), &spans);
    key("/site/name", span(3, 3), &spans);
    value("/site/pages", span(5, 5), &spans);
    key("/site/pages", span(4, 3), &spans);

    value("/site/pages/0", span(5, 7), &spans);
    assert_eq!(spans.key_span("/site/pages/0"), None);
    value("/site/pages/0/name", span(5, 13), &spans);
    key("/site/pages/0/name", span(5, 7), &spans);
    value("/site/pages/0/title", span(6, 14), &spans);
    key("/site/pages/0/title", span(6, 7), &spans);

    value("/site/pages/1", span(7, 7), &spans);
    value("/site/pages/1/name", span(7, 13), &spans);
    key("/site/pages/1/name", span(7, 7), &spans);
    value("/site/pages/1/title", span(8, 14), &spans);
    key("/site/pages/1/title", span(8, 7), &spans);

    assert_eq!(spans.len(), 10);
    assert_eq!(spans.key_iter().count(), 7);
    assert_eq!(spans.get("/missing"), None);
}

#[test]
fn escapes_pointer_tokens_and_covers_flow_and_block_text() {
    let source = "\
\"a/b\": 1
\"a~b\": 2
items: [x, \"y\"]
text: |
  line
";
    let (parsed, spans) = handlist::parse_with_spans(source).expect("valid document");
    assert_eq!(parsed, handlist::parse(source).expect("valid document"));

    value("", span(1, 1), &spans);
    value("/a~1b", span(1, 8), &spans);
    key("/a~1b", span(1, 1), &spans);
    value("/a~0b", span(2, 8), &spans);
    key("/a~0b", span(2, 1), &spans);

    value("/items", span(3, 8), &spans);
    key("/items", span(3, 1), &spans);
    value("/items/0", span(3, 9), &spans);
    value("/items/1", span(3, 12), &spans);
    assert_eq!(spans.key_span("/items/0"), None);
    assert_eq!(spans.key_span("/items/1"), None);

    value("/text", span(4, 7), &spans);
    key("/text", span(4, 1), &spans);

    assert_eq!(spans.len(), 7);
    assert_eq!(spans.key_iter().count(), 4);
    assert_eq!(spans.get(""), Some(span(1, 1)));
}

#[test]
fn tracks_nested_sequences_and_compact_mappings() {
    let source = "\
- name: a
  tags: [x]
- - b
  - c
";
    let (parsed, spans) = handlist::parse_with_spans(source).expect("valid document");
    assert_eq!(parsed, handlist::parse(source).expect("valid document"));

    value("", span(1, 1), &spans);
    value("/0", span(1, 3), &spans);
    assert_eq!(spans.key_span("/0"), None);
    value("/0/name", span(1, 9), &spans);
    key("/0/name", span(1, 3), &spans);
    value("/0/tags", span(2, 9), &spans);
    key("/0/tags", span(2, 3), &spans);
    value("/0/tags/0", span(2, 10), &spans);

    value("/1", span(3, 3), &spans);
    value("/1/0", span(3, 5), &spans);
    value("/1/1", span(4, 5), &spans);
    assert_eq!(spans.key_span("/1"), None);
    assert_eq!(spans.key_span("/1/0"), None);
    assert_eq!(spans.key_span("/1/1"), None);

    // A root scalar and a root block text still have a root span.
    let (parsed, spans) = handlist::parse_with_spans("hello\n").expect("valid scalar");
    assert_eq!(parsed, Value::String("hello".to_owned()));
    assert_eq!(spans.get(""), Some(span(1, 1)));
    assert_eq!(spans.len(), 1);

    let (_, spans) = handlist::parse_with_spans("|\n  text\n").expect("valid block text");
    assert_eq!(spans.get(""), Some(span(1, 1)));
    assert_eq!(spans.len(), 1);
}
