import assert from "node:assert/strict";
import {
  cpSync,
  mkdtempSync,
  mkdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import {
  assertArtifactFileEvidence,
  assertIndependentSizeEvidence,
  assertLegalProjection,
  assertPackageManifest,
} from "./prepack-check.mjs";
import { legalProjectionForArtifactProfile } from "./legal-projection.mjs";
import {
  WASM_RUNTIME_TOP_LEVEL_FILES,
  packageDistFileRecords,
  wasmRuntimeFileRecords,
} from "./wasm-runtime-files.mjs";

test("complete SVG renderer is admitted by capability boundary instead of slim-size threshold", () => {
  const checked = [
    { descriptor: { id: "full" }, packageBytes: 100 },
    { descriptor: { id: "render" }, packageBytes: 95 },
  ];

  assert.doesNotThrow(() => assertIndependentSizeEvidence(checked, checked));
  assert.throws(
    () =>
      assertIndependentSizeEvidence(
        [...checked, { descriptor: { id: "editor" }, packageBytes: 95 }],
        [...checked, { descriptor: { id: "editor" }, packageBytes: 95 }],
      ),
    /at least 15%/,
  );
});

test("package provenance rejects stale copied WASM and entry wrappers", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-provenance-"));
  try {
    const sourceWasmRoot = path.join(root, "pkg", "analysis");
    const packageWasmRoot = path.join(root, "packages", "analysis", "artifacts", "wasm");
    const sourceDistRoot = path.join(root, "dist");
    const packageDistRoot = path.join(root, "packages", "analysis", "dist");
    writeRuntime(sourceWasmRoot);
    cpSync(sourceWasmRoot, packageWasmRoot, { recursive: true });
    writeEntryFiles(sourceDistRoot, "analysis");
    cpSync(sourceDistRoot, packageDistRoot, { recursive: true });

    const artifactFiles = [
      ...wasmRuntimeFileRecords(sourceWasmRoot),
      ...packageDistFileRecords(sourceDistRoot, "analysis"),
    ].sort(compareArtifactRecords);
    const evidence = {
      packageWasmRoot,
      sourceWasmRoot,
      packageDistRoot,
      sourceDistRoot,
      packageId: "analysis",
      artifactFiles,
      label: "@mermanjs/web-analysis",
    };

    assert.doesNotThrow(() => assertArtifactFileEvidence(evidence));

    writeFileSync(path.join(packageWasmRoot, "merman_wasm_bg.wasm"), "stale wasm");
    assert.throws(
      () => assertArtifactFileEvidence(evidence),
      /copied package artifacts do not match their provenance evidence/,
    );

    cpSync(sourceWasmRoot, packageWasmRoot, { recursive: true, force: true });
    writeFileSync(
      path.join(packageDistRoot, "package-entries", "analysis.js"),
      [
        'import "../runtime-core.js";',
        'import("../../artifacts/wasm/merman_wasm.js");',
        "export const value = false;",
        "",
      ].join("\n"),
    );
    assert.throws(
      () => assertArtifactFileEvidence(evidence),
      /copied package artifacts do not match their provenance evidence/,
    );

    cpSync(sourceDistRoot, packageDistRoot, { recursive: true, force: true });
    writeFileSync(
      path.join(packageDistRoot, "editor-semantic-tokens.js"),
      "export const stale = true;\n",
    );
    assert.throws(
      () => assertArtifactFileEvidence(evidence),
      /differs from the analysis static module closure/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("package manifest closes npm files and lifecycle hooks", () => {
  const descriptor = {
    id: "analysis",
    name: "@mermanjs/web-analysis",
    visibility: "public",
    artifact_profile: { id: "web-analysis" },
  };
  const manifest = {
    name: descriptor.name,
    license: "MIT OR Apache-2.0",
    files: [
      "artifacts",
      "dist",
      "LICENSE",
      "README.md",
      "THIRD_PARTY_LICENSES",
      "THIRD_PARTY_NOTICES.md",
    ],
    main: "./dist/package-entries/analysis.js",
    types: "./dist/package-entries/analysis.d.ts",
    exports: {
      ".": {
        import: "./dist/package-entries/analysis.js",
        types: "./dist/package-entries/analysis.d.ts",
      },
    },
    merman: { artifact_profile: "web-analysis" },
    publishConfig: { access: "public" },
  };

  assert.doesNotThrow(() => assertPackageManifest(descriptor, manifest));
  assert.throws(
    () => assertPackageManifest(descriptor, { ...manifest, files: [...manifest.files, "secret.txt"] }),
    /closed package files allowlist/,
  );
  assert.throws(
    () => assertPackageManifest(descriptor, { ...manifest, scripts: { postinstall: "node install.js" } }),
    /must not declare npm lifecycle scripts/,
  );
  assert.throws(
    () =>
      assertPackageManifest(descriptor, {
        ...manifest,
        publishConfig: { access: "public", registry: "https://attacker.invalid" },
      }),
    /must declare only publishConfig\.access=public/,
  );
  assert.throws(
    () => assertPackageManifest(descriptor, { ...manifest, bundleDependencies: ["unexpected"] }),
    /must not declare bundled npm dependencies/,
  );
});

test("package legal material is exactly the artifact-profile closure", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-legal-"));
  const descriptor = {
    id: "analysis",
    name: "@mermanjs/web-analysis",
    artifact_profile: { id: "web-analysis" },
  };
  const packageRoot = path.join(root, "analysis");
  try {
    const projection = legalProjectionForArtifactProfile(
      descriptor.artifact_profile.id,
    );
    const legalRoot = path.join(packageRoot, "THIRD_PARTY_LICENSES");
    mkdirSync(legalRoot, { recursive: true });
    writeFileSync(
      path.join(packageRoot, "THIRD_PARTY_NOTICES.md"),
      projection.notice,
    );
    for (const file of projection.files) {
      const target = path.join(legalRoot, ...file.relative.split("/"));
      mkdirSync(path.dirname(target), { recursive: true });
      cpSync(file.source, target);
    }

    assert.doesNotThrow(() => assertLegalProjection(descriptor, packageRoot));

    writeFileSync(
      path.join(legalRoot, "rust-cargo-dependencies.json"),
      "{}\n",
    );
    assert.throws(
      () => assertLegalProjection(descriptor, packageRoot),
      /Legal projection is stale/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("package provenance rejects files outside the static module closure", () => {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-web-source-maps-"));
  try {
    const packageDistRoot = path.join(root, "packages", "analysis", "dist");
    writeEntryFiles(packageDistRoot, "analysis");
    writeFileSync(path.join(packageDistRoot, "index.js.map"), "{}\n");
    assert.throws(
      () => packageDistFileRecords(packageDistRoot, "analysis"),
      /differs from the analysis static module closure/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function writeRuntime(root) {
  mkdirSync(root, { recursive: true });
  for (const name of WASM_RUNTIME_TOP_LEVEL_FILES) {
    writeFileSync(path.join(root, name), `runtime:${name}`);
  }
}

function writeEntryFiles(root, packageId) {
  const entryRoot = path.join(root, "package-entries");
  mkdirSync(entryRoot, { recursive: true });
  writeFileSync(
    path.join(entryRoot, `${packageId}.js`),
    [
      'import "../runtime-core.js";',
      'import("../../artifacts/wasm/merman_wasm.js");',
      "export const value = true;",
      "",
    ].join("\n"),
  );
  writeFileSync(
    path.join(entryRoot, `${packageId}.d.ts`),
    'export type { RuntimeValue } from "../runtime-core.js";\n',
  );
  writeFileSync(path.join(root, "runtime-core.js"), "export const core = true;\n");
  writeFileSync(
    path.join(root, "runtime-core.d.ts"),
    "export interface RuntimeValue { readonly value: true }\n",
  );
  for (const file of [
    path.join(entryRoot, `${packageId}.d.ts.map`),
    path.join(entryRoot, `${packageId}.js.map`),
    path.join(root, "runtime-core.d.ts.map"),
    path.join(root, "runtime-core.js.map"),
  ]) {
    writeFileSync(file, "{}\n");
  }
}

function compareArtifactRecords(left, right) {
  if (left.path < right.path) return -1;
  if (left.path > right.path) return 1;
  return 0;
}
