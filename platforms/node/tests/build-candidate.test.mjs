import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  candidateDependencyClosure,
  candidateBuildInvocation,
  collectLocalInputEntries,
  resolveCandidateRuntimeContract,
  resolveCandidateRecipe,
  validateCandidateCargoMetadata,
  validateCandidatePackageVersions,
} from "../scripts/build-candidate.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const PACKAGE_VERSION = JSON.parse(
  await readFile(path.join(nodeRoot, "package-surfaces.json"), "utf8"),
).version;

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

test("candidate runtime outputs follow explicit binding operation ownership", () => {
  const contract = resolveCandidateRuntimeContract();

  assert.deepEqual(contract.operationIds, [
    "layout-json",
    "semantic-json",
    "svg",
    "svg-plan-json",
  ]);
  assert.deepEqual(contract.outputIds, ["svg"]);
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

test("candidate source receipt covers generated package contracts", async () => {
  const entries = collectLocalInputEntries({ packages: [] });
  for (const relativePath of [
    "platforms/node/src/generated/binding-contract.mjs",
    "platforms/node/src/generated/capability-surface.mjs",
    "platforms/node/src/generated/node-wire-contract.json",
  ]) {
    const entry = entries.find((item) => item.path === relativePath);
    const contents = await readFile(path.join(repositoryRoot, relativePath));

    assert.ok(entry, `${relativePath} must be a candidate source input`);
    assert.equal(entry.bytes, contents.byteLength);
    assert.equal(
      entry.sha256,
      `sha256:${createHash("sha256").update(contents).digest("hex")}`,
    );
  }
});

test("candidate Cargo packages stay aligned with the private package surface version", () => {
  const metadata = metadataWithPackages([
    ["root", "merman-node-candidate"],
    ["bindings", "merman-bindings-core"],
  ]);
  assert.equal(validateCandidatePackageVersions(metadata), PACKAGE_VERSION);

  const staleCandidate = structuredClone(metadata);
  staleCandidate.packages.find((item) => item.name === "merman-node-candidate").version =
    "0.8.0-alpha.3";
  assert.throws(
    () => validateCandidatePackageVersions(staleCandidate),
    /merman-node-candidate.*0\.8\.0-alpha\.5/i,
  );

  const staleBindings = structuredClone(metadata);
  staleBindings.packages.find((item) => item.name === "merman-bindings-core").version =
    "0.8.0-alpha.3";
  assert.throws(
    () => validateCandidatePackageVersions(staleBindings),
    /merman-bindings-core.*0\.8\.0-alpha\.5/i,
  );
});

test("real locked Cargo metadata matches both candidate transport recipes", () => {
  for (const recipe of [
    resolveCandidateRecipe("node-wasm", null),
    resolveCandidateRecipe("napi", "linux-x64-gnu"),
  ]) {
    assert.equal(validateCandidateCargoMetadata(recipe), PACKAGE_VERSION);
  }
});

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

function metadataWithPackages(entries) {
  return {
    packages: entries.map(([id, name]) => ({
      id,
      name,
      version: name.startsWith("merman-") ? PACKAGE_VERSION : "1.0.0",
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
