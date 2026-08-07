import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

import {
  CHECKED_EDITOR_ARTIFACT_RECEIPT_PATH,
  validateEditorArtifactReceipt,
} from "./contract.mjs";
import {
  collectSourceClosure,
  createTypeScriptSourceGraph,
} from "../typescript-source-graph.mjs";

const PACKAGE_SELECTIONS = Object.freeze({
  editor: Object.freeze({
    "@mermanjs/web": "file:../platforms/web/packages/full",
    "@mermanjs/web-editor": "file:../platforms/web/packages/editor",
  }),
  full: Object.freeze({
    "@mermanjs/web": "file:../platforms/web/packages/full",
  }),
});
const WORKER_PACKAGE_SELECTIONS = Object.freeze({
  editor: Object.freeze(["@mermanjs/web-editor"]),
  full: Object.freeze(["@mermanjs/web"]),
});
const BROWSER_WORKER_ENTRY = "src/editor/worker-browser.ts";

export function verifyEditorArtifactSelectionTopology({
  packageDependencies,
  packageLock,
  receipt,
  workerStartupGraph,
  workerGraph,
}) {
  const validated = validateEditorArtifactReceipt(receipt);
  assert.equal(
    validated.authority.authoritative,
    true,
    `Editor artifact receipt is provisional: ${validated.authority.reasons.join("; ")}`,
  );
  const selectedDependencies = Object.fromEntries(
    Object.entries(packageDependencies)
      .filter(([name]) => name.startsWith("@mermanjs/web"))
      .sort(([left], [right]) => left.localeCompare(right, "en")),
  );
  assert.deepEqual(
    selectedDependencies,
    PACKAGE_SELECTIONS[validated.decision.selected],
    `Playground dependencies do not match the measured ${validated.decision.selected} Worker artifact selection.`,
  );
  const lockedDependencies = selectWebDependencies(
    packageLock?.packages?.[""]?.dependencies ?? {},
  );
  assert.deepEqual(
    lockedDependencies,
    selectedDependencies,
    "Playground package lock does not match the selected Web dependencies.",
  );
  for (const packageName of Object.keys(selectedDependencies)) {
    const lockedPackage = packageLock?.packages?.[`node_modules/${packageName}`];
    assert.equal(
      lockedPackage?.link,
      true,
      `Playground package lock must link ${packageName} to its local Web package.`,
    );
    assert.equal(
      lockedPackage?.resolved,
      selectedDependencies[packageName].replace(/^file:/u, ""),
      `Playground package lock must resolve ${packageName} to the selected local Web package.`,
    );
  }
  const workerEntry = resolveLanguageWorkerEntry(workerStartupGraph);
  assert.deepEqual(
    workerWebPackageImports(workerGraph, workerEntry),
    WORKER_PACKAGE_SELECTIONS[validated.decision.selected],
    `Language Worker imports do not match the measured ${validated.decision.selected} artifact selection.`,
  );
  return Object.freeze({
    receiptCommit: validated.revision.commit,
    selected: validated.decision.selected,
  });
}

export function workerWebPackageImports(
  graph,
  root,
) {
  const runtimeClosure = collectSourceClosure(graph, [root], {
    includeDynamic: true,
  });
  return [
    ...new Set(
      graph.edges
        .filter(
          (edge) =>
            runtimeClosure.has(edge.from) &&
            edge.external &&
            edge.kind !== "type" &&
            edge.specifier.startsWith("@mermanjs/web"),
        )
        .map((edge) => edge.specifier),
    ),
  ].sort((left, right) => left.localeCompare(right, "en"));
}

export function resolveLanguageWorkerEntry(graph) {
  const browserClosure = collectSourceClosure(graph, [BROWSER_WORKER_ENTRY], {
    includeDynamic: true,
  });
  const workerEdges = graph.edges.filter(
    (edge) =>
      browserClosure.has(edge.from) &&
      edge.kind !== "type" &&
      edge.to !== null &&
      /[?&]worker(?:[&=]|$)/u.test(edge.specifier),
  );
  assert.equal(
    workerEdges.length,
    1,
    `Production editor startup must resolve exactly one runtime ?worker import; found ${workerEdges.length}.`,
  );
  return workerEdges[0].to;
}

async function main() {
  const repositoryRoot = path.resolve(import.meta.dirname, "../../..");
  const receiptPath = path.join(
    repositoryRoot,
    CHECKED_EDITOR_ARTIFACT_RECEIPT_PATH,
  );
  const packagePath = path.join(repositoryRoot, "playground/package.json");
  const packageLockPath = path.join(
    repositoryRoot,
    "playground/package-lock.json",
  );
  const [receipt, packageJson, packageLock] = await Promise.all(
    [receiptPath, packagePath, packageLockPath].map(async (file) =>
      JSON.parse(await readFile(file, "utf8")),
    ),
  );
  const workerStartupGraph = createTypeScriptSourceGraph({
    rootDir: path.join(repositoryRoot, "playground"),
    entries: [BROWSER_WORKER_ENTRY],
  });
  const verified = verifyEditorArtifactSelectionTopology({
    packageDependencies: packageJson.dependencies ?? {},
    packageLock,
    receipt,
    workerStartupGraph,
    workerGraph: createTypeScriptSourceGraph({
      rootDir: path.join(repositoryRoot, "playground"),
      entries: [resolveLanguageWorkerEntry(workerStartupGraph)],
    }),
  });
  console.log(
    `[merman-playground] Editor Worker artifact topology matches the recorded ${verified.selected} selection from ${verified.receiptCommit}.`,
  );
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  await main();
}

function selectWebDependencies(dependencies) {
  return Object.fromEntries(
    Object.entries(dependencies)
      .filter(([name]) => name.startsWith("@mermanjs/web"))
      .sort(([left], [right]) => left.localeCompare(right, "en")),
  );
}
