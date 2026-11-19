/**
 * The error type shared by the handlist parser and serializer.
 *
 * Parse errors carry a one-based line and column; serialization errors have
 * `line === null` and `column === null`.
 */
export class HandlistError extends Error {
  /** The reason without the location prefix. */
  readonly reason: string;
  /** One-based line number for parse errors, otherwise `null`. */
  readonly line: number | null;
  /** One-based column (in Unicode code points) for parse errors, otherwise `null`. */
  readonly column: number | null;

  constructor(reason: string, line: number | null = null, column: number | null = null) {
    super(
      line === null || column === null
        ? reason
        : `line ${line}, column ${column}: ${reason}`,
    );
    this.name = "HandlistError";
    this.reason = reason;
    this.line = line;
    this.column = column;
  }
}
