import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const playgroundRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  ".."
);
const outputRoot = path.join(playgroundRoot, ".runtime");
const publicEngineRoot = path.join(playgroundRoot, "public", "opaque-realm");

const bootstraps = [
  ["opaque-compare-bootstrap", "compare", "mermaid-engine"],
  [
    "opaque-benchmark-mermaid-bootstrap",
    "benchmark",
    "mermaid-engine",
  ],
];
const engines = [
  ["mermaid-engine", "mermaid"],
  ["benchmark-merman-engine", "benchmark-merman"],
];
const expectedOutputs = new Set(
  [...bootstraps, ...engines].flatMap(([file]) => [
    `${file}.js`,
    `${file}.json`,
  ])
);

assert.deepEqual((await readdir(outputRoot)).sort(), [...expectedOutputs].sort());

for (const [file, id, engineFile] of bootstraps) {
  const { manifest, source } = await readArtifact(file);
  assert.deepEqual(Object.keys(manifest).sort(), [
    "bytes",
    "cspHash",
    "engineArtifact",
    "id",
    "schemaVersion",
    "sha256",
  ]);
  verifyIdentity(manifest, source, id);
  const { manifest: engineManifest } = await readArtifact(engineFile);
  assert.deepEqual(manifest.engineArtifact, engineManifest);
  assert.equal(
    manifest.cspHash,
    `sha256-${createHash("sha256").update(source).digest("base64")}`
  );
  assert.ok(manifest.bytes < 256 * 1024, `${id} bootstrap is not bounded`);
  assert.doesNotMatch(source, /<\/script/i);
  assert.doesNotMatch(source, /__mermanEngineArtifact|11\.16\.0|@zenuml\/core/);
  assert.doesNotMatch(source, /^\s*import\s/m);
}

for (const [file, id] of engines) {
  const { manifest, source } = await readArtifact(file);
  assert.deepEqual(Object.keys(manifest).sort(), [
    "bytes",
    "id",
    "schemaVersion",
    "sha256",
  ]);
  verifyIdentity(manifest, source, id);
  assert.match(source, /__mermanEngineArtifact/);
  assert.doesNotMatch(source, /^\s*import\s/m);
  assert.doesNotMatch(source, /\bimport\s*\(/);
}

const expectedPublicEngines = engines.map(([file]) => `${file}.js`).sort();
assert.deepEqual(
  (await readdir(publicEngineRoot)).sort(),
  expectedPublicEngines
);
for (const [file] of engines) {
  const { source } = await readArtifact(file);
  assert.equal(
    await readFile(path.join(publicEngineRoot, `${file}.js`), "utf8"),
    source,
    `${file} public asset drifted from the verified engine source`
  );
}

async function readArtifact(file) {
  const source = await readFile(path.join(outputRoot, `${file}.js`), "utf8");
  const manifest = JSON.parse(
    await readFile(path.join(outputRoot, `${file}.json`), "utf8")
  );
  return { manifest, source };
}

function verifyIdentity(manifest, source, id) {
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.id, id);
  assert.equal(manifest.bytes, Buffer.byteLength(source));
  assert.ok(manifest.bytes > 0);
  assert.equal(
    manifest.sha256,
    createHash("sha256").update(source).digest("hex")
  );
}
