import { mkdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";

import {
  DEFAULT_EDITOR_ARTIFACT_RECEIPT_PATH,
  EDITOR_ARTIFACT_VARIANTS,
  createEditorArtifactReceipt,
} from "./contract.mjs";
import {
  EDITOR_ARTIFACT_FAMILY_COUNT,
  EDITOR_ARTIFACT_QUERY_KINDS,
} from "./equivalence-shared.mjs";
import {
  buildVariant,
  inspectBuild,
  loadEquivalenceBaselines,
  prepareMeasurementInputs,
} from "./measurement-artifacts.mjs";
import {
  measureSemanticEquivalence,
  measureVariantPair,
} from "./measurement-browser.mjs";
import {
  assertSameRevision,
  consistentBrowserVersion,
  parseOptions,
  printUsage,
  repositoryRevision,
} from "./measurement-cli.mjs";
import { createMeasurementServers } from "./measurement-server.mjs";
import { editorArtifactSelectionInputs } from "./selection-inputs.mjs";

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
  "contracts/editor-language/token-equivalence-v1.json",
);
const measurementPaths = {
  buildsRoot,
  measurementConfig,
  playgroundRoot,
  repositoryRoot,
  viteCli,
};
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
const generatedEquivalenceEvidence =
  await loadEquivalenceBaselines(equivalenceEvidencePath);
const initialRevision = repositoryRevision(repositoryRoot);

await mkdir(buildsRoot, { recursive: true });
if (!options.skipBuild) {
  prepareMeasurementInputs(measurementPaths);
  for (const variant of EDITOR_ARTIFACT_VARIANTS) {
    buildVariant(variant, measurementPaths);
  }
}

const builds = Object.fromEntries(
  await Promise.all(
    EDITOR_ARTIFACT_VARIANTS.map(async (variant) => [
      variant,
      await inspectBuild(variant, measurementPaths),
    ]),
  ),
);
const selectionInputs = editorArtifactSelectionInputs(repositoryRoot);
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
      chromium,
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
        chromium,
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

const revision = repositoryRevision(repositoryRoot);
assertSameRevision(initialRevision, revision);
const finalSelectionInputs = editorArtifactSelectionInputs(repositoryRoot);
if (!isDeepStrictEqual(selectionInputs, finalSelectionInputs)) {
  throw new Error(
    "Editor artifact selection inputs changed during measurement.",
  );
}
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
      "contracts/editor-language/token-equivalence-v1.json generated from playground/examples/manifest.json",
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
  selectionInputs,
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
