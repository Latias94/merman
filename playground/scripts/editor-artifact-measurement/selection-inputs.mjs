import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

import { packageRuntimeDistClosure } from "../../../platforms/web/scripts/package-dist-closure.mjs";
import { OPAQUE_REALM_ARTIFACT_PLAN } from "../opaque-realm-artifact-plan.mjs";
import { EDITOR_ARTIFACT_SELECTION_INPUT_SCHEMA_VERSION } from "./contract-shared.mjs";
import {
  collectSourceClosure,
  createTypeScriptSourceGraph,
} from "../typescript-source-graph.mjs";

export { EDITOR_ARTIFACT_SELECTION_INPUT_SCHEMA_VERSION };

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
const BUILD_INPUTS = Object.freeze([
  ...OPAQUE_REALM_ARTIFACT_PLAN.pages.map(
    (page) => `playground/${page.source}`,
  ),
  "playground/package-lock.json",
  "playground/package.json",
  "playground/postcss.config.mjs",
  "playground/scripts/csp-policy.mjs",
  "playground/scripts/opaque-realm-artifact-plan.mjs",
  "playground/scripts/opaque-realm-csp.mjs",
  "playground/tsconfig.json",
  "playground/vite.config.ts",
]);
const BUILD_RUNTIME_ENTRIES = Object.freeze(
  OPAQUE_REALM_ARTIFACT_PLAN.pages.map((page) => page.entry),
);
export function editorArtifactSelectionInputs(repositoryRoot) {
  const playgroundRoot = path.join(repositoryRoot, "playground");
  const graph = createTypeScriptSourceGraph({
    rootDir: playgroundRoot,
    entries: [
      ...BUILD_RUNTIME_ENTRIES,
      "src/editor/merman-language.worker.ts",
      "scripts/editor-artifact-measurement/semantic-equivalence.ts",
    ],
  });
  const buildRuntimeClosure = editorArtifactBuildRuntimeClosure(graph);
  const workerClosure = new Set([
    ...collectSourceClosure(
      graph,
      ["src/editor/merman-language.worker.ts"],
      { includeDynamic: true },
    ),
    ...collectSourceClosure(
      graph,
      ["scripts/editor-artifact-measurement/semantic-equivalence.ts"],
      { includeDynamic: true },
    ),
  ]);
  const equivalenceEvidence = readFileSync(
    path.join(repositoryRoot, "contracts/editor-language/token-equivalence-v1.json"),
  );

  return Object.freeze({
    schemaVersion: EDITOR_ARTIFACT_SELECTION_INPUT_SCHEMA_VERSION,
    measurementContractSha256: measurementContractDigest(repositoryRoot),
    buildRuntimeClosureSha256: digestEntries(
      [
        ...entriesForFiles(repositoryRoot, BUILD_INPUTS),
        ...entriesForFiles(
          repositoryRoot,
          [...buildRuntimeClosure].map((file) => `playground/${file}`),
        ),
      ],
      "build-runtime-closure",
    ),
    workerClosureSha256: digestEntries(
      entriesForFiles(
        repositoryRoot,
        [...workerClosure].map((file) => `playground/${file}`),
      ),
      "worker-closure",
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

export function editorArtifactBuildRuntimeClosure(
  graph,
  roots = BUILD_RUNTIME_ENTRIES,
) {
  return collectSourceClosure(graph, roots, {
    includeDynamic: true,
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
    `merman-editor-artifact-selection-inputs/v2/${namespace}\0`,
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
  const provenanceArtifacts = new Map(
    provenance.artifact_files.map((artifact) => {
      const relativePath = requiredPackageArtifactPath(artifact?.path, packageId);
      return [relativePath, artifact];
    }),
  );
  const wasmRuntimeArtifactPaths = [...provenanceArtifacts.keys()].filter(
    (relativePath) =>
      relativePath.startsWith("artifacts/wasm/") &&
      /\.(?:js|wasm)$/u.test(relativePath),
  );
  const runtimeArtifactPaths = runtimePackageArtifactPaths({
    javascriptModules: packageRuntimeDistClosure(
      path.join(packageRoot, "dist"),
      packageId,
    ).javascriptModules,
    wasmArtifactPaths: wasmRuntimeArtifactPaths,
  });
  const selectedArtifactPaths = new Set(runtimeArtifactPaths);
  const artifactFiles = [
    ...new Set([...runtimeArtifactPaths, ...wasmRuntimeArtifactPaths]),
  ]
    .sort((left, right) => left.localeCompare(right, "en"))
    .flatMap((relativePath) => {
      const artifact = provenanceArtifacts.get(relativePath);
      if (!artifact) {
        throw new Error(
          `Web package ${packageId} provenance is missing runtime artifact ${relativePath}.`,
        );
      }
      const bytes = readFileSync(path.join(packageRoot, relativePath));
      if (
        artifact.bytes !== bytes.byteLength ||
        artifact.sha256 !== `sha256:${sha256(bytes)}`
      ) {
        throw new Error(
          `Web package ${packageId} provenance is stale for ${relativePath}.`,
        );
      }
      return selectedArtifactPaths.has(relativePath)
        ? [
            {
              bytes,
              path: `platforms/web/packages/${packageId}/${relativePath}`,
            },
          ]
        : [];
    });
  const normalizedProvenance = Buffer.from(
    JSON.stringify(
      runtimePackageProvenanceContract(provenance, runtimeArtifactPaths),
    ),
  );
  return digestEntries(
    [
      {
        bytes: Buffer.from(JSON.stringify(runtimePackageManifest(packageJson))),
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

export function runtimePackageProvenanceContract(
  provenance,
  runtimeArtifactPaths,
) {
  const packageId = provenance.package?.id ?? "unknown";
  const artifactsByPath = new Map(
    provenance.artifact_files.map((artifact) => [artifact.path, artifact]),
  );
  const wasmSource = {
    path: requiredPackageArtifactPath(provenance.wasm?.path, packageId),
    sha256: requiredSha256(
      provenance.wasm?.source_digest,
      `Web package ${packageId} WASM source digest`,
    ),
  };
  return {
    schemaVersion: provenance.schema_version,
    package: provenance.package,
    artifactProfile: provenance.artifact_profile,
    runtimeCapabilityIds: provenance.runtime_capability_ids,
    outputs: provenance.outputs,
    artifactFiles: runtimeArtifactPaths.map((relativePath) =>
      artifactsByPath.get(relativePath),
    ),
    wasmSource,
  };
}

export function runtimePackageArtifactPaths({
  javascriptModules,
  wasmArtifactPaths,
}) {
  return [
    ...new Set([
      ...javascriptModules.map((relativePath) => `dist/${relativePath}`),
      ...wasmArtifactPaths.filter((relativePath) =>
        /^artifacts\/wasm\/.+\.js$/u.test(relativePath),
      ),
    ]),
  ].sort((left, right) => left.localeCompare(right, "en"));
}

function runtimePackageManifest(packageJson) {
  return {
    name: packageJson.name,
    version: packageJson.version,
    type: packageJson.type,
    main: packageJson.main,
    exports: {
      ".": {
        import: packageJson.exports?.["."]?.import,
      },
    },
    merman: {
      artifact_profile: packageJson.merman?.artifact_profile,
    },
  };
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

function requiredSha256(value, label) {
  if (typeof value !== "string" || !/^[a-f0-9]{64}$/u.test(value)) {
    throw new Error(`${label} is invalid.`);
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
