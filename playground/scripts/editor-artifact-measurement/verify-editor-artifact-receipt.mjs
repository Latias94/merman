import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

import {
  CHECKED_EDITOR_ARTIFACT_RECEIPT_PATH,
  validateEditorArtifactReceipt,
} from "./contract.mjs";
import { editorArtifactSelectionInputs } from "./selection-inputs.mjs";
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

export function verifyEditorArtifactAuthority({
  packageDependencies,
  packageLock,
  receipt,
  selectionInputs,
  workerGraph,
}) {
  const validated = validateEditorArtifactReceipt(receipt);
  assert.equal(
    validated.authority.authoritative,
    true,
    `Editor artifact receipt is provisional: ${validated.authority.reasons.join("; ")}`,
  );
  assert.equal(
    validated.selectionInputs.schemaVersion,
    selectionInputs.schemaVersion,
    "Editor artifact selection-input contract changed. Advance the receipt schema and rerun R16 measurement.",
  );
  for (const field of [
    "measurementContractSha256",
    "startupClosureSha256",
    "workerClosureSha256",
    "fullPackageProvenanceSha256",
    "editorPackageProvenanceSha256",
    "equivalenceEvidenceSha256",
  ]) {
    assert.equal(
      validated.selectionInputs[field],
      selectionInputs[field],
      `Editor artifact ${field} changed. Run the on-demand R16 measurement from a clean worktree.`,
    );
  }
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
  }
  assert.deepEqual(
    workerWebPackageImports(workerGraph),
    WORKER_PACKAGE_SELECTIONS[validated.decision.selected],
    `Language Worker imports do not match the measured ${validated.decision.selected} artifact selection.`,
  );
  return Object.freeze({
    commit: validated.revision.commit,
    selected: validated.decision.selected,
    selectionInputs,
  });
}

export function workerWebPackageImports(
  graph,
  root = "src/editor/merman-language.worker.ts",
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
  const verified = verifyEditorArtifactAuthority({
    packageDependencies: packageJson.dependencies ?? {},
    packageLock,
    receipt,
    selectionInputs: editorArtifactSelectionInputs(repositoryRoot),
    workerGraph: createTypeScriptSourceGraph({
      rootDir: path.join(repositoryRoot, "playground"),
      entries: ["src/editor/merman-language.worker.ts"],
    }),
  });
  console.log(
    `[merman-playground] Editor Worker artifact: ${verified.selected}; selection-input schema ${verified.selectionInputs.schemaVersion}.`,
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
