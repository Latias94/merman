import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { webPackages } from "./surface-manifest.mjs";
import {
  packageDistFileRecords,
  wasmRuntimeFileRecords,
} from "./wasm-runtime-files.mjs";

const webRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.join(webRoot, "..", "..");
const canonicalLicenseRoot = path.join(repositoryRoot, "THIRD_PARTY_LICENSES");
const canonicalNotices = path.join(repositoryRoot, "THIRD_PARTY_NOTICES.md");
const PACKAGE_FILE_ALLOWLIST = Object.freeze([
  "LICENSE",
  "README.md",
  "THIRD_PARTY_LICENSES",
  "THIRD_PARTY_NOTICES.md",
  "artifacts",
  "dist",
]);

if (isMainModule()) {
  try {
    verifyPackageGroup();
    console.log(`[merman-web] package group verified (${webPackages.length} package artifacts).`);
  } catch (error) {
    console.error(`prepack: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}

export function verifyPackageGroup({ descriptors = webPackages } = {}) {
  const checked = descriptors.map(verifyPackage);
  const publicPackages = checked.filter((item) => item.descriptor.visibility === "public");
  assertLockstepVersions(publicPackages);
  assertIndependentSizeEvidence(checked, publicPackages);
  return checked;
}

function verifyPackage(descriptor) {
  const packageRoot = path.join(webRoot, descriptor.package_dir);
  const manifest = readJson(path.join(packageRoot, "package.json"));
  assertPackageManifest(descriptor, manifest);
  const artifactRoot = path.join(packageRoot, "artifacts");
  const wasmPath = path.join(artifactRoot, "wasm", "merman_wasm_bg.wasm");
  const provenance = readJson(path.join(artifactRoot, "provenance.json"));

  assertArtifactRoot(artifactRoot, descriptor.name);

  for (const file of [
    path.join(packageRoot, "README.md"),
    path.join(packageRoot, "LICENSE"),
    path.join(packageRoot, "THIRD_PARTY_NOTICES.md"),
    path.join(packageRoot, "dist", "package-entries", `${descriptor.id}.js`),
    path.join(packageRoot, "dist", "package-entries", `${descriptor.id}.d.ts`),
    path.join(artifactRoot, "wasm", "merman_wasm.js"),
    path.join(artifactRoot, "wasm", "merman_wasm.d.ts"),
    path.join(artifactRoot, "wasm", "merman_wasm_bg.wasm.d.ts"),
    wasmPath,
  ]) {
    assertFile(file);
  }

  const wasmFiles = walkFiles(artifactRoot).filter((file) => file.endsWith(".wasm"));
  if (wasmFiles.length !== 1 || path.resolve(wasmFiles[0]) !== path.resolve(wasmPath)) {
    throw new Error(
      `${descriptor.name} must contain exactly one WASM binary at artifacts/wasm/merman_wasm_bg.wasm.`,
    );
  }
  if (existsSync(path.join(packageRoot, "pkg"))) {
    throw new Error(`${descriptor.name} must not package the legacy pkg directory.`);
  }
  assertPackageEntryFiles(descriptor, packageRoot);
  assertProvenance(descriptor, manifest, provenance, artifactRoot, packageRoot);
  assertLegalProjection(packageRoot);
  return { descriptor, manifest, wasmBytes: statSync(wasmPath).size, packageBytes: treeSize(packageRoot) };
}

export function assertPackageManifest(descriptor, manifest) {
  if (manifest.name !== descriptor.name) {
    throw new Error(`${descriptor.package_dir}/package.json must name ${descriptor.name}.`);
  }
  if (manifest.merman?.artifact_profile !== descriptor.artifact_profile.id) {
    throw new Error(`${descriptor.name} must reference artifact profile ${descriptor.artifact_profile.id}.`);
  }
  if ((manifest.private === true) !== (descriptor.visibility === "candidate")) {
    throw new Error(`${descriptor.name} private flag must match descriptor visibility.`);
  }
  if (manifest.license !== "MIT OR Apache-2.0") {
    throw new Error(`${descriptor.name} must declare the repository SPDX license expression.`);
  }
  if (
    !Array.isArray(manifest.files) ||
    JSON.stringify([...manifest.files].sort()) !== JSON.stringify(PACKAGE_FILE_ALLOWLIST)
  ) {
    throw new Error(`${descriptor.name} must declare the closed package files allowlist.`);
  }
  if (manifest.scripts !== undefined) {
    throw new Error(`${descriptor.name} must not declare npm lifecycle scripts.`);
  }
  if (manifest.bundleDependencies !== undefined || manifest.bundledDependencies !== undefined) {
    throw new Error(`${descriptor.name} must not declare bundled npm dependencies.`);
  }
  if (descriptor.visibility === "candidate") {
    if (manifest.publishConfig !== undefined) {
      throw new Error(`${descriptor.name} private candidate must not declare publishConfig.`);
    }
  } else if (JSON.stringify(manifest.publishConfig) !== JSON.stringify({ access: "public" })) {
    throw new Error(`${descriptor.name} must declare only publishConfig.access=public.`);
  }
  const exports = manifest.exports;
  if (!exports || typeof exports !== "object" || Array.isArray(exports)) {
    throw new Error(`${descriptor.name} must declare a closed exports object.`);
  }
  if (JSON.stringify(Object.keys(exports).sort()) !== JSON.stringify(["."])) {
    throw new Error(`${descriptor.name} may export only its package root.`);
  }
  const entryJavaScript = `./dist/package-entries/${descriptor.id}.js`;
  const entryTypes = `./dist/package-entries/${descriptor.id}.d.ts`;
  if (manifest.main !== entryJavaScript || manifest.types !== entryTypes) {
    throw new Error(`${descriptor.name} must point main and types at its own package entry.`);
  }
  const rootExport = exports["."];
  if (!rootExport || typeof rootExport !== "object" || Array.isArray(rootExport)) {
    throw new Error(`${descriptor.name} package-root export must be an object.`);
  }
  if (JSON.stringify(Object.keys(rootExport).sort()) !== JSON.stringify(["import", "types"])) {
    throw new Error(`${descriptor.name} package-root export must contain only import and types.`);
  }
  if (rootExport.import !== entryJavaScript || rootExport.types !== entryTypes) {
    throw new Error(`${descriptor.name} package-root export must point at its own package entry.`);
  }
}

function assertProvenance(descriptor, manifest, provenance, artifactRoot, packageRoot) {
  if (!provenance || typeof provenance !== "object" || Array.isArray(provenance)) {
    throw new Error(`${descriptor.name} provenance is invalid.`);
  }
  if (provenance.schema_version !== 2) {
    throw new Error(`${descriptor.name} provenance schema must be 2.`);
  }
  if (
    provenance.package?.id !== descriptor.id ||
    provenance.package?.name !== descriptor.name ||
    provenance.package?.version !== manifest.version ||
    provenance.package?.visibility !== descriptor.visibility
  ) {
    throw new Error(`${descriptor.name} provenance package identity is stale.`);
  }
  if (provenance.artifact_profile !== descriptor.artifact_profile.id) {
    throw new Error(`${descriptor.name} provenance references the wrong artifact profile.`);
  }
  assertEqualArray(
    provenance.runtime_capability_ids,
    descriptor.artifact_profile.expected.runtime_ids,
    `${descriptor.name} provenance runtime capability IDs`,
  );
  assertEqualArray(
    provenance.outputs,
    descriptor.artifact_profile.expected.outputs,
    `${descriptor.name} provenance outputs`,
  );
  if (provenance.wasm?.path !== "wasm/merman_wasm_bg.wasm") {
    throw new Error(`${descriptor.name} provenance must name its one WASM artifact.`);
  }
  if (typeof provenance.wasm?.input_digest !== "string" || typeof provenance.wasm?.source_digest !== "string") {
    throw new Error(`${descriptor.name} provenance is missing input evidence.`);
  }
  assertArtifactFileEvidence({
    packageWasmRoot: path.join(artifactRoot, "wasm"),
    sourceWasmRoot: path.join(webRoot, "pkg", descriptor.id),
    packageDistRoot: path.join(packageRoot, "dist"),
    sourceDistRoot: path.join(webRoot, "dist"),
    packageId: descriptor.id,
    artifactFiles: provenance.artifact_files,
    label: descriptor.name,
  });
}

export function assertArtifactFileEvidence({
  packageWasmRoot,
  sourceWasmRoot,
  packageDistRoot,
  sourceDistRoot,
  packageId,
  artifactFiles,
  label,
}) {
  assertArtifactFileManifest(artifactFiles, label);
  const packageRecords = wasmRuntimeFileRecords(packageWasmRoot, { strictTopLevel: true });
  const sourceRecords = wasmRuntimeFileRecords(sourceWasmRoot);
  const packageDistRecords = packageDistFileRecords(packageDistRoot, packageId);
  const sourceDistRecords = packageDistFileRecords(sourceDistRoot, packageId, {
    allowSiblingPackageEntries: true,
  });
  const expected = [...artifactFiles].sort(compareArtifactRecords);
  assertEqualArtifactRecords(
    expected,
    [...packageRecords, ...packageDistRecords].sort(compareArtifactRecords),
    `${label} copied package artifacts`,
  );
  assertEqualArtifactRecords(
    expected,
    [...sourceRecords, ...sourceDistRecords].sort(compareArtifactRecords),
    `${label} source package artifacts`,
  );
}

function assertArtifactRoot(artifactRoot, label) {
  const entries = readdirSync(artifactRoot, { withFileTypes: true });
  const actual = entries.map((entry) => entry.name).sort();
  if (JSON.stringify(actual) !== JSON.stringify(["provenance.json", "wasm"])) {
    throw new Error(`${label} artifacts must contain only provenance.json and wasm/.`);
  }
  if (!entries.find((entry) => entry.name === "provenance.json")?.isFile()) {
    throw new Error(`${label} artifacts/provenance.json must be a regular file.`);
  }
  if (!entries.find((entry) => entry.name === "wasm")?.isDirectory()) {
    throw new Error(`${label} artifacts/wasm must be a directory.`);
  }
}

function assertPackageEntryFiles(descriptor, packageRoot) {
  const entryRoot = path.join(packageRoot, "dist", "package-entries");
  const expected = [
    `${descriptor.id}.d.ts`,
    `${descriptor.id}.d.ts.map`,
    `${descriptor.id}.js`,
    `${descriptor.id}.js.map`,
  ];
  const actual = readdirSync(entryRoot).sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${descriptor.name} may package only its own compiled entry wrapper.`);
  }
}

function assertArtifactFileManifest(records, label) {
  if (!Array.isArray(records) || records.length === 0) {
    throw new Error(`${label} provenance must contain copied package artifact hashes.`);
  }
  let previousPath = "";
  for (const record of records) {
    if (!record || typeof record !== "object" || Array.isArray(record)) {
      throw new Error(`${label} provenance artifact entry is invalid.`);
    }
    if (JSON.stringify(Object.keys(record).sort()) !== JSON.stringify(["bytes", "path", "sha256"])) {
      throw new Error(`${label} provenance artifact entry must contain path, bytes, and sha256.`);
    }
    if (
      typeof record.path !== "string" ||
      !(record.path.startsWith("artifacts/wasm/") || record.path.startsWith("dist/")) ||
      record.path.includes("\\") ||
      record.path.includes("..") ||
      record.path <= previousPath
    ) {
      throw new Error(`${label} provenance artifact paths must be sorted, unique package-relative paths.`);
    }
    if (!Number.isSafeInteger(record.bytes) || record.bytes <= 0) {
      throw new Error(`${label} provenance artifact bytes must be positive integers.`);
    }
    if (typeof record.sha256 !== "string" || !/^sha256:[0-9a-f]{64}$/.test(record.sha256)) {
      throw new Error(`${label} provenance artifact sha256 is invalid.`);
    }
    previousPath = record.path;
  }
}

function assertEqualArtifactRecords(expected, actual, label) {
  if (JSON.stringify(expected) !== JSON.stringify(actual)) {
    throw new Error(`${label} do not match their provenance evidence.`);
  }
}

function compareArtifactRecords(left, right) {
  if (left.path < right.path) return -1;
  if (left.path > right.path) return 1;
  return 0;
}

function assertLockstepVersions(publicPackages) {
  const versions = new Set(publicPackages.map((item) => item.manifest.version));
  if (versions.size !== 1) {
    throw new Error("Admitted @mermanjs browser packages must use one lockstep version.");
  }
}

function assertIndependentSizeEvidence(checked, publicPackages) {
  const full = checked.find((item) => item.descriptor.id === "full");
  if (!full) throw new Error("The Web package group is missing the full package.");
  for (const item of publicPackages) {
    if (item.descriptor.id === "full") continue;
    const saving = 1 - item.packageBytes / full.packageBytes;
    if (saving < 0.15) {
      throw new Error(
        `${item.descriptor.name} is only ${(saving * 100).toFixed(1)}% smaller than @mermanjs/web; public slim packages require at least 15% unpacked-size evidence.`,
      );
    }
  }
}

function assertLegalProjection(packageRoot) {
  assertSameFile(path.join(packageRoot, "THIRD_PARTY_NOTICES.md"), canonicalNotices);
  for (const canonical of walkFiles(canonicalLicenseRoot)) {
    const projected = path.join(packageRoot, "THIRD_PARTY_LICENSES", path.relative(canonicalLicenseRoot, canonical));
    assertSameFile(projected, canonical);
  }
}

function assertSameFile(actual, expected) {
  assertFile(actual);
  assertFile(expected);
  if (!readFileSync(actual).equals(readFileSync(expected))) {
    throw new Error(`Legal projection is stale: ${actual}.`);
  }
}

function assertEqualArray(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${label} does not match its artifact profile.`);
  }
}

function readJson(file) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    throw new Error(`Cannot read ${file}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function assertFile(file) {
  if (!existsSync(file) || !statSync(file).isFile() || statSync(file).size === 0) {
    throw new Error(`Missing package file: ${file}.`);
  }
}

function walkFiles(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const entryPath = path.join(directory, entry.name);
      return entry.isDirectory() ? walkFiles(entryPath) : [entryPath];
    })
    .sort();
}

function treeSize(directory) {
  return walkFiles(directory).reduce((size, file) => size + statSync(file).size, 0);
}

function isMainModule() {
  return process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}
