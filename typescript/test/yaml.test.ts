import assert from "node:assert/strict";
import test from "node:test";
import { parseDocument } from "yaml";

import { parseOrdered, stringify } from "../src/index.js";

/**
 * A corpus of accepted handlist snippets that exercise the edges of the
 * syntax. Every one of them must also be readable by a YAML 1.2 parser: that is
 * the "handlist is a syntactic YAML subset" guarantee.
 */
const accepted: Record<string, string> = {
  "url in block": "url: http://example.com/a#frag\n",
  "url in flow": "items: [http://example.com/a#frag]\n",
  "colon without space": "value: a:b\n",
  "colon without space in flow": "items: [a:b]\n",
  "hash without space in flow": "items: [C#]\n",
  "apostrophe in flow": "items: [Tom's]\n",
  "brackets braces commas in block plain": "value: a [b] {c} d, e\n",
  "indicators inside block plain":
    "value: a & b * c ! d > e | f % g @ h ` i\n",
  "indicators inside flow elements":
    "items: [a&b, c*d, e!f, g>h, i|j, k%l, m@n, o`p, q?r]\n",
  "negative number root": "-5\n",
  "dash word value": "value: -foo\n",
  "dash word in flow": "items: [-foo]\n",
  "document marker as value": "value: ---\n",
  "document end as value": "value: ...\n",
  "root block text": "|\n  hello\n",
  "sequence block text": "- |\n  hello\n",
  "compact mapping block text": "- text: |\n    hello\n",
  "quoted numeric key": '"10": x\n',
  "boolean-looking key": "true: x\n",
  "null-looking key": "null: x\n",
  "numeric key": "10: x\n",
  "quoted key containing a colon": '"a: b": x\n',
  "empty collections with spaces": "a: { }\nb: [ ]\n",
  "tab inside block text": "text: |\n  a\tb\n",
  "tab-only content at base indentation": "text: |\n  \t\n  b\n",
  "tab in comment": "# comment\twith tab\nkey: value\n",
  "inline comment": "key: value # note\n",
  "empty inline comment": "key: value #\n",
  "empty collections": "a: []\nb: {}\n",
  "flow scalars": 'items: [null, true, false, -5, 1.5, "x"]\n',
  "nested sequences": "- - - a\n",
  "quoted key in compact mapping": '- "a b": c\n',
  "question mark inside plain": "value: a?b\n",
  "percent inside plain": "value: 100%\n",
  "unicode text": "value: 中文 😀\n",
  "multiple spaces after colon": "key:    value\n",
  "empty block text": "text: |\nnext: 1\n",
  "empty root block text": "|\n",
  "empty root mapping": "{ }\n",
  "empty root sequence": "[ ]\n",
  "root quoted scalar": '"hello" # note\n',
  "flow with spaces": "items: [ a , b ]\n",
  "quoted empty flow element": 'items: [""]\n',
  "underscore key": "_x: y\n",
  "digit-hyphen key": "a-1_b: y\n",
  "trailing spaces after value": "key: value   \n",
};

test("accepted handlist edge cases are valid YAML 1.2", (t) => {
  for (const [name, source] of Object.entries(accepted)) {
    assert.doesNotThrow(
      () => parseOrdered(source),
      `${name}: handlist rejected an accepted snippet`,
    );

    const document = parseDocument(source, { version: "1.2" });
    assert.equal(
      document.errors.length,
      0,
      `${name}: YAML rejected the snippet:\n${document.errors
        .map((error) => error.message)
        .join("\n")}`,
    );

    const canonical = stringify(parseOrdered(source));
    const canonicalDocument = parseDocument(canonical, { version: "1.2" });
    assert.equal(
      canonicalDocument.errors.length,
      0,
      `${name}: canonical output is not valid YAML`,
    );
    assert.deepEqual(
      parseOrdered(canonical),
      parseOrdered(source),
      `${name}: canonical output did not round-trip`,
    );
    t.diagnostic(`ok ${name}`);
  }
});
