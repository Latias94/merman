import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  candidateArtifactDirectory,
  candidateDependencyClosure,
  candidateBuildInvocation,
  collectLocalInputEntries,
  resolveCandidateRuntimeContract,
  resolveCandidateRecipe,
  validateCandidateBuildHost,
  validateCandidateCargoMetadata,
  validateCandidatePackageVersions,
} from "../scripts/build-candidate.mjs";
import { NODE_WIRE_CONTRACT } from "../src/errors.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const PACKAGE_VERSION = JSON.parse(
  await readFile(path.join(nodeRoot, "package-surfaces.json"), "utf8"),
).version;

test("candidate builds project diagram selection, capabilities, and one transport", async () => {
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
    assert.deepEqual(recipe.diagramFamilies, descriptor.expected.diagram_families);
    assert.equal(recipe.capabilityFeatures.includes("all-diagrams"), false);
    assert.equal(recipe.capabilityFeatures.includes("math"), true);
    assert.deepEqual(
      invocation.args[featureIndex + 1].split(","),
      [...descriptor.cargo.features, ...descriptor.capability_recipe.capabilities, item.transportFeature].sort(),
    );
    assert.equal(invocation.args.includes("--no-default-features"), true);
    assert.equal(invocation.args.includes("--locked"), true);
    assert.equal(invocation.args.includes("-j1"), true);
    assert.equal(invocation.args.join(" ").includes("rust-static-svg"), false);
    if (item.candidate === "napi") {
      assert.equal(recipe.cargoProfile, "native-sdk");
      assert.equal(invocation.args[invocation.args.indexOf("--profile") + 1], "native-sdk");
      assert.equal(invocation.args[invocation.args.indexOf("--package") + 1], "merman-node-candidate");
      assert.equal(invocation.args.includes("--release"), false);
    } else {
      assert.equal(recipe.cargoProfile, "release");
      assert.equal(invocation.args.includes("--release"), true);
      assert.equal(invocation.args.includes("--profile"), false);
    }
  }
  assert.equal(resolveCandidateRecipe("napi", "linux-x64-gnu").glibcFloor, "2.31");
  assert.equal(resolveCandidateRecipe("napi", "linux-x64-musl").glibcFloor, null);
  assert.equal(resolveCandidateRecipe("napi", "linux-arm64-gnu").glibcFloor, "2.31");
  assert.equal(resolveCandidateRecipe("napi", "linux-arm64-gnu").rustTarget, "aarch64-unknown-linux-gnu");
  assert.equal(resolveCandidateRecipe("napi", "linux-arm64-musl").rustTarget, "aarch64-unknown-linux-musl");
});

test("each local Shoal recipe adds one feature and keeps its transport and profile", () => {
  const baselineContract = resolveCandidateRuntimeContract();
  for (const [candidate, target] of [["napi", "darwin-arm64"], ["node-wasm", null]]) {
    const baseline = resolveCandidateRecipe(candidate, target);
    const local = resolveCandidateRecipe(candidate, target, { localShoal: true });
    const baselineBuild = candidateBuildInvocation(baseline, "/tmp/merman-node-candidate");
    const localBuild = candidateBuildInvocation(local, "/tmp/merman-node-candidate");
    assert.equal(local.cargoProfile, baseline.cargoProfile);
    assert.deepEqual(local.cargoFeatures, [...baseline.cargoFeatures, "layout-shoal"].sort());
    const expectedArgs = [...baselineBuild.args];
    expectedArgs[expectedArgs.indexOf("--features") + 1] = local.cargoFeatures.join(",");
    assert.deepEqual(localBuild, { ...baselineBuild, args: expectedArgs });
    assert.deepEqual(resolveCandidateRuntimeContract(local), {
      ...baselineContract,
      capabilityRecipe: {
        ...baselineContract.capabilityRecipe,
        capabilities: local.capabilityFeatures,
      },
      capabilityIds: local.capabilityFeatures,
    });
    assert.equal(candidateArtifactDirectory(local), path.join(
      repositoryRoot, "target", "bench", "node-shoal", "artifacts", target ?? candidate,
    ));
    assert.notEqual(candidateArtifactDirectory(local), candidateArtifactDirectory(baseline));
  }
});

test("full candidate parser families match the core artifact contract", async () => {
  const profiles = await readJson(path.join(repositoryRoot, "capabilities/artifact-profiles-v2.json"));
  const core = profiles.profiles.find((profile) => profile.id === "rust-core");
  const contract = resolveCandidateRuntimeContract();
  assert.deepEqual(contract.diagramFamilies, core.expected.diagram_families);
  assert.equal(contract.capabilityIds.includes("all-diagrams"), false);
  assert.equal(contract.diagramFamilies.includes("error"), false);
});

test("Windows native candidates request reproducible MSVC linking", () => {
  const windows = candidateBuildInvocation(
    resolveCandidateRecipe("napi", "win32-x64-msvc"),
    "/tmp/merman-node-candidate",
  );
  const configIndex = windows.args.indexOf("--config");

  assert.notEqual(configIndex, -1);
  assert.equal(
    windows.args[configIndex + 1],
    'target.x86_64-pc-windows-msvc.rustflags=["-C","link-arg=/Brepro"]',
  );

  const darwin = candidateBuildInvocation(
    resolveCandidateRecipe("napi", "darwin-arm64"),
    "/tmp/merman-node-candidate",
  );
  assert.equal(darwin.args.includes("/Brepro"), false);
  assert.equal(darwin.args.includes("--config"), false);
});

test("native candidate builds require their matching runtime host", () => {
  const recipe = resolveCandidateRecipe("napi", "linux-x64-musl");

  assert.doesNotThrow(() =>
    validateCandidateBuildHost(recipe, "linux-x64-musl")
  );
  assert.throws(
    () => validateCandidateBuildHost(recipe, "linux-x64-gnu"),
    /matching runtime host/i,
  );
  for (const target of ["linux-arm64-gnu", "linux-arm64-musl"]) {
    const arm64 = resolveCandidateRecipe("napi", target);
    assert.doesNotThrow(() => validateCandidateBuildHost(arm64, target));
    assert.throws(() => validateCandidateBuildHost(arm64, target.replace("arm64", "x64")),
      /matching runtime host/i);
  }
});

test("native ARM64 musl uses the host linker and preserves an explicit override", () => {
  const recipe = resolveCandidateRecipe("napi", "linux-arm64-musl");
  const key = "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER";
  const native = candidateBuildInvocation(recipe, "/tmp/merman-node-candidate", {});
  assert.equal(native.env?.[key], "cc");
  const configured = candidateBuildInvocation(recipe, "/tmp/merman-node-candidate", {
    [key]: "/opt/toolchain/bin/custom-linker",
  });
  assert.equal(configured.env?.[key], "/opt/toolchain/bin/custom-linker");
  const darwin = candidateBuildInvocation(
    resolveCandidateRecipe("napi", "darwin-arm64"), "/tmp/merman-node-candidate", {},
  );
  assert.equal(darwin.env?.[key], undefined);
});

test("candidate runtime outputs follow explicit binding operation ownership", () => {
  const contract = resolveCandidateRuntimeContract();

  assert.deepEqual(contract.capabilityIds, NODE_WIRE_CONTRACT.artifact.capability_ids);
  assert.deepEqual(contract.operationIds, [
    "export-theme-preset",
    "layout-json",
    "resolve-theme",
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
    ...descriptor.cargo.features,
    ...descriptor.capability_recipe.capabilities,
    "layout-shoal",
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

test("workspace dependency evidence follows the selected tree and rejects transport or Shoal drift", () => {
  const metadata = metadataWithPackages([
    ["root", "merman-node-candidate"],
    ["bindings", "merman-bindings-core"],
    ["napi", "napi"],
    ["napi-build", "napi-build"],
    ["napi-derive", "napi-derive"],
    ["wasm-bindgen", "wasm-bindgen"],
    ["shoal", "merman-layout-shoal"],
    ["unrelated", "merman-cli"],
  ]);
  const tree = (...ids) => ids.map((id) => {
    const { name, version } = metadata.packages.find((item) => item.id === id);
    return `${name} v${version}`;
  }).join("\n");
  const closure = (recipe, ...ids) => candidateDependencyClosure(metadata, tree(...ids), recipe);
  const napi = resolveCandidateRecipe("napi", "darwin-arm64");
  const wasm = resolveCandidateRecipe("node-wasm", null);
  const napiTree = ["root", "bindings", "napi", "napi-build", "napi-derive"];
  const wasmTree = ["root", "bindings", "wasm-bindgen"];
  const baseline = closure(napi, ...napiTree, "bindings");
  assert.deepEqual(baseline.packages.map((item) => item.name), [
    "merman-bindings-core", "merman-node-candidate", "napi", "napi-build", "napi-derive",
  ]);
  assert.ok(collectLocalInputEntries(baseline).some(({ path }) => path === "crates/merman-node/src/wire.rs"));
  assert.deepEqual(closure(wasm, ...wasmTree).packages.map((item) => item.name), [
    "merman-bindings-core", "merman-node-candidate", "wasm-bindgen",
  ]);
  for (const [recipe, baselineTree] of [[napi, napiTree], [wasm, wasmTree]]) {
    const local = resolveCandidateRecipe(recipe.candidate, recipe.targetId, { localShoal: true });
    assert.ok(closure(local, ...baselineTree, "shoal").packages.some((item) => item.name === "merman-layout-shoal"));
    assert.throws(() => closure(recipe, ...baselineTree, "shoal"), /contain Shoal only/);
    assert.throws(() => closure(local, ...baselineTree), /contain Shoal only/);
  }
  assert.throws(() => closure(wasm, ...wasmTree, "napi-build"), /napi-build.*wasm/i);
  assert.throws(() => closure(napi, "root", "bindings", "napi", "napi-derive"), /napi-build.*napi/i);
  assert.throws(() => closure(napi, "unrelated", ...napiTree), /tree must start at merman-node-candidate/i);
});

test("candidate source receipt covers runtime wrappers and generated package contracts", async () => {
  const entries = collectLocalInputEntries({ packages: [] });
  for (const relativePath of [
    "Cargo.lock",
    "platforms/node/src/candidates/native.mjs",
    "platforms/node/src/candidates/wasm.mjs",
    "platforms/node/src/candidates/wrap-engine.mjs",
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
  const escapedPackageVersion = PACKAGE_VERSION.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
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
    new RegExp(`merman-node-candidate.*${escapedPackageVersion}`, "i"),
  );

  const staleBindings = structuredClone(metadata);
  staleBindings.packages.find((item) => item.name === "merman-bindings-core").version =
    "0.8.0-alpha.3";
  assert.throws(
    () => validateCandidatePackageVersions(staleBindings),
    new RegExp(`merman-bindings-core.*${escapedPackageVersion}`, "i"),
  );
  assert.throws(
    () => validateCandidatePackageVersions({ ...metadata, workspace_members: [] }),
    /root Cargo workspace and lockfile/i,
  );
});

test("real locked Cargo metadata and tree match the default and local candidate recipes", () => {
  for (const recipe of [
    resolveCandidateRecipe("node-wasm", null),
    resolveCandidateRecipe("napi", "linux-x64-gnu"),
    resolveCandidateRecipe("napi", "linux-x64-gnu", { localShoal: true }),
    resolveCandidateRecipe("node-wasm", null, { localShoal: true }),
  ]) {
    assert.equal(validateCandidateCargoMetadata(recipe), PACKAGE_VERSION);
  }
});

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

function metadataWithPackages(entries) {
  return {
    workspace_root: repositoryRoot,
    workspace_members: ["root", "bindings"],
    packages: entries.map(([id, name]) => ({
      id,
      name,
      version: name.startsWith("merman-") ? PACKAGE_VERSION : "1.0.0",
      source: name.startsWith("merman-") ? null : "registry+test",
      manifest_path: path.join(repositoryRoot, "crates", name === "merman-node-candidate" ? "merman-node" : name, "Cargo.toml"),
    })),
  };
}

function declaredCargoFeatures(manifest) {
  manifest = manifest.replaceAll("\r\n", "\n");
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
