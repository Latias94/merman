import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  buildWasmInputManifest,
  cargoMetadataForPreset,
  collectWasmInputEntries,
  rustcWasmInputPaths,
  verifyWasmInputManifest,
} from "./wasm-input-manifest.mjs";

const roots = [];
const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
);

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("WASM input manifest", () => {
  it("reads preset metadata through a lock-bound member-only probe", () => {
    const fixture = createFixture();
    const lockedRepositoryMetadata = {
      packages: [{ id: "merman-wasm" }],
      resolve: { root: "workspace", nodes: [] },
    };
    const expected = {
      packages: [
        {
          id: "merman-wasm-freshness-probe",
          name: "merman-wasm-freshness-probe",
          version: "0.0.0",
        },
        { id: "merman-wasm", name: "merman-wasm", version: "0.8.0-alpha.4" },
      ],
      resolve: { root: "merman-wasm-freshness-probe", nodes: [] },
    };
    let sourceObserved;
    let probeObserved;

    const metadata = cargoMetadataForPreset({
      preset: preset({ features: ["editor", "analysis"] }),
      repoRoot: fixture.repoRoot,
      capture(command, args, cwd) {
        if (!args.includes("--offline")) {
          sourceObserved = { command, args, cwd };
          return JSON.stringify(lockedRepositoryMetadata);
        }
        const manifestPath = args[args.indexOf("--manifest-path") + 1];
        const probeRoot = path.dirname(manifestPath);
        probeObserved = {
          command,
          args,
          cwd,
          lock: readFileSync(path.join(probeRoot, "Cargo.lock"), "utf8"),
          manifest: readFileSync(manifestPath, "utf8"),
          manifestPath,
        };
        return JSON.stringify(expected);
      },
    });

    assert.deepEqual(metadata, expected);
    assert.equal(sourceObserved.command, "cargo");
    assert.equal(sourceObserved.cwd, fixture.repoRoot);
    assert.deepEqual(sourceObserved.args, [
      "metadata",
      "--format-version",
      "1",
      "--locked",
      "--filter-platform",
      "wasm32-unknown-unknown",
      "--manifest-path",
      path.join(fixture.repoRoot, "Cargo.toml"),
    ]);
    assert.equal(probeObserved.command, "cargo");
    assert.equal(probeObserved.cwd, fixture.repoRoot);
    assert.notEqual(
      probeObserved.manifestPath,
      path.join(fixture.repoRoot, "crates", "merman-wasm", "Cargo.toml"),
    );
    assert.equal(probeObserved.lock, read(fixture, "Cargo.lock"));
    assert.match(probeObserved.manifest, /name = "merman-wasm-freshness-probe"/);
    assert.match(
      probeObserved.manifest,
      /merman-wasm = \{ path = .*default-features = false, features = \["analysis", "editor"\] \}/,
    );
    assert.deepEqual(probeObserved.args, [
      "metadata",
      "--format-version",
      "1",
      "--offline",
      "--filter-platform",
      "wasm32-unknown-unknown",
      "--manifest-path",
      probeObserved.manifestPath,
    ]);
  });

  it("reuses repository metadata across profile probes", () => {
    const fixture = createFixture();
    const repositoryMetadata = {
      packages: [{ id: "merman-wasm", name: "merman-wasm", version: "0.8.0-alpha.4" }],
      resolve: { root: "workspace", nodes: [] },
    };
    const expected = {
      packages: [
        {
          id: "merman-wasm-freshness-probe",
          name: "merman-wasm-freshness-probe",
          version: "0.0.0",
        },
        { id: "merman-wasm", name: "merman-wasm", version: "0.8.0-alpha.4" },
      ],
      resolve: { root: "merman-wasm-freshness-probe", nodes: [] },
    };
    let calls = 0;

    const metadata = cargoMetadataForPreset({
      preset: preset(),
      repoRoot: fixture.repoRoot,
      repositoryMetadata,
      capture(_command, args) {
        calls += 1;
        assert.equal(args.includes("--offline"), true);
        return JSON.stringify(expected);
      },
    });

    assert.deepEqual(metadata, expected);
    assert.equal(calls, 1);
  });

  it("rejects an offline probe package absent from the locked repository graph", () => {
    const fixture = createFixture();
    const lockedRepositoryMetadata = {
      packages: [{ id: "merman-wasm" }],
      resolve: { root: "workspace", nodes: [] },
    };
    const unlockedProbeMetadata = {
      packages: [
        { id: "probe", name: "probe", version: "0.0.0" },
        { id: "unlocked", name: "unlocked", version: "1.2.3" },
      ],
      resolve: { root: "probe", nodes: [] },
    };

    assert.throws(
      () =>
        cargoMetadataForPreset({
          preset: preset(),
          repoRoot: fixture.repoRoot,
          capture(_command, args) {
            return JSON.stringify(
              args.includes("--offline") ? unlockedProbeMetadata : lockedRepositoryMetadata,
            );
          },
        }),
      /resolution contains packages absent from the repository lock: unlocked@1\.2\.3/,
    );
  });

  it("invalidates canonical build inputs but ignores documentation", () => {
    const fixture = createFixture();
    const manifest = buildManifest(fixture);
    assert.deepEqual(buildManifest(fixture), manifest);
    assert.deepEqual(verify(fixture, manifest), { ok: true, reasons: [] });
    assert.deepEqual(manifest.preset.runtime_output_ids, ["svg"]);
    assert.equal(
      manifest.inputs.some(
        (entry) => entry.path === "crates/merman-core/assets/zenuml/actor.svg",
      ),
      true,
    );
    assert.equal(
      manifest.inputs.some(
        (entry) => entry.path === "crates/merman-core/assets/font-profile.bin",
      ),
      true,
    );

    write(fixture, "crates/merman-wasm/src/lib.rs", "pub fn changed() {}\n");
    assertInvalid(fixture, manifest, "crates/merman-wasm/src/lib.rs");

    write(fixture, "crates/merman-wasm/src/lib.rs", "pub fn render() {}\n");
    write(fixture, "crates/merman-core/assets/zenuml/actor.svg", "<svg>changed</svg>\n");
    assertInvalid(fixture, manifest, "crates/merman-core/assets/zenuml/actor.svg");
    write(fixture, "crates/merman-core/assets/zenuml/actor.svg", "<svg>actor</svg>\n");

    write(fixture, "crates/merman-core/assets/font-profile.bin", "changed bytes");
    assertInvalid(fixture, manifest, "crates/merman-core/assets/font-profile.bin");
    write(fixture, "crates/merman-core/assets/font-profile.bin", "font bytes");

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
      "contracts/abi/text-measurement-v1.json",
      "capabilities/artifact-profiles-v1.json",
      "capabilities/feature-surface-v1.json",
      "platforms/web/web-surface-descriptor.schema.json",
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
    const changedOutputs = buildManifest(fixture, {
      runtime_output_ids: ["ascii", "svg"],
    });
    assert.notEqual(changedOutputs.input_digest, manifest.input_digest);
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

  it("accepts package-declared inputs without parsing Rust macro syntax", () => {
    const fixture = createFixture();
    fixture.compilerInputs = [];
    write(
      fixture,
      "crates/merman-core/src/lib.rs",
      'pub const ACTOR: &str = include_str!(concat!("../assets/", "actor.svg"));\n',
    );

    assert.equal(
      buildManifest(fixture).inputs.some(
        (entry) => entry.path === "crates/merman-core/assets/zenuml/actor.svg",
      ),
      true,
    );
  });

  it("rejects embedded inputs whose parent symlink escapes the repository", () => {
    const fixture = createFixture();
    const external = mkdtempSync(path.join(os.tmpdir(), "merman-wasm-external-"));
    roots.push(external);
    writeFileSync(path.join(external, "secret.svg"), "<svg>external</svg>\n");
    const link = resolve(fixture, "crates/merman-core/assets/external");
    symlinkSync(external, link, process.platform === "win32" ? "junction" : "dir");
    write(
      fixture,
      "crates/merman-core/src/lib.rs",
      'pub const SECRET: &str = include_str!("../assets/external/secret.svg");\n',
    );
    fixture.compilerInputs.push(path.join(link, "secret.svg"));

    assert.throws(
      () => buildManifest(fixture),
      /symbolic link|resolves outside its repository root/i,
    );
  });

  it("captures root-crate compiler dep-info inputs outside Rust source trees", () => {
    const targetDirectory = mkdtempSync(path.join(os.tmpdir(), "merman-wasm-dep-info-"));
    roots.push(targetDirectory);
    const manifestPath = path.join(
      repositoryRoot,
      "crates",
      "merman-render",
      "Cargo.toml",
    );
    const metadata = {
      target_directory: targetDirectory,
      packages: [
        {
          id: "merman-render",
          manifest_path: manifestPath,
          source: null,
          targets: [
            {
              kind: ["lib"],
              src_path: path.join(repositoryRoot, "crates", "merman-render", "src", "lib.rs"),
            },
          ],
        },
      ],
      resolve: {
        root: "merman-render",
        nodes: [{ id: "merman-render", deps: [] }],
      },
    };
    const compilerInputs = [
      path.join(repositoryRoot, "playground/examples/manifest.json"),
      "/registry/example/src/lib.rs",
    ];
    const depInfoPath = path.join(
      targetDirectory,
      "wasm32-unknown-unknown",
      "wasm-size",
      "merman_wasm.d",
    );
    mkdirSync(path.dirname(depInfoPath), { recursive: true });
    writeFileSync(
      depInfoPath,
      `${escapeMakefilePath(path.join(targetDirectory, "merman_wasm.wasm"))}: ${compilerInputs.map(escapeMakefilePath).join(" ")}\n`,
    );
    const paths = new Set(
      collectWasmInputEntries({
        additionalInputs: rustcWasmInputPaths({ metadata, repoRoot: repositoryRoot }),
        metadata,
        repoRoot: repositoryRoot,
      }).map((entry) => entry.path),
    );

    assert.equal(
      paths.has("playground/examples/manifest.json"),
      true,
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
    runtime_output_ids: ["svg"],
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
        metadata: { merman: { "wasm-inputs": ["assets"] } },
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
    "contracts/abi/text-measurement-v1.json": "{\"protocol_version\":1}\n",
    "capabilities/artifact-profiles-v1.json": "{\"schema_version\":1}\n",
    "capabilities/feature-surface-v1.json": "{\"schema_version\":1}\n",
    "crates/merman-core/Cargo.toml": "[package]\nname = \"merman-core\"\n",
    "crates/merman-core/src/lib.rs": [
      'pub const ACTOR: &str = include_str!("../assets/zenuml/actor.svg");',
      'pub const FONT_PROFILE: &[u8] = include_bytes![r"../assets/font-profile.bin"];',
      '// include_str!(NOT_A_REAL_INPUT)',
      'pub const EXAMPLE: &str = "include_bytes!(NOT_A_REAL_INPUT)";',
      "pub fn is_different(include_str: usize) -> bool { include_str != 0 }",
      "",
    ].join("\n"),
    "crates/merman-core/assets/zenuml/actor.svg": "<svg>actor</svg>\n",
    "crates/merman-core/assets/font-profile.bin": "font bytes",
    "crates/merman-wasm/Cargo.toml": "[package]\nname = \"merman-wasm\"\n",
    "crates/merman-wasm/src/lib.rs": "pub fn render() {}\n",
    "crates/test-helper/Cargo.toml": "[package]\nname = \"test-helper\"\n",
    "crates/test-helper/src/lib.rs": "pub fn helper() {}\n",
    "platforms/web/web-surface-descriptor.json": "{\"schema_version\":3}\n",
    "platforms/web/web-surface-descriptor.schema.json": "{\"type\":\"object\"}\n",
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
  return {
    compilerInputs: [
      path.join(repoRoot, "crates/merman-core/assets/zenuml/actor.svg"),
      path.join(repoRoot, "crates/merman-core/assets/font-profile.bin"),
    ],
    metadata,
    outputRoot,
    packageRoot,
    repoRoot,
  };
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

function escapeMakefilePath(value) {
  return value
    .replaceAll("$", "$$")
    .replaceAll("\\", "\\\\")
    .replaceAll(" ", "\\ ")
    .replaceAll("#", "\\#")
    .replaceAll(":", "\\:");
}
