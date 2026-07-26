import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { legalProjectionForArtifactProfile } from "./legal-projection.mjs";
import { webPackages } from "./surface-manifest.mjs";
import { WASM_INPUT_MANIFEST_NAME } from "./wasm-build/input-manifest.mjs";
import {
  WASM_RUNTIME_TOP_LEVEL_FILES,
  packageDistFileRecords,
  wasmRuntimeFileRecords,
} from "./wasm-runtime-files.mjs";

const webRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoot = path.join(webRoot, "src");
const entriesRoot = path.join(sourceRoot, "package-entries");
const packageBuildRoot = path.join(webRoot, "pkg");
const distRoot = path.join(webRoot, "dist");

if (isMainModule()) {
  const phase = process.argv[2];
  if (phase === "--entries") {
    generatePackageEntries();
  } else if (phase === "--assemble") {
    assemblePackageArtifacts();
  } else {
    console.error("usage: node scripts/build-surface-packages.mjs --entries|--assemble");
    process.exitCode = 2;
  }
}

export function generatePackageEntries() {
  const stage = siblingStage(entriesRoot, "entries");
  const backup = siblingStage(entriesRoot, "entries-backup");
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(stage, { recursive: true });
  try {
    for (const descriptor of webPackages) {
      writeFileSync(path.join(stage, `${descriptor.id}.ts`), packageEntrySource(descriptor));
    }
    replaceDirectory({ target: entriesRoot, stage, backup });
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }
}

export function assemblePackageArtifacts() {
  assertFile(path.join(distRoot, "index.js"), "compiled TypeScript output");
  assertFile(path.join(distRoot, "package-entries", "full.js"), "compiled package entry");
  for (const descriptor of webPackages) {
    assemblePackageArtifact(descriptor);
  }
}

export function packageEntrySource(descriptor) {
  const surfaceRuntimeKeys = [
    "initMerman",
    ...descriptor.runtimeExportNames.filter((name) => name !== "initMerman"),
  ];
  return [
    'import { assertBrowserRuntime, bindSurfaceRuntime, type SurfaceRuntime } from "../surface-runtime.js";',
    "import type {",
    "  MermanInitInput as SharedMermanInitInput,",
    "  MermanInitOptions as SharedMermanInitOptions,",
    "  MermanWasmLoader as SharedMermanWasmLoader,",
    "  MermanWasmModule as SharedMermanWasmModule,",
    '} from "../index.js";',
    'export type * from "../index.js";',
    "",
    "export type MermanWasmModule = Required<Pick<SharedMermanWasmModule,",
    ...descriptor.wasmExportNames.map((name) => `  | ${JSON.stringify(name)}`),
    ">>;",
    "export type MermanWasmLoader = SharedMermanWasmLoader<MermanWasmModule>;",
    "export type MermanInitOptions = SharedMermanInitOptions<MermanWasmModule>;",
    "export type MermanInitInput = SharedMermanInitInput<MermanWasmModule>;",
    "export {",
    ...descriptor.valueExportNames.map((name) => `  ${name},`),
    '} from "../index.js";',
    "",
    "export const MERMAN_WASM_URL = new URL(\"../../artifacts/wasm/merman_wasm_bg.wasm\", import.meta.url).href;",
    "",
    "export function loadMermanWasmModule(): Promise<MermanWasmModule> {",
    "  assertBrowserRuntime();",
    "  // @ts-ignore -- wasm-bindgen output is assembled after TypeScript compilation.",
    '  return import("../../artifacts/wasm/merman_wasm.js");',
    "}",
    "",
    "const runtime: Pick<SurfaceRuntime<MermanWasmModule>,",
    ...surfaceRuntimeKeys.map((name) => `  | ${JSON.stringify(name)}`),
    "> = bindSurfaceRuntime(loadMermanWasmModule);",
    "",
    "export function initMerman(init?: MermanInitInput): Promise<MermanWasmModule> {",
    "  assertBrowserRuntime();",
    "  return runtime.initMerman(init);",
    "}",
    "",
    ...descriptor.runtimeExportNames
      .filter((name) => name !== "initMerman")
      .map(
        (name) =>
          `export const ${name}: SurfaceRuntime<MermanWasmModule>[${JSON.stringify(name)}] = runtime.${name};`,
      ),
    "",
  ].join("\n");
}

function assemblePackageArtifact(descriptor) {
  const packageRoot = path.join(webRoot, descriptor.package_dir);
  const wasmSource = path.join(packageBuildRoot, descriptor.id);
  const stage = siblingStage(path.join(packageRoot, "artifacts"), "artifacts");
  const backup = siblingStage(path.join(packageRoot, "artifacts"), "artifacts-backup");
  const packageJson = readJson(path.join(packageRoot, "package.json"));

  if (packageJson.name !== descriptor.name) {
    throw new Error(`Package manifest ${descriptor.package_dir} must name ${descriptor.name}.`);
  }
  if (descriptor.visibility === "candidate" ? packageJson.private !== true : packageJson.private === true) {
    throw new Error(
      `Package manifest ${descriptor.package_dir} private flag disagrees with ${descriptor.visibility} visibility.`,
    );
  }
  assertFile(path.join(wasmSource, WASM_INPUT_MANIFEST_NAME), `WASM output for ${descriptor.id}`);

  rmSync(stage, { recursive: true, force: true });
  mkdirSync(path.join(stage, "wasm"), { recursive: true });
  try {
    copyWasmRuntime(wasmSource, path.join(stage, "wasm"));
    projectPackageDist(descriptor, packageRoot);
    writeFileSync(
      path.join(stage, "provenance.json"),
      `${JSON.stringify(buildProvenance(descriptor, packageJson, wasmSource, path.join(stage, "wasm"), path.join(packageRoot, "dist", "package-entries")), null, 2)}\n`,
    );
    replaceDirectory({ target: path.join(packageRoot, "artifacts"), stage, backup });
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }

  projectLegalMaterial(descriptor, packageRoot);
}

function projectPackageDist(descriptor, packageRoot) {
  const target = path.join(packageRoot, "dist");
  const stage = siblingStage(target, "projection");
  const backup = siblingStage(target, "projection-backup");
  rmSync(stage, { recursive: true, force: true });
  try {
    mkdirSync(stage, { recursive: true });
    for (const entry of readdirSync(distRoot, { withFileTypes: true })) {
      if (entry.name === "package-entries") continue;
      cpSync(path.join(distRoot, entry.name), path.join(stage, entry.name), {
        filter: (source) => !source.endsWith(".map"),
        recursive: entry.isDirectory(),
      });
    }
    const sourceEntries = path.join(distRoot, "package-entries");
    const targetEntries = path.join(stage, "package-entries");
    mkdirSync(targetEntries, { recursive: true });
    for (const suffix of [".js", ".d.ts", ".js.map", ".d.ts.map"]) {
      const source = path.join(sourceEntries, `${descriptor.id}${suffix}`);
      if (existsSync(source)) cpSync(source, path.join(targetEntries, `${descriptor.id}${suffix}`));
    }
    replaceDirectory({ target, stage, backup });
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }
}

function copyWasmRuntime(source, target) {
  for (const name of WASM_RUNTIME_TOP_LEVEL_FILES) {
    assertFile(path.join(source, name), `WASM artifact ${name}`);
    cpSync(path.join(source, name), path.join(target, name));
  }
  const snippets = path.join(source, "snippets");
  if (existsSync(snippets)) cpSync(snippets, path.join(target, "snippets"), { recursive: true });
}

function buildProvenance(descriptor, packageJson, wasmSource, copiedWasmRoot, entryRoot) {
  const input = readJson(path.join(wasmSource, WASM_INPUT_MANIFEST_NAME));
  return {
    schema_version: 2,
    package: {
      id: descriptor.id,
      name: descriptor.name,
      version: packageJson.version,
      visibility: descriptor.visibility,
    },
    artifact_profile: descriptor.artifact_profile.id,
    runtime_capability_ids: descriptor.artifact_profile.expected.runtime_ids,
    outputs: descriptor.artifact_profile.expected.outputs,
    artifact_files: [
      ...wasmRuntimeFileRecords(copiedWasmRoot, { strictTopLevel: true }),
      ...packageDistFileRecords(path.dirname(entryRoot), descriptor.id),
    ].sort(compareArtifactRecords),
    wasm: {
      path: "wasm/merman_wasm_bg.wasm",
      input_digest: input.input_digest,
      source_digest: input.source_digest,
      tool_versions: input.tool_versions,
    },
  };
}

function projectLegalMaterial(descriptor, packageRoot) {
  const legal = legalProjectionForArtifactProfile(descriptor.artifact_profile.id);
  copyProjection(path.join(webRoot, "LICENSE"), path.join(packageRoot, "LICENSE"));
  writeFileSync(path.join(packageRoot, "THIRD_PARTY_NOTICES.md"), legal.notice);
  replaceScopedLegalDirectory({
    files: legal.files,
    target: path.join(packageRoot, "THIRD_PARTY_LICENSES"),
  });
}

function copyProjection(source, target) {
  assertFile(source, source);
  mkdirSync(path.dirname(target), { recursive: true });
  cpSync(source, target, { force: true });
}

function replaceScopedLegalDirectory({ files, target }) {
  if (!Array.isArray(files) || files.length === 0) {
    throw new Error("Third-party legal projection must contain at least one file.");
  }
  const stage = siblingStage(target, "projection");
  const backup = siblingStage(target, "projection-backup");
  rmSync(stage, { recursive: true, force: true });
  try {
    mkdirSync(stage, { recursive: true });
    for (const file of files) {
      copyProjection(file.source, path.join(stage, ...file.relative.split("/")));
    }
    replaceDirectory({ target, stage, backup });
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }
}

export function replaceDirectory({ target, stage, backup, fsOps = { existsSync, readdirSync, renameSync, rmSync } }) {
  recoverInterruptedReplacement({ target, backup, fsOps });
  try {
    if (fsOps.existsSync(target)) fsOps.renameSync(target, backup);
    fsOps.renameSync(stage, target);
    fsOps.rmSync(backup, { recursive: true, force: true });
  } catch (error) {
    if (!fsOps.existsSync(target) && fsOps.existsSync(backup)) {
      fsOps.renameSync(backup, target);
    }
    throw error;
  }
}

function recoverInterruptedReplacement({ target, backup, fsOps }) {
  if (!fsOps.existsSync(backup)) return;
  if (fsOps.existsSync(target)) {
    fsOps.rmSync(backup, { recursive: true, force: true });
    return;
  }
  fsOps.renameSync(backup, target);
}

function siblingStage(target, kind) {
  return path.join(
    path.dirname(target),
    `.${path.basename(target)}.merman-${kind}-${process.pid}-${Date.now()}`,
  );
}

function readJson(file) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    throw new Error(`Cannot read JSON ${file}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function assertFile(file, label) {
  if (!existsSync(file) || !statSync(file).isFile() || statSync(file).size === 0) {
    throw new Error(`Missing ${label}: ${file}.`);
  }
}

function compareArtifactRecords(left, right) {
  if (left.path < right.path) return -1;
  if (left.path > right.path) return 1;
  return 0;
}

function isMainModule() {
  return process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
