# handlist 1.0

**A data description language for large, human-edited configuration files.**

| | |
|---|---|
| Version | 1.0 |
| File extension | `.handlist` |
| Status | Stable |
| Based on | YAML 1.2 (syntactic subset) |
| Reference implementations | `rust/`, `typescript/` |

---

## 1. Introduction

### 1.1 Purpose

handlist is a data description language designed to be written and maintained by
people. It was originally created to describe the directory structure and page
configuration of a website, but it is suitable for any structured data that is
edited by hand: site indexes, catalogs, menus, documentation metadata, and
similar configuration.

Syntactically, every handlist document is also a valid YAML 1.2 document.
handlist is deliberately a **small, strict subset** of YAML: it keeps the parts
that are pleasant to read and edit, and removes every construct whose meaning
depends on subtle or invisible rules.

The name is written in lower case: *handlist*.

### 1.2 Design goals

1. **Obvious structure.** Indentation alone determines nesting, and the
   indentation step is always exactly two spaces. There are no "indentless"
   sequences and no alternative alignments.
2. **One way to write a thing.** Scalars are plain or double-quoted; there are
   no single-quoted strings, folded blocks, anchors, tags, or flow mappings.
3. **No implicit surprises.** `key:` is never an implicit null. `yes` is a
   string, not a boolean. `1e3` is a string, not a number. Mapping keys are
   always strings. Duplicate keys are errors.
4. **Human-readable text.** Long prose and HTML are written as literal block
   text on their own lines, with line breaks preserved.
5. **Machine-friendly.** The data model maps directly to JSON-like values and
   to Rust `serde`; a canonical serialization form round-trips exactly.
6. **A strict YAML subset.** Any accepted document can be read by a YAML 1.2
   parser. Users can keep using YAML tooling for syntax highlighting or
   validation, even though handlist's value semantics are narrower.

### 1.3 Non-goals

- handlist is not a YAML extension and does not aim to express every YAML
  document.
- It is not a markup language, a template language, or a programming language.
- It does not support anchors, aliases, merge keys, custom tags, binary data,
  multiple documents, or schema definition in the file itself.
- It does not preserve comments, blank lines, or formatting on a round-trip
  through the data model. A separate source-to-source formatter MAY preserve
  comments as trivia (see Section 7.1).
- It does not aim to be the most compact possible notation. Verbosity in
  exchange for clarity is acceptable.

### 1.4 Conformance terminology

The key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY**
are to be interpreted as described in RFC 2119.

A *conforming parser* accepts exactly the language described here and rejects
everything else. A *conforming serializer* produces only canonical-form output
(Section 7). An implementation that only reads or only writes is still
conforming in the direction it implements.

---

## 2. File format

### 2.1 Encoding

- A handlist document MUST be encoded as UTF-8.
- A single UTF-8 byte-order mark (U+FEFF) MAY appear at the very beginning of
  the file. It is not part of the content and MUST be ignored. U+FEFF anywhere
  else in the file is an error.
- The document is a sequence of Unicode characters. Unpaired surrogates and
  invalid UTF-8 sequences are not valid handlist.

### 2.2 Line breaks

- A line break is LF (U+000A) or CRLF (U+000D U+000A). CRLF is treated exactly
  like LF.
- A bare CR (U+000D) that is not immediately followed by LF is an error.
- A final line break at the end of the file is OPTIONAL.
- Implementations MUST report line numbers starting at 1 regardless of line
  ending style.

### 2.3 Character restrictions

- The noncharacters U+FFFE and U+FFFF are not permitted anywhere, including
  block text and comments.
- Raw control characters are not permitted in a document, with these
  exceptions:
  - TAB (U+0009) is permitted inside block text content and inside comment
    text.
  - LF and CRLF are line breaks.
- In particular, TAB MUST NOT be used for indentation, between tokens, inside
  keys, inside plain scalars, or as a raw character inside a double-quoted
  string. Use the `\t` escape in a double-quoted string to represent a tab.
- Raw control characters in the ranges U+0000–U+001F, U+007F–U+009F are not
  permitted anywhere else. Double-quoted strings can represent LF and TAB
  through escapes; no other control character can be represented in handlist.

### 2.4 Empty files

An empty file, a file containing only blank lines, and a file containing only
comments are all invalid: a document MUST contain exactly one root node.

---

## 3. Structure

### 3.1 Lines and indentation

handlist is line-oriented:

- The document contains one root node.
- Indentation is made of ASCII SPACE (U+0020) only.
- The indentation step is **exactly two spaces**. A child block is always
  indented exactly two spaces relative to its parent.
- The root node starts at column 0. Significant lines may not be indented at
  column 0 unless they belong to the root node.
- Blank lines and comment lines may have any number of leading spaces. They are
  ignored for structure. TAB is not indentation, even on a comment line.

The following rules are normative:

1. **Block mappings.** All keys of one mapping start at the same column `n`.
   The value of an entry is either on the same line, or a block starting at
   column `n + 2`.
2. **Block sequences.** All item markers (`-`) of one sequence start at the
   same column `n`. The content of an item starts at column `n + 2`.
3. **Compact mappings.** A mapping may begin on the same line as a sequence
   item (`- key: value`). Its keys start at the item content column
   (`n + 2`, where `n` is the sequence's marker column), and its child blocks
   start at `n + 4`.
4. **Nested sequences.** A sequence item may itself be a sequence
   (`- - value`). The nested sequence's markers start at the outer item content
   column (`n + 2`), and its items' content starts at `n + 4`.
5. **Block text.** The content of a `|` block starts at the column of the
   *container* plus two:
   - mapping entry at column `n` → content at `n + 2`;
   - sequence item at column `n` → content at `n + 2`;
   - compact mapping key at column `n + 2` → content at `n + 4`.
6. **No other structural indentation is valid.** A significant line whose
   indentation is greater than expected is an error. A significant line whose
   indentation is less than expected closes the current block and is handled by
   its parent. (Additional indentation inside block text is content, not
   structure.)

This strictness is the main structural difference from YAML. In particular,
YAML's *indentless sequences* are not accepted:

```handlist
# valid handlist
key:
  - one
  - two
```

```handlist
# INVALID handlist (indentless sequence); YAML accepts this
key:
- one
- two
```

### 3.2 Comments

There are two comment forms:

1. **Full-line comment.** A line whose first non-space character is `#`. The
   rest of the line is ignored. Any text, including tabs, may follow the `#`.
   Indentation before the `#` may consist of spaces only.

   ```handlist
   # TODO: replace this entry
   key: value
   ```

2. **Inline comment.** A comment after a complete node on the same line. It
   MUST be written as one or more spaces, then `#`, then **a space**, then the
   comment text. An empty comment may end immediately after the `#`:

   ```handlist
   title: 系统安装 # 工具
   ```

   A space after `#` is mandatory when comment text follows (`# 工具`, not
   `#工具`); `#` at the end of the line is an empty comment. A `#` that is
   neither at the start of a line nor preceded by at least one space is an
   ordinary character, not a comment. This allows values such as `C#` and URL
   fragments such as `https://example.com/page#section`.

   If a `#` is preceded by whitespace but is followed by a character other than
   a space (and is not at the end of the line), the document is invalid. For
   example, `title: C #语言` is an error; write `title: "C #语言"` instead. This
   rule exists so that a value can never be silently cut short by an accidental
   `#`.

- Inline comments are not permitted inside a flow sequence.
- Inside quoted strings and block text, `#` is ordinary content.
- A space after `#` is not required for full-line comments.
- Inside block text, every line is content. A line beginning with `#` there is
  not a comment.
- Comments are ignored by the data model and are not preserved by serializers
  that start from that model. A source-to-source formatter MAY retain them as
  formatting trivia (Section 7.1).

### 3.3 Node kinds

A handlist document contains one root node. A node is one of:

| Node | Example |
|---|---|
| Block mapping | `key: value` |
| Block sequence | `- value` |
| Plain scalar | `hello` |
| Double-quoted scalar | `"hello\n"` |
| Flow sequence | `[a, b, c]` |
| Empty mapping | `{}` |
| Block text | `\|` followed by lines |

The root node may be any of these. It is very common for the root to be a
mapping or a sequence.

---

## 4. Grammar

### 4.1 Notation

The grammar below uses EBNF:

- `x` — the literal terminal `x`
- `(` `)` — grouping
- `[ ]` — optional
- `{ }` — zero or more repetitions
- `|` — alternation
- `,` — concatenation

Terminals such as `SPACE`, `NEWLINE`, and `DASH` are defined lexically below.
Whitespace shown in a production is significant and exact. Indentation
constraints (Section 3.1) are normative and are not fully expressible in the
EBNF; the prose rules take precedence.

### 4.2 Lexical grammar

```ebnf
SPACE          = " " ;
TAB            = "\t" ;
NEWLINE        = "\n" | "\r\n" ;
DASH           = "-" ;
COLON          = ":" ;
HASH           = "#" ;

letter         = "A".."Z" | "a".."z" ;
digit          = "0".."9" ;
nonzero        = "1".."9" ;
key-start      = letter | digit | "_" ;
key-char       = letter | digit | "_" | "-" ;
bare-key       = key-start, { key-char } ;

blank-line     = { SPACE }, NEWLINE ;
comment-line   = { SPACE }, HASH, { character - NEWLINE }, NEWLINE ;
inline-comment = { SPACE }+, HASH, [ SPACE, { character - NEWLINE } ] ;
```

Column 0 is the first character of a line. Indentation is measured in SPACE
characters.

### 4.3 Document

```ebnf
document       = { blank-line | comment-line },
                 root-node,
                 { blank-line | comment-line },
                 EOF ;
```

A document MUST NOT contain a YAML document marker (`---` or `...` at the
start of a line at column 0), a directive (`%...`), or a second root node.

### 4.4 Block nodes

```ebnf
block-mapping  = mapping-entry, { mapping-entry } ;
mapping-entry  = key, COLON, entry-value ;
entry-value    = { SPACE }+, inline-node
               | { SPACE }+, block-text-header
               | { SPACE }*                       (* child block follows *)
               ;

block-sequence = sequence-item, { sequence-item } ;
sequence-item  = DASH, SPACE, item-value ;
item-value     = inline-node
               | mapping-entry                     (* compact mapping *)
               | DASH, SPACE, item-value           (* nested sequence *)
               | block-text-header
               ;

key            = bare-key | double-quoted-key ;
double-quoted-key = double-quoted-string ;         (* semantic restrictions apply *)
```

A mapping entry with no inline value (the `{ SPACE }*` alternative) MUST be
followed by a child block: a significant line at exactly `n + 2` columns, where
`n` is the key's column. The child block MUST be a block mapping or a block
sequence. A scalar on the following line is not a child block; write it on the
key's line instead:

```handlist
# valid
key: value
key2:
  nested: value

# INVALID: scalar child block
key3:
  value
```

A block sequence MUST NOT contain a bare `-` with nothing after it. Empty
sequences are written `[]`; empty mappings are written `{}`.

### 4.5 Inline nodes

```ebnf
inline-node    = plain-scalar
               | double-quoted-string
               | flow-sequence
               | empty-mapping ;
```

Inline nodes end at the end of the line or at an inline comment. Inline nodes
cannot span lines.

### 4.6 Flow sequences and empty mappings

```ebnf
flow-sequence  = "[", { SPACE },
                 [ flow-element, { { SPACE }, ",", { SPACE }, flow-element } ],
                 { SPACE }, "]" ;
flow-element   = flow-plain-scalar | double-quoted-string ;

empty-mapping  = "{", { SPACE }, "}" ;
```

- `[]` is the empty sequence; `{}` is the empty mapping. Spaces are allowed
  inside the brackets/braces (`[ ]`, `{ }`).
- A non-empty flow mapping is not supported; only `{}` is valid.
- A flow sequence MUST be written on one line.
- Flow sequences MUST NOT be nested.
- Elements are separated by commas. A trailing comma is invalid. Empty
  elements are invalid.
- No comments are allowed inside a flow sequence.

### 4.7 Double-quoted strings

```ebnf
double-quoted-string = '"', { quoted-char | escape }, '"' ;
quoted-char          = <printable character except '"' and "\"> ;
escape               = "\", ( '"' | "\" | "n" | "t" ) ;
```

Exactly four escape sequences are defined:

| Escape | Meaning |
|---|---|
| `\"` | quotation mark U+0022 |
| `\\` | reverse solidus U+005C |
| `\n` | line feed U+000A |
| `\t` | tab U+0009 |

- A double-quoted string MUST be written on one line.
- Raw tabs and raw control characters are not permitted inside a quoted string;
  use `\t` for a tab.
- Unknown escapes, including all Unicode escapes such as `\u0041` or
  `\u{41}`, are errors. See Section 9.4 for why handlist does not provide
  Unicode escapes.
- A quoted string directly followed by other content (except spaces and an
  inline comment) is an error. There is no concatenation of adjacent quoted
  strings.

### 4.8 Plain scalars

A plain scalar is any non-empty sequence of characters that is not one of the
other node kinds and that satisfies all of the following restrictions.

**Forbidden first character** (the scalar's first character MUST NOT be):

- SPACE or TAB (leading whitespace is structural, not content)
- `#` (would start a comment)
- `"` or `'` (quoted-string indicators; single quotes are not supported)
- `[`, `]`, `{`, `}`, `,` (flow/collection indicators)
- `&`, `*`, `!` (anchor, alias, tag indicators)
- `|`, `>` (block scalar indicators)
- `%`, `@`, `` ` `` (YAML reserved characters)
- `?` or `:` (always, in handlist, regardless of what follows)

**Forbidden forms**

- A plain scalar MUST NOT begin with `-` when the next character is a space or
  when the scalar is exactly `-`. (`-5` and `-foo` are valid.)
- A plain scalar MUST NOT contain `: ` (colon followed by space) and MUST NOT
  end with `:`. A colon not followed by a space is ordinary content
  (`http://…`, `17:29`).
- A plain scalar MUST NOT contain SPACE followed by `#`. Such a sequence starts
  an inline comment, or, if `#` is followed by a character other than a space
  and is not at the end of the line, makes the document invalid (Section 3.2).
- A plain scalar MUST NOT contain TAB or control characters.
- A plain scalar MUST NOT be empty. Use `""` for the empty string.

All other characters, including Unicode text, internal spaces, single quotes,
`#` not preceded by a space, and `:` not followed by a space, are permitted.

A plain scalar ends at the end of the line or at an inline comment. Leading and
trailing spaces are not part of the scalar.

Flow-sequence elements use the same plain-scalar rules, with these additional
restrictions: they MUST NOT contain `,`, `[`, `]`, `{`, or `}`; they MUST NOT
contain ` #`; and they MUST NOT be empty.

### 4.9 Scalars and type resolution

A plain scalar is typed according to the **first** matching row of this table:

| Form | Type |
|---|---|
| `null` | null |
| `true` | boolean |
| `false` | boolean |
| integer per Section 4.10 | integer |
| float per Section 4.10 | float |
| anything else | string |

Matching is exact and case-sensitive. In particular, all of these are
**strings**, not special values: `Null`, `NULL`, `~`, `yes`, `no`, `on`, `off`,
`True`, `FALSE`, `+1`, `01`, `0x10`, `0o17`, `1_000`, `1e3`, `.5`, `1.`,
`1:30`, `2024-01-01`, `.inf`, `.nan`, `NaN`, `Infinity`.

A double-quoted scalar is always a string. A flow element is typed by the same
table. Mapping keys are always strings and are never typed.

### 4.10 Numbers

```ebnf
integer = [ "-" ], ( "0" | nonzero, { digit } ) ;
float   = [ "-" ], ( "0" | nonzero, { digit } ), ".", digit, { digit } ;
```

- No leading `+`.
- No leading zero in a multi-digit integer (`01` is a string).
- No underscores.
- No exponents.
- No hexadecimal, octal, or binary notation.
- A float MUST contain a decimal point and at least one digit before and after
  it.
- Integers MUST be in the inclusive range `-9007199254740991` … `9007199254740991`
  (the interoperable JSON safe-integer range). An integer literal outside that
  range is an error, not a string.
- Floats are IEEE 754 binary64 values and MUST be finite. An overflowing float
  literal (for example a decimal with hundreds of integer digits) is an error.
- Negative zero is a valid float literal. `-0` is the integer zero.
- `NaN`, infinity, and negative infinity have no literal form.

### 4.11 Block text

```ebnf
block-text-header = "|", { SPACE }, [ inline-comment ] ;
```

The header is written on the same line as the mapping key or sequence item
marker, after the mandatory space:

```handlist
text: |
  第一行
  第二行 <b>HTML</b>
```

Rules:

1. The marker MUST be exactly `|`. The YAML folded marker `>`, chomping
   indicators (`|-`, `|+`), and indentation indicators (`|2`) are errors.
2. Content starts on the next line, indented two spaces beyond the container
   (Section 3.1). Those two spaces are stripped. Any additional indentation is
   content and is preserved.
3. Content ends at the first non-blank line indented at or below the container
   column, or at end of file. A non-blank line indented between the container
   and the required content column is an error.
4. Blank lines inside the block are content. A line consisting only of spaces
   is blank. Trailing spaces and tabs on every content line are removed.
5. Leading and trailing blank lines are not part of the value.
6. The value is the remaining lines joined with LF (`\n`). There is **no**
   trailing line break. An empty block is the empty string.
7. Lines inside block text may contain `#` and any other printable characters;
   they are content, not comments. Tabs are allowed.
8. Control characters other than TAB are not allowed in block text.

Example:

```handlist
description: |
  <p>第一行</p>
  <p>第二行</p>

  <p>第四行</p>
```

The value is:

```text
<p>第一行</p>
<p>第二行</p>

<p>第四行</p>
```

(no trailing newline).

### 4.12 Mapping keys

A key is either a bare key or a double-quoted key.

- A bare key MUST match `[A-Za-z0-9_][A-Za-z0-9_-]*`. It is always a string.
- A double-quoted key uses the double-quoted string syntax. It MUST be
  non-empty and MUST NOT contain a line break, any control character, or the
  noncharacters U+FFFE/U+FFFF.
- There MUST be no space between the key and its `:`.
- Keys are always strings. `10:` and `"10":` denote the same key, the string
  `"10"`. `true:` denotes the string `"true"`, not the boolean `true`.
- A mapping MUST NOT contain the same key twice. Duplicate keys are an error,
  regardless of the values.
- Mapping order is significant and MUST be preserved by parsers and
  serializers.

### 4.13 Complete document examples

```handlist
# A small complete document
title: Example
enabled: true
count: 3
ratio: 1.5
tags: [hardware, info, other-toolbox]
empty: []
mapping: {}

items:
  - name: first
    label: First item
    text: |
      Multiple lines.
      Everything after the marker is literal.
  - name: second
    list:
      - alpha
      - beta

grid:
  - - name: a
    - name: b
```

---

## 5. Data model

### 5.1 Values

A parsed handlist document is a tree of values:

| Kind | Contents |
|---|---|
| null | the single value `null` |
| boolean | `true` or `false` |
| integer | signed integer in the safe range |
| float | finite IEEE 754 binary64 |
| string | Unicode scalar values |
| sequence | ordered list of values |
| mapping | ordered list of string-keyed entries |

Implementations written in a host language that has only one numeric type MAY
expose a single numeric type, as long as parsing and serialization preserve the
numeric value. The Rust reference implementation exposes integers and floats
separately; JavaScript has only `number`, so the TypeScript implementation
serializes safe-range integral values in integer form and larger integral
values as floats.

### 5.2 Equality and identity

- Values compare structurally. Mapping comparison accounts for order; two
  mappings with the same keys in a different order are different values.
- There are no references, aliases, or shared substructures.
- There is no distinction between a string and a key; mapping keys are always
  strings.

### 5.3 Empty values

- There is no implicit null. A mapping key with neither a value nor a child
  block is an error.
- Null is written `null`.
- The empty string is written `""`.
- The empty sequence is written `[]`.
- The empty mapping is written `{}`.
- An empty block text (`key: |` followed immediately by a less-indented line or
  by end of file) denotes the empty string.

### 5.4 Mapping order

Source order MUST be preserved by a conforming parser, and serializers MUST emit
entries in the value's order. This makes generated configuration stable and
diff-friendly.

---

## 6. Parsing semantics

### 6.1 Mapping entries

- A mapping entry is `key:`, followed by either:
  - one or more spaces and an inline node (`key: value`), or
  - one or more spaces and a block-text header (`key: |`), or
  - nothing else on the line (spaces are ignored), in which case the entry's
    value is a child block at column `n + 2`.
- If a child block is required, the next significant line MUST be indented
  exactly `n + 2`, and MUST begin a block mapping or a block sequence. If it
  does not, the document is invalid.
- If a value is present on the key's line, it MUST be complete on that line.
  There is no continuation of inline values on following lines.
- An inline comment may follow a completed inline node or a block-text header.
  It may not be the only thing after `key:`.

### 6.2 Sequence items

- A sequence item is `-` followed by exactly one space and an item value.
- A bare `-` at the end of a line is invalid.
- The item value is one of:
  - an inline node (`- value`);
  - a compact mapping (`- key: value`), whose keys are at the item content
    column and whose child blocks are two columns deeper;
  - a nested sequence (`- - value`), whose markers are at the item content
    column;
  - a block-text header (`- |`).
- Compact mappings and nested sequences follow the same rules recursively.

### 6.3 Block text

The header line is consumed, then content lines are collected as described in
Section 4.11. Parsing resumes at the first line that did not belong to the
block.

### 6.4 Flow sequences

Elements are separated by commas, with optional spaces around them. The whole
sequence is on one line. Elements are inline nodes: plain scalars or
double-quoted strings. A comment marker inside a flow sequence is an error
because the sequence cannot be terminated before the end of the line.

### 6.5 End of file

After the root node, only blank lines and comment lines may follow. Any other
content is an error.

### 6.6 Root restrictions

- The root node starts at column 0.
- The root may be a mapping, a sequence, a scalar, a flow sequence, an empty
  mapping, or block text.
- A YAML document marker (`---` or `...`) followed by a space or the end of the
  line at column 0 is not accepted. A marker embedded in a scalar is ordinary
  content: `---foo` and `...foo` are valid root scalars.
- YAML directives (`%...`) are not accepted at column 0, and `%` cannot begin a
  plain scalar.
- Exactly one root node is allowed.

---

## 7. Canonical form

A conforming serializer emits a canonical form with the following properties.
Canonical output is deterministic and always parses back to an equal value.

### 7.1 General layout

- UTF-8, no BOM.
- LF line endings.
- Exactly one final LF at the end of the document.
- Two-space indentation, following Section 3.1.
- One space after `:` before an inline value; one space after `-`.
- Mappings and sequences are emitted in block style.
- Flow sequences are never emitted; use block sequences.
- Comments are never emitted by a serializer that starts from the data model.
  A source-to-source formatter MAY retain comments while emitting otherwise
  canonical data lines. It MAY re-indent full-line comments and normalize blank
  lines; comment text is not part of the data model.

### 7.2 Empty collections and null

- Empty mapping: `{}`.
- Empty sequence: `[]`.
- Null: `null`.
- Booleans: `true`, `false`.
- Empty string: `""`.

### 7.3 Keys

A key is emitted bare when it matches `[A-Za-z0-9_][A-Za-z0-9_-]*`; otherwise it
is emitted as a double-quoted string. Keys that are empty or contain line
breaks, control characters, U+FEFF, or noncharacters cause a serialization
error, since they cannot be represented by handlist syntax.

### 7.4 Scalars

A string is emitted as a plain scalar when all of the following hold:

- it is non-empty;
- it contains no LF, TAB, U+FEFF, noncharacters, or control characters;
- it has no leading or trailing space;
- it does not match any of the boolean, null, integer, or float forms in
  Section 4.9 (otherwise it would be parsed as a different type);
- it does not contain `: ` and does not end with `:`;
- it does not contain a SPACE followed by `#`;
- its first character is not one of the forbidden first characters in
  Section 4.8, it is not exactly `-`, and it does not begin with `- `, `?`, or
  `:`;
- it does not begin with `---` or `...` (kept simple so a root scalar is always
  unambiguous).

Otherwise the string is emitted as a double-quoted string, escaping `"`, `\`,
LF as `\n`, and TAB as `\t`. Strings containing any other raw control character,
U+FEFF, or a noncharacter cannot be represented and cause a serialization
error.

An integer is emitted as decimal digits (with `-` if negative). A float is
emitted as the shortest decimal representation that round-trips to the same
binary64 value, written without exponent notation and always containing a
decimal point. Non-finite floats and integers outside the safe range cause a
serialization error.

### 7.5 Multi-line strings

A string is emitted as block text (`|`) when all of the following hold:

- it contains at least one LF;
- it does not start or end with LF;
- it contains no CR, U+FEFF, noncharacters, or other control character except
  LF and TAB;
- no line has a trailing space or TAB (otherwise the value would change when
  read back);
- every line is otherwise representable.

Block text content is indented two spaces beyond the container. Empty lines in
the middle of the value are emitted as empty lines. All other multi-line strings
are emitted as double-quoted strings with `\n` escapes.

### 7.6 Canonical examples

```handlist
name: example
description: |
  first line
  second line
items:
  - one
  - two
matrix:
  - - a
    - b
empty_map: {}
empty_seq: []
quoted: "true"
```

---

## 8. Errors

- A parser MUST reject a document that is not in the language and MUST report
  an error with the line and column at which the problem was detected.
- Line and column numbers are 1-based. Columns count Unicode scalar values
  from the start of the line, not bytes and not UTF-16 code units.
- Implementations SHOULD stop at the first error.
- Error messages SHOULD identify the rule that was violated and SHOULD suggest
  a correction when one is obvious. Message text is not part of the format and
  is not required to match across implementations.
- A serializer MUST return an error when a value cannot be represented in
  canonical handlist (unrepresentable control characters, U+FEFF, or
  noncharacters, an empty mapping key, a key containing a line break,
  non-finite floats, out-of-range integers, invalid Unicode scalar values, or
  cyclic data structures in implementations that accept host-language values).

---

## 9. Relationship to YAML

### 9.1 Subset guarantee

Every document accepted by handlist is also **syntactically valid YAML 1.2**.
handlist narrows YAML's syntax; it does not add syntax. This makes it possible
to use YAML syntax highlighters, editors, and validators on `.handlist` files.

The guarantee is syntactic. A YAML parser may assign a different value to a
handlist document because handlist deliberately uses a narrower type system and
a fixed block-text convention. See Section 9.3.

### 9.2 Removed YAML features

| Removed feature | Why it is removed |
|---|---|
| Anchors, aliases, merge keys (`&`, `*`, `<<`) | Hidden shared state; merge semantics are surprising and hard to audit by hand. |
| Tags and custom types (`!!str`, `!foo`) | Implicit schema extension; handlist has one fixed, small type system. |
| Folded block text (`>`) | Folding rules (paragraphs, more-indented lines, trailing spaces) are the most error-prone part of YAML. |
| Chomping and indentation indicators (`|-`, `|+`, `|2`) | Invisible whitespace control; handlist fixes one block-text convention. |
| Single-quoted strings (`'…'`) | Two quoting systems for a single string type; only double quotes are kept. |
| Unicode and other escapes (`\uXXXX`, `\x41`, `\0`, …) | See Section 9.4. Only `\"`, `\\`, `\n`, and `\t` remain. |
| Multi-line plain and quoted scalars | Implicit line folding and continuation rules; handlist scalars are one line. |
| Non-empty flow mappings (`{a: b}`) | Poor readability for the structured records handlist targets. Only `{}` is kept. |
| Nested flow collections | Ambiguous and dense; handlist allows one flat, single-line list of scalars. |
| Indentless sequences | The list visually appears to be a sibling of the key that owns it. |
| Arbitrary indentation widths | Alignment can drift and still parse; handlist always uses two spaces. |
| Implicit null (`key:`) | A missing value is almost always a mistake; null must be explicit. |
| Duplicate keys | Silent last-wins behavior hides data loss; duplicates are errors. |
| YAML 1.1 booleans (`yes`, `no`, `on`, `off`) | The "Norway problem" (`NO` → false); only `true`/`false` are booleans. |
| Hex/octal/binary integers, exponents, leading zeros, dates, sexagesimal numbers | Multiple spellings for numbers, and values such as `1:30` or `1e3` change type between parsers. |
| Document markers, directives, multiple documents | A handlist file is exactly one configuration document. |
| Tags on keys and implicit key typing | Keys are always strings; key order is preserved. |

### 9.3 Semantic differences from YAML

Even where the syntax is a subset, some values differ from what a YAML parser
would produce:

1. **Mapping keys are always strings.** YAML may resolve `10:` as an integer
   key; handlist always uses `"10"`.
2. **Block text has no trailing newline.** YAML's default clip mode keeps one
   trailing line break. A handlist `|` value never ends with a line break.
3. **Leading and trailing blank lines in block text are dropped,** and trailing
   spaces/tabs on each line are removed.
4. **Type resolution is narrowed.** `1e3`, `01`, `0x10`, `2024-01-01`, and
   `yes` are strings in handlist even where a YAML schema would type them.
5. **An empty value after a key is an error,** not null.
6. **Inline comment after `#` requires a space or end of line.** When text
   follows, it must start with a space; YAML would accept ` #comment`; handlist
   rejects it to avoid silently cutting a value.
7. **Indentless sequences and arbitrary indentation are rejected,** as are
   `>` block scalars, anchors, tags, and flow mappings.

These differences are intentional. The syntax subset is kept for interoperability
with YAML tooling; the value model is handlist's own.

### 9.4 Why handlist has no Unicode escapes

handlist supports only four escapes: `\"`, `\\`, `\n`, and `\t`. It deliberately
does not support `\uXXXX`, `\UXXXXXXXX`, or Rust-style `\u{...}`. This was a
conscious choice:

- **The escape alphabet stays tiny and teachable.** Four escapes cover the
  characters that cannot be typed literally in a single-line string.
- **Human-edited text stays readable.** Non-ASCII text is written directly as
  UTF-8, so `名字: 说明` reads as itself instead of a sequence of hex code
  points. Diffs and reviews stay meaningful.
- **There is exactly one representation of a character.** Hex escapes create a
  second spelling of the same text and invite invisible differences and
  normalization debates.
- **It avoids a YAML trap.** Rust-style `\u{...}` is not YAML syntax; YAML uses
  `\uXXXX`. Supporting either inside a language that claims to be a strict YAML
  subset would be confusing. Supporting none keeps the subset claim simple.
- **The limitation is acceptable for the target use case.** handlist is for
  hand-written configuration and prose. Control characters other than LF and
  TAB are not meaningful there. If a host-language value contains such a
  character, serialization fails rather than emitting an unreadable escape.
- **The data model is still fully Unicode.** All printable Unicode scalar
  values can appear directly in quoted strings and block text.

The unavoidable consequence is that strings containing arbitrary control
characters cannot be represented in handlist. This is documented in
Section 7.4.

---

## 10. Security considerations

- handlist has no executable content, no tags, and no aliases, so there is no
  expansion attack and no code execution surface.
- Duplicate keys are rejected instead of taking a last-wins interpretation,
  which removes an ambiguity that has been used to hide configuration.
- Mapping keys are always strings and are never evaluated as expressions.
- Implementations MAY impose protective limits (maximum input size, maximum
  nesting depth) and MUST report an error rather than crashing or overflowing.
- Serializers MUST reject cyclic host-language structures instead of looping
  forever.

---

## 11. Versioning

This document specifies handlist 1.0. The file extension is `.handlist`.

Future versions may add syntax or value types, but should preserve the
following invariants:

- existing valid documents remain valid;
- existing values remain unchanged;
- canonical output remains readable and round-trips;
- the accepted language remains a syntactic subset of YAML 1.2.

A parser MAY expose the format version, but handlist files do not carry a
version marker.

---

## 12. Examples

Complete, real-world examples are in the [`examples/`](../examples) directory.
They are adapted from the YAML page configuration of the LDTstore homepage:

| File | Source |
|---|---|
| `examples/homepage/home/major.handlist` | `page/home/major.yml` |
| `examples/homepage/home/sides.handlist` | `page/home/sides.yml` |
| `examples/homepage/legacy/buttons.handlist` | `page/legacy/buttons.yml` |
| `examples/homepage/public/sides.handlist` | `page/public/sides.yml` |
| `examples/homepage/tool/major.handlist` | `page/tool/major.yml` |
| `examples/homepage/tool/category.handlist` | `page/tool/category.yml` |
| `examples/homepage/tool/sides.handlist` | `page/tool/sides.yml` |
| `examples/homepage/tool/tools.handlist` | `page/tool/tools.yml` |

The YAML originals used folded block text (`>`), indentless sequences, and a few
quoted keys. The examples show the handlist rewrite: `|` blocks, fixed
two-space indentation, and bare string keys.

A minimal document:

```handlist
# Site index
site:
  name: Example
  pages:
    - name: home
      title: 首页
      action: page
    - name: tools
      title: 工具
      action: page
```

---

## Appendix A: At-a-glance comparison

| YAML | handlist |
|---|---|
| `key: value` | same |
| `- value` | same |
| `key:\n  - value` | same (child indented two spaces) |
| `key:\n- value` | **invalid** (indentless sequence) |
| `- key: value` | same |
| `- - value` | same |
| `key: \|` + lines | same, but no trailing newline and fixed content column |
| `key: >` | **invalid** |
| `key: \|-`, `key: \|+`, `key: \|2` | **invalid** |
| `"text\n"` | same (escapes limited to `\"`, `\\`, `\n`, `\t`) |
| `'text'` | **invalid** |
| `[a, b]` | same (single line, scalars only) |
| `[]`, `{}` | same |
| `{a: b}` | **invalid** (only `{}`) |
| `key:` (nothing follows) | **invalid** (no implicit null) |
| `null` | same |
| `true`, `false` | same |
| `yes`, `no`, `on`, `off` | strings |
| `0x10`, `01`, `1e3`, `1:30`, dates | strings |
| `10:` | key is always the string `"10"` |
| `---`, `%YAML`, multiple documents | **invalid** |
| `&a`, `*a`, `<<`, `!!str`, `!tag` | **invalid** |
| duplicate keys | error |
| inline `#comment` after a space | **invalid**; write `# comment` |
| tabs for indentation | **invalid** |
| arbitrary indentation | **invalid**; exactly two spaces |
| map order | preserved (same intent as YAML, explicitly required) |

---

## Appendix B: Conformance checklist

A conforming implementation should at least cover these behaviors:

**Accept**

- root mapping, root sequence, and root scalar;
- nested mappings and sequences;
- compact mappings (`- key: value`) and nested sequences (`- - value`);
- plain Unicode strings and strings with internal spaces and apostrophes;
- double-quoted strings with all four escapes;
- flow sequences of plain and quoted scalars, `[]`, and `{}`;
- block text with blank lines, indented content, and no trailing newline;
- full-line comments and inline comments with a space after `#` (or an empty
  comment ending at `#`);
- explicit `null`, booleans, safe integers, and finite floats;
- bare and double-quoted mapping keys;
- mapping order preserved;
- empty block text as the empty string.

**Reject**

- empty documents (no root node);
- U+FEFF anywhere except the start of the file, and U+FFFE/U+FFFF anywhere;
- indentless sequences and non-two-space indentation;
- `key:` with no child block;
- bare `-`;
- duplicate keys;
- tabs in indentation, plain scalars, or quoted strings;
- single-quoted strings;
- `>` block text, `|-`, `|+`, `|2`;
- unknown escapes;
- non-empty flow mappings and nested flow sequences;
- trailing commas and empty flow elements;
- comments inside flow sequences;
- inline `#comment` without the space after `#`;
- integers outside the safe range, overflowing floats, `NaN`/`Infinity`;
- document markers, multiple documents, anchors, aliases, tags;
- content after the root node;
- invalid UTF-8 and raw control characters.
