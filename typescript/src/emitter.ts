import { HandlistError } from "./error.js";
import {
  isControlCodePoint,
  isFloatSyntax,
  isForbiddenPlainStart,
  isIntSyntax,
  isLoneSurrogate,
  isNoncharacter,
} from "./syntax.js";
import type { HandlistValue, OrderedHandlistValue } from "./value.js";

type Entries = Array<[string, unknown]>;

/**
 * Serializes a handlist value into canonical handlist text.
 *
 * The output always ends with a single LF. Plain objects are accepted as
 * mappings; their keys follow JavaScript object ordering (integer-like keys are
 * sorted numerically). Use `Map` for exact order.
 */
export function stringify(value: HandlistValue | OrderedHandlistValue): string {
  const output: string[] = [];
  const stack = new Set<unknown>();
  emitRoot(value, output, stack);
  return output.join("");
}

function emitRoot(
  value: unknown,
  output: string[],
  stack: Set<unknown>,
): void {
  if (value instanceof Map || isPlainObject(value)) {
    const entries = mapEntries(value);
    if (entries.length > 0) {
      guard(value, stack);
      emitMapEntries(entries, 0, output, stack, null);
      unguard(value, stack);
      return;
    }
  } else if (Array.isArray(value) && value.length > 0) {
    guard(value, stack);
    emitSeq(value, 0, output, stack, null);
    unguard(value, stack);
    return;
  } else if (typeof value === "string" && isBlockString(value)) {
    output.push("|\n");
    writeBlock(value, 2, output);
    return;
  }

  output.push(inlineValue(value), "\n");
}

function emitMapEntries(
  entries: Entries,
  indent: number,
  output: string[],
  stack: Set<unknown>,
  firstPrefix: string | null,
): void {
  entries.forEach(([key, value], index) => {
    if (index === 0 && firstPrefix !== null) {
      output.push(firstPrefix);
    } else {
      output.push(" ".repeat(indent));
    }
    output.push(formatKey(key), ":");
    emitValueAfterColon(value, indent, output, stack);
  });
}

function emitValueAfterColon(
  value: unknown,
  indent: number,
  output: string[],
  stack: Set<unknown>,
): void {
  if (value instanceof Map || isPlainObject(value)) {
    const entries = mapEntries(value);
    if (entries.length > 0) {
      guard(value, stack);
      output.push("\n");
      emitMapEntries(entries, indent + 2, output, stack, null);
      unguard(value, stack);
      return;
    }
  } else if (Array.isArray(value) && value.length > 0) {
    guard(value, stack);
    output.push("\n");
    emitSeq(value, indent + 2, output, stack, null);
    unguard(value, stack);
    return;
  } else if (typeof value === "string" && isBlockString(value)) {
    output.push(" |\n");
    writeBlock(value, indent + 2, output);
    return;
  }

  output.push(" ", inlineValue(value), "\n");
}

function emitSeq(
  values: readonly unknown[],
  indent: number,
  output: string[],
  stack: Set<unknown>,
  firstPrefix: string | null,
): void {
  values.forEach((item, index) => {
    if (index === 0 && firstPrefix !== null) {
      output.push(firstPrefix);
    } else {
      output.push(" ".repeat(indent));
    }
    output.push("- ");
    emitItemValue(item, indent, output, stack);
  });
}

function emitItemValue(
  item: unknown,
  indent: number,
  output: string[],
  stack: Set<unknown>,
): void {
  if (item instanceof Map || isPlainObject(item)) {
    const entries = mapEntries(item);
    if (entries.length > 0) {
      guard(item, stack);
      emitMapEntries(entries, indent + 2, output, stack, "");
      unguard(item, stack);
      return;
    }
  } else if (Array.isArray(item) && item.length > 0) {
    guard(item, stack);
    emitSeq(item, indent + 2, output, stack, "");
    unguard(item, stack);
    return;
  } else if (typeof item === "string" && isBlockString(item)) {
    output.push("|\n");
    writeBlock(item, indent + 2, output);
    return;
  }

  output.push(inlineValue(item), "\n");
}

function inlineValue(value: unknown): string {
  if (value === null) {
    return "null";
  }

  switch (typeof value) {
    case "boolean":
      return value ? "true" : "false";
    case "number":
      return formatNumber(value);
    case "string":
      return plainSafe(value) ? value : quoted(value);
    case "object":
      if (value instanceof Map) {
        if (value.size === 0) {
          return "{}";
        }
        return serializationFail("cannot inline a non-empty mapping");
      }
      if (Array.isArray(value)) {
        if (value.length === 0) {
          return "[]";
        }
        return serializationFail("cannot inline a non-empty sequence");
      }
      if (isPlainObject(value)) {
        if (Object.keys(value).length === 0) {
          return "{}";
        }
        return serializationFail("cannot inline a non-empty mapping");
      }
      return serializationFail("unsupported object value");
    default:
      return serializationFail(
        `unsupported value of type ${typeof value}`,
      );
  }
}

function mapEntries(value: unknown): Entries {
  if (value instanceof Map) {
    const entries: Entries = [];
    for (const [key, child] of value) {
      if (typeof key !== "string") {
        serializationFail("handlist mapping keys must be strings");
      }
      entries.push([key, child]);
    }
    return entries;
  }

  if (!isPlainObject(value)) {
    serializationFail("expected a mapping");
  }
  const record = value as Record<string, unknown>;
  return Object.keys(record).map((key) => [key, record[key]]);
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    value instanceof Map
  ) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function guard(value: object, stack: Set<unknown>): void {
  if (stack.has(value)) {
    serializationFail("cyclic data structures cannot be serialized");
  }
  stack.add(value);
}

function unguard(value: object, stack: Set<unknown>): void {
  stack.delete(value);
}

function formatKey(key: string): string {
  if (key === "") {
    serializationFail("mapping keys must not be empty");
  }
  for (const ch of key) {
    const codePoint = ch.codePointAt(0) as number;
    if (
      isControlCodePoint(codePoint) ||
      isLoneSurrogate(codePoint) ||
      isNoncharacter(codePoint) ||
      codePoint === 0xfeff
    ) {
      serializationFail(
        "mapping keys containing control characters or noncharacters cannot be represented",
      );
    }
  }
  if (/^[A-Za-z0-9_][A-Za-z0-9_-]*$/.test(key)) {
    return key;
  }
  return quoted(key);
}

function plainSafe(value: string): boolean {
  if (
    value === "" ||
    value.startsWith(" ") ||
    value.endsWith(" ") ||
    value.includes("\uFEFF")
  ) {
    return false;
  }
  for (const ch of value) {
    const codePoint = ch.codePointAt(0) as number;
    if (
      isControlCodePoint(codePoint) ||
      isLoneSurrogate(codePoint) ||
      isNoncharacter(codePoint)
    ) {
      return false;
    }
  }
  if (value === "true" || value === "false" || value === "null") {
    return false;
  }
  if (isIntSyntax(value) || isFloatSyntax(value)) {
    return false;
  }
  if (value.includes(": ") || value.endsWith(":") || value.includes(" #")) {
    return false;
  }
  if (value.startsWith("---") || value.startsWith("...")) {
    return false;
  }

  const first = String.fromCodePoint(value.codePointAt(0) as number);
  if (isForbiddenPlainStart(first) || first === "?" || first === ":") {
    return false;
  }
  if (value === "-" || value.startsWith("- ")) {
    return false;
  }

  return true;
}

function quoted(value: string): string {
  let output = '"';
  for (const ch of value) {
    const codePoint = ch.codePointAt(0) as number;
    switch (ch) {
      case '"':
        output += '\\"';
        continue;
      case "\\":
        output += "\\\\";
        continue;
      case "\n":
        output += "\\n";
        continue;
      case "\t":
        output += "\\t";
        continue;
      default:
        break;
    }

    if (codePoint === 0xfeff) {
      serializationFail("U+FEFF cannot be represented in a handlist string");
    }
    if (isNoncharacter(codePoint)) {
      serializationFail(
        `noncharacter U+${codePoint
          .toString(16)
          .toUpperCase()
          .padStart(4, "0")} cannot be represented in a handlist string`,
      );
    }
    if (isControlCodePoint(codePoint)) {
      serializationFail(
        `control character U+${codePoint
          .toString(16)
          .toUpperCase()
          .padStart(4, "0")} cannot be represented in a handlist string`,
      );
    }
    if (isLoneSurrogate(codePoint)) {
      serializationFail("invalid Unicode scalar value in string");
    }
    output += ch;
  }
  output += '"';
  return output;
}

function isBlockString(value: string): boolean {
  if (
    !value.includes("\n") ||
    value.startsWith("\n") ||
    value.endsWith("\n")
  ) {
    return false;
  }
  if (value.includes("\r")) {
    return false;
  }
  for (const ch of value) {
    if (ch === "\n" || ch === "\t") {
      continue;
    }
    const codePoint = ch.codePointAt(0) as number;
    if (
      isControlCodePoint(codePoint) ||
      isLoneSurrogate(codePoint) ||
      isNoncharacter(codePoint) ||
      codePoint === 0xfeff
    ) {
      return false;
    }
  }
  return !value
    .split("\n")
    .some((line) => line.endsWith(" ") || line.endsWith("\t"));
}

function writeBlock(value: string, indent: number, output: string[]): void {
  const padding = " ".repeat(indent);
  for (const line of value.split("\n")) {
    if (line !== "") {
      output.push(padding, line);
    }
    output.push("\n");
  }
}

function formatNumber(value: number): string {
  if (!Number.isFinite(value)) {
    serializationFail("non-finite numbers cannot be represented in handlist");
  }
  if (Number.isSafeInteger(value)) {
    return Object.is(value, -0) ? "0" : String(value);
  }

  let text = String(value);
  if (/[eE]/.test(text)) {
    text = expandExponential(text);
  }
  if (/[eE]/.test(text)) {
    serializationFail("could not format the number without an exponent");
  }
  if (!text.includes(".")) {
    text += ".0";
  }
  return text;
}

function expandExponential(value: string): string {
  const match = /^(-?)(\d+)(?:\.(\d+))?[eE]([+-]?\d+)$/.exec(value);
  if (match === null) {
    return value;
  }

  const sign = match[1] ?? "";
  const integer = match[2] ?? "";
  const fraction = match[3] ?? "";
  const exponent = Number(match[4] ?? "0");
  const digits = integer + fraction;
  const point = integer.length + exponent;

  let result: string;
  if (point <= 0) {
    result = `0.${"0".repeat(-point)}${digits}`;
  } else if (point >= digits.length) {
    result = `${digits}${"0".repeat(point - digits.length)}`;
  } else {
    result = `${digits.slice(0, point)}.${digits.slice(point)}`;
  }
  return sign + result;
}

function serializationFail(reason: string): never {
  throw new HandlistError(reason);
}
