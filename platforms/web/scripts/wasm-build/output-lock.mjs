import { randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
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

export function acquireOutputLock(outputRoot, options = {}) {
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
  mkdirSync(path.dirname(lockDirectory), { recursive: true });
  const deadline = now() + timeoutMs;

  while (true) {
    let created = false;
    try {
      mkdirSync(lockDirectory);
      created = true;
    } catch (error) {
      if (!isAlreadyExists(error)) throw error;
    }

    if (created) {
      return claimDirectoryLock(lockDirectory, {
        processId,
      });
    }

    if (recoverStaleLock(lockDirectory, { now, processAlive })) continue;

    if (now() >= deadline) {
      throw new Error(
        `Timed out waiting for the WASM output lock: ${lockDirectory}`,
      );
    }
    Atomics.wait(
      waiter,
      0,
      0,
      Math.max(1, Math.min(pollMs, deadline - now())),
    );
  }
}

function claimDirectoryLock(lockDirectory, { processId }) {
  const owner = {
    pid: processId,
    token: randomUUID(),
  };

  try {
    writeFileSync(
      path.join(lockDirectory, "owner.json"),
      serializeOwner(owner),
      { flag: "wx" },
    );
  } catch (error) {
    rmSync(lockDirectory, { recursive: true, force: true });
    throw error;
  }

  return () => releaseDirectoryLock(lockDirectory, owner.token);
}

function recoverStaleLock(lockDirectory, { now, processAlive }) {
  let stat;
  try {
    stat = lstatSync(lockDirectory);
  } catch (error) {
    if (isMissing(error)) return true;
    throw error;
  }

  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error(
      `WASM output lock path is not a regular directory: ${lockDirectory}`,
    );
  }

  const owner = readLockOwner(lockDirectory);
  if (owner) {
    if (processAlive(owner.pid)) return false;
  } else if (now() - stat.mtimeMs < INCOMPLETE_OWNER_GRACE_MS) {
    return false;
  }

  const quarantine = `${lockDirectory}.quarantine-${randomUUID()}`;
  try {
    renameSync(lockDirectory, quarantine);
  } catch (error) {
    if (isMissing(error)) return true;
    throw error;
  }
  rmSync(quarantine, { recursive: true });
  return true;
}

function releaseDirectoryLock(lockDirectory, token) {
  const owner = readLockOwner(lockDirectory);
  if (!owner || owner.token !== token) throw ownershipChanged(lockDirectory);

  try {
    rmSync(lockDirectory, { recursive: true });
  } catch (error) {
    if (isMissing(error)) throw ownershipChanged(lockDirectory);
    throw error;
  }
}

function readLockOwner(lockDirectory) {
  let contents;
  try {
    contents = readFileSync(path.join(lockDirectory, "owner.json"), "utf8");
  } catch (error) {
    if (isMissing(error)) return null;
    throw error;
  }
  return parseOwner(contents);
}

function parseOwner(contents) {
  try {
    const owner = JSON.parse(contents);
    if (
      !owner ||
      typeof owner !== "object" ||
      !Number.isInteger(owner.pid) ||
      owner.pid <= 0 ||
      typeof owner.token !== "string" ||
      owner.token.length === 0
    ) {
      return null;
    }
    return owner;
  } catch (error) {
    if (error instanceof SyntaxError) return null;
    throw error;
  }
}

function serializeOwner(owner) {
  return `${JSON.stringify(owner, null, 2)}\n`;
}

function ownershipChanged(lockDirectory) {
  return new Error(
    `WASM output lock ownership changed unexpectedly: ${lockDirectory}`,
  );
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
