/**
 * Shared lexical predicates used by both the parser and the serializer.
 */

/** True for Unicode general category Cc (C0, DEL, and C1 controls). */
export function isControlCodePoint(codePoint: number): boolean {
  return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
}

/** True for a UTF-16 surrogate code point that is not part of a valid pair. */
export function isLoneSurrogate(codePoint: number): boolean {
  return codePoint >= 0xd800 && codePoint <= 0xdfff;
}

/** True for the YAML-forbidden noncharacters U+FFFE and U+FFFF. */
export function isNoncharacter(codePoint: number): boolean {
  return codePoint === 0xfffe || codePoint === 0xffff;
}

/** Characters that may not begin a plain scalar. */
export function isForbiddenPlainStart(ch: string): boolean {
  return (
    ch === " " ||
    ch === "\t" ||
    ch === "#" ||
    ch === '"' ||
    ch === "'" ||
    ch === "[" ||
    ch === "]" ||
    ch === "{" ||
    ch === "}" ||
    ch === "," ||
    ch === "&" ||
    ch === "*" ||
    ch === "!" ||
    ch === "|" ||
    ch === ">" ||
    ch === "%" ||
    ch === "@" ||
    ch === "`"
  );
}

/** Matches the handlist integer literal grammar. */
export function isIntSyntax(value: string): boolean {
  return /^-?(?:0|[1-9][0-9]*)$/.test(value);
}

/** Matches the handlist float literal grammar. */
export function isFloatSyntax(value: string): boolean {
  return /^-?(?:0|[1-9][0-9]*)\.[0-9]+$/.test(value);
}
