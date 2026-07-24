import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, it } from "node:test";

import {
  buildWasmInputManifest,
  verifyWasmInputManifest,
} from "./wasm-input-manifest.mjs";

const roots = [];

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("WASM input manifest", () => {
  it("invalidates canonical build inputs but ignores documentation", () => {
    const fixture = createFixture();
    const manifest = buildManifest(fixture);
    assert.deepEqual(buildManifest(fixture), manifest);
    assert.deepEqual(verify(fixture, manifest), { ok: true, reasons: [] });

    write(fixture, "crates/merman-wasm/src/lib.rs", "pub fn changed() {}\n");
    assertInvalid(fixture, manifest, "crates/merman-wasm/src/lib.rs");

    write(fixture, "crates/merman-wasm/src/lib.rs", "pub fn render() {}\n");
    write(fixture, "README.md", "documentation only\n");
    assert.deepEqual(verify(fixture, manifest), { ok: true, reasons: [] });

    write(fixture, "crates/test-helper/src/lib.rs", "pub fn changed_test_helper() {}\n");
    assert.deepEqual(verify(fixture, manifest), { ok: true, reasons: [] });

    write(
      fixture,
      "platforms/web/scripts/wasm-build/added-module.mjs",
      "export const added = true;\n",
    );
    assertInvalid(fixture, manifest, "added-module.mjs");
    rmSync(resolve(fixture, "platforms/web/scripts/wasm-build/added-module.mjs"));

    for (const changed of [
      "Cargo.toml",
      "Cargo.lock",
      "rust-toolchain.toml",
      "abi/text-measurement-v1.json",
      "capabilities/artifact-profiles-v1.json",
      "capabilities/feature-surface-v1.json",
      "platforms/web/web-surface-descriptor.json",
      "platforms/web/scripts/build-wasm.mjs",
      "platforms/web/scripts/wasm-build/input-manifest.mjs",
    ]) {
      const original = read(fixture, changed);
      write(fixture, changed, `${original.trimEnd()}\n# changed\n`);
      assertInvalid(fixture, manifest, changed);
      write(fixture, changed, original);
    }
  });

  it("invalidates additions, dependency sources, artifact profile features, and artifacts", () => {
    const fixture = createFixture();
    const manifest = buildManifest(fixture);

    write(fixture, "crates/merman-core/src/new_module.rs", "pub struct Added;\n");
    assertInvalid(fixture, manifest, "crates/merman-core/src/new_module.rs");
    rmSync(resolve(fixture, "crates/merman-core/src/new_module.rs"));

    const changedProfile = buildManifest(fixture, {
      features: ["editor", "layout-elk"],
    });
    assert.notEqual(changedProfile.input_digest, manifest.input_digest);
    const changedTool = buildManifest(fixture, {}, { rustc: "rustc 1.96.0" });
    assert.notEqual(changedTool.input_digest, manifest.input_digest);
    assertInvalid(
      fixture,
      manifest,
      "build tool versions",
      { rustc: "rustc 1.96.0" }
    );

    write(fixture, "platforms/web/pkg/full/merman_wasm_bg.wasm", "different bytes");
    assertInvalid(fixture, manifest, "artifact");
  });

  it("fails closed for sibling WASM artifacts", () => {
    const fixture = createFixture();
    const manifest = buildManifest(fixture);
    write(
      fixture,
      "platforms/web/pkg/full/core/merman_wasm_bg.wasm",
      "independent core surface"
    );
    assert.match(verify(fixture, manifest).reasons.join("\n"), /unowned.*core/i);
  });

  it("fails closed for missing, corrupt, or structurally stale manifests", () => {
    const fixture = createFixture();
    assert.match(
      verifyWasmInputManifest({
        ...fixture,
        manifest: null,
        preset: preset(),
        toolVersions: toolVersions(),
      }).reasons.join("\n"),
      /missing/i
    );
    assert.match(
      verifyWasmInputManifest({
        ...fixture,
        manifest: { schema_version: 99 },
        preset: preset(),
        toolVersions: toolVersions(),
      }).reasons.join("\n"),
      /schema/i
    );
  });
});

function buildManifest(fixture, overrides = {}, toolOverrides = {}) {
  return buildWasmInputManifest({
    ...fixture,
    preset: preset(overrides),
    toolVersions: toolVersions(toolOverrides),
  });
}

function verify(fixture, manifest, toolOverrides = {}) {
  return verifyWasmInputManifest({
    ...fixture,
    manifest,
    preset: preset(),
    toolVersions: toolVersions(toolOverrides),
  });
}

function assertInvalid(fixture, manifest, expected, toolOverrides = {}) {
  const result = verify(fixture, manifest, toolOverrides);
  assert.equal(result.ok, false);
  assert.match(result.reasons.join("\n"), new RegExp(escapeRegex(expected), "i"));
}

function toolVersions(overrides = {}) {
  return {
    cargo: "cargo 1.95.0",
    node: "v24.0.0",
    rustc: "rustc 1.95.0",
    wasm_pack: "wasm-pack 0.14.0",
    ...overrides,
  };
}

function preset(overrides = {}) {
  return {
    name: "web-full",
    surface: "web",
    default_features: false,
    features: ["editor"],
    runtime_capability_ids: ["analysis", "editor", "svg"],
    ...overrides,
  };
}

function createFixture() {
  const repoRoot = mkdtempSync(path.join(os.tmpdir(), "merman-wasm-inputs-"));
  roots.push(repoRoot);
  const packageRoot = path.join(repoRoot, "platforms", "web");
  const outputRoot = path.join(packageRoot, "pkg", "full");
  const metadata = {
    packages: [
      {
        id: "merman-wasm",
        name: "merman-wasm",
        manifest_path: path.join(repoRoot, "crates", "merman-wasm", "Cargo.toml"),
        source: null,
      },
      {
        id: "merman-core",
        name: "merman-core",
        manifest_path: path.join(repoRoot, "crates", "merman-core", "Cargo.toml"),
        source: null,
      },
      {
        id: "serde",
        name: "serde",
        manifest_path: "/registry/serde/Cargo.toml",
        source: "registry+https://github.com/rust-lang/crates.io-index",
      },
      {
        id: "test-helper",
        name: "test-helper",
        manifest_path: path.join(repoRoot, "crates", "test-helper", "Cargo.toml"),
        source: null,
      },
    ],
    resolve: {
      root: "merman-wasm",
      nodes: [
        {
          id: "merman-wasm",
          deps: [
            { pkg: "merman-core", dep_kinds: [{ kind: null, target: null }] },
            { pkg: "serde", dep_kinds: [{ kind: null, target: null }] },
            { pkg: "test-helper", dep_kinds: [{ kind: "dev", target: null }] },
          ],
        },
        {
          id: "merman-core",
          deps: [{ pkg: "serde", dep_kinds: [{ kind: null, target: null }] }],
        },
        { id: "serde", deps: [] },
        { id: "test-helper", deps: [] },
      ],
    },
  };

  for (const [relative, contents] of Object.entries({
    "Cargo.toml": "[workspace]\nmembers = [\"crates/*\"]\n",
    "Cargo.lock": "version = 4\n",
    "rust-toolchain.toml": "[toolchain]\nchannel = \"1.95.0\"\n",
    "README.md": "initial docs\n",
    "abi/text-measurement-v1.json": "{\"protocol_version\":1}\n",
    "capabilities/artifact-profiles-v1.json": "{\"schema_version\":1}\n",
    "capabilities/feature-surface-v1.json": "{\"schema_version\":1}\n",
    "crates/merman-core/Cargo.toml": "[package]\nname = \"merman-core\"\n",
    "crates/merman-core/src/lib.rs": "pub struct Core;\n",
    "crates/merman-wasm/Cargo.toml": "[package]\nname = \"merman-wasm\"\n",
    "crates/merman-wasm/src/lib.rs": "pub fn render() {}\n",
    "crates/test-helper/Cargo.toml": "[package]\nname = \"test-helper\"\n",
    "crates/test-helper/src/lib.rs": "pub fn helper() {}\n",
    "platforms/web/web-surface-descriptor.json": "{\"schema_version\":3}\n",
    "platforms/web/scripts/build-wasm.mjs": "// build\n",
    "platforms/web/scripts/wasm-build/input-manifest.mjs": "// manifest\n",
    "platforms/web/scripts/wasm-build/new-owned-module.mjs": "// owned\n",
    "platforms/web/pkg/full/merman_wasm.js": "export default {};\n",
    "platforms/web/pkg/full/merman_wasm.d.ts": "export default function init(): void;\n",
    "platforms/web/pkg/full/merman_wasm_bg.wasm": "wasm bytes",
    "platforms/web/pkg/full/merman_wasm_bg.wasm.d.ts": "export const memory: WebAssembly.Memory;\n",
    "platforms/web/pkg/full/merman_wasm_artifact_profile.json": "{\"artifact_profile\":\"web-full\"}\n",
    "platforms/web/pkg/full/package.json": "{\"type\":\"module\"}\n",
  })) {
    write({ repoRoot }, relative, contents);
  }
  return { metadata, outputRoot, packageRoot, repoRoot };
}

function resolve(fixture, relative) {
  return path.join(fixture.repoRoot, ...relative.split("/"));
}

function write(fixture, relative, contents) {
  const target = resolve(fixture, relative);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, contents);
}

function read(fixture, relative) {
  return readFileSync(resolve(fixture, relative), "utf8");
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
