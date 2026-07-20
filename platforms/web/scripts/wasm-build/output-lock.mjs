import { randomUUID } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000;
const DEFAULT_POLL_MS = 50;
const INCOMPLETE_OWNER_GRACE_MS = 5_000;
const waiter = new Int32Array(new SharedArrayBuffer(4));

export function outputLockDirectory(outputRoot) {
  const resolved = path.resolve(outputRoot);
  return path.join(
    path.dirname(resolved),
    `.${path.basename(resolved)}.merman-wasm.lock`,
  );
}

export function workspaceWasmBuildLockDirectory(
  repositoryRoot,
  { cargoTargetDirectory = process.env.CARGO_TARGET_DIR } = {},
) {
  const targetRoot = cargoTargetDirectory
    ? path.resolve(repositoryRoot, cargoTargetDirectory)
    : path.join(path.resolve(repositoryRoot), "target");
  return path.join(targetRoot, ".merman-wasm-build.lock");
}

export function acquireWorkspaceWasmBuildLock(repositoryRoot, options = {}) {
  const { cargoTargetDirectory, ...lockOptions } = options;
  return acquireDirectoryLock(
    workspaceWasmBuildLockDirectory(repositoryRoot, { cargoTargetDirectory }),
    lockOptions,
  );
}

export function acquireOutputLock(
  outputRoot,
  options = {},
) {
  return acquireDirectoryLock(outputLockDirectory(outputRoot), options);
}

export function acquireDirectoryLock(
  lockDirectory,
  {
    timeoutMs = DEFAULT_TIMEOUT_MS,
    pollMs = DEFAULT_POLL_MS,
    now = Date.now,
    processId = process.pid,
    processAlive = isProcessAlive,
  } = {},
) {
  const ownerPath = path.join(lockDirectory, "owner.json");
  const deadline = now() + timeoutMs;
  const token = randomUUID();
  mkdirSync(path.dirname(lockDirectory), { recursive: true });

  while (true) {
    try {
      mkdirSync(lockDirectory);
      try {
        writeFileSync(
          ownerPath,
          `${JSON.stringify({ pid: processId, started_at_ms: now(), token }, null, 2)}\n`,
          { flag: "wx" },
        );
      } catch (error) {
        rmSync(lockDirectory, { recursive: true, force: true });
        throw error;
      }
      return () => releaseOutputLock(lockDirectory, token);
    } catch (error) {
      if (!isAlreadyExists(error)) throw error;
    }

    if (removeStaleLock(lockDirectory, ownerPath, { now, processAlive })) {
      continue;
    }
    if (now() >= deadline) {
      throw new Error(
        `Timed out waiting for the WASM output lock: ${lockDirectory}`,
      );
    }
    Atomics.wait(waiter, 0, 0, Math.max(1, Math.min(pollMs, deadline - now())));
  }
}

function releaseOutputLock(lockDirectory, token) {
  const owner = readOwner(path.join(lockDirectory, "owner.json"));
  if (!owner || owner.token !== token) {
    throw new Error(`WASM output lock ownership changed unexpectedly: ${lockDirectory}`);
  }
  rmSync(lockDirectory, { recursive: true, force: true });
}

function removeStaleLock(lockDirectory, ownerPath, { now, processAlive }) {
  const owner = readOwner(ownerPath);
  if (owner && Number.isInteger(owner.pid) && owner.pid > 0) {
    if (processAlive(owner.pid)) return false;
    rmSync(lockDirectory, { recursive: true, force: true });
    return true;
  }

  let ageMs = 0;
  try {
    ageMs = now() - statSync(lockDirectory).mtimeMs;
  } catch (error) {
    if (isMissing(error)) return true;
    throw error;
  }
  if (ageMs < INCOMPLETE_OWNER_GRACE_MS) return false;
  rmSync(lockDirectory, { recursive: true, force: true });
  return true;
}

function readOwner(ownerPath) {
  try {
    const value = JSON.parse(readFileSync(ownerPath, "utf8"));
    return value && typeof value === "object" ? value : null;
  } catch (error) {
    if (isMissing(error) || error instanceof SyntaxError) return null;
    throw error;
  }
}

function isProcessAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return Boolean(error && typeof error === "object" && error.code === "EPERM");
  }
}

function isAlreadyExists(error) {
  return Boolean(error && typeof error === "object" && error.code === "EEXIST");
}

function isMissing(error) {
  return Boolean(error && typeof error === "object" && error.code === "ENOENT");
}

export function lockExists(outputRoot) {
  return existsSync(outputLockDirectory(outputRoot));
}
