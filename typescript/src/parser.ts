import { HandlistError } from "./error.js";
import {
  isControlCodePoint,
  isFloatSyntax,
  isForbiddenPlainStart,
  isIntSyntax,
  isLoneSurrogate,
  isNoncharacter,
} from "./syntax.js";
import type { OrderedHandlistValue } from "./value.js";

interface SourceLine {
  number: number;
  raw: string;
  indent: number;
  content: string;
}

interface InlinePosition {
  text: string;
  line: number;
  column: number;
}

interface KeyPrefix {
  key: string;
  afterColon: number;
}

interface ParsedNode {
  value: OrderedHandlistValue;
  end: number;
}

interface QuotedResult {
  value: string;
  end: number;
}

/**
 * Parses a handlist document into a value whose mappings are `Map`s.
 *
 * This representation preserves mapping order exactly, including integer-like
 * keys. Use `parse()` from the package entry point for plain objects.
 */
export function parseOrdered(source: string): OrderedHandlistValue {
  if (typeof source !== "string") {
    throw new TypeError("handlist source must be a string");
  }
  return new Parser(source).parseDocument();
}

class Parser {
  private readonly lines: SourceLine[];
  private pos = 0;

  constructor(source: string) {
    const withoutBom = source.startsWith("\uFEFF") ? source.slice(1) : source;
    validateSource(withoutBom);

    this.lines = withoutBom.split("\n").map((part, index) => {
      const raw = part.endsWith("\r") ? part.slice(0, -1) : part;
      let indent = 0;
      while (raw[indent] === " ") {
        indent += 1;
      }
      return {
        number: index + 1,
        raw,
        indent,
        content: raw.slice(indent),
      };
    });
  }

  parseDocument(): OrderedHandlistValue {
    this.skipTrivia();
    if (this.pos >= this.lines.length) {
      this.failEof("a handlist document must contain a root node");
    }

    const { number, indent, content } = this.snapshot();
    if (indent !== 0) {
      fail("the root node must start at column 1", number, indent + 1);
    }
    if (isDocumentMarker(content)) {
      fail(
        "YAML document markers ('---' and '...') are not supported",
        number,
        1,
      );
    }

    let value: OrderedHandlistValue;
    if (isSequenceStart(content)) {
      value = this.parseSequence(0, null);
    } else if (splitKeyPrefix(content, number, 1) !== null) {
      value = this.parseMapping(0, null);
    } else {
      value = this.parseRootScalar(number, content);
    }

    this.skipTrivia();
    if (this.pos < this.lines.length) {
      const trailing = this.snapshot();
      fail(
        "unexpected content after the root node",
        trailing.number,
        trailing.indent + 1,
      );
    }

    return value;
  }

  private parseRootScalar(line: number, text: string): OrderedHandlistValue {
    this.pos += 1;
    const result = this.parseInlineNode(text, line, 1, 0);
    this.finishInline(text, result.end, line, 1);
    return result.value;
  }

  private parseMapping(
    indent: number,
    first: InlinePosition | null,
  ): OrderedHandlistValue {
    const map = new Map<string, OrderedHandlistValue>();

    if (first !== null) {
      const entry = this.parseEntryFragment(
        indent,
        first.text,
        first.line,
        first.column,
      );
      insertEntry(map, entry.key, entry.value, first.line, first.column);
    }

    for (;;) {
      this.skipTrivia();
      if (this.pos >= this.lines.length) {
        break;
      }

      const { number, indent: actual, content } = this.snapshot();
      if (actual < indent) {
        break;
      }
      if (actual > indent) {
        fail(
          `unexpected indentation: expected ${indent} spaces for this mapping`,
          number,
          actual + 1,
        );
      }
      if (content === "") {
        this.pos += 1;
        continue;
      }
      if (isSequenceStart(content)) {
        fail(
          "expected a mapping key, found a sequence item",
          number,
          actual + 1,
        );
      }

      const prefix = splitKeyPrefix(content, number, indent + 1);
      if (prefix === null) {
        fail("expected a mapping key such as 'name:'", number, indent + 1);
      }

      this.pos += 1;
      const value = this.parseEntryValue(
        indent,
        content,
        prefix.afterColon,
        number,
        indent + 1,
      );
      insertEntry(map, prefix.key, value, number, indent + 1);
    }

    return map;
  }

  private parseEntryFragment(
    mappingIndent: number,
    text: string,
    line: number,
    column: number,
  ): { key: string; value: OrderedHandlistValue } {
    const prefix = splitKeyPrefix(text, line, column);
    if (prefix === null) {
      fail("expected a mapping key", line, column);
    }
    const value = this.parseEntryValue(
      mappingIndent,
      text,
      prefix.afterColon,
      line,
      column,
    );
    return { key: prefix.key, value };
  }

  private parseEntryValue(
    mappingIndent: number,
    text: string,
    afterColon: number,
    line: number,
    baseColumn: number,
  ): OrderedHandlistValue {
    const rest = text.slice(afterColon);
    if (trimEndSpaces(rest) === "") {
      return this.parseChildBlock(mappingIndent + 2);
    }

    let spaces = 0;
    while (rest[spaces] === " ") {
      spaces += 1;
    }
    if (spaces === 0) {
      fail(
        "expected a space between ':' and the value",
        line,
        baseColumn + charCount(text.slice(0, afterColon)),
      );
    }

    const valueText = rest.slice(spaces);
    const valueColumn =
      baseColumn + charCount(text.slice(0, afterColon + spaces));
    if (valueText.startsWith("#")) {
      fail("mapping value is missing before the comment", line, valueColumn);
    }
    if (valueText.startsWith("\t")) {
      fail(
        "tabs are not allowed between ':' and the value",
        line,
        valueColumn,
      );
    }

    const result = this.parseInlineNode(
      valueText,
      line,
      valueColumn,
      mappingIndent,
    );
    this.finishInline(valueText, result.end, line, valueColumn);
    return result.value;
  }

  private parseChildBlock(indent: number): OrderedHandlistValue {
    this.skipTrivia();
    if (this.pos >= this.lines.length) {
      this.failEof("expected a child block");
    }

    const { number, indent: actual, content } = this.snapshot();
    if (actual > indent) {
      fail(
        `unexpected indentation: child blocks must be indented exactly ${indent} spaces`,
        number,
        actual + 1,
      );
    }
    if (actual < indent) {
      fail(
        `expected a child block indented ${indent} spaces`,
        number,
        actual + 1,
      );
    }

    if (isSequenceStart(content)) {
      return this.parseSequence(indent, null);
    }
    if (splitKeyPrefix(content, number, indent + 1) !== null) {
      return this.parseMapping(indent, null);
    }
    fail(
      "a value on a new line must be a mapping or a sequence; write scalars on the key line",
      number,
      indent + 1,
    );
  }

  private parseSequence(
    indent: number,
    first: InlinePosition | null,
  ): OrderedHandlistValue {
    const items: OrderedHandlistValue[] = [];

    if (first !== null) {
      items.push(
        this.parseSequenceItem(indent, first.text, first.line, first.column),
      );
    }

    for (;;) {
      this.skipTrivia();
      if (this.pos >= this.lines.length) {
        break;
      }

      const { number, indent: actual, content } = this.snapshot();
      if (actual < indent) {
        break;
      }
      if (actual > indent) {
        fail(
          `unexpected indentation: expected ${indent} spaces for this sequence`,
          number,
          actual + 1,
        );
      }
      if (!isSequenceStart(content)) {
        fail("expected a sequence item ('- ')", number, actual + 1);
      }

      this.pos += 1;
      items.push(this.parseSequenceItem(indent, content, number, indent + 1));
    }

    return items;
  }

  private parseSequenceItem(
    indent: number,
    text: string,
    line: number,
    column: number,
  ): OrderedHandlistValue {
    if (text === "-") {
      fail("a sequence item must have a value", line, column);
    }
    if (!text.startsWith("- ")) {
      fail(
        "a sequence item marker must be '-' followed by one space",
        line,
        column,
      );
    }

    const after = text.slice(2);
    if (after === "") {
      fail("a sequence item must have a value", line, column + 2);
    }
    if (after.startsWith(" ")) {
      fail("exactly one space is allowed after '-'", line, column + 2);
    }

    return this.parseItemValue(indent, after, line, column + 2);
  }

  private parseItemValue(
    indent: number,
    text: string,
    line: number,
    column: number,
  ): OrderedHandlistValue {
    if (text === "-" || text.startsWith("- ")) {
      return this.parseSequence(indent + 2, { text, line, column });
    }

    if (splitKeyPrefix(text, line, column) !== null) {
      return this.parseMapping(indent + 2, { text, line, column });
    }

    if (text.startsWith("#")) {
      fail(
        "sequence item value is missing before the comment",
        line,
        column,
      );
    }

    const result = this.parseInlineNode(text, line, column, indent);
    this.finishInline(text, result.end, line, column);
    return result.value;
  }

  private parseInlineNode(
    text: string,
    line: number,
    column: number,
    entryIndent: number,
  ): ParsedNode {
    if (text === "") {
      fail("expected a value", line, column);
    }
    const first = String.fromCodePoint(text.codePointAt(0) as number);

    switch (first) {
      case "|":
        return {
          value: this.parseBlockText(text, line, column, entryIndent),
          end: text.length,
        };
      case ">":
        fail("folded block text ('>') is not supported; use '|'", line, column);
        break;
      case '"': {
        const quoted = scanQuoted(text, line, column);
        return { value: quoted.value, end: quoted.end };
      }
      case "[":
        return this.parseFlowSequence(text, line, column);
      case "{":
        return this.parseEmptyMapping(text, line, column);
      case "'":
        fail(
          "single-quoted strings are not supported; use double quotes",
          line,
          column,
        );
        break;
      case "#":
        fail("expected a value before the comment", line, column);
        break;
      default:
        if (first === "-" && (text.length === 1 || text[1] === " ")) {
          fail(
            "an inline node cannot start with '- '; use a block sequence",
            line,
            column,
          );
        }
        return this.parsePlain(text, line, column);
    }
  }

  private parsePlain(text: string, line: number, column: number): ParsedNode {
    let end = text.length;

    for (let index = 0; index < text.length; ) {
      const codePoint = text.codePointAt(index) as number;
      const ch = String.fromCodePoint(codePoint);
      if (ch === "#" && index > 0 && text[index - 1] === " ") {
        const next = text.codePointAt(index + 1);
        if (next === undefined || next === 0x20) {
          end = index - 1;
          break;
        }
        fail(
          "an inline comment must be written as '# ' (hash followed by a space) or end with '#' at the end of the line",
          line,
          column + charCount(text.slice(0, index)),
        );
      }
      index += ch.length;
    }

    const valueText = trimEndSpaces(text.slice(0, end));
    if (valueText === "") {
      fail("expected a value", line, column);
    }

    validatePlain(valueText, line, column, false);
    return {
      value: resolvePlain(valueText, line, column),
      end,
    };
  }

  private parseFlowSequence(
    text: string,
    line: number,
    column: number,
  ): ParsedNode {
    const values: OrderedHandlistValue[] = [];
    let index = skipFlowSpaces(text, 1, line, column);

    if (index >= text.length) {
      fail(
        "unterminated flow sequence",
        line,
        column + charCount(text),
      );
    }
    if (text[index] === "]") {
      return { value: values, end: index + 1 };
    }

    for (;;) {
      if (index >= text.length) {
        fail(
          "unterminated flow sequence",
          line,
          column + charCount(text),
        );
      }

      const elementColumn = column + charCount(text.slice(0, index));
      const first = String.fromCodePoint(text.codePointAt(index) as number);

      if (first === "]" || first === ",") {
        fail("empty flow-sequence element", line, elementColumn);
      }
      if (first === "#") {
        fail(
          "comments are not allowed inside a flow sequence",
          line,
          elementColumn,
        );
      }
      if (first === "[" || first === "{") {
        fail(
          "nested flow collections are not supported inside a flow sequence",
          line,
          elementColumn,
        );
      }

      if (first === '"') {
        const quoted = scanQuoted(text.slice(index), line, elementColumn);
        values.push(quoted.value);
        index += quoted.end;
      } else {
        let end = text.length;
        let terminated = false;
        for (let cursor = index; cursor < text.length; ) {
          const codePoint = text.codePointAt(cursor) as number;
          const ch = String.fromCodePoint(codePoint);
          if (ch === "," || ch === "]") {
            end = cursor;
            terminated = true;
            break;
          }
          cursor += ch.length;
        }
        if (!terminated) {
          fail(
            "a flow sequence must be closed on the same line",
            line,
            elementColumn,
          );
        }

        const element = trimEndSpaces(text.slice(index, end));
        if (element === "") {
          fail("empty flow-sequence element", line, elementColumn);
        }
        validatePlain(element, line, elementColumn, true);
        values.push(resolvePlain(element, line, elementColumn));
        index = end;
      }

      index = skipFlowSpaces(text, index, line, column);
      if (index >= text.length) {
        fail(
          "unterminated flow sequence",
          line,
          column + charCount(text),
        );
      }

      const delimiter = text[index];
      if (delimiter === ",") {
        index += 1;
        index = skipFlowSpaces(text, index, line, column);
        if (index >= text.length) {
          fail(
            "unterminated flow sequence",
            line,
            column + charCount(text),
          );
        }
        const next = text[index];
        if (next === "," || next === "]") {
          fail(
            "empty flow-sequence element",
            line,
            column + charCount(text.slice(0, index)),
          );
        }
      } else if (delimiter === "]") {
        return { value: values, end: index + 1 };
      } else {
        fail(
          `expected ',' or ']' in a flow sequence, found '${delimiter}'`,
          line,
          column + charCount(text.slice(0, index)),
        );
      }
    }
  }

  private parseEmptyMapping(
    text: string,
    line: number,
    column: number,
  ): ParsedNode {
    const index = skipFlowSpaces(text, 1, line, column);
    if (index >= text.length || text[index] !== "}") {
      fail("only the empty flow mapping '{}' is supported", line, column);
    }
    return { value: new Map<string, OrderedHandlistValue>(), end: index + 1 };
  }

  private parseBlockText(
    text: string,
    line: number,
    column: number,
    entryIndent: number,
  ): string {
    const rest = text.slice(1);
    if (rest !== "") {
      let index = 0;
      while (rest[index] === " ") {
        index += 1;
      }
      if (index < rest.length) {
        if (rest[index] === "#") {
          if (index === 0) {
            fail(
              "an inline comment must be separated from '|' by a space",
              line,
              column + 1,
            );
          }
          if (
            rest[index + 1] !== undefined &&
            rest[index + 1] !== " "
          ) {
            fail(
              "an inline comment must be written as '# ' (hash followed by a space) or end with '#' at the end of the line",
              line,
              column + 1 + index,
            );
          }
        } else {
          fail(
            "the block text marker must be exactly '|'; '>', '|-', '|+' and indentation indicators are not supported",
            line,
            column,
          );
        }
      }
    }

    const base = entryIndent + 2;
    const contentLines: string[] = [];

    while (this.pos < this.lines.length) {
      const { raw, indent, number } = this.lines[this.pos] as SourceLine;

      if (/^ *$/.test(raw)) {
        contentLines.push("");
        this.pos += 1;
        continue;
      }

      if (indent < base) {
        if (indent > entryIndent) {
          fail(
            `block text content must be indented ${base} spaces`,
            number,
            indent + 1,
          );
        }
        break;
      }

      contentLines.push(trimEndSpacesTabs(raw.slice(base)));
      this.pos += 1;
    }

    while (contentLines.length > 0 && contentLines[0] === "") {
      contentLines.shift();
    }
    while (
      contentLines.length > 0 &&
      contentLines[contentLines.length - 1] === ""
    ) {
      contentLines.pop();
    }

    return contentLines.join("\n");
  }

  private finishInline(
    text: string,
    end: number,
    line: number,
    column: number,
  ): void {
    const rest = text.slice(end);
    let index = 0;
    let spaces = 0;
    while (rest[index] === " ") {
      index += 1;
      spaces += 1;
    }
    if (index === rest.length) {
      return;
    }

    const ch = rest[index];
    const errorColumn = column + charCount(text.slice(0, end + index));
    if (ch !== "#") {
      fail(`unexpected character '${ch}' after the value`, line, errorColumn);
    }
    if (spaces === 0) {
      fail(
        "an inline comment must be preceded by a space",
        line,
        errorColumn,
      );
    }
    if (rest[index + 1] !== undefined && rest[index + 1] !== " ") {
      fail(
        "an inline comment must be written as '# ' (hash followed by a space) or end with '#' at the end of the line",
        line,
        errorColumn + 1,
      );
    }
  }

  private skipTrivia(): void {
    while (this.pos < this.lines.length) {
      const line = this.lines[this.pos] as SourceLine;
      if (line.content === "" || line.content.startsWith("#")) {
        this.pos += 1;
      } else {
        break;
      }
    }
  }

  private snapshot(): {
    number: number;
    indent: number;
    content: string;
  } {
    const line = this.lines[this.pos] as SourceLine;
    return {
      number: line.number,
      indent: line.indent,
      content: trimEndSpaces(line.content),
    };
  }

  private failEof(message: string): never {
    const last = this.lines[this.lines.length - 1] as SourceLine;
    fail(message, last.number, charCount(last.raw) + 1);
  }
}

function insertEntry(
  map: Map<string, OrderedHandlistValue>,
  key: string,
  value: OrderedHandlistValue,
  line: number,
  column: number,
): void {
  if (map.has(key)) {
    fail(`duplicate mapping key ${JSON.stringify(key)}`, line, column);
  }
  map.set(key, value);
}

function validateSource(source: string): void {
  let line = 1;
  let column = 1;
  let index = 0;

  while (index < source.length) {
    const codePoint = source.codePointAt(index) as number;
    const ch = String.fromCodePoint(codePoint);

    if (ch === "\n") {
      line += 1;
      column = 1;
      index += 1;
      continue;
    }
    if (ch === "\r") {
      if (source[index + 1] === "\n") {
        line += 1;
        column = 1;
        index += 2;
        continue;
      }
      fail(
        "a bare carriage return is not allowed; use LF or CRLF",
        line,
        column,
      );
    }
    if (ch === "\t") {
      column += 1;
      index += 1;
      continue;
    }
    if (codePoint === 0xfeff) {
      fail(
        "a byte-order mark is only allowed at the very start of the file",
        line,
        column,
      );
    }
    if (isLoneSurrogate(codePoint)) {
      fail("invalid Unicode scalar value", line, column);
    }
    if (isNoncharacter(codePoint)) {
      fail(
        `noncharacter U+${codePoint
          .toString(16)
          .toUpperCase()
          .padStart(4, "0")} is not allowed`,
        line,
        column,
      );
    }
    if (isControlCodePoint(codePoint)) {
      fail(
        `control character U+${codePoint
          .toString(16)
          .toUpperCase()
          .padStart(4, "0")} is not allowed`,
        line,
        column,
      );
    }

    column += 1;
    index += ch.length;
  }
}

function isDocumentMarker(content: string): boolean {
  for (const marker of ["---", "..."]) {
    if (content.startsWith(marker)) {
      const rest = content.slice(marker.length);
      if (rest === "" || rest.startsWith(" ")) {
        return true;
      }
    }
  }
  return false;
}

function isSequenceStart(content: string): boolean {
  return content === "-" || content.startsWith("- ");
}

function splitKeyPrefix(
  text: string,
  line: number,
  column: number,
): KeyPrefix | null {
  if (text.startsWith('"')) {
    const quoted = scanQuoted(text, line, column);
    if (text[quoted.end] !== ":" || !colonIsSeparator(text, quoted.end)) {
      return null;
    }
    validateKey(quoted.value, line, column);
    return { key: quoted.value, afterColon: quoted.end + 1 };
  }

  const match = /^[A-Za-z0-9_][A-Za-z0-9_-]*/.exec(text);
  if (match === null) {
    return null;
  }
  const end = match[0].length;
  if (text[end] !== ":" || !colonIsSeparator(text, end)) {
    return null;
  }
  return { key: match[0], afterColon: end + 1 };
}

function colonIsSeparator(text: string, colon: number): boolean {
  const after = text.slice(colon + 1);
  return after === "" || after.startsWith(" ");
}

function validateKey(key: string, line: number, column: number): void {
  if (key === "") {
    fail("mapping keys must not be empty", line, column);
  }
  for (const ch of key) {
    const codePoint = ch.codePointAt(0) as number;
    if (
      isControlCodePoint(codePoint) ||
      isLoneSurrogate(codePoint) ||
      codePoint === 0xfeff
    ) {
      fail(
        `mapping keys must not contain control characters or U+FEFF (found U+${codePoint
          .toString(16)
          .toUpperCase()
          .padStart(4, "0")})`,
        line,
        column,
      );
    }
  }
}

function scanQuoted(
  text: string,
  line: number,
  column: number,
): QuotedResult {
  let value = "";
  let index = 1;

  while (index < text.length) {
    const codePoint = text.codePointAt(index) as number;
    const ch = String.fromCodePoint(codePoint);
    const charColumn = column + charCount(text.slice(0, index));

    if (ch === '"') {
      return { value, end: index + 1 };
    }
    if (ch === "\\") {
      const nextCodePoint = text.codePointAt(index + 1);
      if (nextCodePoint === undefined) {
        fail("unterminated escape sequence", line, charColumn);
      }
      const next = String.fromCodePoint(nextCodePoint);
      switch (next) {
        case '"':
          value += '"';
          break;
        case "\\":
          value += "\\";
          break;
        case "n":
          value += "\n";
          break;
        case "t":
          value += "\t";
          break;
        case "u":
        case "U":
        case "x":
          fail(
            "Unicode and hex escapes are not supported; write the character directly as UTF-8",
            line,
            charColumn,
          );
          break;
        default:
          fail(
            `unsupported escape sequence '\\${next}'`,
            line,
            charColumn,
          );
      }
      index += 1 + next.length;
      continue;
    }
    if (ch === "\t") {
      fail(
        "raw tabs must be written as '\\t' inside a double-quoted string",
        line,
        charColumn,
      );
    }
    if (isControlCodePoint(codePoint) || isLoneSurrogate(codePoint)) {
      fail(
        `control character U+${codePoint
          .toString(16)
          .toUpperCase()
          .padStart(4, "0")} is not allowed`,
        line,
        charColumn,
      );
    }

    value += ch;
    index += ch.length;
  }

  fail(
    "unterminated double-quoted string",
    line,
    column + charCount(text),
  );
}

function validatePlain(
  value: string,
  line: number,
  column: number,
  flow: boolean,
): void {
  const firstCodePoint = value.codePointAt(0);
  if (firstCodePoint === undefined) {
    fail("expected a value", line, column);
  }
  const first = String.fromCodePoint(firstCodePoint);

  if (first === "\t") {
    fail("tabs are not allowed in plain scalars", line, column);
  }
  if (isForbiddenPlainStart(first) || first === "?" || first === ":") {
    fail(`a plain scalar cannot start with '${first}'`, line, column);
  }
  if (first === "-" && (value.length === 1 || value[1] === " ")) {
    fail("a plain scalar cannot start with '- '", line, column);
  }

  for (let index = 0; index < value.length; ) {
    const codePoint = value.codePointAt(index) as number;
    const ch = String.fromCodePoint(codePoint);
    const charColumn = column + charCount(value.slice(0, index));

    if (ch === "\t") {
      fail("tabs are not allowed in plain scalars", line, charColumn);
    }
    if (isControlCodePoint(codePoint) || isLoneSurrogate(codePoint)) {
      fail(
        `control character U+${codePoint
          .toString(16)
          .toUpperCase()
          .padStart(4, "0")} is not allowed`,
        line,
        charColumn,
      );
    }
    if (ch === ":") {
      const next = value.codePointAt(index + 1);
      if (next === undefined || next === 0x20) {
        fail(
          "a colon in a plain scalar must not be followed by a space or end the line; quote the value",
          line,
          charColumn,
        );
      }
    }
    if (flow && (ch === "," || ch === "[" || ch === "]" || ch === "{" || ch === "}")) {
      fail(
        `'${ch}' is not allowed in a flow-sequence element`,
        line,
        charColumn,
      );
    }
    if (ch === "#" && index > 0 && value[index - 1] === " ") {
      fail(
        "a plain scalar cannot contain a space followed by '#'; quote the value",
        line,
        charColumn,
      );
    }

    index += ch.length;
  }
}

function resolvePlain(
  value: string,
  line: number,
  column: number,
): OrderedHandlistValue {
  switch (value) {
    case "null":
      return null;
    case "true":
      return true;
    case "false":
      return false;
    default:
      break;
  }

  if (isIntSyntax(value)) {
    const parsed = Number(value);
    if (!Number.isSafeInteger(parsed)) {
      fail(
        `integer ${JSON.stringify(value)} is outside the safe integer range`,
        line,
        column,
      );
    }
    return parsed;
  }

  if (isFloatSyntax(value)) {
    const parsed = Number(value);
    if (!Number.isFinite(parsed)) {
      fail(`float ${JSON.stringify(value)} is out of range`, line, column);
    }
    return parsed;
  }

  return value;
}

function skipFlowSpaces(
  text: string,
  start: number,
  line: number,
  column: number,
): number {
  let index = start;
  while (index < text.length) {
    const ch = text[index];
    if (ch === " ") {
      index += 1;
    } else if (ch === "\t") {
      fail(
        "tabs are not allowed inside a flow sequence",
        line,
        column + charCount(text.slice(0, index)),
      );
    } else {
      break;
    }
  }
  return index;
}

function trimEndSpaces(value: string): string {
  let end = value.length;
  while (end > 0 && value[end - 1] === " ") {
    end -= 1;
  }
  return value.slice(0, end);
}

function trimEndSpacesTabs(value: string): string {
  let end = value.length;
  while (end > 0 && (value[end - 1] === " " || value[end - 1] === "\t")) {
    end -= 1;
  }
  return value.slice(0, end);
}

function charCount(value: string): number {
  let count = 0;
  for (const _ of value) {
    count += 1;
  }
  return count;
}

function fail(reason: string, line: number, column: number): never {
  throw new HandlistError(reason, line, column);
}
