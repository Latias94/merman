import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  candidateDependencyClosure,
  candidateBuildInvocation,
  resolveCandidateRecipe,
} from "../scripts/build-candidate.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");

test("candidate builds project its private capability recipe plus one transport", async () => {
  const descriptor = await readJson(path.join(nodeRoot, "candidate-builds.json"));
  const featureSurface = await readJson(
    path.join(repositoryRoot, descriptor.capability_recipe.descriptor),
  );
  const knownCapabilities = new Map(
    featureSurface.capabilities.map((capability) => [capability.id, capability]),
  );
  assert.equal(descriptor.capability_recipe.target, "native");
  for (const capabilityId of descriptor.capability_recipe.capabilities) {
    const capability = knownCapabilities.get(capabilityId);
    assert.ok(capability, `unknown capability ${capabilityId}`);
    assert.equal(capability.targets.includes("native"), true);
    for (const implication of capability.implications) {
      assert.equal(descriptor.capability_recipe.capabilities.includes(implication), true);
    }
  }

  const cases = [
    {
      candidate: "node-wasm",
      target: null,
      transportFeature: "transport-wasm",
    },
    {
      candidate: "napi",
      target: "darwin-arm64",
      transportFeature: "transport-napi",
    },
  ];
  for (const item of cases) {
    const recipe = resolveCandidateRecipe(item.candidate, item.target);
    const invocation = candidateBuildInvocation(recipe, "/tmp/merman-node-candidate");
    const featureIndex = invocation.args.indexOf("--features");
    assert.notEqual(featureIndex, -1);
    assert.deepEqual(recipe.capabilityFeatures, descriptor.capability_recipe.capabilities);
    assert.equal(recipe.targetId, item.target);
    assert.deepEqual(
      invocation.args[featureIndex + 1].split(","),
      [...descriptor.capability_recipe.capabilities, item.transportFeature].sort(),
    );
    assert.equal(invocation.args.includes("--no-default-features"), true);
    assert.equal(invocation.args.includes("-j1"), true);
    assert.equal(invocation.args.join(" ").includes("rust-static-svg"), false);
  }
});

test("merman-node forwards every candidate capability leaf without a private aggregate", async () => {
  const descriptor = await readJson(path.join(nodeRoot, "candidate-builds.json"));

  const manifest = await readFile(
    path.join(repositoryRoot, descriptor.cargo.manifest),
    "utf8",
  );
  const features = declaredCargoFeatures(manifest);
  for (const feature of [
    ...descriptor.capability_recipe.capabilities,
    "transport-napi",
    "transport-wasm",
  ]) {
    assert.equal(features.has(feature), true, `missing Cargo feature ${feature}`);
  }
  assert.equal(features.has("rust-static-svg"), false);
  assert.match(
    manifest,
    /transport-napi\s*=\s*\[[^\]]*"dep:napi-build"[^\]]*\]/s,
  );
  assert.match(
    manifest,
    /\[build-dependencies\][\s\S]*?napi-build\s*=\s*\{[^}]*optional\s*=\s*true[^}]*\}/,
  );
});

test("candidate dependency closures isolate transport-only packages", () => {
  const basePackages = [
    ["root", "merman-node-candidate"],
    ["bindings", "merman-bindings-core"],
  ];
  const napiMetadata = metadataWithPackages([
    ...basePackages,
    ["napi", "napi"],
    ["napi-build", "napi-build"],
    ["napi-derive", "napi-derive"],
  ]);
  const wasmMetadata = metadataWithPackages([
    ...basePackages,
    ["serde-wasm-bindgen", "serde-wasm-bindgen"],
    ["wasm-bindgen", "wasm-bindgen"],
  ]);

  assert.equal(
    candidateDependencyClosure(
      napiMetadata,
      resolveCandidateRecipe("napi", "darwin-arm64"),
    ).packages.some((item) => item.name === "napi-build"),
    true,
  );
  assert.equal(
    candidateDependencyClosure(
      wasmMetadata,
      resolveCandidateRecipe("node-wasm", null),
    ).packages.some((item) => item.name === "napi-build"),
    false,
  );

  assert.throws(
    () =>
      candidateDependencyClosure(
        metadataWithPackages([...basePackages, ["napi-build", "napi-build"]]),
        resolveCandidateRecipe("node-wasm", null),
      ),
    /napi-build.*wasm/i,
  );
  assert.throws(
    () =>
      candidateDependencyClosure(
        metadataWithPackages([
          ...basePackages,
          ["napi", "napi"],
          ["napi-derive", "napi-derive"],
        ]),
        resolveCandidateRecipe("napi", "darwin-arm64"),
      ),
    /napi-build.*napi/i,
  );
});

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

function metadataWithPackages(entries) {
  return {
    packages: entries.map(([id, name]) => ({
      id,
      name,
      version: "1.0.0",
      source: name.startsWith("merman-") ? null : "registry+test",
      manifest_path: path.join(repositoryRoot, "crates", name, "Cargo.toml"),
    })),
    resolve: {
      nodes: entries.map(([id]) => ({ id })),
    },
  };
}

function declaredCargoFeatures(manifest) {
  const marker = "[features]\n";
  const start = manifest.indexOf(marker);
  assert.notEqual(start, -1, "Cargo manifest must contain a [features] section");
  const remainder = manifest.slice(start + marker.length);
  const nextSection = remainder.search(/^\[/m);
  const body = nextSection === -1 ? remainder : remainder.slice(0, nextSection);
  return new Set(
    [...body.matchAll(/^([A-Za-z0-9_-]+)\s*=/gm)].map((match) => match[1]),
  );
}
