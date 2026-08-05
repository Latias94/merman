import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import ts from "typescript";

import {
  CHECKED_EDITOR_ARTIFACT_RECEIPT_PATH,
  validateEditorArtifactReceipt,
} from "./contract.mjs";
import { editorArtifactSelectionInputs } from "./selection-inputs.mjs";

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
  workerSource,
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
    "webSurfaceSha256",
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
    workerWebPackageImports(workerSource),
    WORKER_PACKAGE_SELECTIONS[validated.decision.selected],
    `Language Worker imports do not match the measured ${validated.decision.selected} artifact selection.`,
  );
  return Object.freeze({
    commit: validated.revision.commit,
    selected: validated.decision.selected,
    selectionInputs,
  });
}

export function workerWebPackageImports(source) {
  const sourceFile = ts.createSourceFile(
    "merman-language.worker.ts",
    source,
    ts.ScriptTarget.Latest,
    false,
    ts.ScriptKind.TS,
  );
  const packages = new Set();
  for (const statement of sourceFile.statements) {
    if (
      ts.isImportDeclaration(statement) &&
      ts.isStringLiteralLike(statement.moduleSpecifier) &&
      statement.moduleSpecifier.text.startsWith("@mermanjs/web")
    ) {
      packages.add(statement.moduleSpecifier.text);
    }
  }
  return [...packages].sort((left, right) => left.localeCompare(right, "en"));
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
  const workerPath = path.join(
    repositoryRoot,
    "playground/src/editor/merman-language.worker.ts",
  );
  const [receipt, packageJson, packageLock, workerSource] = await Promise.all([
    ...[receiptPath, packagePath, packageLockPath].map(async (file) =>
      JSON.parse(await readFile(file, "utf8")),
    ),
    readFile(workerPath, "utf8"),
  ]);
  const verified = verifyEditorArtifactAuthority({
    packageDependencies: packageJson.dependencies ?? {},
    packageLock,
    receipt,
    selectionInputs: editorArtifactSelectionInputs(repositoryRoot),
    workerSource,
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
