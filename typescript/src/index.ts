import { parseOrdered } from "./parser.js";
import { toObject, type HandlistValue } from "./value.js";

export { HandlistError } from "./error.js";
export { stringify } from "./emitter.js";
export { parseOrdered } from "./parser.js";
export { toObject } from "./value.js";
export type {
  HandlistMap,
  HandlistValue,
  OrderedHandlistMap,
  OrderedHandlistValue,
} from "./value.js";

/**
 * Parses a handlist document into JSON-friendly values: mappings become plain
 * objects and sequences become arrays.
 *
 * JavaScript objects order integer-like keys numerically. Use `parseOrdered`
 * when the exact source mapping order must be preserved.
 */
export function parse(source: string): HandlistValue {
  return toObject(parseOrdered(source));
}

/**
 * Parses a handlist document into order-preserving values: mappings become
 * `Map`s and sequences become arrays.
 */
export { parseOrdered as parseWithMaps };
