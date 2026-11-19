import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import test from "node:test";
import { parseDocument } from "yaml";

import { parse, parseOrdered, stringify, type OrderedHandlistValue } from "../src/index.js";

const examplesRoot = resolve(process.cwd(), "../examples");
const canonicalRoot = join(examplesRoot, "canonical");

function collect(dir: string, skipCanonical: boolean): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (skipCanonical && entry.name === "canonical") {
        continue;
      }
      files.push(...collect(path, skipCanonical));
    } else if (entry.name.endsWith(".handlist")) {
      files.push(path);
    }
  }
  return files;
}

function assertValidYaml(source: string, label: string): void {
  const document = parseDocument(source, { version: "1.2" });
  assert.equal(
    document.errors.length,
    0,
    `${label} is not valid YAML:\n${document.errors.map((error) => error.message).join("\n")}`,
  );
}

test("every example matches its canonical fixture and round-trips", (t) => {
  const sources = collect(examplesRoot, true).sort();
  const fixtures = collect(canonicalRoot, false).sort();
  assert.equal(sources.length, 8);
  assert.equal(fixtures.length, sources.length, "stale canonical fixtures");

  const fixtureSet = new Set(fixtures.map((path) => relative(canonicalRoot, path)));
  for (const path of sources) {
    const relativePath = relative(examplesRoot, path);
    assert.ok(fixtureSet.delete(relativePath), `missing canonical fixture for ${relativePath}`);
    const source = readFileSync(path, "utf8");
    assertValidYaml(source, relativePath);

    const value = parseOrdered(source);
    const canonical = stringify(value);
    const fixture = readFileSync(join(canonicalRoot, relativePath), "utf8");
    assert.equal(
      canonical,
      fixture,
      `canonical output differs for ${relativePath}`,
    );

    const reparsed = parseOrdered(canonical);
    assert.deepEqual(reparsed, value, `round-trip mismatch for ${relativePath}`);
    assert.equal(stringify(reparsed), canonical);
    assert.doesNotThrow(() => parse(source), `plain-object parse failed for ${relativePath}`);
    t.diagnostic(`ok ${relativePath}`);
  }
  assert.equal(fixtureSet.size, 0, `stale canonical fixtures: ${[...fixtureSet].join(", ")}`);
});

test("canonical fixtures are valid YAML and idempotent", () => {
  for (const path of collect(canonicalRoot, false).sort()) {
    const relativePath = relative(canonicalRoot, path);
    const source = readFileSync(path, "utf8");
    assertValidYaml(source, `canonical/${relativePath}`);
    const value: OrderedHandlistValue = parseOrdered(source);
    assert.equal(stringify(value), source, `canonical/${relativePath} is not idempotent`);
  }
});
