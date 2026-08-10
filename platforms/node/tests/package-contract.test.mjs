import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  inspectPackageManifests,
  verifyPackedFileOwnership,
} from "../scripts/package-contract.mjs";
import { assembleNativePackages } from "../scripts/assemble-packages.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

test("public alpha manifests preserve the intended napi package shape", async () => {
  const descriptor = JSON.parse(
    await readFile(path.join(nodeRoot, "package-surfaces.json"), "utf8"),
  );
  const inspected = await inspectPackageManifests(nodeRoot, descriptor);

  assert.equal(descriptor.admission_status, "public-alpha");
  assert.equal(inspected.root.manifest.name, "@mermanjs/node");
  assert.equal(inspected.root.manifest.private, undefined);
  assert.equal(inspected.root.manifest.publishConfig.access, "public");
  assert.deepEqual(inspected.root.nodeFiles, []);
  assert.deepEqual(inspected.root.wasmFiles, []);
  assert.equal(inspected.root.hasLifecycleDownload, false);
  assert.equal(inspected.root.hasBrowserFallback, false);
  assert.equal(inspected.root.manifest.engines.node, descriptor.node_engine);

  const expectedTargets = [
    "darwin-arm64",
    "darwin-x64",
    "linux-x64-gnu",
    "linux-x64-musl",
    "win32-x64-msvc",
  ];
  assert.deepEqual(
    inspected.targets.map(({ target }) => target),
    expectedTargets,
  );
  for (const item of inspected.targets) {
    assert.equal(item.manifest.private, undefined);
    assert.equal(item.manifest.publishConfig.access, "public");
    assert.equal(item.manifest.engines.node, descriptor.node_engine);
    assert.equal(item.nodeArtifact, "merman.node");
    assert.equal(item.manifest.version, descriptor.version);
    assert.equal(item.manifest.files.includes("build-receipt.json"), false);
    assert.equal(
      inspected.root.manifest.optionalDependencies[item.manifest.name],
      descriptor.version,
    );
  }
});

test("packed ownership allows no native binary in root and exactly one in a target package", () => {
  assert.doesNotThrow(() =>
    verifyPackedFileOwnership({
      packageName: "@mermanjs/node",
      role: "loader",
      files: [
        { path: "package/dist/index.mjs" },
        { path: "package/dist/index.d.ts" },
        { path: "package/package.json" },
      ],
    }),
  );
  assert.doesNotThrow(() =>
    verifyPackedFileOwnership({
      packageName: "@mermanjs/node-linux-x64-gnu",
      role: "platform",
      files: [
        { path: "package/merman.node" },
        { path: "package/package.json" },
      ],
    }),
  );
  assert.throws(
    () =>
      verifyPackedFileOwnership({
        packageName: "@mermanjs/node",
        role: "loader",
        files: [{ path: "package/merman.node" }],
      }),
    /must not contain native binaries/i,
  );
  assert.throws(
    () =>
      verifyPackedFileOwnership({
        packageName: "@mermanjs/node",
        role: "loader",
        files: [{ path: "package/merman_node_bg.wasm" }],
      }),
    /must not contain WASM binaries/i,
  );
  assert.throws(
    () =>
      verifyPackedFileOwnership({
        packageName: "@mermanjs/node-linux-x64-gnu",
        role: "platform",
        files: [
          { path: "package/merman.node" },
          { path: "package/other.node" },
        ],
      }),
    /exactly one \.node/i,
  );
  assert.throws(
    () =>
      verifyPackedFileOwnership({
        packageName: "@mermanjs/node-linux-x64-gnu",
        role: "platform",
        files: [
          { path: "package/merman.node" },
          { path: "package/merman_node_bg.wasm" },
        ],
      }),
    /must not contain WASM binaries/i,
  );
});

test("candidate recipes pin the approved napi baseline and an explicit Node WASM target", async () => {
  const recipes = JSON.parse(
    await readFile(path.join(nodeRoot, "candidate-builds.json"), "utf8"),
  );
  assert.equal(recipes.schema_version, 3);
  assert.equal(recipes.status, "napi-selected-for-alpha");
  assert.deepEqual(recipes.capability_recipe, {
    descriptor: "capabilities/feature-surface-v1.json",
    target: "native",
    capabilities: ["layout-cytoscape", "layout-elk", "svg"],
  });
  assert.equal(recipes.cargo.default_features, false);
  assert.equal("features" in recipes.cargo, false);
  assert.deepEqual(recipes.candidates.napi.versions, {
    napi: "3.11.0",
    napi_derive: "3.6.0",
    napi_build: "2.3.2",
    napi_cli: "3.7.4",
  });
  assert.equal(recipes.candidates["node-wasm"].wasm_pack_target, "nodejs");
  assert.equal(recipes.candidates["node-wasm"].browser_package_reuse, false);
  assert.equal(recipes.candidates.napi.postinstall_download, false);
  assert.equal(recipes.candidates.napi.aggregate_all_targets, false);
});

test("assembled native packages pass real npm pack ownership inspection", async () => {
  const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), "merman-node-pack-test-"));
  try {
    const descriptor = JSON.parse(
      await readFile(path.join(nodeRoot, "package-surfaces.json"), "utf8"),
    );
    const binary = path.join(temporaryRoot, "candidate.node");
    const output = path.join(temporaryRoot, "packages");
    const preflightCalls = [];
    await writeFile(binary, "synthetic native candidate");
    assembleNativePackages("darwin-arm64", output, binary, {
      readReceipt(artifact) {
        preflightCalls.push(artifact);
        return { candidate: "napi", target: "darwin-arm64" };
      },
    });
    assert.deepEqual(preflightCalls, [binary]);

    const rootPack = npmPackDryRun(path.join(output, "node"));
    const targetPack = npmPackDryRun(path.join(output, "darwin-arm64"));
    verifyPackedFileOwnership({
      packageName: "@mermanjs/node",
      role: "loader",
      files: rootPack.files,
    });
    verifyPackedFileOwnership({
      packageName: "@mermanjs/node-darwin-arm64",
      role: "platform",
      files: targetPack.files,
    });
    assert.equal(
      rootPack.files.some((item) => item.path === "dist/generated/capability-surface.mjs"),
      true,
    );
    assert.equal(
      rootPack.files.some((item) => item.path === "dist/generated/binding-contract.mjs"),
      true,
    );
    assert.equal(
      rootPack.files.some((item) => item.path === "dist/generated/node-wire-contract.json"),
      true,
    );
    assert.equal(
      rootPack.files.some((item) => item.path === "dist/transport-contract.mjs"),
      true,
    );
    assert.equal(targetPack.files.some((item) => item.path === "build-receipt.json"), false);

    const assembledLoader = await import(
      `${pathToFileURL(path.join(output, "node", "dist", "native-loader.mjs")).href}?assembled`
    );
    assert.equal(assembledLoader.nodeLoaderPackageVersion(), descriptor.version);
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("native package assembly requires canonical candidate and target provenance", async (context) => {
  const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), "merman-node-provenance-test-"));
  context.after(() => rm(temporaryRoot, { recursive: true, force: true }));
  const binary = path.join(temporaryRoot, "merman.node");
  const output = path.join(temporaryRoot, "packages");
  await writeFile(binary, "synthetic native candidate");

  assert.throws(
    () => assembleNativePackages("darwin-arm64", output, binary),
    /missing.*candidate build receipt/i,
  );

  for (const [receipt, expected] of [
    [{ candidate: "node-wasm", target: null }, /candidate.*napi/i],
    [{ candidate: "napi", target: "darwin-x64" }, /target.*darwin-arm64/i],
  ]) {
    assert.throws(
      () =>
        assembleNativePackages("darwin-arm64", output, binary, {
          readReceipt: () => receipt,
        }),
      expected,
    );
  }

  const staleSource = new Error(
    "candidate build receipt source_digest is stale for the current source tree.",
  );
  assert.throws(
    () =>
      assembleNativePackages("darwin-arm64", output, binary, {
        readReceipt() {
          throw staleSource;
        },
      }),
    (error) => error === staleSource,
  );
});

function npmPackDryRun(packageRoot) {
  const result = spawnSync("npm", ["pack", "--json", "--dry-run"], {
    cwd: packageRoot,
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout)[0];
}
