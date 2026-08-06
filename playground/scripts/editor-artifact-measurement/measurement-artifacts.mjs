import { readFileSync } from "node:fs";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { gzipSync } from "node:zlib";

import { EDITOR_ARTIFACT_FAMILY_COUNT } from "./equivalence-shared.mjs";
import { npmCommand, runCommand } from "./measurement-cli.mjs";
import {
  contentTypeFor,
  isCompressible,
  normalizeSource,
  sha256,
} from "./measurement-shared.mjs";

export function prepareMeasurementInputs({ playgroundRoot, repositoryRoot }) {
  runCommand(npmCommand(), ["run", "prepare:browser-runtime"], playgroundRoot);
  runCommand(
    process.execPath,
    [
      path.join(repositoryRoot, "platforms/web/scripts/verify-wasm-inputs.mjs"),
      "--package",
      "editor",
    ],
    repositoryRoot,
  );
}

export function buildVariant(
  variant,
  { buildsRoot, measurementConfig, playgroundRoot, viteCli },
) {
  const outDir = variantOutDir(buildsRoot, variant);
  runCommand(
    process.execPath,
    [viteCli, "build", "--config", measurementConfig, "--mode", "production"],
    playgroundRoot,
    {
      ...process.env,
      MERMAN_EDITOR_ARTIFACT_OUT_DIR: outDir,
      MERMAN_EDITOR_ARTIFACT_VARIANT: variant,
    },
  );
}

export async function inspectBuild(variant, { buildsRoot }) {
  const outDir = variantOutDir(buildsRoot, variant);
  const manifestPath = path.join(outDir, ".vite/manifest.json");
  const manifestBytes = await readFile(manifestPath);
  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  const mainWasm = findManifestAsset(
    manifest,
    "platforms/web/packages/full/artifacts/wasm/merman_wasm_bg.wasm",
    outDir,
  );
  const workerBundle = await findWorkerBundle(outDir);
  const workerSource = await readFile(
    path.join(outDir, workerBundle.file),
    "utf8",
  );
  const workerWasm =
    variant === "editor"
      ? await findWorkerWasmAsset(
          workerSource,
          outDir,
          "platforms/web/packages/editor/artifacts/wasm/merman_wasm_bg.wasm",
        )
      : mainWasm;
  if (variant === "editor" && mainWasm.file === workerWasm.file) {
    throw new Error(
      "Editor measurement variant did not emit a distinct Worker WASM artifact.",
    );
  }
  const expectedWorkerWasm = path.basename(workerWasm.file);
  if (!workerSource.includes(expectedWorkerWasm)) {
    throw new Error(
      `${variant} measurement Worker does not reference ${expectedWorkerWasm}.`,
    );
  }
  if (
    variant === "editor" &&
    workerSource.includes(path.basename(mainWasm.file))
  ) {
    throw new Error(
      "Editor measurement Worker still references the full WASM artifact.",
    );
  }
  const staticTree = await loadStaticTree(outDir);
  const equivalenceHtml = findStaticFile(
    staticTree.files,
    "/scripts/editor-artifact-measurement/semantic-equivalence.html",
    "semantic-equivalence HTML",
  );
  assertWasmRealmOwnership({
    mainWasm,
    staticFiles: staticTree.files,
    variant,
    workerBundle,
    workerWasm,
  });

  return {
    equivalenceHtml,
    id: variant,
    manifestSha256: sha256(manifestBytes),
    mainWasm,
    outDir,
    staticBytes: staticTree.stats,
    staticFiles: staticTree.files,
    workerBundle,
    workerWasm,
  };
}

export async function loadEquivalenceBaselines(equivalenceEvidencePath) {
  const evidenceBytes = await readFile(equivalenceEvidencePath);
  const evidence = JSON.parse(evidenceBytes.toString("utf8"));
  requiredString(evidence?.generated_by, "generated evidence producer");
  if (
    evidence?.source_manifest !== "playground/examples/manifest.json" ||
    !Array.isArray(evidence.family_cases) ||
    evidence.family_cases.length !== EDITOR_ARTIFACT_FAMILY_COUNT
  ) {
    throw new Error(
      `Generated editor token equivalence evidence must contain one baseline for all ${EDITOR_ARTIFACT_FAMILY_COUNT} Playground families.`,
    );
  }
  const baselines = evidence.family_cases.map((entry, index) => {
    const label = `generated family baseline ${index}`;
    if (!entry || typeof entry !== "object") {
      throw new Error(`${label} must be an object.`);
    }
    return {
      id: requiredString(entry.id, `${label} id`),
      family: requiredString(entry.family, `${label} family`),
      fixture: requiredString(entry.fixture, `${label} fixture`),
      source: requiredString(entry.source, `${label} source`),
      sourceSha256: generatedSha256(
        entry.source_sha256,
        `${label} source_sha256`,
      ),
      detectionValidity: requiredString(
        entry.detection_validity,
        `${label} detection_validity`,
      ),
      syntaxId: requiredString(entry.syntax_id, `${label} syntax_id`),
      effectiveLayoutId: requiredString(
        entry.effective_layout_id,
        `${label} effective_layout_id`,
      ),
      semanticTokensSha256: generatedSha256(
        entry.packed_sha256,
        `${label} packed_sha256`,
      ),
    };
  });
  if (
    new Set(baselines.map((entry) => entry.family)).size !==
    EDITOR_ARTIFACT_FAMILY_COUNT
  ) {
    throw new Error(
      "Generated editor token evidence contains duplicate families.",
    );
  }
  return { baselines, sha256: sha256(evidenceBytes) };
}

function generatedSha256(value, label) {
  const text = requiredString(value, label);
  if (!/^sha256:[a-f0-9]{64}$/u.test(text)) {
    throw new Error(`${label} must be a generated SHA-256 digest.`);
  }
  return text.slice("sha256:".length);
}

function requiredString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${label} must be a non-empty string.`);
  }
  return value;
}

function findStaticFile(files, suffix, label) {
  const matches = [...files.keys()].filter((file) => file.endsWith(suffix));
  if (matches.length !== 1) {
    throw new Error(`Expected one ${label}, found ${matches.length}.`);
  }
  return matches[0];
}

function assertWasmRealmOwnership({
  mainWasm,
  staticFiles,
  variant,
  workerBundle,
  workerWasm,
}) {
  const mainWasmName = path.basename(mainWasm.file);
  const workerWasmName = path.basename(workerWasm.file);
  const workerPath = `/${normalizeSource(workerBundle.file)}`;
  const javascriptReferences = (assetName) =>
    [...staticFiles.entries()]
      .filter(([pathname]) => pathname.endsWith(".js"))
      .filter(([, file]) => file.body.toString("utf8").includes(assetName))
      .map(([pathname]) => pathname)
      .sort();

  const mainReferences = javascriptReferences(mainWasmName);
  if (!mainReferences.some((pathname) => pathname !== workerPath)) {
    throw new Error(
      `${variant} measurement main graph does not reference the full WASM artifact.`,
    );
  }
  if (variant !== "editor") return;

  const editorReferences = javascriptReferences(workerWasmName);
  const nonWorkerReferences = editorReferences.filter(
    (pathname) => pathname !== workerPath,
  );
  if (nonWorkerReferences.length > 0) {
    throw new Error(
      `Editor WASM leaked outside the Worker bundle: ${nonWorkerReferences.join(", ")}.`,
    );
  }
  if (!editorReferences.includes(workerPath)) {
    throw new Error("Editor Worker bundle does not own the editor WASM URL.");
  }
}

async function loadStaticTree(root) {
  const files = new Map();
  let gzipBytes = 0;
  let rawBytes = 0;
  for (const file of await walkFiles(root)) {
    const relative = path.relative(root, file).split(path.sep).join("/");
    const body = await readFile(file);
    const contentType = contentTypeFor(file);
    const gzip = isCompressible(contentType)
      ? gzipSync(body, { level: 9 })
      : null;
    files.set(`/${relative}`, {
      body,
      contentType,
      gzip,
      immutable: relative.startsWith("assets/"),
    });
    rawBytes += body.byteLength;
    gzipBytes += gzip?.byteLength ?? body.byteLength;
  }
  return {
    files,
    stats: { files: files.size, gzipBytes, rawBytes },
  };
}

async function walkFiles(root) {
  const files = [];
  const pending = [root];
  while (pending.length > 0) {
    const directory = pending.pop();
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) pending.push(file);
      else if (entry.isFile()) files.push(file);
    }
  }
  return files.sort();
}

function findManifestAsset(manifest, sourceSuffix, outDir) {
  const matches = Object.entries(manifest).filter(([key, chunk]) =>
    normalizeSource(chunk.src ?? key).endsWith(sourceSuffix),
  );
  if (matches.length !== 1) {
    throw new Error(
      `Expected one manifest asset for ${sourceSuffix}, found ${matches.length}.`,
    );
  }
  const [source, chunk] = matches[0];
  const bytes = readFileSync(path.join(outDir, chunk.file));
  return {
    bytes: bytes.byteLength,
    file: chunk.file,
    sha256: sha256(bytes),
    source: normalizeSource(chunk.src ?? source),
  };
}

async function findWorkerWasmAsset(workerSource, outDir, source) {
  const matches = [
    ...workerSource.matchAll(/merman_wasm_bg-[A-Za-z0-9_-]+\.wasm/gu),
  ].map((match) => match[0]);
  const files = [...new Set(matches)].sort();
  if (files.length !== 1) {
    throw new Error(
      `Expected one emitted Worker WASM reference, found ${files.length}.`,
    );
  }
  const file = path.join("assets", files[0]);
  const bytes = await readFile(path.join(outDir, file));
  return {
    bytes: bytes.byteLength,
    file: normalizeSource(file),
    sha256: sha256(bytes),
    source,
  };
}

async function findWorkerBundle(outDir) {
  const assetsDir = path.join(outDir, "assets");
  const matches = (await readdir(assetsDir))
    .filter((file) => /^merman-language\.worker-.*\.js$/u.test(file))
    .sort();
  if (matches.length !== 1) {
    throw new Error(
      `Expected one emitted Merman language Worker, found ${matches.length}.`,
    );
  }
  const file = path.join("assets", matches[0]);
  const bytes = await readFile(path.join(outDir, file));
  return {
    bytes: bytes.byteLength,
    file: normalizeSource(file),
    sha256: sha256(bytes),
  };
}

function variantOutDir(buildsRoot, variant) {
  return path.join(buildsRoot, variant);
}
