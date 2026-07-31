import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, it } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  acquireOutputLock,
  acquireWorkspaceWasmBuildLock,
  lockExists,
  outputLockDirectory,
  workspaceWasmBuildLockDirectory,
} from "./wasm-build/output-lock.mjs";
import {
  createOutputStage,
  outputBackupDirectory,
  publishStagedOutput,
  recoverOutputTransaction,
} from "./wasm-build/output-transaction.mjs";

const roots = [];
const lockModuleUrl = pathToFileURL(
  path.join(
    path.dirname(fileURLToPath(import.meta.url)),
    "wasm-build",
    "output-lock.mjs",
  ),
).href;

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("WASM output transaction", () => {
  it("atomically replaces one package-owned artifact directory", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg", "full");
    write(output, "merman_wasm.js", "old");
    write(output, "merman_wasm_inputs.json", "old manifest");
    const stage = createOutputStage(output);
    write(stage, "merman_wasm.js", "new");
    write(stage, "merman_wasm_inputs.json", "new manifest");

    publishStagedOutput(stage, output);

    assert.equal(read(output, "merman_wasm.js"), "new");
    assert.equal(read(output, "merman_wasm_inputs.json"), "new manifest");
    assert.equal(existsSync(stage), false);
    assert.equal(existsSync(outputBackupDirectory(output)), false);
  });

  it("does not preserve stale sibling artifacts", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg", "editor");
    write(output, "old.txt", "old");
    write(output, "stale/merman_wasm_bg.wasm", "stale");
    write(output, "merman_wasm_inputs.json", "old manifest");
    const stage = createOutputStage(output);
    write(stage, "new.txt", "new");
    write(stage, "merman_wasm_inputs.json", "new manifest");

    publishStagedOutput(stage, output);

    assert.equal(existsSync(path.join(output, "old.txt")), false);
    assert.equal(existsSync(path.join(output, "stale")), false);
    assert.equal(read(output, "new.txt"), "new");
    assert.equal(existsSync(outputBackupDirectory(output)), false);
  });

  it("rolls back a failed package-owned publication", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    write(output, "merman_wasm.js", "old js");
    write(output, "merman_wasm_bg.wasm", "old wasm");
    write(output, "merman_wasm_inputs.json", "old manifest");
    const stage = createOutputStage(output);
    write(stage, "merman_wasm.js", "new js");
    write(stage, "merman_wasm_bg.wasm", "new wasm");
    write(stage, "merman_wasm_inputs.json", "new manifest");

    assert.throws(
      () => publishStagedOutput(stage, output, {
        onPublishStep(step) {
          if (step === "new-output-published") {
            throw new Error("injected publish failure");
          }
        },
      }),
      /injected publish failure/,
    );

    assert.equal(read(output, "merman_wasm.js"), "old js");
    assert.equal(read(output, "merman_wasm_bg.wasm"), "old wasm");
    assert.equal(read(output, "merman_wasm_inputs.json"), "old manifest");
    assert.equal(existsSync(stage), false);
    assert.equal(existsSync(outputBackupDirectory(output)), false);
  });

  it("restores an interrupted child publication and removes abandoned stages", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg", "editor");
    write(output, "old.txt", "old");
    write(output, "merman_wasm_inputs.json", "old manifest");
    const backup = outputBackupDirectory(output);
    renameSync(output, backup);
    const stage = createOutputStage(output);
    write(stage, "partial.txt", "partial");

    recoverOutputTransaction(output);

    assert.equal(read(output, "old.txt"), "old");
    assert.equal(existsSync(backup), false);
    assert.equal(existsSync(stage), false);
  });

  it("recovers a dead process lock", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    writeOwner(lock, {
      pid: 2_147_483_647,
      started_at_ms: 0,
      token: "dead",
    });

    const release = acquireOutputLock(output, { timeoutMs: 500, pollMs: 5 });
    release();

    assert.equal(lockExists(output), false);
    assert.deepEqual(
      readdirSync(root).filter((name) => name.includes(".quarantine-")),
      [],
    );
  });

  it("keeps an incomplete owner during its grace period, then recovers it", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    mkdirSync(lock, { recursive: true });

    assert.throws(
      () => acquireOutputLock(output, { timeoutMs: 20, pollMs: 5 }),
      /timed out waiting for the WASM output lock/i,
    );
    assert.equal(existsSync(lock), true);

    utimesSync(lock, new Date(0), new Date(0));
    const release = acquireOutputLock(output, { timeoutMs: 500, pollMs: 5 });
    release();

    assert.equal(lockExists(output), false);
  });

  it("does not release a lock after its ownership token changes", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    const successor = {
      pid: process.pid,
      started_at_ms: Date.now(),
      token: "successor",
    };
    const release = acquireOutputLock(output);
    writeFileSync(path.join(lock, "owner.json"), serializeOwner(successor));

    assert.throws(release, /ownership changed unexpectedly/i);
    assert.deepEqual(
      JSON.parse(readFileSync(path.join(lock, "owner.json"), "utf8")),
      successor,
    );
  });

  it("serializes two independent processes targeting the same output", async () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const first = lockChild(output, 250);
    const firstExit = once(first, "exit");
    await waitForLine(first, "locked");
    const startedAt = Date.now();
    const second = lockChild(output, 0);
    const secondExit = once(second, "exit");

    await waitForLine(second, "locked");
    assert.ok(Date.now() - startedAt >= 150, "second process bypassed the output lock");

    const [firstResult, secondResult] = await Promise.all([
      firstExit,
      secondExit,
    ]);
    assert.equal(firstResult[0], 0);
    assert.equal(secondResult[0], 0);
    assert.equal(lockExists(output), false);
    assert.deepEqual(
      readdirSync(root).filter((name) => name.includes("merman-wasm")),
      [],
    );
  });

  it("writes the Rust-compatible workspace owner.json protocol", () => {
    const root = fixtureRoot();
    const lock = workspaceWasmBuildLockDirectory(root);
    const release = acquireWorkspaceWasmBuildLock(root);
    const owner = JSON.parse(readFileSync(path.join(lock, "owner.json"), "utf8"));

    assert.deepEqual(Object.keys(owner), ["pid", "started_at_ms", "token"]);
    assert.equal(owner.pid, process.pid);
    assert.equal(Number.isSafeInteger(owner.started_at_ms), true);
    assert.equal(owner.started_at_ms >= 0, true);
    assert.equal(typeof owner.token, "string");
    assert.notEqual(owner.token, "");

    release();
    assert.equal(existsSync(lock), false);
  });

  it("places the build lock in the configured Cargo target directory", () => {
    const root = fixtureRoot();
    assert.equal(
      workspaceWasmBuildLockDirectory(root),
      path.join(root, "target", ".merman-wasm-build.lock"),
    );
    assert.equal(
      workspaceWasmBuildLockDirectory(root, { cargoTargetDirectory: "build" }),
      path.join(root, "build", ".merman-wasm-build.lock"),
    );
  });
});

function lockChild(output, holdMs) {
  const source = [
    `import { acquireOutputLock } from ${JSON.stringify(lockModuleUrl)};`,
    "const release = acquireOutputLock(process.argv[1], { timeoutMs: 5000, pollMs: 10 });",
    'console.log("locked");',
    `setTimeout(() => { release(); }, ${holdMs});`,
  ].join("\n");
  return spawn(process.execPath, ["--input-type=module", "--eval", source, output], {
    stdio: ["ignore", "pipe", "pipe"],
  });
}

async function waitForLine(child, expected) {
  child.stdout.setEncoding("utf8");
  let output = "";
  for await (const chunk of child.stdout) {
    output += chunk;
    if (output.includes(expected)) return;
  }
  const error = await streamText(child.stderr);
  assert.fail(`child exited before ${expected}: ${output}${error}`);
}

async function streamText(stream) {
  stream.setEncoding("utf8");
  let output = "";
  for await (const chunk of stream) output += chunk;
  return output;
}

function fixtureRoot() {
  const root = mkdtempSync(path.join(os.tmpdir(), "merman-wasm-transaction-"));
  roots.push(root);
  return root;
}

function write(root, relative, contents) {
  const target = path.join(root, ...relative.split("/"));
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, contents);
}

function read(root, relative) {
  return readFileSync(path.join(root, ...relative.split("/")), "utf8");
}

function serializeOwner(owner) {
  return `${JSON.stringify(owner, null, 2)}\n`;
}

function writeOwner(lock, owner) {
  mkdirSync(lock, { recursive: true });
  writeFileSync(
    path.join(lock, "owner.json"),
    serializeOwner(owner),
  );
}
