import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

import {
  collectSourceClosure,
  createTypeScriptSourceGraph,
} from "../typescript-source-graph.mjs";

export const EDITOR_ARTIFACT_SELECTION_INPUT_SCHEMA_VERSION = 1;

const MEASUREMENT_INPUTS = Object.freeze([
  "playground/scripts/typescript-source-graph.mjs",
  "playground/vite.editor-artifact-measurement.config.ts",
]);
const MEASUREMENT_CONTRACT_EXTENSIONS = new Set([
  ".html",
  ".mjs",
  ".mts",
  ".ts",
  ".tsx",
]);
const STARTUP_BUILD_INPUTS = Object.freeze([
  "playground/index.html",
  "playground/package-lock.json",
  "playground/package.json",
  "playground/tsconfig.json",
  "playground/vite.config.ts",
]);
const WEB_SURFACE_INPUTS = Object.freeze([
  "platforms/web/package-lock.json",
  "platforms/web/package.json",
  "platforms/web/scripts/build-surface-packages.mjs",
  "platforms/web/scripts/package-dist-closure.mjs",
  "platforms/web/scripts/surface-manifest.mjs",
  "platforms/web/scripts/verify-wasm-inputs.mjs",
  "platforms/web/scripts/wasm-runtime-files.mjs",
  "platforms/web/web-surface-descriptor.json",
  "platforms/web/web-surface-descriptor.schema.json",
]);

export function editorArtifactSelectionInputs(repositoryRoot) {
  const playgroundRoot = path.join(repositoryRoot, "playground");
  const graph = createTypeScriptSourceGraph({
    rootDir: playgroundRoot,
    entries: [
      "src/main.tsx",
      "src/editor/merman-language.worker.ts",
      "scripts/editor-artifact-measurement/semantic-equivalence.ts",
    ],
  });
  const startupClosure = collectSourceClosure(graph, ["src/main.tsx"], {
    includeTypeOnly: true,
  });
  const workerClosure = new Set([
    ...collectSourceClosure(
      graph,
      ["src/editor/merman-language.worker.ts"],
      { includeTypeOnly: true },
    ),
    ...collectSourceClosure(
      graph,
      ["scripts/editor-artifact-measurement/semantic-equivalence.ts"],
      { includeTypeOnly: true },
    ),
  ]);
  const equivalenceEvidence = readFileSync(
    path.join(repositoryRoot, "editor-language/token-equivalence-v1.json"),
  );

  return Object.freeze({
    schemaVersion: EDITOR_ARTIFACT_SELECTION_INPUT_SCHEMA_VERSION,
    measurementContractSha256: measurementContractDigest(repositoryRoot),
    startupClosureSha256: digestEntries(
      [
        ...entriesForFiles(repositoryRoot, STARTUP_BUILD_INPUTS),
        ...entriesForFiles(
          repositoryRoot,
          [...startupClosure].map((file) => `playground/${file}`),
        ),
      ],
      "startup-closure",
    ),
    workerClosureSha256: digestEntries(
      entriesForFiles(
        repositoryRoot,
        [...workerClosure].map((file) => `playground/${file}`),
      ),
      "worker-closure",
    ),
    webSurfaceSha256: digestEntries(
      [
        ...entriesForFiles(repositoryRoot, WEB_SURFACE_INPUTS),
        ...collectDirectoryEntries(repositoryRoot, "platforms/web/src", {
          excludeTests: true,
        }),
        ...collectDirectoryEntries(
          repositoryRoot,
          "platforms/web/scripts/wasm-build",
          { excludeTests: true },
        ),
      ],
      "web-surface",
    ),
    fullPackageProvenanceSha256: packageProvenanceDigest(
      repositoryRoot,
      "full",
    ),
    editorPackageProvenanceSha256: packageProvenanceDigest(
      repositoryRoot,
      "editor",
    ),
    equivalenceEvidenceSha256: sha256(equivalenceEvidence),
  });
}

export function measurementContractDigest(repositoryRoot) {
  return digestEntries(
    [
      ...entriesForFiles(repositoryRoot, MEASUREMENT_INPUTS),
      ...collectDirectoryEntries(
        repositoryRoot,
        "playground/scripts/editor-artifact-measurement",
        {
          exclude: new Set(["verify-editor-artifact-receipt.mjs"]),
          excludeTests: true,
          extensions: MEASUREMENT_CONTRACT_EXTENSIONS,
        },
      ),
    ],
    "measurement-contract",
  );
}

export function digestEntries(entries, namespace = "fixture") {
  const normalized = entries
    .map((entry) => {
      if (!entry || typeof entry.path !== "string" || entry.path.length === 0) {
        throw new TypeError("Selection input paths must be non-empty strings.");
      }
      if (!Buffer.isBuffer(entry.bytes)) {
        throw new TypeError(`Selection input ${entry.path} must provide Buffer bytes.`);
      }
      return {
        bytes: entry.bytes,
        path: normalizeRelativePath(entry.path),
      };
    })
    .sort((left, right) => left.path.localeCompare(right.path, "en"));
  const seen = new Set();
  const hash = createHash("sha256").update(
    `merman-editor-artifact-selection-inputs/v1/${namespace}\0`,
  );
  for (const entry of normalized) {
    if (seen.has(entry.path)) {
      throw new TypeError(`Selection input ${entry.path} is duplicated.`);
    }
    seen.add(entry.path);
    hash.update(entry.path).update("\0");
    hash.update(String(entry.bytes.byteLength)).update("\0");
    hash.update(entry.bytes).update("\0");
  }
  return hash.digest("hex");
}

function packageProvenanceDigest(repositoryRoot, packageId) {
  const packageRoot = path.join(
    repositoryRoot,
    "platforms/web/packages",
    packageId,
  );
  const packageJson = readJson(path.join(packageRoot, "package.json"));
  const provenance = readJson(
    path.join(packageRoot, "artifacts/provenance.json"),
  );
  if (
    provenance.schema_version !== 2 ||
    provenance.package?.id !== packageId ||
    !Array.isArray(provenance.artifact_files)
  ) {
    throw new Error(`Web package ${packageId} provenance is invalid.`);
  }
  const artifactFiles = provenance.artifact_files.map((artifact) => {
    const relativePath = requiredPackageArtifactPath(artifact?.path, packageId);
    const bytes = readFileSync(path.join(packageRoot, relativePath));
    if (
      artifact.bytes !== bytes.byteLength ||
      artifact.sha256 !== `sha256:${sha256(bytes)}`
    ) {
      throw new Error(
        `Web package ${packageId} provenance is stale for ${relativePath}.`,
      );
    }
    return {
      bytes,
      path: `platforms/web/packages/${packageId}/${relativePath}`,
    };
  });
  const normalizedProvenance = Buffer.from(
    JSON.stringify({
      schemaVersion: provenance.schema_version,
      package: provenance.package,
      artifactProfile: provenance.artifact_profile,
      runtimeCapabilityIds: provenance.runtime_capability_ids,
      outputs: provenance.outputs,
      artifactFiles: provenance.artifact_files,
      wasm: {
        path: provenance.wasm?.path,
        inputDigest: provenance.wasm?.input_digest,
        sourceDigest: provenance.wasm?.source_digest,
      },
    }),
  );
  return digestEntries(
    [
      {
        bytes: Buffer.from(JSON.stringify(packageJson)),
        path: `platforms/web/packages/${packageId}/package.json`,
      },
      {
        bytes: normalizedProvenance,
        path: `platforms/web/packages/${packageId}/artifacts/provenance.contract.json`,
      },
      ...artifactFiles,
    ],
    `${packageId}-package-provenance`,
  );
}

function collectDirectoryEntries(
  repositoryRoot,
  relativeRoot,
  { exclude = new Set(), excludeTests = false, extensions = null } = {},
) {
  const entries = [];
  const visit = (relativeDirectory) => {
    const absoluteDirectory = path.join(repositoryRoot, relativeDirectory);
    const members = readdirSync(absoluteDirectory, { withFileTypes: true }).sort(
      (left, right) => left.name.localeCompare(right.name, "en"),
    );
    for (const member of members) {
      if (member.name === ".DS_Store" || exclude.has(member.name)) continue;
      const relativePath = path.posix.join(relativeDirectory, member.name);
      if (member.isDirectory()) {
        visit(relativePath);
      } else if (member.isFile()) {
        if (extensions && !extensions.has(path.extname(relativePath))) continue;
        if (
          excludeTests &&
          /\.(?:spec|test)\.(?:mjs|ts|tsx)$/u.test(relativePath)
        ) {
          continue;
        }
        entries.push(readInputEntry(repositoryRoot, relativePath));
      } else {
        throw new Error(`Selection input ${relativePath} must be a regular file.`);
      }
    }
  };
  visit(relativeRoot);
  return entries;
}

function entriesForFiles(repositoryRoot, relativePaths) {
  return [...new Set(relativePaths)]
    .sort((left, right) => left.localeCompare(right, "en"))
    .map((relativePath) => readInputEntry(repositoryRoot, relativePath));
}

function readInputEntry(repositoryRoot, relativePath) {
  return {
    bytes: readFileSync(path.join(repositoryRoot, relativePath)),
    path: normalizeRelativePath(relativePath),
  };
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function requiredPackageArtifactPath(value, packageId) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.startsWith("/") ||
    value === ".." ||
    value.startsWith("../") ||
    value.includes("/../")
  ) {
    throw new Error(`Web package ${packageId} artifact path is invalid.`);
  }
  return value;
}

function normalizeRelativePath(relativePath) {
  const normalized = relativePath.replaceAll("\\", "/");
  if (
    normalized.startsWith("/") ||
    normalized === ".." ||
    normalized.startsWith("../") ||
    normalized.includes("/../")
  ) {
    throw new TypeError(
      `Selection input path is not repository-relative: ${relativePath}`,
    );
  }
  return normalized;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}
