import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import {
  OPAQUE_REALM_ARTIFACT_PLAN,
  artifactOutputFiles,
  defineOpaqueRealmArtifactPlan,
  publicEngineFiles,
} from "./opaque-realm-artifact-plan.mjs";
import { renderOpaqueRealmBrowserProjections } from "./opaque-realm-browser-projection.mjs";

const playgroundRoot = path.resolve(import.meta.dirname, "..");

test("canonical plan owns every generated and public opaque artifact", () => {
  assert.deepEqual(artifactOutputFiles(OPAQUE_REALM_ARTIFACT_PLAN), [
    "benchmark-merman-engine.js",
    "benchmark-merman-engine.json",
    "mermaid-engine.js",
    "mermaid-engine.json",
    "opaque-benchmark-mermaid-bootstrap.js",
    "opaque-benchmark-mermaid-bootstrap.json",
    "opaque-compare-bootstrap.js",
    "opaque-compare-bootstrap.json",
  ]);
  assert.deepEqual(publicEngineFiles(OPAQUE_REALM_ARTIFACT_PLAN), [
    "benchmark-merman-engine.js",
    "mermaid-engine.js",
  ]);
});

test("plan validation rejects duplicate outputs and broken references", () => {
  const duplicate = structuredClone(OPAQUE_REALM_ARTIFACT_PLAN);
  duplicate.engines[1].outputBase = duplicate.engines[0].outputBase;
  assert.throws(
    () => defineOpaqueRealmArtifactPlan(duplicate),
    /Duplicate artifact output/u,
  );

  const missingEngine = structuredClone(OPAQUE_REALM_ARTIFACT_PLAN);
  missingEngine.engines.shift();
  assert.throws(
    () => defineOpaqueRealmArtifactPlan(missingEngine),
    /unknown engine mermaid/u,
  );

  const malformed = structuredClone(OPAQUE_REALM_ARTIFACT_PLAN);
  malformed.realms[0].page = "benchmarkRealm";
  assert.throws(
    () => defineOpaqueRealmArtifactPlan(malformed),
    /cannot declare a page/u,
  );
});

test("plan projection owns known fields without traversing unknown cycles", () => {
  const input = structuredClone(OPAQUE_REALM_ARTIFACT_PLAN);
  const unknownCycle = {};
  unknownCycle.self = unknownCycle;
  input.unknown = unknownCycle;
  input.engines[0].unknown = unknownCycle;
  input.realms[0].bootstrap.unknown = unknownCycle;

  const plan = defineOpaqueRealmArtifactPlan(input);
  assert.equal(Object.hasOwn(plan, "unknown"), false);
  assert.equal(Object.hasOwn(plan.engines[0], "unknown"), false);
  assert.equal(Object.hasOwn(plan.realms[0].bootstrap, "unknown"), false);
  assert.notEqual(plan.engines, input.engines);
  assert.notEqual(plan.engines[0].exports, input.engines[0].exports);

  input.engines[0].id = "mutated-engine";
  input.engines[0].exports.push("mutatedExport");
  assert.equal(plan.engines[0].id, "mermaid");
  assert.deepEqual(plan.engines[0].exports, [
    "benchmarkEngineAdapter",
    "renderWithMermaid",
  ]);
  assert.throws(() => plan.engines[0].exports.push("forbidden"), TypeError);
});

test("artifact additions and renames flow through derived output ownership", () => {
  const expanded = structuredClone(OPAQUE_REALM_ARTIFACT_PLAN);
  expanded.engines.push({
    id: "fixture-engine",
    entry: "src/fixture-engine.ts",
    outputBase: "fixture-engine",
    publish: false,
    maxBytes: 1024,
    resourcePolicy: "none-v1",
    exports: ["fixtureEngine"],
  });
  const plan = defineOpaqueRealmArtifactPlan(expanded);
  assert.ok(artifactOutputFiles(plan).includes("fixture-engine.json"));
  assert.ok(!publicEngineFiles(plan).includes("fixture-engine.js"));

  const renamed = structuredClone(OPAQUE_REALM_ARTIFACT_PLAN);
  renamed.realms[0].bootstrap.outputBase = "renamed-compare-bootstrap";
  renamed.roots.publicEngines = "public/browser-engines";
  const projections = renderOpaqueRealmBrowserProjections(
    defineOpaqueRealmArtifactPlan(renamed),
  );
  assert.match(
    projections.get(
      "src/runtime/realm/generated/compare-mermaid.generated.ts",
    ),
    /renamed-compare-bootstrap\.js\?raw/u,
  );
  assert.match(
    projections.get(
      "src/runtime/realm/generated/compare-mermaid.generated.ts",
    ),
    /publicPath: "browser-engines\/mermaid-engine\.js"/u,
  );
});

test("each engine owns a positive artifact byte budget", () => {
  assert.deepEqual(
    OPAQUE_REALM_ARTIFACT_PLAN.engines.map(({ id, maxBytes }) => [id, maxBytes]),
    [
      ["mermaid", 12 * 1024 * 1024],
      ["benchmark-merman", 256 * 1024],
    ],
  );

  const malformed = structuredClone(OPAQUE_REALM_ARTIFACT_PLAN);
  malformed.engines[0].maxBytes = 0;
  assert.throws(
    () => defineOpaqueRealmArtifactPlan(malformed),
    /engine mermaid byte budget/u,
  );
});

test("checked-in browser projections are current and split by activation owner", async () => {
  const projections = renderOpaqueRealmBrowserProjections(
    OPAQUE_REALM_ARTIFACT_PLAN,
  );
  for (const [file, expected] of projections) {
    assert.equal(await readFile(path.join(playgroundRoot, file), "utf8"), expected);
  }
  const compare = projections.get(
    "src/runtime/realm/generated/compare-mermaid.generated.ts",
  );
  assert.doesNotMatch(compare, /benchmark-mermaid-bootstrap/u);
  const benchmark = projections.get(
    "src/benchmark/realm/generated/benchmark-mermaid.generated.ts",
  );
  assert.doesNotMatch(benchmark, /opaque-compare-bootstrap/u);
});
