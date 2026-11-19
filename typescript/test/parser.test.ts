import assert from "node:assert/strict";
import test from "node:test";

import {
  HandlistError,
  parse,
  parseOrdered,
  parseWithMaps,
  stringify,
  toObject,
  type OrderedHandlistValue,
} from "../src/index.js";

function expectError(source: string, needle: string): HandlistError {
  try {
    parse(source);
  } catch (error) {
    assert.ok(error instanceof HandlistError, `expected HandlistError, got ${error}`);
    assert.ok(
      error.message.includes(needle),
      `expected error containing ${JSON.stringify(needle)}, got ${error.message}\n--- source ---\n${source}`,
    );
    assert.notEqual(error.line, null, "parse errors must have a line");
    assert.notEqual(error.column, null, "parse errors must have a column");
    return error;
  }
  assert.fail(`expected an error for:\n${source}`);
}

function orderedMap(value: OrderedHandlistValue): Map<string, OrderedHandlistValue> {
  assert.ok(value instanceof Map, "expected a Map");
  return value;
}

test("parses basic mappings and minimal types", () => {
  assert.deepEqual(parse("name: example\nenabled: true\ncount: 3\nratio: 1.5\nnothing: null\n"), {
    name: "example",
    enabled: true,
    count: 3,
    ratio: 1.5,
    nothing: null,
  });

  for (const source of [
    "yes",
    "no",
    "on",
    "off",
    "True",
    "FALSE",
    "~",
    "Null",
    "01",
    "+1",
    "0x10",
    "1e3",
    ".5",
    "1.",
    "1:30",
    "2024-01-01",
    "NaN",
  ]) {
    assert.equal(parse(source), source, `expected a string for ${source}`);
  }
  assert.equal(parse("---foo"), "---foo");
  assert.equal(parse("...foo"), "...foo");
});

test("preserves mapping order with parseOrdered", () => {
  const value = parseOrdered("10: ten\n2: two\n1: one\n");
  assert.deepEqual([...orderedMap(value).keys()], ["10", "2", "1"]);
  assert.equal(parseWithMaps, parseOrdered);

  const object = parse("10: ten\n2: two\n1: one\n") as Record<string, string>;
  assert.equal(object["10"], "ten");
  assert.equal(object["2"], "two");
  assert.equal(object["1"], "one");
});

test("parses nested, compact, and nested sequence forms", () => {
  const source = `root:
  - name: first
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
`;
  assert.deepEqual(parse(source), {
    root: [
      { name: "first", values: [1, 2] },
      { name: "second", nested: { key: "value" } },
      ["a", "b"],
      [{ name: "c" }, { name: "d" }],
    ],
  });

  assert.deepEqual(parse("- name:\n    list:\n      - one\n      - two\n  title: after\n"), [
    { name: { list: ["one", "two"] }, title: "after" },
  ]);
});

test("parses block text with fixed semantics", () => {
  assert.deepEqual(
    parse("text: |\n  first\n  second\n\n  fourth\nnext: 1\n"),
    { text: "first\nsecond\n\nfourth", next: 1 },
  );
  assert.deepEqual(parse("text: |\n  a\n    b\n"), { text: "a\n  b" });
  assert.deepEqual(parse("text: |\nnext: 1\n"), { text: "", next: 1 });
  assert.deepEqual(parse("- |\n  item text\n- second\n"), ["item text", "second"]);
  assert.deepEqual(
    parse("text: | # note\n  content\n"),
    { text: "content" },
  );
  assert.equal(parse("|\n"), "");
  assert.deepEqual(parse("text: |\n"), { text: "" });
  assert.deepEqual(parse("- |\n"), [""]);
});

test("parses flow arrays and empty collections", () => {
  assert.deepEqual(parse("items: [a, \"b, c\", 3, true, null]\n"), {
    items: ["a", "b, c", 3, true, null],
  });
  assert.deepEqual(parse("items: [ ]\n"), { items: [] });
  assert.deepEqual(parse("a: {}\nb: []\n"), { a: {}, b: [] });
});

test("parses comments with the strict hash-space rule", () => {
  assert.deepEqual(
    parse(`# leading comment
name: value # trailing comment
url: https://example.com/#fragment
csharp: C#
empty: [] # empty list
`),
    {
      name: "value",
      url: "https://example.com/#fragment",
      csharp: "C#",
      empty: [],
    },
  );
  assert.deepEqual(parse("name: value #\n"), { name: "value" });
  assert.deepEqual(parse("text: | #\n  content\n"), { text: "content" });
  expectError("name: value #comment\n", "hash followed by a space");
  expectError("name: # comment\n", "missing before the comment");
});

test("keys are always strings", () => {
  assert.deepEqual(parse("10: ten\ntrue: yes\n\"a b\": spaced\n"), {
    "10": "ten",
    true: "yes",
    "a b": "spaced",
  });
  expectError("10: a\n\"10\": b\n", "duplicate mapping key");
  expectError("\"\": value\n", "must not be empty");
});

test("rejects missing values and scalar child blocks", () => {
  expectError("key:\n", "expected a child block");
  expectError("-\n", "must have a value");
  expectError("-  value\n", "exactly one space");
  expectError("key:\n  plain scalar\n", "must be a mapping or a sequence");
});

test("rejects bad indentation and indentless sequences", () => {
  expectError("key:\n- value\n", "expected a child block indented 2 spaces");
  expectError("key:\n   value: 1\n", "unexpected indentation");
  expectError("a: 1\n  b: 2\n", "unexpected indentation");
  expectError(" key: value\n", "root node must start at column 1");
});

test("rejects tabs outside comments and block text", () => {
  expectError("\tkey: value\n", "tabs are not allowed");
  expectError("key: \tvalue\n", "tabs are not allowed");
  expectError("items: [a,\tb]\n", "tabs are not allowed inside a flow sequence");
  expectError("value: \"raw\ttab\"\n", "raw tabs");
});

test("rejects tab-only lines with insufficient block indentation", () => {
  expectError("text: |\n  a\n\t\n  b\n", "expected a mapping key");
  expectError(
    "text: |\n  a\n \t\n  b\n",
    "block text content must be indented 2 spaces",
  );
});

test("rejects block marker variants and escapes", () => {
  expectError("text: >\n  folded\n", "folded block text");
  expectError("text: |-\n  text\n", "block text marker must be exactly");
  expectError("text: |2\n  text\n", "block text marker must be exactly");
  expectError("value: \"\\u0041\"\n", "Unicode and hex escapes");
  expectError("value: \"\\q\"\n", "unsupported escape");
  expectError("value: 'single'\n", "single-quoted strings");
});

test("rejects bad flow syntax", () => {
  expectError("items: [a,]\n", "empty flow-sequence element");
  expectError("items: [a,,b]\n", "empty flow-sequence element");
  expectError("items: [,a]\n", "empty flow-sequence element");
  expectError("items: [a\n", "flow sequence");
  expectError("items: [a # comment]\n", "space followed by '#'");
  expectError("items: [[a]]\n", "nested flow collections");
  expectError("items: {a: b}\n", "only the empty flow mapping");
});

test("rejects noncharacters", () => {
  expectError("value: \uFFFE\n", "noncharacter");
  expectError('value: "\uFFFF"\n', "noncharacter");
});

test("rejects empty documents, markers, and trailing content", () => {
  expectError("", "must contain a root node");
  expectError("\n# comments only\n", "must contain a root node");
  expectError("---\na: 1\n", "document markers");
  expectError("a: 1\nscalar\n", "expected a mapping key");
});

test("reports line and column information", () => {
  const error = expectError("a: 1\nb: [x,,y]\n", "empty flow-sequence element");
  assert.equal(error.line, 2);
  assert.equal(typeof error.column, "number");

  const keyError = expectError("key: # comment\n", "missing before the comment");
  assert.equal(keyError.line, 1);
  assert.equal(keyError.column, 6);
});

test("round-trips special values through canonical output", () => {
  const values: OrderedHandlistValue[] = [
    "true",
    "null",
    "01",
    "1e3",
    "a: b",
    "a # b",
    "- item",
    "---",
    "line one\nline two",
    "line with trailing space\nnext",
    "tab\there",
    'quote " and backslash \\',
    "中文与 emoji 😀",
    [new Map([["a", ["x\ny"]]])],
    new Map(),
    [],
    null,
    false,
    -9007199254740991,
    1.0,
    0.1,
  ];

  for (const value of values) {
    const text = stringify(value);
    const reparsed = parseOrdered(text);
    assert.deepEqual(reparsed, value, `round-trip failed for:\n${text}`);
    assert.equal(stringify(reparsed), text, "canonical output is not idempotent");
    assert.ok(text.endsWith("\n"));
  }
});

test("canonical output is stable and readable", () => {
  const value = new Map<string, OrderedHandlistValue>([
    ["name", "example"],
    ["description", "first line\nsecond line"],
    ["items", ["one", "two"]],
    ["empty_map", new Map()],
    ["empty_seq", []],
    ["numbers", [1, 1.5]],
    ["quoted", "true"],
    ["matrix", [["a", "b"]]],
  ]);
  assert.equal(
    stringify(value),
    `name: example
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
quoted: "true"
matrix:
  - - a
    - b
`,
  );
});

test("formats numbers deterministically", () => {
  assert.equal(stringify(1), "1\n");
  assert.equal(stringify(1.5), "1.5\n");
  assert.equal(stringify(1e21), "1000000000000000000000.0\n");
  assert.equal(stringify(1e-7), "0.0000001\n");
  assert.equal(stringify(-0), "0\n");
});

test("serializer rejects unrepresentable values", () => {
  assert.throws(() => stringify(Number.NaN), HandlistError);
  assert.throws(() => stringify(Number.POSITIVE_INFINITY), HandlistError);
  assert.throws(() => stringify(new Map([["", 1]])), HandlistError);
  assert.throws(() => stringify(new Map([["a\nb", 1]])), HandlistError);
  assert.throws(() => stringify("bell \u0007"), HandlistError);
  assert.throws(() => stringify("bom \uFEFF"), HandlistError);
  assert.throws(() => stringify("noncharacter \uFFFE"), HandlistError);

  const cyclic = new Map<string, OrderedHandlistValue>();
  cyclic.set("self", cyclic);
  assert.throws(() => stringify(cyclic), /cyclic/);

  const shared = new Map<string, OrderedHandlistValue>([["x", 1]]);
  assert.equal(stringify([shared, shared]), "- x: 1\n- x: 1\n");
});

test("accepts plain objects and Maps when serializing", () => {
  assert.equal(stringify({ name: "x", items: ["a", "b"] }), "name: x\nitems:\n  - a\n  - b\n");
  assert.equal(
    stringify(new Map<string, OrderedHandlistValue>([["name", "x"]])),
    "name: x\n",
  );
});

test("protects against __proto__ keys when converting to objects", () => {
  const object = parse("__proto__: polluted\nconstructor: value\n") as Record<string, unknown>;
  assert.equal(Object.getPrototypeOf(object), Object.prototype);
  assert.equal(Object.prototype.hasOwnProperty.call(object, "__proto__"), true);
  assert.equal(object["__proto__"], "polluted");
  assert.equal(object["constructor"], "value");
});

test("toObject converts ordered maps recursively", () => {
  const ordered = parseOrdered("a:\n  - b\n  - c\n");
  assert.deepEqual(toObject(ordered), { a: ["b", "c"] });
});
