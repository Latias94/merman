import { randomUUID } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmdirSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

const DEFAULT_TIMEOUT_MS = 10 * 60 * 1000;
const DEFAULT_POLL_MS = 50;
const INCOMPLETE_OWNER_GRACE_MS = 5_000;
const UUID_PATTERN = "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}";
const CHOOSING_PATTERN = new RegExp(`^choosing-(${UUID_PATTERN})\\.json$`);
const TICKET_PATTERN = new RegExp(`^ticket-([1-9][0-9]*)-(${UUID_PATTERN})\\.json$`);
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
    onLockStep = () => {},
    processId = process.pid,
    processAlive = isProcessAlive,
  } = {},
) {
  const deadline = now() + timeoutMs;
  const token = randomUUID();
  const owner = { pid: processId, started_at_ms: now(), token };
  const preparedPath = `${lockDirectory}.claim-${token}.tmp`;
  const choosingPath = path.join(lockDirectory, `choosing-${token}.json`);
  let preparedCreated = false;
  let ticketPath = null;
  mkdirSync(path.dirname(lockDirectory), { recursive: true });
  removeStalePreparedClaims(lockDirectory, {
    now,
    onLockStep,
    processAlive,
  });

  try {
    writeFileSync(preparedPath, serializeOwner(owner), { flag: "wx" });
    preparedCreated = true;
    onLockStep("claim-ready", { claimPath: preparedPath, token });
    publishPreparedClaim(preparedPath, choosingPath, lockDirectory, {
      deadline,
      now,
      onLockStep,
      pollMs,
      token,
    });

    const ticket = nextTicket(lockDirectory);
    ticketPath = path.join(lockDirectory, `ticket-${ticket}-${token}.json`);
    renameSync(choosingPath, ticketPath);

    while (true) {
      const state = inspectLockDirectory(lockDirectory, {
        now,
        onLockStep,
        ownToken: token,
        processAlive,
      });
      const ownTicket = state.tickets.find((claim) => claim.token === token);
      if (!ownTicket || ownTicket.path !== ticketPath || ownTicket.ticket !== ticket) {
        throw ownershipChanged(lockDirectory);
      }

      const earlierTicket = state.tickets.some(
        (claim) =>
          claim.token !== token &&
          compareTickets(claim, ownTicket) < 0,
      );
      if (
        state.stable &&
        !state.blocked &&
        state.choosing.length === 0 &&
        !earlierTicket
      ) {
        return () =>
          releaseDirectoryLock(lockDirectory, {
            onLockStep,
            ticketPath,
            token,
          });
      }

      onLockStep("claim-contended", { claimPath: ticketPath, token });
      if (now() >= deadline) {
        throw new Error(
          `Timed out waiting for the WASM output lock: ${lockDirectory}`,
        );
      }
      Atomics.wait(waiter, 0, 0, Math.max(1, Math.min(pollMs, deadline - now())));
    }
  } catch (error) {
    if (preparedCreated) removeOwnedClaim(preparedPath, token);
    removeOwnedClaim(choosingPath, token);
    if (ticketPath) removeOwnedClaim(ticketPath, token);
    removeEmptyLockDirectory(lockDirectory);
    throw error;
  }
}

function removeStalePreparedClaims(
  lockDirectory,
  { now, onLockStep, processAlive },
) {
  const parent = path.dirname(lockDirectory);
  const pattern = new RegExp(
    `^${escapeRegExp(path.basename(lockDirectory))}\\.claim-(${UUID_PATTERN})\\.tmp$`,
  );
  for (const entry of readdirSync(parent, { withFileTypes: true })) {
    const match = pattern.exec(entry.name);
    if (!match || !entry.isFile() || entry.isSymbolicLink()) continue;
    const entryPath = path.join(parent, entry.name);
    const snapshot = readSnapshot(entryPath);
    if (!snapshot) continue;
    const owner = parseOwner(snapshot.contents, match[1]);
    if (owner) {
      if (processAlive(owner.pid)) continue;
      onLockStep("stale-ownership-checked", {
        claimPath: entryPath,
        token: owner.token,
      });
      removeSnapshotIfUnchanged(entryPath, snapshot, lockDirectory);
    } else {
      removeIncompleteEntry(entryPath, { now, onLockStep, snapshot });
    }
  }
}

function nextTicket(lockDirectory) {
  let maximum = 0;
  for (const entry of readLockEntries(lockDirectory)) {
    const match = TICKET_PATTERN.exec(entry.name);
    if (!match) continue;
    const ticket = Number(match[1]);
    if (Number.isSafeInteger(ticket)) maximum = Math.max(maximum, ticket);
  }
  if (maximum >= Number.MAX_SAFE_INTEGER) {
    throw new Error(`WASM output lock ticket space is exhausted: ${lockDirectory}`);
  }
  return maximum + 1;
}

function inspectLockDirectory(
  lockDirectory,
  { now, onLockStep, ownToken, processAlive },
) {
  const before = readLockEntries(lockDirectory);
  onLockStep("claims-listed", { claimPath: lockDirectory, token: ownToken });
  const state = {
    blocked: false,
    choosing: [],
    stable: false,
    tickets: [],
  };

  for (const entry of before) {
    const entryPath = path.join(lockDirectory, entry.name);
    const choosingMatch = CHOOSING_PATTERN.exec(entry.name);
    const ticketMatch = TICKET_PATTERN.exec(entry.name);
    const legacyOwner = entry.name === "owner.json";

    if (!entry.isFile() || entry.isSymbolicLink()) {
      state.blocked = true;
      continue;
    }
    if (!choosingMatch && !ticketMatch && !legacyOwner) {
      if (!removeIncompleteEntry(entryPath, { now, onLockStep })) {
        state.blocked = true;
      }
      continue;
    }

    const snapshot = readSnapshot(entryPath);
    if (!snapshot) continue;
    const expectedToken = choosingMatch?.[1] ?? ticketMatch?.[2] ?? null;
    const owner = parseOwner(snapshot.contents, expectedToken);
    if (!owner) {
      if (!removeIncompleteEntry(entryPath, { now, onLockStep, snapshot })) {
        state.blocked = true;
      }
      continue;
    }

    if (owner.token !== ownToken && !processAlive(owner.pid)) {
      onLockStep("stale-ownership-checked", {
        claimPath: entryPath,
        token: owner.token,
      });
      removeSnapshotIfUnchanged(entryPath, snapshot, lockDirectory);
      continue;
    }

    if (legacyOwner) {
      state.blocked = true;
    } else if (choosingMatch) {
      state.choosing.push({ ...owner, path: entryPath });
    } else {
      const ticket = Number(ticketMatch[1]);
      if (!Number.isSafeInteger(ticket)) {
        state.blocked = true;
      } else {
        state.tickets.push({ ...owner, path: entryPath, ticket });
      }
    }
  }

  state.stable = sameEntryNames(before, readLockEntries(lockDirectory));
  return state;
}

function removeIncompleteEntry(
  entryPath,
  { now, onLockStep, snapshot = readSnapshot(entryPath) },
) {
  if (!snapshot) return true;
  if (now() - snapshot.mtimeMs < INCOMPLETE_OWNER_GRACE_MS) return false;
  onLockStep("stale-ownership-checked", { claimPath: entryPath, token: null });
  removeSnapshotIfUnchanged(entryPath, snapshot, path.dirname(entryPath));
  return true;
}

function releaseDirectoryLock(
  lockDirectory,
  { onLockStep, ticketPath, token },
) {
  const snapshot = readSnapshot(ticketPath);
  const owner = snapshot ? parseOwner(snapshot.contents, token) : null;
  if (!snapshot || !owner) throw ownershipChanged(lockDirectory);

  onLockStep("release-ownership-checked", { claimPath: ticketPath, token });
  removeSnapshotIfUnchanged(ticketPath, snapshot, lockDirectory, {
    missingIsSuccess: false,
  });
  removeEmptyLockDirectory(lockDirectory);
}

function removeOwnedClaim(claimPath, token) {
  const snapshot = readSnapshot(claimPath);
  if (!snapshot) return;
  const owner = parseOwner(snapshot.contents, token);
  if (!owner) throw ownershipChanged(claimPath);
  removeSnapshotIfUnchanged(claimPath, snapshot, claimPath);
}

function removeSnapshotIfUnchanged(
  entryPath,
  expected,
  lockDirectory,
  { missingIsSuccess = true } = {},
) {
  const current = readSnapshot(entryPath);
  if (!current) {
    if (missingIsSuccess) return;
    throw ownershipChanged(lockDirectory);
  }
  if (!sameSnapshot(current, expected)) throw ownershipChanged(lockDirectory);
  try {
    unlinkSync(entryPath);
  } catch (error) {
    if (!isMissing(error)) throw error;
    if (!missingIsSuccess) throw ownershipChanged(lockDirectory);
  }
}

function readSnapshot(entryPath) {
  let stat;
  let contents;
  try {
    stat = lstatSync(entryPath);
    if (!stat.isFile() || stat.isSymbolicLink()) return null;
    contents = readFileSync(entryPath, "utf8");
  } catch (error) {
    if (isMissing(error)) return null;
    throw error;
  }
  return {
    contents,
    dev: stat.dev,
    ino: stat.ino,
    mtimeMs: stat.mtimeMs,
    size: stat.size,
  };
}

function sameSnapshot(left, right) {
  return (
    left.contents === right.contents &&
    left.dev === right.dev &&
    left.ino === right.ino &&
    left.mtimeMs === right.mtimeMs &&
    left.size === right.size
  );
}

function parseOwner(contents, expectedToken) {
  try {
    const owner = JSON.parse(contents);
    if (
      !owner ||
      typeof owner !== "object" ||
      !Number.isInteger(owner.pid) ||
      owner.pid <= 0 ||
      !Number.isSafeInteger(owner.started_at_ms) ||
      owner.started_at_ms < 0 ||
      typeof owner.token !== "string" ||
      owner.token.length === 0 ||
      (expectedToken !== null && owner.token !== expectedToken)
    ) {
      return null;
    }
    return owner;
  } catch (error) {
    if (error instanceof SyntaxError) return null;
    throw error;
  }
}

function publishPreparedClaim(
  preparedPath,
  choosingPath,
  lockDirectory,
  { deadline, now, onLockStep, pollMs, token },
) {
  while (true) {
    try {
      ensureLockDirectory(lockDirectory);
      onLockStep("claim-directory-ready", {
        claimPath: preparedPath,
        token,
      });
      renameSync(preparedPath, choosingPath);
      return;
    } catch (error) {
      if (!isMissing(error)) throw error;
      if (!existsSync(preparedPath)) throw ownershipChanged(lockDirectory);
      if (now() >= deadline) {
        throw new Error(
          `Timed out publishing the WASM output lock claim: ${lockDirectory}`,
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
}

function ensureLockDirectory(lockDirectory) {
  try {
    mkdirSync(lockDirectory);
  } catch (error) {
    if (!isAlreadyExists(error)) throw error;
  }
  const stat = lstatSync(lockDirectory);
  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error(`WASM output lock path is not a regular directory: ${lockDirectory}`);
  }
}

function readLockEntries(lockDirectory) {
  try {
    return readdirSync(lockDirectory, { withFileTypes: true });
  } catch (error) {
    if (isMissing(error)) return [];
    throw error;
  }
}

function sameEntryNames(left, right) {
  if (left.length !== right.length) return false;
  const leftNames = left.map((entry) => entry.name).sort(compareNames);
  const rightNames = right.map((entry) => entry.name).sort(compareNames);
  return leftNames.every((name, index) => name === rightNames[index]);
}

function removeEmptyLockDirectory(lockDirectory) {
  try {
    rmdirSync(lockDirectory);
  } catch (error) {
    if (!isMissing(error) && !isNotEmpty(error)) throw error;
  }
}

function serializeOwner(owner) {
  return `${JSON.stringify(owner, null, 2)}\n`;
}

function compareTickets(left, right) {
  if (left.ticket !== right.ticket) return left.ticket - right.ticket;
  return compareNames(left.token, right.token);
}

function compareNames(left, right) {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function ownershipChanged(lockDirectory) {
  return new Error(`WASM output lock ownership changed unexpectedly: ${lockDirectory}`);
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

function isNotEmpty(error) {
  return Boolean(
    error &&
      typeof error === "object" &&
      (error.code === "ENOTEMPTY" || error.code === "EEXIST"),
  );
}

export function lockExists(outputRoot) {
  const lockDirectory = outputLockDirectory(outputRoot);
  return (
    existsSync(lockDirectory) &&
    readLockEntries(lockDirectory).length > 0
  );
}
