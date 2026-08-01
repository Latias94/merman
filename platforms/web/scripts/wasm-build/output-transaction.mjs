import { randomUUID } from "node:crypto";
import {
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
  { preserveStage = null } = {},
) {
  const resolved = path.resolve(outputRoot);
  const backup = backupDirectory(resolved);
  if (existsSync(backup)) {
    if (hasCommittedManifest(resolved)) {
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
  { onPublishStep = () => {} } = {},
) {
  const stage = path.resolve(stageRoot);
  const output = path.resolve(outputRoot);
  const stagedManifest = path.join(stage, WASM_INPUT_MANIFEST_NAME);
  if (!existsSync(stagedManifest)) {
    throw new Error(`Staged WASM input manifest is missing: ${stagedManifest}`);
  }

  recoverOutputTransaction(output, { preserveStage: stage });
  const backup = backupDirectory(output);
  rmSync(backup, { recursive: true, force: true });

  if (existsSync(output)) renameSync(output, backup);
  try {
    onPublishStep("old-output-backed-up");
    renameSync(stage, output);
    onPublishStep("new-output-published");
    rmSync(backup, { recursive: true, force: true });
  } catch (error) {
    rmSync(output, { recursive: true, force: true });
    if (existsSync(backup)) renameSync(backup, output);
    rmSync(backup, { recursive: true, force: true });
    rmSync(stage, { recursive: true, force: true });
    throw error;
  }
}

export function cleanupOutputStage(stageRoot) {
  rmSync(stageRoot, { recursive: true, force: true });
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
