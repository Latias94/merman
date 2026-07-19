import { randomUUID } from "node:crypto";
import {
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  renameSync,
  rmSync,
} from "node:fs";
import path from "node:path";

import { WASM_INPUT_MANIFEST_NAME } from "./input-manifest.mjs";

export function createOutputStage(outputRoot) {
  const resolved = path.resolve(outputRoot);
  const stage = path.join(
    path.dirname(resolved),
    `.${path.basename(resolved)}.merman-wasm.stage-${process.pid}-${randomUUID()}`,
  );
  mkdirSync(stage, { recursive: false });
  return stage;
}

export function outputBackupDirectory(outputRoot) {
  return backupDirectory(path.resolve(outputRoot));
}

export function recoverOutputTransaction(
  outputRoot,
  { preserveStage = null, rootPackage = false } = {},
) {
  const resolved = path.resolve(outputRoot);
  const backup = backupDirectory(resolved);
  if (existsSync(backup)) {
    if (hasCommittedManifest(resolved)) {
      rmSync(backup, { recursive: true, force: true });
    } else if (rootPackage) {
      removeRootOwnedEntries(resolved);
      mkdirSync(resolved, { recursive: true });
      moveDirectoryEntries(backup, resolved, { manifestLast: true });
      rmSync(backup, { recursive: true, force: true });
    } else {
      rmSync(resolved, { recursive: true, force: true });
      renameSync(backup, resolved);
    }
  }
  removeAbandonedStages(resolved, preserveStage);
}

export function publishStagedOutput(
  stageRoot,
  outputRoot,
  { onPublishStep = () => {}, rootPackage = false } = {},
) {
  const stage = path.resolve(stageRoot);
  const output = path.resolve(outputRoot);
  const stagedManifest = path.join(stage, WASM_INPUT_MANIFEST_NAME);
  if (!existsSync(stagedManifest)) {
    throw new Error(`Staged WASM input manifest is missing: ${stagedManifest}`);
  }

  recoverOutputTransaction(output, { preserveStage: stage, rootPackage });
  const backup = backupDirectory(output);
  rmSync(backup, { recursive: true, force: true });

  if (!rootPackage) {
    if (existsSync(output)) renameSync(output, backup);
    try {
      onPublishStep("old-output-backed-up");
      renameSync(stage, output);
      onPublishStep("new-output-published");
      rmSync(backup, { recursive: true, force: true });
    } catch (error) {
      rmSync(output, { recursive: true, force: true });
      if (existsSync(backup)) renameSync(backup, output);
      rmSync(stage, { recursive: true, force: true });
      throw error;
    }
    return;
  }

  mkdirSync(output, { recursive: true });
  mkdirSync(backup);
  let backupComplete = false;
  try {
    copyRootOwnedEntries(output, backup);
    backupComplete = true;
    removeRootOwnedEntries(output);
    onPublishStep("old-output-backed-up");
    moveDirectoryEntries(stage, output, {
      manifestLast: true,
      onMoved: (name) => onPublishStep(`new-entry-published:${name}`),
    });
    rmSync(backup, { recursive: true, force: true });
    rmSync(stage, { recursive: true, force: true });
  } catch (error) {
    if (backupComplete) {
      removeRootOwnedEntries(output);
      moveDirectoryEntries(backup, output, { manifestLast: true });
    }
    rmSync(backup, { recursive: true, force: true });
    rmSync(stage, { recursive: true, force: true });
    throw error;
  }
}

export function cleanupOutputStage(stageRoot) {
  rmSync(stageRoot, { recursive: true, force: true });
}

function copyRootOwnedEntries(from, to) {
  if (!existsSync(from)) return;
  mkdirSync(to, { recursive: true });
  const entries = readdirSync(from, { withFileTypes: true })
    .filter((entry) => entry.isFile() || (entry.isDirectory() && entry.name === "snippets"))
    .sort((left, right) => compareNames(left.name, right.name));
  for (const entry of entries) {
    const source = path.join(from, entry.name);
    const target = path.join(to, entry.name);
    if (entry.isDirectory()) {
      cpSync(source, target, { recursive: true, errorOnExist: true });
    } else {
      copyFileSync(source, target);
    }
  }
}

function moveDirectoryEntries(
  from,
  to,
  { manifestLast = false, onMoved = () => {} } = {},
) {
  if (!existsSync(from)) return;
  mkdirSync(to, { recursive: true });
  const entries = readdirSync(from, { withFileTypes: true })
    .sort((left, right) => compareNames(left.name, right.name));
  if (manifestLast) {
    entries.sort((left, right) => manifestRank(right.name) - manifestRank(left.name));
  }
  for (const entry of entries) {
    renameSync(path.join(from, entry.name), path.join(to, entry.name));
    onMoved(entry.name);
  }
}

function removeRootOwnedEntries(output) {
  if (!existsSync(output)) return;
  for (const entry of readdirSync(output, { withFileTypes: true })) {
    if (entry.isFile() || (entry.isDirectory() && entry.name === "snippets")) {
      rmSync(path.join(output, entry.name), { recursive: true, force: true });
    }
  }
}

function removeAbandonedStages(output, preserveStage) {
  const parent = path.dirname(output);
  if (!existsSync(parent)) return;
  const prefix = `.${path.basename(output)}.merman-wasm.stage-`;
  for (const entry of readdirSync(parent, { withFileTypes: true })) {
    if (entry.isDirectory() && entry.name.startsWith(prefix)) {
      const candidate = path.join(parent, entry.name);
      if (!preserveStage || path.resolve(candidate) !== path.resolve(preserveStage)) {
        rmSync(candidate, { recursive: true, force: true });
      }
    }
  }
}

function backupDirectory(output) {
  return path.join(
    path.dirname(output),
    `.${path.basename(output)}.merman-wasm.backup`,
  );
}

function hasCommittedManifest(output) {
  return existsSync(path.join(output, WASM_INPUT_MANIFEST_NAME));
}

function manifestRank(name) {
  return name === WASM_INPUT_MANIFEST_NAME ? 0 : 1;
}

function compareNames(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}
