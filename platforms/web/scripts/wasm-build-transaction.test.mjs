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
  rmdirSync,
  rmSync,
  unlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, it } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  acquireOutputLock,
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
    mkdirSync(lock, { recursive: true });
    writeFileSync(
      path.join(lock, "owner.json"),
      `${JSON.stringify({ pid: 2_147_483_647, started_at_ms: 0, token: "dead" })}\n`,
    );

    const release = acquireOutputLock(output, { timeoutMs: 500, pollMs: 5 });
    release();

    assert.equal(lockExists(output), false);
  });

  it("recovers a dead ticket from the current lock protocol", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    const token = "00000000-0000-4000-8000-000000000001";
    mkdirSync(lock, { recursive: true });
    writeFileSync(
      path.join(lock, `ticket-1-${token}.json`),
      serializeOwner({ pid: 2_147_483_647, started_at_ms: 0, token }),
    );

    const release = acquireOutputLock(output, { timeoutMs: 500, pollMs: 5 });
    release();

    assert.equal(lockExists(output), false);
  });

  it("removes an unpublished claim left by a dead process", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    const token = "00000000-0000-4000-8000-000000000006";
    const prepared = `${lock}.claim-${token}.tmp`;
    writeFileSync(
      prepared,
      serializeOwner({ pid: 2_147_483_647, started_at_ms: 0, token }),
    );

    const release = acquireOutputLock(output, { timeoutMs: 500, pollMs: 5 });
    release();

    assert.equal(existsSync(prepared), false);
    assert.equal(lockExists(output), false);
  });

  it("recovers a stale lock whose owner was never published", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    mkdirSync(lock, { recursive: true });
    const incomplete = path.join(lock, "initializing");
    writeFileSync(incomplete, "");
    utimesSync(incomplete, new Date(0), new Date(0));

    const release = acquireOutputLock(output, {
      now: () => 10_000,
      timeoutMs: 500,
      pollMs: 5,
    });
    release();

    assert.equal(lockExists(output), false);
  });

  it("publishes ownership atomically across a delayed claimant interleaving", async () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const ready = path.join(root, "claim-ready");
    const resume = path.join(root, "resume-claim");
    const contended = path.join(root, "claim-contended");
    const first = delayedClaimChild(output, { contended, ready, resume });
    await waitForPath(ready);
    assert.equal(lockExists(output), false, "an unpublished claim became the canonical lock");

    const second = lockChild(output, 400);
    await waitForLine(second, "locked");
    const lock = outputLockDirectory(output);
    const secondTicket = onlyTicketPath(lock);
    const secondOwner = readFileSync(secondTicket, "utf8");

    writeFileSync(resume, "");
    await waitForPath(contended);
    assert.equal(
      readFileSync(secondTicket, "utf8"),
      secondOwner,
      "the delayed claimant removed or replaced the current owner",
    );

    const [firstExit, secondExit] = await Promise.all([
      once(first, "exit"),
      once(second, "exit"),
    ]);
    assert.equal(firstExit[0], 0);
    assert.equal(secondExit[0], 0);
    assert.equal(lockExists(output), false);
  });

  it("retries when the empty coordination directory disappears before publication", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    let removed = false;

    const release = acquireOutputLock(output, {
      pollMs: 1,
      onLockStep(step) {
        if (step !== "claim-directory-ready" || removed) return;
        removed = true;
        rmdirSync(lock);
      },
    });
    release();

    assert.equal(removed, true);
    assert.equal(lockExists(output), false);
  });

  it("rechecks the claim set when a choosing claim becomes a ticket mid-scan", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    const competitor = {
      pid: process.pid,
      started_at_ms: Date.now(),
      token: "00000000-0000-4000-8000-000000000005",
    };
    const choosing = path.join(lock, `choosing-${competitor.token}.json`);
    const ticket = path.join(lock, `ticket-1-${competitor.token}.json`);
    mkdirSync(lock, { recursive: true });
    writeFileSync(choosing, serializeOwner(competitor));
    let interleaved = false;
    let waited = false;

    const release = acquireOutputLock(output, {
      pollMs: 1,
      onLockStep(step) {
        if (step === "claims-listed" && !interleaved) {
          interleaved = true;
          renameSync(choosing, ticket);
        } else if (step === "claim-contended" && existsSync(ticket)) {
          waited = true;
          unlinkSync(ticket);
        }
      },
    });
    release();

    assert.equal(interleaved, true);
    assert.equal(waited, true);
    assert.equal(lockExists(output), false);
  });

  it("does not delete a successor when ownership changes during release", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    const successor = {
      pid: process.pid,
      started_at_ms: Date.now(),
      token: "00000000-0000-4000-8000-000000000002",
    };
    let claimPath;
    const release = acquireOutputLock(output, {
      onLockStep(step, detail) {
        if (step !== "release-ownership-checked") return;
        claimPath = detail.claimPath;
        unlinkSync(claimPath);
        writeFileSync(claimPath, serializeOwner(successor));
      },
    });

    assert.throws(release, /ownership changed unexpectedly/i);
    assert.deepEqual(
      JSON.parse(readFileSync(claimPath, "utf8")),
      successor,
    );
  });

  it("does not delete a successor when ownership changes during stale recovery", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    mkdirSync(lock, { recursive: true });
    writeFileSync(
      path.join(lock, "owner.json"),
      `${JSON.stringify({ pid: 2_147_483_647, started_at_ms: 0, token: "dead" })}\n`,
    );
    const successor = {
      pid: process.pid,
      started_at_ms: Date.now(),
      token: "00000000-0000-4000-8000-000000000003",
    };

    assert.throws(
      () =>
        acquireOutputLock(output, {
          processAlive: () => false,
          onLockStep(step, detail) {
            if (step === "stale-ownership-checked") {
              writeFileSync(detail.claimPath, serializeOwner(successor));
            }
          },
        }),
      /ownership changed unexpectedly/i,
    );
    assert.deepEqual(
      JSON.parse(readFileSync(path.join(lock, "owner.json"), "utf8")),
      successor,
    );
  });

  it("cleans up only its unique claim when acquisition setup fails", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    const successor = {
      pid: process.pid,
      started_at_ms: Date.now(),
      token: "00000000-0000-4000-8000-000000000004",
    };

    assert.throws(
      () =>
        acquireOutputLock(output, {
          onLockStep(step) {
            if (step !== "claim-ready") return;
            writeLegacyOwner(lock, successor);
            throw new Error("injected claim failure");
          },
        }),
      /injected claim failure/,
    );
    assert.deepEqual(
      JSON.parse(readFileSync(path.join(lock, "owner.json"), "utf8")),
      successor,
    );
    assert.deepEqual(
      readdirSync(root).filter((name) => name.includes(".claim-")),
      [],
    );
  });

  it("serializes two independent processes targeting the same output", async () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const first = lockChild(output, 250);
    await waitForLine(first, "locked");
    const startedAt = Date.now();
    const second = lockChild(output, 0);

    const [firstExit, secondExit] = await Promise.all([
      once(first, "exit"),
      once(second, "exit"),
    ]);
    assert.equal(firstExit[0], 0);
    assert.equal(secondExit[0], 0);
    assert.ok(Date.now() - startedAt >= 150, "second process bypassed the output lock");
    assert.equal(lockExists(output), false);
    assert.deepEqual(
      readdirSync(root).filter((name) => name.includes("merman-wasm")),
      [],
    );
  });

  it("keeps a burst of independent claimants out of the same critical section", async () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const criticalSection = path.join(root, "critical-section");
    const children = Array.from({ length: 8 }, () =>
      contentionChild(output, criticalSection),
    );

    const results = await Promise.all(
      children.map(async (child) => {
        const stderr = streamText(child.stderr);
        const exit = await once(child, "exit");
        return { code: exit[0], stderr: await stderr };
      }),
    );

    assert.deepEqual(
      results,
      Array.from({ length: children.length }, () => ({ code: 0, stderr: "" })),
    );
    assert.equal(existsSync(criticalSection), false);
    assert.equal(lockExists(output), false);
  });

  it("removes only the timed-out claimant ticket", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    const release = acquireOutputLock(output);

    assert.throws(
      () => acquireOutputLock(output, { timeoutMs: 20, pollMs: 5 }),
      /timed out waiting for the WASM output lock/i,
    );
    assert.equal(
      readdirSync(lock).filter((name) => name.startsWith("ticket-")).length,
      1,
    );

    release();
    assert.equal(lockExists(output), false);
  });

  it("serializes independent package builds through one workspace build lock", async () => {
    const root = fixtureRoot();
    const first = workspaceLockChild(root, 250);
    await waitForLine(first, "locked");
    const startedAt = Date.now();
    const second = workspaceLockChild(root, 0);

    const [firstExit, secondExit] = await Promise.all([
      once(first, "exit"),
      once(second, "exit"),
    ]);
    assert.equal(firstExit[0], 0);
    assert.equal(secondExit[0], 0);
    assert.ok(Date.now() - startedAt >= 150, "second process bypassed the build lock");
    assert.equal(existsSync(workspaceWasmBuildLockDirectory(root)), false);
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

function workspaceLockChild(root, holdMs) {
  const source = [
    `import { acquireWorkspaceWasmBuildLock } from ${JSON.stringify(lockModuleUrl)};`,
    "const release = acquireWorkspaceWasmBuildLock(process.argv[1], { timeoutMs: 5000, pollMs: 10 });",
    'console.log("locked");',
    `setTimeout(() => { release(); }, ${holdMs});`,
  ].join("\n");
  return spawn(process.execPath, ["--input-type=module", "--eval", source, root], {
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function contentionChild(output, criticalSection) {
  const source = [
    'import { mkdirSync, rmdirSync } from "node:fs";',
    `import { acquireOutputLock } from ${JSON.stringify(lockModuleUrl)};`,
    "const waiter = new Int32Array(new SharedArrayBuffer(4));",
    "const release = acquireOutputLock(process.argv[1], { timeoutMs: 5000, pollMs: 5 });",
    "mkdirSync(process.argv[2]);",
    "Atomics.wait(waiter, 0, 0, 20);",
    "rmdirSync(process.argv[2]);",
    "release();",
  ].join("\n");
  return spawn(
    process.execPath,
    ["--input-type=module", "--eval", source, output, criticalSection],
    { stdio: ["ignore", "ignore", "pipe"] },
  );
}

function delayedClaimChild(output, { contended, ready, resume }) {
  const source = [
    'import { existsSync, writeFileSync } from "node:fs";',
    `import { acquireOutputLock } from ${JSON.stringify(lockModuleUrl)};`,
    "const waiter = new Int32Array(new SharedArrayBuffer(4));",
    "const release = acquireOutputLock(process.argv[1], {",
    "  timeoutMs: 5000,",
    "  pollMs: 10,",
    "  onLockStep(step) {",
    '    if (step === "claim-ready") {',
    "      writeFileSync(process.argv[2], '');",
    "      while (!existsSync(process.argv[3])) Atomics.wait(waiter, 0, 0, 10);",
    '    } else if (step === "claim-contended") {',
    "      writeFileSync(process.argv[4], '');",
    "    }",
    "  },",
    "});",
    "release();",
  ].join("\n");
  return spawn(
    process.execPath,
    ["--input-type=module", "--eval", source, output, ready, resume, contended],
    { stdio: ["ignore", "ignore", "pipe"] },
  );
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

async function waitForPath(target, timeoutMs = 5_000) {
  const deadline = Date.now() + timeoutMs;
  while (!existsSync(target)) {
    if (Date.now() >= deadline) {
      assert.fail(`timed out waiting for path: ${target}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
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

function onlyTicketPath(lock) {
  const tickets = readdirSync(lock).filter((name) => name.startsWith("ticket-"));
  assert.equal(tickets.length, 1);
  return path.join(lock, tickets[0]);
}

function serializeOwner(owner) {
  return `${JSON.stringify(owner, null, 2)}\n`;
}

function writeLegacyOwner(lock, owner) {
  mkdirSync(lock, { recursive: true });
  writeFileSync(
    path.join(lock, "owner.json"),
    serializeOwner(owner),
  );
}
