import { randomBytes } from "node:crypto";
import {
  closeSync,
  existsSync,
  fstatSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  renameSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

export function replaceDirectory(stage, output, { ownershipRoot = null } = {}) {
  if (ownershipRoot === null) mkdirSync(path.dirname(output), { recursive: true });
  else ensureOwnedDirectory(ownershipRoot, path.dirname(output));
  assertRegularDirectory(stage, "replacement stage");
  assertRegularDirectory(path.dirname(output), "replacement output parent");
  if (existsSync(output)) assertRegularDirectory(output, "replacement output");
  const lock = acquireExclusiveFileLock(`${output}.replace-lock`, {
    purpose: "directory replacement",
  });
  const backup = `${output}.backup-${process.pid}-${randomToken()}`;
  try {
    lock.assertOwned();
    if (existsSync(output)) renameSync(output, backup);
    try {
      lock.assertOwned();
      renameSync(stage, output);
      rmSync(backup, { recursive: true, force: true });
    } catch (error) {
      rmSync(output, { recursive: true, force: true });
      if (existsSync(backup)) renameSync(backup, output);
      throw error;
    }
  } finally {
    lock.release();
  }
}

export function ensureOwnedDirectory(root, directory) {
  const resolvedRoot = path.resolve(root);
  const resolvedDirectory = path.resolve(directory);
  const relative = path.relative(resolvedRoot, resolvedDirectory);
  if (
    relative.startsWith(`..${path.sep}`) ||
    path.isAbsolute(relative)
  ) {
    throw new Error(`Owned directory escapes its root: ${resolvedDirectory}`);
  }
  assertRegularDirectory(resolvedRoot, "owned directory root");
  let current = resolvedRoot;
  for (const component of relative.split(path.sep).filter(Boolean)) {
    current = path.join(current, component);
    if (!existsSync(current)) mkdirSync(current);
    assertRegularDirectory(current, "owned directory component");
  }
  return resolvedDirectory;
}

export function acquireExclusiveFileLock(lockPath, {
  purpose = "operation",
  owner = `${process.pid}`,
} = {}) {
  const parent = path.dirname(lockPath);
  mkdirSync(parent, { recursive: true });
  assertRegularDirectory(parent, `${purpose} lock parent`);
  const parentIdentity = lstatSync(parent);
  const token = randomToken();
  const payload = `${JSON.stringify({
    schema_version: 1,
    owner,
    pid: process.pid,
    token,
  })}\n`;
  let descriptor;
  try {
    descriptor = openSync(lockPath, "wx", 0o600);
  } catch (error) {
    if (error?.code === "EEXIST") {
      throw new Error(`${purpose} is already in progress: ${lockPath}`);
    }
    throw error;
  }

  let identity;
  try {
    writeFileSync(descriptor, payload, "utf8");
    identity = fstatSync(descriptor);
  } catch (error) {
    closeSync(descriptor);
    try {
      unlinkSync(lockPath);
    } catch {
      // Preserve the original lock creation failure.
    }
    throw error;
  }
  closeSync(descriptor);

  let released = false;
  const assertOwned = () => {
    const currentParent = lstatSync(parent);
    if (
      !currentParent.isDirectory() ||
      currentParent.isSymbolicLink() ||
      currentParent.dev !== parentIdentity.dev ||
      currentParent.ino !== parentIdentity.ino
    ) {
      throw new Error(`${purpose} lock parent changed: ${parent}`);
    }
    const current = lstatSync(lockPath);
    if (
      !current.isFile() ||
      current.dev !== identity.dev ||
      current.ino !== identity.ino ||
      readFileSync(lockPath, "utf8") !== payload
    ) {
      throw new Error(`${purpose} lock ownership changed: ${lockPath}`);
    }
  };
  return {
    path: lockPath,
    assertOwned,
    release() {
      if (released) return;
      assertOwned();
      unlinkSync(lockPath);
      released = true;
    },
  };
}

function randomToken() {
  return randomBytes(12).toString("hex");
}

function assertRegularDirectory(directory, label) {
  const stat = lstatSync(directory);
  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error(`${label} must be a non-symlink directory: ${directory}`);
  }
}
