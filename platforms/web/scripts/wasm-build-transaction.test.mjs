import assert from "node:assert/strict";
import childProcess, { spawn } from "node:child_process";
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
import { syncBuiltinESMExports } from "node:module";
import { afterEach, describe, it, mock } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import { buildWasm, wasmPackEnvironment } from "./wasm-build/build.mjs";
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
import { repositoryRoot } from "./wasm-build/paths.mjs";
import { defaultWebPackage } from "./wasm-build/web-surface-descriptor.mjs";

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
  for (const [label, fields, expectedHomepage] of [
    ["absent", {}, "https://github.com/example/merman"],
    ["null", { homepage: null }, "https://github.com/example/merman"],
    ["empty", { homepage: "" }, "https://github.com/example/merman"],
    ["explicit", { homepage: "https://example.com/merman" }, "https://example.com/merman"],
  ]) {
    it(`publishes the npm homepage from ${label} Cargo homepage metadata`, () => {
      const output = buildPackageFixture(fields);
      const packageJson = JSON.parse(read(output, "package.json"));

      assert.equal(packageJson.homepage, expectedHomepage);
      assert.deepEqual(packageJson.repository, {
        type: "git",
        url: "https://github.com/example/merman",
      });
      const manifest = JSON.parse(read(output, "merman_wasm_inputs.json"));
      assert.ok(manifest.artifacts.some((entry) => entry.path === "package.json"));
      assert.equal(read(output, "merman_wasm.js"), "export {};\n");
      assert.equal(existsSync(path.join(output, "old.txt")), false);
      assert.equal(existsSync(outputBackupDirectory(output)), false);
      assert.equal(lockExists(output), false);
      assert.deepEqual(readdirSync(path.dirname(output)), [path.basename(output)]);
    });
  }

  for (const field of ["description", "license", "repository"]) {
    it(`rejects missing Cargo ${field} even with an explicit homepage`, () => {
      assert.throws(
        () => buildPackageFixture({ [field]: "", homepage: "https://example.com/merman" }),
        { message: `Cargo metadata field merman-wasm.${field} is missing.` },
      );
    });
  }

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

  it("retries immediately when a stale lock disappears before retirement", () => {
    const root = fixtureRoot();
    const output = path.join(root, "pkg");
    const lock = outputLockDirectory(output);
    writeOwner(lock, {
      pid: 4242,
      token: "dead",
    });

    const release = acquireOutputLock(output, {
      timeoutMs: 0,
      processAlive() {
        rmSync(lock, { recursive: true });
        return false;
      },
    });
    release();

    assert.equal(lockExists(output), false);
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

    assert.deepEqual(Object.keys(owner), ["pid", "token"]);
    assert.equal(owner.pid, process.pid);
    assert.equal(typeof owner.token, "string");
    assert.notEqual(owner.token, "");

    release();
    assert.equal(existsSync(lock), false);
  });

  it("places the build lock in the configured Cargo target directory", () => {
    const root = fixtureRoot();
    assert.equal(
      workspaceWasmBuildLockDirectory(root, { cargoTargetDirectory: null }),
      path.join(root, "target", ".merman-wasm-build.lock"),
    );
    assert.equal(
      workspaceWasmBuildLockDirectory(root, { cargoTargetDirectory: "build" }),
      path.join(root, "build", ".merman-wasm-build.lock"),
    );
  });

  it("pins wasm-pack to the absolute Cargo metadata target directory", () => {
    const root = fixtureRoot();
    const targetDirectory = path.join(root, "target", "isolated");

    assert.deepEqual(
      wasmPackEnvironment(
        { target_directory: targetDirectory },
        { CARGO_TARGET_DIR: "target/isolated", KEEP: "yes" },
      ),
      { CARGO_TARGET_DIR: targetDirectory, KEEP: "yes" },
    );
  });
});

function buildPackageFixture(fields) {
  const root = fixtureRoot();
  const output = path.join(root, "pkg", "full");
  const targetDirectory = path.join(root, "target");
  const packageInfo = {
    id: "merman-wasm",
    name: "merman-wasm",
    version: "1.0.0",
    source: null,
    manifest_path: path.join(repositoryRoot, "crates", "merman-wasm", "Cargo.toml"),
    description: "WASM build transaction fixture",
    license: "MIT OR Apache-2.0",
    repository: "https://github.com/example/merman",
    authors: [],
    keywords: [],
    ...fields,
  };
  write(output, "old.txt", "old artifact");
  const commands = [];
  // Intercept every external tool call; no Cargo or WASM compiler process is launched.
  const tool = mock.method(childProcess, "spawnSync", (command, args, options) => {
    commands.push(`${command} ${args[0]}`);
    if (command === "cargo" && args[0] === "metadata") {
      return {
        status: 0,
        stdout: JSON.stringify({
          packages: [
            {
              id: "probe",
              name: "merman-wasm-freshness-probe",
              version: "0.0.0",
              manifest_path: args[args.indexOf("--manifest-path") + 1],
            },
            packageInfo,
          ],
          resolve: { root: "probe", nodes: [] },
        }),
      };
    }
    if (command === "cargo" && args[0] === "tree") {
      return { status: 0, stdout: "merman-wasm-freshness-probe v0.0.0\nmerman-wasm v1.0.0\n" };
    }
    assert.equal(command, "wasm-pack");
    assert.equal(args[0], "build");
    assert.equal(options.env.CARGO_TARGET_DIR, targetDirectory);
    const stage = args[args.indexOf("--out-dir") + 1];
    for (const name of ["merman_wasm.js", "merman_wasm.d.ts", "merman_wasm_bg.wasm.d.ts"]) {
      write(stage, name, "export {};\n");
    }
    write(stage, "merman_wasm_bg.wasm", Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]));
    write(stage, ".gitignore", "*\n");
    write(
      targetDirectory,
      "wasm32-unknown-unknown/wasm-size/merman_wasm.d",
      "merman_wasm.wasm: crates/merman-wasm/src/lib.rs\n",
    );
    return { status: 0 };
  });
  const previousTargetDirectory = process.env.CARGO_TARGET_DIR;
  process.env.CARGO_TARGET_DIR = targetDirectory;
  syncBuiltinESMExports();
  try {
    buildWasm({
      outputDir: { absolute: output },
      descriptor: defaultWebPackage,
      repositoryMetadata: { packages: [packageInfo], target_directory: targetDirectory },
      toolVersions: { cargo: "fixture", node: process.version, rustc: "fixture", wasm_pack: "fixture" },
    });
  } finally {
    tool.mock.restore();
    syncBuiltinESMExports();
    if (previousTargetDirectory === undefined) delete process.env.CARGO_TARGET_DIR;
    else process.env.CARGO_TARGET_DIR = previousTargetDirectory;
  }
  assert.deepEqual(commands, ["cargo metadata", "cargo tree", "wasm-pack build"]);
  assert.equal(
    existsSync(workspaceWasmBuildLockDirectory(repositoryRoot, { cargoTargetDirectory: targetDirectory })),
    false,
  );
  return output;
}

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
