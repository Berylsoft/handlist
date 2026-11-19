# handlist (TypeScript)

TypeScript implementation of [handlist 1.0](../spec/handlist-spec.md): a strict,
human-friendly subset of YAML for hand-edited configuration files.

Zero runtime dependencies. ESM, CommonJS, and TypeScript declarations are
produced in `dist/`.

```bash
npm install
npm run build
```

```ts
import {
  HandlistError,
  parse,
  parseOrdered,
  stringify,
} from "handlist";

const config = parse(source);          // mappings are plain objects
const ordered = parseOrdered(source);  // mappings are Maps, order preserved
const text = stringify(config);        // canonical handlist
```

- `parse(source)` returns JSON-friendly values with plain-object mappings.
- `parseOrdered(source)` returns order-preserving values with `Map` mappings.
- `stringify(value)` accepts plain objects, `Map`s, arrays, and scalars.
- Parse errors are `HandlistError` instances with `reason`, `line`, and
  `column`.

## JavaScript host limitations

1. Plain objects order integer-like keys numerically. Use `parseOrdered()` when
   exact mapping order matters.
2. JavaScript has a single `number` type, so integers and floats are not
   distinguished. Integral values are serialized in integer form. Use the Rust
   implementation when exact int/float fidelity is required.

See the [root README](../README.md) for the full syntax overview, examples, and
design rationale, and the [specification](../spec/handlist-spec.md) for the
normative rules.

## Development

```bash
npm test
```

The tests cover parsing, canonical serialization, ordered mappings, errors, the
eight homepage examples, canonical fixtures, and the guarantee that accepted
documents are syntactically valid YAML 1.2.
