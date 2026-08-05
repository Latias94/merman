import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { gzipSync } from "node:zlib";

import {
  DEFAULT_EDITOR_ARTIFACT_RECEIPT_PATH,
  EDITOR_ARTIFACT_VARIANTS,
  createEditorArtifactReceipt,
} from "./contract.mjs";
import {
  EDITOR_ARTIFACT_FAMILY_COUNT,
  EDITOR_ARTIFACT_QUERY_KINDS,
} from "./equivalence-shared.mjs";

const playgroundRoot = path.resolve(import.meta.dirname, "../..");
const repositoryRoot = path.resolve(playgroundRoot, "..");
const measurementRoot = path.join(
  repositoryRoot,
  "target/playground/editor-artifact-measurement",
);
const buildsRoot = path.join(measurementRoot, "builds");
const measurementConfig = path.join(
  playgroundRoot,
  "vite.editor-artifact-measurement.config.ts",
);
const viteCli = path.join(playgroundRoot, "node_modules/vite/bin/vite.js");
const equivalenceEvidencePath = path.join(
  repositoryRoot,
  "editor-language/token-equivalence-v1.json",
);
const options = parseOptions(process.argv.slice(2));

if (options.help) {
  printUsage();
  process.exit(0);
}

const requireFromBrowserTests = createRequire(
  path.join(playgroundRoot, "tests/package.json"),
);
const { chromium } = requireFromBrowserTests("playwright");
const playwrightVersion = requireFromBrowserTests(
  "playwright/package.json",
).version;
const generatedEquivalenceEvidence = await loadEquivalenceBaselines();
const initialRevision = repositoryRevision();

await mkdir(buildsRoot, { recursive: true });
if (!options.skipBuild) {
  prepareMeasurementInputs();
  for (const variant of EDITOR_ARTIFACT_VARIANTS) buildVariant(variant);
}

const builds = Object.fromEntries(
  await Promise.all(
    EDITOR_ARTIFACT_VARIANTS.map(async (variant) => [
      variant,
      await inspectBuild(variant),
    ]),
  ),
);
const servers = await createMeasurementServers(builds);

const runs = [];
const equivalence = {};
let browserVersion = null;
try {
  for (const variant of EDITOR_ARTIFACT_VARIANTS) {
    console.log(
      `[merman-playground] Running ${variant} ${EDITOR_ARTIFACT_FAMILY_COUNT}-family × ${EDITOR_ARTIFACT_QUERY_KINDS.length}-query semantic-equivalence matrix.`,
    );
    const measured = await measureSemanticEquivalence({
      baselines: generatedEquivalenceEvidence.baselines,
      build: builds[variant],
      headed: options.headed,
      server: servers[variant],
    });
    browserVersion = consistentBrowserVersion(
      browserVersion,
      measured.browserVersion,
    );
    equivalence[variant] = measured.matrix;
  }
  for (let block = 1; block <= options.blocks; block += 1) {
    const order = block % 2 === 1 ? ["full", "editor"] : ["editor", "full"];
    for (const [index, variant] of order.entries()) {
      console.log(
        `[merman-playground] Measuring block ${block}/${options.blocks}, ${variant} (${index + 1}/2).`,
      );
      const measured = await measureVariantPair({
        build: builds[variant],
        headed: options.headed,
        server: servers[variant],
      });
      browserVersion = consistentBrowserVersion(
        browserVersion,
        measured.browserVersion,
      );
      runs.push({
        block,
        cold: measured.cold,
        position: index + 1,
        variant,
        warm: measured.warm,
      });
    }
  }
} finally {
  await Promise.all(Object.values(servers).map((server) => server.close()));
}

const revision = repositoryRevision();
assertSameRevision(initialRevision, revision);
const receipt = createEditorArtifactReceipt({
  builds: Object.fromEntries(
    Object.entries(builds).map(([variant, build]) => [
      variant,
      {
        manifestSha256: build.manifestSha256,
        mainWasm: build.mainWasm,
        outDir: path.relative(repositoryRoot, build.outDir),
        staticBytes: build.staticBytes,
        workerBundle: build.workerBundle,
        workerWasm: build.workerWasm,
      },
    ]),
  ),
  environment: {
    architecture: process.arch,
    browser: `Chromium ${browserVersion ?? "unknown"}`,
    cpu: os.cpus()[0]?.model ?? "unknown",
    logicalCpuCount: os.cpus().length,
    memoryBytes: os.totalmem(),
    node: process.version,
    operatingSystem: `${os.platform()} ${os.release()}`,
    playwright: playwrightVersion,
    transferEncoding: "gzip",
  },
  generatedAt: new Date().toISOString(),
  equivalence,
  parameters: {
    blocks: options.blocks,
    browserMode: options.headed ? "headed" : "headless",
    buildMode: options.skipBuild ? "reuse-existing" : "fresh-dedicated-builds",
    cachePolicy: {
      hashedAssets: "public, max-age=31536000, immutable",
      html: "no-cache",
    },
    coldDefinition: "fresh Chromium process and browser context",
    equivalenceDefinition: `one generated family-baseline source for each of ${EDITOR_ARTIFACT_FAMILY_COUNT} families; all ${EDITOR_ARTIFACT_QUERY_KINDS.length} production WorkerClient queries execute in an explicit module Worker and each canonical result or request-local error is SHA-256 bound`,
    equivalenceEvidence:
      "editor-language/token-equivalence-v1.json generated from playground/examples/manifest.json",
    equivalenceEvidenceSha256: generatedEquivalenceEvidence.sha256,
    memoryDefinition:
      "maximum sampled startup bytes from performance.measureUserAgentSpecificMemory in cross-origin-isolated Chromium; the measurement fails instead of substituting a narrower heap scope",
    order: "odd blocks full/editor; even blocks editor/full",
    primaryLatencies: [
      "workerReadyMs",
      "firstDiagnosticsMs",
      "mainFirstResultMs",
    ],
    transferDefinition:
      "sum of gzip response-body bytes served by the dedicated same-origin measurement server across page and Worker requests; HTTP headers are excluded",
    warmDefinition:
      "same browser context after navigating the measured cold page to about:blank; HTTP/code caches remain while Window and Worker realms are recreated",
  },
  revision,
  runs,
});

const receiptPath = path.resolve(
  repositoryRoot,
  options.out ?? DEFAULT_EDITOR_ARTIFACT_RECEIPT_PATH,
);
await mkdir(path.dirname(receiptPath), { recursive: true });
await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(
  [
    `[merman-playground] Editor artifact receipt: ${path.relative(repositoryRoot, receiptPath)}`,
    `  Authority: ${receipt.authority.authoritative ? "authoritative" : "provisional"}`,
    `  Selected: ${receipt.decision.selected}`,
    ...receipt.authority.reasons.map((reason) => `  - ${reason}`),
    ...receipt.decision.reasons.map((reason) => `  - ${reason}`),
  ].join("\n"),
);

function prepareMeasurementInputs() {
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

function buildVariant(variant) {
  const outDir = variantOutDir(variant);
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

async function createMeasurementServers(builds) {
  const servers = {};
  try {
    for (const variant of EDITOR_ARTIFACT_VARIANTS) {
      servers[variant] = await createMeasurementServer(
        builds[variant].staticFiles,
      );
    }
    return servers;
  } catch (error) {
    await Promise.all(Object.values(servers).map((server) => server.close()));
    throw error;
  }
}

async function inspectBuild(variant) {
  const outDir = variantOutDir(variant);
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

async function loadEquivalenceBaselines() {
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

async function measureSemanticEquivalence({
  baselines,
  build,
  headed,
  server,
}) {
  const browser = await chromium.launch({
    channel: "chromium",
    headless: !headed,
  });
  const browserVersion = browser.version();
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    await page.goto(new URL(build.equivalenceHtml, server.url).href, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    });
    await page.waitForFunction(
      () => globalThis.__mermanEditorArtifactEquivalenceV1?.status === "ready",
      undefined,
      { timeout: 30_000 },
    );
    await page.evaluate((fixtures) => {
      globalThis.__runMermanEditorArtifactEquivalenceV1(fixtures);
    }, baselines);
    await page.waitForFunction(
      () => {
        const status =
          globalThis.__mermanEditorArtifactEquivalenceV1?.status ?? "missing";
        return status === "complete" || status === "error";
      },
      undefined,
      { timeout: 180_000 },
    );
    const state = await page.evaluate(
      () => globalThis.__mermanEditorArtifactEquivalenceV1,
    );
    await context.close();
    if (state.status === "error") {
      throw new Error(
        `${build.id} semantic-equivalence matrix failed: ${state.error.message}${state.error.stack ? `\n${state.error.stack}` : ""}`,
      );
    }
    if (state.status !== "complete") {
      throw new Error(
        `${build.id} semantic-equivalence matrix returned ${state.status}.`,
      );
    }
    return { browserVersion, matrix: state.matrix };
  } finally {
    await browser.close();
  }
}

async function measureVariantPair({ build, headed, server }) {
  const browser = await chromium.launch({
    channel: "chromium",
    headless: !headed,
    args: ["--enable-precise-memory-info"],
  });
  const browserVersion = browser.version();
  try {
    const context = await browser.newContext();
    await context.addInitScript(installBrowserMeasurementInstrumentation);
    const page = await context.newPage();

    const cold = await measureNavigation({
      build,
      mode: "cold",
      page,
      server,
    });
    await page.goto("about:blank", { waitUntil: "load" });
    const warm = await measureNavigation({
      build,
      mode: "warm",
      page,
      server,
    });

    await context.close();
    return { browserVersion, cold, warm };
  } finally {
    await browser.close();
  }
}

async function measureNavigation({ build, mode, page, server }) {
  server.beginObservation();
  await page.goto(server.url, {
    waitUntil: "domcontentloaded",
    timeout: 60_000,
  });
  await page.waitForFunction(
    () => {
      const state = globalThis.__mermanEditorArtifactMeasurementV1;
      return (
        state &&
        (typeof state.firstDiagnosticsError === "string" ||
          (Number.isFinite(state.workerReadyAtMs) &&
            Number.isFinite(state.firstDiagnosticsAtMs) &&
            Number.isFinite(state.mainReadyAtMs) &&
            Number.isFinite(state.mainFirstResultAtMs)))
      );
    },
    undefined,
    { timeout: 60_000 },
  );
  await page.waitForTimeout(300);
  await page.evaluate(() => {
    globalThis.__mermanEditorArtifactMeasurementV1.stopMemory = true;
  });
  await page
    .waitForFunction(
      () => globalThis.__mermanEditorArtifactMeasurementV1?.memoryDone === true,
      undefined,
      { timeout: 5_000 },
    )
    .catch(() => undefined);
  const state = await page.evaluate(() => {
    const value = globalThis.__mermanEditorArtifactMeasurementV1;
    return {
      firstDiagnosticsError: value.firstDiagnosticsError,
      firstDiagnosticsAtMs: value.firstDiagnosticsAtMs,
      mainFirstResultAtMs: value.mainFirstResultAtMs,
      mainReadyAtMs: value.mainReadyAtMs,
      memoryErrors: value.memoryErrors,
      memorySamples: value.memorySamples,
      memoryScope: value.memoryScope,
      timeOrigin: performance.timeOrigin,
      workerReadyAtMs: value.workerReadyAtMs,
    };
  });
  const serverNetwork = server.endObservation();
  if (typeof state.firstDiagnosticsError === "string") {
    throw new Error(
      `${build.id} ${mode} first diagnostics failed: ${state.firstDiagnosticsError}.`,
    );
  }
  const mainArtifactReadyAtMs = artifactReadyAtMs(
    serverNetwork.requests,
    build.mainWasm.file,
    state.timeOrigin,
    mode,
  );
  const workerArtifactReadyAtMs = artifactReadyAtMs(
    serverNetwork.requests,
    build.workerWasm.file,
    state.timeOrigin,
    mode,
  );
  const memorySamples = state.memorySamples.filter(
    (sample) => Number.isFinite(sample.bytes) && sample.bytes >= 0,
  );
  if (memorySamples.length === 0 || typeof state.memoryScope !== "string") {
    throw new Error(
      `${build.id} ${mode} run returned no peak-memory samples: ${state.memoryErrors.join("; ") || "unknown reason"}.`,
    );
  }

  return {
    firstDiagnosticsMs: state.firstDiagnosticsAtMs,
    mainCompileInitializeMs: Math.max(
      0,
      state.mainReadyAtMs - mainArtifactReadyAtMs,
    ),
    mainFirstResultMs: state.mainFirstResultAtMs,
    network: serverNetwork,
    peakMemory: {
      bytes: Math.max(...memorySamples.map((sample) => sample.bytes)),
      samples: memorySamples,
      scope: state.memoryScope,
    },
    totalTransferBytes: serverNetwork.bodyBytes,
    workerCompileInitializeMs: Math.max(
      0,
      state.workerReadyAtMs - workerArtifactReadyAtMs,
    ),
    workerReadyMs: state.workerReadyAtMs,
  };
}

async function createMeasurementServer(files) {
  let observation = null;
  const server = createServer((request, response) => {
    let pathname;
    try {
      pathname = decodeURIComponent(
        new URL(request.url ?? "/", "http://local").pathname,
      );
    } catch {
      response.writeHead(400).end("Bad request");
      return;
    }
    if (pathname === "/") pathname = "/index.html";
    const file = files.get(pathname);
    if (!file) {
      response.writeHead(404).end("Not found");
      return;
    }
    const acceptsGzip = /(?:^|,)\s*gzip\s*(?:,|$)/iu.test(
      request.headers["accept-encoding"] ?? "",
    );
    const body = acceptsGzip && file.gzip ? file.gzip : file.body;
    const headers = {
      "Cache-Control": file.immutable
        ? "public, max-age=31536000, immutable"
        : "no-cache",
      "Content-Length": String(body.byteLength),
      "Content-Type": file.contentType,
      "Cross-Origin-Embedder-Policy": "require-corp",
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Resource-Policy": "same-origin",
    };
    if (acceptsGzip && file.gzip) headers["Content-Encoding"] = "gzip";
    response.writeHead(200, headers);
    if (observation) {
      const bodyBytes = request.method === "HEAD" ? 0 : body.byteLength;
      const observedRequest = {
        bodyBytes,
        cacheControl: headers["Cache-Control"],
        contentEncoding: headers["Content-Encoding"] ?? "identity",
        finishedWallTimeMs: null,
        method: request.method ?? "GET",
        pathname,
      };
      observation.bodyBytes += bodyBytes;
      observation.requests.push(observedRequest);
      response.once("finish", () => {
        observedRequest.finishedWallTimeMs = Date.now();
      });
    }
    if (request.method !== "HEAD") response.end(body);
    else response.end();
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("Measurement server did not expose a TCP address.");
  }

  return {
    beginObservation() {
      if (observation) throw new Error("Server measurement is already active.");
      observation = { bodyBytes: 0, requests: [] };
    },
    close: () => new Promise((resolve) => server.close(resolve)),
    endObservation() {
      if (!observation) throw new Error("Server measurement is not active.");
      const result = observation;
      observation = null;
      return result;
    },
    url: `http://127.0.0.1:${address.port}/`,
  };
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

function artifactReadyAtMs(requests, artifactFile, timeOrigin, mode) {
  const normalized = `/${artifactFile.replaceAll("\\", "/")}`;
  const completions = requests
    .filter((request) => request.pathname === normalized)
    .map((request) => request.finishedWallTimeMs)
    .filter(Number.isFinite);
  if (completions.length > 0) {
    return Math.max(0, Math.max(...completions) - timeOrigin);
  }
  if (mode === "warm") return 0;
  throw new Error(
    `Cold run did not observe artifact completion for ${artifactFile}.`,
  );
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

function repositoryRevision() {
  const commit = captureCommand("git", ["rev-parse", "HEAD"], repositoryRoot);
  const status = captureCommand(
    "git",
    ["status", "--porcelain=v1", "--untracked-files=all"],
    repositoryRoot,
  );
  return {
    commit,
    dirty: status.length > 0,
    statusSha256: sha256(Buffer.from(status)),
  };
}

function assertSameRevision(before, after) {
  if (
    before.commit !== after.commit ||
    before.dirty !== after.dirty ||
    before.statusSha256 !== after.statusSha256
  ) {
    throw new Error(
      "Repository revision or worktree status changed during editor artifact measurement.",
    );
  }
}

function consistentBrowserVersion(current, observed) {
  if (current !== null && current !== observed) {
    throw new Error(
      `Editor artifact measurement switched Chromium versions from ${current} to ${observed}.`,
    );
  }
  return observed;
}

function runCommand(command, args, cwd, env = process.env) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    env,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status}.`,
    );
  }
}

function captureCommand(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      result.stderr.trim() || `${command} failed with ${result.status}.`,
    );
  }
  return result.stdout.trim();
}

function parseOptions(args) {
  const parsed = {
    blocks: 4,
    headed: false,
    help: false,
    out: null,
    skipBuild: false,
  };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--headed") parsed.headed = true;
    else if (arg === "--help" || arg === "-h") parsed.help = true;
    else if (arg === "--skip-build") parsed.skipBuild = true;
    else if (arg === "--blocks" || arg === "--out") {
      const value = args[index + 1];
      if (!value || value.startsWith("--"))
        throw new Error(`Missing value for ${arg}.`);
      index += 1;
      if (arg === "--blocks") {
        parsed.blocks = Number(value);
        if (
          !Number.isSafeInteger(parsed.blocks) ||
          parsed.blocks < 2 ||
          parsed.blocks % 2 !== 0
        ) {
          throw new Error(
            "--blocks must be an even integer of at least 2 for balanced AB/BA evidence.",
          );
        }
      } else parsed.out = value;
    } else throw new Error(`Unknown argument: ${arg}`);
  }
  return parsed;
}

function printUsage() {
  console.log(
    [
      "usage: node scripts/editor-artifact-measurement/measure.mjs [options]",
      "",
      "  --blocks <count>  balanced AB/BA block count (default: 4, even and at least 2)",
      "  --headed          show Chromium during measurement",
      `  --out <path>      receipt path (default: ${DEFAULT_EDITOR_ARTIFACT_RECEIPT_PATH})`,
      "  --skip-build      reuse existing dedicated full/editor build directories",
    ].join("\n"),
  );
}

function variantOutDir(variant) {
  return path.join(buildsRoot, variant);
}

function npmCommand() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}

function normalizeSource(source) {
  return source.replaceAll("\\", "/");
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function contentTypeFor(file) {
  switch (path.extname(file).toLowerCase()) {
    case ".css":
      return "text/css; charset=utf-8";
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
    case ".mjs":
      return "text/javascript; charset=utf-8";
    case ".json":
    case ".map":
      return "application/json; charset=utf-8";
    case ".svg":
      return "image/svg+xml";
    case ".wasm":
      return "application/wasm";
    case ".woff":
      return "font/woff";
    case ".woff2":
      return "font/woff2";
    default:
      return "application/octet-stream";
  }
}

function isCompressible(contentType) {
  return (
    contentType.startsWith("text/") ||
    contentType.startsWith("application/json") ||
    contentType === "application/wasm" ||
    contentType === "image/svg+xml"
  );
}

function installBrowserMeasurementInstrumentation() {
  const state = {
    firstDiagnosticsAtMs: null,
    firstDiagnosticsError: null,
    mainFirstResultAtMs: null,
    mainReadyAtMs: null,
    memoryDone: false,
    memoryErrors: [],
    memorySamples: [],
    memoryScope: "user-agent-specific-memory",
    stopMemory: false,
    workerReadyAtMs: null,
  };
  Object.defineProperty(globalThis, "__mermanEditorArtifactMeasurementV1", {
    configurable: false,
    enumerable: false,
    value: state,
    writable: false,
  });
  try {
    localStorage.setItem("merman-language", "en");
  } catch {
    // The measured HTTP document provides storage; opaque bootstrap documents may not.
  }

  const pending = new Map();
  const NativeWorker = globalThis.Worker;
  class MeasuredWorker extends NativeWorker {
    constructor(scriptURL, options) {
      super(scriptURL, options);
      this.__mermanMeasured = /merman-language\.worker/iu.test(
        String(scriptURL),
      );
      if (!this.__mermanMeasured) return;
      this.addEventListener("message", (event) => {
        const message = event.data;
        if (!message || typeof message !== "object") return;
        const request = pending.get(message.requestId);
        if (message.type === "ready" && request?.type === "initialize") {
          state.workerReadyAtMs ??= performance.now();
        }
        if (request?.type === "query" && request.kind === "diagnostics") {
          if (message.type === "queryResult") {
            state.firstDiagnosticsAtMs ??= performance.now();
          } else if (message.type === "error") {
            state.firstDiagnosticsError ??= `${String(message.code ?? "QUERY_FAILED")}: ${String(message.message ?? "unknown error")}`;
          }
        }
        if (Number.isSafeInteger(message.requestId))
          pending.delete(message.requestId);
      });
    }

    postMessage(message, transferOrOptions) {
      if (
        this.__mermanMeasured &&
        message &&
        typeof message === "object" &&
        Number.isSafeInteger(message.requestId)
      ) {
        pending.set(message.requestId, {
          kind: message.query?.kind ?? null,
          type: message.type,
        });
      }
      return arguments.length === 1
        ? super.postMessage(message)
        : super.postMessage(message, transferOrOptions);
    }
  }
  Object.defineProperty(globalThis, "Worker", {
    configurable: true,
    value: MeasuredWorker,
    writable: true,
  });

  const shadowRoots = new Set();
  const inspectPresentation = () => {
    if (
      state.mainReadyAtMs === null &&
      /WASM:\s*Ready\b/u.test(document.body?.innerText ?? "")
    ) {
      state.mainReadyAtMs = performance.now();
    }
    if (state.mainFirstResultAtMs !== null) return;
    for (const root of shadowRoots) {
      if (root.querySelector("svg")) {
        state.mainFirstResultAtMs = performance.now();
        return;
      }
    }
    for (const host of document.querySelectorAll(".preview-container > div")) {
      if (host.shadowRoot?.querySelector("svg")) {
        state.mainFirstResultAtMs = performance.now();
        return;
      }
    }
  };
  const observer = new MutationObserver(inspectPresentation);
  const nativeAttachShadow = Element.prototype.attachShadow;
  Element.prototype.attachShadow = function attachMeasuredShadow(init) {
    const root = nativeAttachShadow.call(this, init);
    shadowRoots.add(root);
    observer.observe(root, { childList: true, subtree: true });
    queueMicrotask(inspectPresentation);
    return root;
  };
  document.addEventListener(
    "DOMContentLoaded",
    () => {
      observer.observe(document.documentElement, {
        childList: true,
        subtree: true,
      });
      inspectPresentation();
    },
    { once: true },
  );

  const sampleMemory = async () => {
    if (typeof performance.measureUserAgentSpecificMemory !== "function") {
      state.memoryErrors.push("measureUserAgentSpecificMemory is unavailable");
      return;
    }
    try {
      const measured = await performance.measureUserAgentSpecificMemory();
      state.memorySamples.push({
        atMs: performance.now(),
        bytes: measured.bytes,
      });
    } catch (error) {
      state.memoryErrors.push(String(error));
    }
  };
  void (async () => {
    while (!state.stopMemory && performance.now() < 60_000) {
      await sampleMemory();
      if (state.stopMemory) break;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    if (state.memorySamples.length === 0) await sampleMemory();
    state.memoryDone = true;
  })();
}
