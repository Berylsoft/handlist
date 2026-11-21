# handlist

**English** | [简体中文](README.zh.md)

A data description language built for hand editing: a subset of YAML that keeps the readable block structure and removes every rule that depends on implicit behavior or is easy to get wrong. File extension: `.handlist`.

It was originally created to describe a website's directory and page configuration, and it also suits menus, indexes, documentation metadata, and other structured data that is written and maintained by hand.

- Specification: [`spec/handlist-spec.md`](spec/handlist-spec.md) (English, 1.0)
- Rust implementation: [`rust/`](rust/) (parser + comment-preserving formatter + JSON Schema validation + canonical serializer + serde + `handlist` check/fmt CLI)
- TypeScript implementation: [`typescript/`](typescript/) (zero runtime dependencies, ESM/CJS + types)
- Examples: [`examples/`](examples/)

---

## Design goals

1. **Structure is indentation only.** Every level is exactly two spaces. YAML's "indentless sequences" (a `-` at the same column as the key that owns it) and arbitrary indentation widths are forbidden.
2. **One way to write each thing.** Strings are plain or double-quoted. There are no single-quoted strings, folded `>` blocks, anchors, aliases, tags, or flow mappings.
3. **No implicit surprises.** `key:` is not null; `yes`, `1e3`, `0x10`, and `01` are strings; mapping keys are always strings; duplicate keys are errors.
4. **Multi-line text keeps its line breaks.** Use `|` for HTML or long prose; lines are preserved and there is no trailing newline.
5. **Machine-friendly.** The data model maps to JSON-like values; Rust has full serde support; canonical serialization round-trips reliably.
6. **A strict YAML 1.2 syntax subset.** Every valid handlist document can be read by a YAML parser, so YAML highlighting, editors, and validators remain usable. Value semantics follow handlist's rules.

## Syntax overview

```handlist
# Site index
site:
  name: Example
  enabled: true
  pages:
    - name: home
      title: 首页
      action: page
    - name: tools
      title: 工具
      action: page
      tags: [hardware, info]
  description: |
    First line
    Second line with <b>HTML</b>
```

### Structure

- Indentation is exactly 2 spaces; a child block is exactly 2 spaces deeper than its parent.
- Mappings: `key: value`. When the value is on the next line, that line must be a mapping or a sequence (`key:` with nothing after it is not null).
- Sequences: `- value`, with exactly one space after `-`. Compact mappings (`- key: value`) and nested sequences (`- - value`) are supported.
- Comments: full-line comments use `# ...`; inline comments use ` # comment` (with a space after `#`). A `#` followed directly by the end of the line is an empty comment. `C#` and URL fragments are ordinary content.
- Block text: `key: |` or `- |`; content is indented 2 spaces beyond the parent, line breaks are preserved, trailing whitespace and leading/trailing blank lines are removed, and there is no trailing newline.
- Inline arrays: `[a, b, c]`, single-line scalar elements only; empty arrays are `[]` and empty mappings are `{}`.
- Mapping keys are always strings. Bare keys are limited to `[A-Za-z0-9_][A-Za-z0-9_-]*`; other keys use double quotes. `10:` and `"10":` are equivalent.
- Scalar types: only `true`/`false`/`null`, decimal integers (no leading zeros, within the safe integer range), and decimals with a decimal point (no exponents). Everything else is a string.
- Documents: one document and one root node. YAML document markers (`---`/`...`), `%YAML`, anchors, aliases, tags, and multiple documents are not supported.

These constructs are rejected explicitly: `yes`/`no` as booleans, implicit null with `key:`, indentless sequences, `>` folded blocks, `|-`/`|+`, single-quoted strings, non-empty flow mappings, trailing commas, inline `#comment` without a space, tab indentation, duplicate keys, and multiple root nodes. The specification defines the complete rules.

## Repository layout

```text
.
├── spec/
│   └── handlist-spec.md          Specification (EBNF, semantics, canonical form, errors, YAML comparison)
├── rust/
│   ├── src/                      Parser, spans, formatter, JSON Schema, serializer, serde
│   │   └── bin/handlist.rs       check/fmt CLI
│   ├── tests/                    Syntax, spans, serde, formatter, schema, CLI, examples
│   └── examples/canonicalize.rs  Print/generate canonical output
├── typescript/
│   ├── src/                      Zero-dependency TypeScript implementation
│   ├── test/                     Syntax, serialization, examples, YAML-subset checks
│   └── dist/                     Build output (ESM/CJS/types)
└── examples/
    ├── homepage/                 8 .handlist examples converted from YAML
    └── canonical/                Corresponding canonical output (shared Rust/TS contract)
```

## Rust

```toml
[dependencies]
handlist = { path = "rust" }
```

```rust
use handlist::Value;

fn main() -> Result<(), handlist::Error> {
    let source = r#"
site:
  name: Example
  pages:
    - name: home
      title: 首页
"#;

    let value: Value = handlist::parse(source)?;
    println!("{}", handlist::to_string_value(&value)?);
    Ok(())
}
```

Main API:

- `handlist::parse(&str) -> Result<Value, Error>`: parse into an ordered `Value`.
- `handlist::parse_with_spans(&str) -> Result<(Value, SpanMap), Error>`: parse and return JSON Pointer spans (`Span { line, column }`). `SpanMap::get(pointer)` is the value's start; `SpanMap::key_span(pointer)` is the start of the mapping key that owns it. Pointers use RFC 6901 escaping.
- `handlist::to_string_value(&Value) -> Result<String, Error>`: canonical serialization, always available.
- `handlist::format(&str) -> Result<String, Error>` (feature `format`): canonical source formatting that preserves comments. Full-line comments are re-indented with their node; inline comments stay on the value line where possible.
- `handlist::format_with_config(&str, &FormatConfig)` and `FormatConfig::parse(&str)` (feature `format`): apply a `.fmt.handlist` formatting configuration. The optional `non-complex-one-line-seq` rule keeps source flow sequences whose elements are all non-complex on one line; `one-line-in-multi-line-text` keeps source `|` block text whose content is zero or one line in block form. Both are presentation-only and leave the value unchanged.
- `handlist::validate_json_schema(&str, &str) -> Result<SchemaValidation, Error>` (feature `schema`): convert the document to JSON and validate it against a schema. `SchemaValidation::{is_valid, errors}` and `SchemaError::{instance_path, schema_path, message}` are available. Supported drafts are draft-07 and 2020-12; internal `$ref`s are supported, external references are not resolved, and a schema without `$schema` defaults to 2020-12.
- `Value::{Null, Bool, Int, Float, String, Seq, Map}`; `Map` preserves insertion order.
- `Error::{message, span, line, column}`; locations are 1-based, and columns count Unicode scalar values.
- The `serde` feature is enabled by default:
  - `handlist::from_str::<T>(&str)`
  - `handlist::to_string(&T)`
  - `handlist::to_value(&T)`
  - `Value`/`Map` implement `Serialize`/`Deserialize` themselves.
- `from_str` parses with spans, so serde errors (wrong field type, missing or unknown field, bad sequence element, bad map key) carry the position of the field currently being decoded. Missing-field errors use the last consumed key, or the container/root when the mapping is empty.
- Numeric mapping keys also deserialize correctly from string keys; for example, `BTreeMap<u8, String>` can read `10: ...`.
- The default feature is only `serde`. `format` enables `format`, `format_with_config`, and `FormatConfig`; `schema` enables `validate_json_schema`; `cli` enables the `handlist` binary and implies `schema` and `format`. `parse`, `parse_with_spans`, `Value`, and `to_string_value` are available without optional features. For a parser-only dependency use `default-features = false` (or keep `serde` for typed deserialization).

### CLI

The crate builds the `handlist` binary with the `cli` feature. Run it from `rust/`:

```bash
cargo run --features cli --bin handlist -- check ../examples
cargo run --features cli --bin handlist -- check config.handlist --schema schema.json
cargo run --features cli --bin handlist -- fmt --check config.handlist
cargo run --features cli --bin handlist -- fmt config.handlist
```

Install the CLI with `cargo install --path rust --features cli`.

`fmt` looks for `.fmt.handlist` in the current working directory, or uses `--config PATH`. The optional configuration is itself a handlist document:

```handlist
version: 1
allow:
  - non-complex-one-line-seq
  - one-line-in-multi-line-text
```

The `non-complex-one-line-seq` rule only preserves lists that were already written as one-line flow sequences with scalar elements; block sequences are not collapsed. `one-line-in-multi-line-text` only preserves source `|` blocks with zero or one content line; inline strings are not expanded. The configuration is strict: `version` must be `1` (and `allow` may be omitted), unknown fields/options and duplicate options are errors.

- `check` parses files and directories. Explicit files are checked regardless of extension; directories are searched recursively for names ending in `.handlist`. With `--schema`, the JSON projection is validated against a draft-07 or 2020-12 JSON Schema. Without `--schema`, only syntax is checked.
- `fmt` writes canonical formatting in place. `--check` reports differences without writing and exits `1` when changes are needed. Comments are preserved: full-line comments are re-indented with their node, inline comments stay on the value line where possible, and a comment on a flow sequence that expands to a block sequence becomes a standalone comment after the value. Blank lines adjacent to comments and between top-level nodes are normalized to at most one.
- Both commands use `.` when no path is given, skip `.git`, `target`, and `node_modules` by default, and accept `-a`/`--all` to include them. Directory symlinks are not followed. Diagnostics use `path:line:column: error: ...`; failures exit `1`.
- Arguments are parsed with [`argh`](https://crates.io/crates/argh).

Tests:

```bash
cd rust
cargo test --all-features
```

## TypeScript

```bash
cd typescript
npm install
npm run build
```

```ts
import {
  parse,
  parseOrdered,
  stringify,
} from "handlist";

const config = parse(source);            // mappings are plain objects, ready for JSON
const ordered = parseOrdered(source);    // mappings are Maps, source order preserved

console.log(stringify(config));          // canonical handlist text
```

- Zero runtime dependencies, Node.js >= 18; ESM, CJS, and `.d.ts` output.
- `parse()` returns JSON-friendly plain objects; `parseOrdered()` returns `Map`s whose key order (including numeric-looking keys such as `10`, `2`, and `1`) follows the source exactly.
- `stringify()` accepts plain objects, `Map`s, arrays, and scalars, and emits canonical text.
- Parse errors throw `HandlistError` with `reason`, `line`, and `column`.

**Two JavaScript host-language limitations:**

1. Plain objects order numeric-looking keys numerically in JavaScript (`"10"`, `"2"`, `"1"` become `"1"`, `"2"`, `"10"`). Use `parseOrdered()` (which returns `Map`) or the Rust implementation when exact order matters.
2. JavaScript has one `number` type and cannot distinguish integers from floats. `parseOrdered()` returns the number `1` for both `1.0` and `1`; `stringify()` emits safe-range integral values in integer form and larger integral values in float form. Use the Rust implementation when exact int/float distinction matters.

Tests:

```bash
cd typescript
npm test
```

The tests use the [`yaml`](https://www.npmjs.com/package/yaml) package to verify that every `.handlist` example and canonical fixture is also valid YAML 1.2.

## Examples

The eight files in [`examples/homepage/`](examples/homepage/) come from the YAML configuration in [`ldtstore-homepage/page`](https://github.com/stackinspector/ldtstore-homepage):

| handlist | Original YAML |
|---|---|
| `home/major.handlist` | `page/home/major.yml` |
| `home/sides.handlist` | `page/home/sides.yml` |
| `legacy/buttons.handlist` | `page/legacy/buttons.yml` |
| `public/sides.handlist` | `page/public/sides.yml` |
| `tool/major.handlist` | `page/tool/major.yml` |
| `tool/category.handlist` | `page/tool/category.yml` |
| `tool/sides.handlist` | `page/tool/sides.yml` |
| `tool/tools.handlist` | `page/tool/tools.yml` |

The rewrite made these adjustments:

- Folded blocks `>` became literal `|` blocks (HTML rendering is unaffected, but line breaks are preserved).
- Indentless sequences gained the required 2 spaces of indentation.
- A few YAML "scalar on the next line" cases (a value after `burning:`) became same-line scalars, because a next-line value in handlist must be a mapping or sequence.
- Quoted numeric keys such as `"10":` became bare keys such as `10:` (still equivalent to the string key `"10"`).
- Comments are kept in the examples; canonical fixtures do not contain comments.

[`examples/canonical/`](examples/canonical/) contains canonical serialization output. Both the Rust and TypeScript tests require the canonical text for each example to match its fixture byte for byte, and both implementations share the same fixtures. Regenerate them with:

```bash
cd rust
cargo run --example canonicalize
```

## Specification and trade-offs

The complete specification is [`spec/handlist-spec.md`](spec/handlist-spec.md). It covers:

- file format, encoding, indentation, and comment rules;
- the full EBNF and node semantics;
- type resolution, the safe integer range, and empty-value/empty-collection rules;
- canonical serialization rules;
- error-reporting conventions;
- a feature comparison with YAML and the reasons for each removal.

Key trade-offs:

- **Strict YAML 1.2 subset.** Removed syntax includes anchors/aliases/merge keys, tags, `>` folded blocks, chomping indicators, single-quoted strings, multi-line plain/quoted scalars, non-empty flow mappings, indentless sequences, arbitrary indentation, implicit null, duplicate keys, YAML 1.1 booleans, hexadecimal/octal/exponent/date/sexagesimal forms, and more.
- **Keys are always strings.** This avoids `10:`, `true:`, and `no:` becoming different types under different schemas; mapping order must be preserved.
- **Unique mapping keys.** Duplicate keys are errors, never last-wins.
- **Fixed block-text semantics.** Only `|` is allowed; `>` and `|-`/`|+` are not. Content is indented exactly two spaces beyond its container and the value has no trailing newline.
- **No Unicode or hex escapes.** Only `\"`, `\\`, `\n`, and `\t` are supported. This keeps the escape alphabet tiny, lets non-ASCII text appear directly as UTF-8, produces stable readable diffs, and avoids the ambiguity between Rust-style `\u{...}` and YAML's `\uXXXX`. The cost is that other control characters cannot be represented in strings: serializers report an error instead of emitting unreadable escapes. The rationale is also recorded in Section 9.4 of the specification.

## Test coverage

- Rust (`--all-features`): 29 parser/serializer tests, 3 span tests, 17 serde tests, 12 CLI tests, 12 formatter tests, 8 JSON Schema tests, 1 example + canonical fixture test, and 1 doc test.
- TypeScript: 26 parser/serializer/example/YAML-subset tests, covering ordered Maps, `__proto__` key protection, cycles, number formatting, and error positions.
- The canonical output for all eight examples is byte-identical between the two implementations.

## Status

handlist 1.0 is stable. The extension is `.handlist`; a document has one root node. Future versions should preserve these invariants: existing valid documents remain valid, existing values remain unchanged, canonical output stays readable and round-trips, and the accepted language remains a YAML 1.2 syntax subset.
