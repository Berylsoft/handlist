/**
 * Value types produced by the handlist parser.
 *
 * `OrderedHandlistValue` is the faithful representation: mappings are
 * JavaScript `Map`s, so insertion order (including integer-like keys) is
 * preserved exactly. `HandlistValue` is the JSON-friendly representation used
 * by `parse()`: mappings are plain objects.
 *
 * Note that JavaScript objects order integer-like keys numerically. That is a
 * JavaScript language rule, not a handlist rule; use `parseOrdered()` when the
 * exact mapping order matters.
 */

/** A handlist mapping that preserves insertion order exactly. */
export type OrderedHandlistMap = Map<string, OrderedHandlistValue>;

/** A parsed handlist value with order-preserving maps. */
export type OrderedHandlistValue =
  | null
  | boolean
  | number
  | string
  | OrderedHandlistValue[]
  | OrderedHandlistMap;

/** A JSON-friendly handlist mapping. */
export interface HandlistMap {
  [key: string]: HandlistValue;
}

/** A parsed handlist value with plain-object mappings. */
export type HandlistValue =
  | null
  | boolean
  | number
  | string
  | HandlistValue[]
  | HandlistMap;

/**
 * Converts an ordered value into JSON-friendly plain objects and arrays.
 *
 * Mappings become objects created with `Object.defineProperty`, so a key named
 * `__proto__` is a normal own property and cannot pollute the prototype.
 * JavaScript object key ordering applies to integer-like keys.
 */
export function toObject(value: OrderedHandlistValue): HandlistValue {
  if (value instanceof Map) {
    const object: HandlistMap = {};
    for (const [key, child] of value) {
      Object.defineProperty(object, key, {
        value: toObject(child),
        enumerable: true,
        configurable: true,
        writable: true,
      });
    }
    return object;
  }
  if (Array.isArray(value)) {
    return value.map(toObject);
  }
  return value;
}
