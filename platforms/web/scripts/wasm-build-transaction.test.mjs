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
  it("publishes root artifacts with the manifest last and preserves owned surfaces", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    write(output, "merman_wasm.js", "old");
    write(output, "merman_wasm_inputs.json", "old manifest");
    write(output, "editor/keep.txt", "surface");
    const stage = createOutputStage(output);
    write(stage, "merman_wasm.js", "new");
    write(stage, "merman_wasm_inputs.json", "new manifest");

    publishStagedOutput(stage, output, { rootPackage: true });

    assert.equal(read(output, "merman_wasm.js"), "new");
    assert.equal(read(output, "merman_wasm_inputs.json"), "new manifest");
    assert.equal(read(output, "editor/keep.txt"), "surface");
    assert.equal(existsSync(stage), false);
    assert.equal(existsSync(outputBackupDirectory(output)), false);
  });

  it("atomically replaces a child surface directory", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg", "editor");
    write(output, "old.txt", "old");
    write(output, "merman_wasm_inputs.json", "old manifest");
    const stage = createOutputStage(output);
    write(stage, "new.txt", "new");
    write(stage, "merman_wasm_inputs.json", "new manifest");

    publishStagedOutput(stage, output);

    assert.equal(existsSync(path.join(output, "old.txt")), false);
    assert.equal(read(output, "new.txt"), "new");
    assert.equal(existsSync(outputBackupDirectory(output)), false);
  });

  it("rolls back a partial root publication before the manifest commit point", () => {
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
      () =>
        publishStagedOutput(stage, output, {
          rootPackage: true,
          onPublishStep(step) {
            if (step.startsWith("new-entry-published:")) {
              assert.equal(
                existsSync(path.join(output, "merman_wasm_inputs.json")),
                false,
                "partial root output exposed a committed manifest",
              );
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
