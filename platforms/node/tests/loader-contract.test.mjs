import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { loadNativeTransport } from "../src/candidates/native.mjs";
import { loadNodeWasmTransport } from "../src/candidates/wasm.mjs";
import {
  loadNativeBinding,
  nativePackageName,
  resolveNodeTarget,
} from "../src/native-loader.mjs";
import {
  MermanMissingPlatformPackageError,
  MermanOperationError,
  MermanUnsupportedTargetError,
} from "../src/errors.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const glibcReport = { header: { glibcVersionRuntime: "2.39" }, sharedObjects: [] };
const muslReport = {
  header: {},
  sharedObjects: ["/lib/ld-musl-x86_64.so.1"],
};

test("target resolution distinguishes OS, CPU, and Linux libc", () => {
  assert.equal(
    resolveNodeTarget({ platform: "darwin", arch: "arm64" }),
    "darwin-arm64",
  );
  assert.equal(
    resolveNodeTarget({ platform: "darwin", arch: "x64" }),
    "darwin-x64",
  );
  assert.equal(
    resolveNodeTarget({ platform: "linux", arch: "x64", report: glibcReport }),
    "linux-x64-gnu",
  );
  assert.equal(
    resolveNodeTarget({ platform: "linux", arch: "x64", report: muslReport }),
    "linux-x64-musl",
  );
  assert.equal(
    resolveNodeTarget({ platform: "win32", arch: "x64" }),
    "win32-x64-msvc",
  );
  assert.throws(
    () => resolveNodeTarget({ platform: "linux", arch: "arm64", report: glibcReport }),
    MermanUnsupportedTargetError,
  );
  assert.throws(
    () => resolveNodeTarget({ platform: "linux", arch: "x64", report: { header: {} } }),
    /cannot determine Linux libc/i,
  );
});

test("the loader resolves one target-specific package and no browser package", () => {
  const requested = [];
  const binding = { NativeEngine: class NativeEngine {} };
  const loaded = loadNativeBinding({
    platform: "darwin",
    arch: "arm64",
    loadPackage(packageName) {
      requested.push(packageName);
      return binding;
    },
  });

  assert.equal(loaded, binding);
  assert.deepEqual(requested, ["@mermanjs/node-darwin-arm64"]);
  assert.equal(nativePackageName("linux-x64-musl"), "@mermanjs/node-linux-x64-musl");
});

test("a corrupt browser WASM package cannot become a silent fallback", () => {
  const requested = [];
  const corruptBrowserPackage = new Error("corrupt browser WASM");

  assert.throws(
    () =>
      loadNativeBinding({
        platform: "darwin",
        arch: "arm64",
        loadPackage(packageName) {
          requested.push(packageName);
          if (packageName === "@mermanjs/web") throw corruptBrowserPackage;
          const error = new Error(`Cannot find module '${packageName}'`);
          error.code = "MODULE_NOT_FOUND";
          throw error;
        },
      }),
    MermanMissingPlatformPackageError,
  );
  assert.deepEqual(requested, ["@mermanjs/node-darwin-arm64"]);
});

test("the explicit Node WASM artifact keeps its CommonJS boundary inside the ESM workspace", async (context) => {
  const artifactRoot = mkdtempSync(path.join(nodeRoot, ".wasm-loader-contract-"));
  context.after(() => rmSync(artifactRoot, { recursive: true, force: true }));
  writeFileSync(
    path.join(artifactRoot, "package.json"),
    `${JSON.stringify({ private: true, type: "commonjs" })}\n`,
  );
  const modulePath = path.join(artifactRoot, "binding.js");
  writeFileSync(
    modulePath,
    `module.exports = {
  WasmEngine: class WasmEngine {
    execute(value) { return value; }
    executeSync(value) { return value; }
    runtimeCatalogJson() { return "{}"; }
  },
};
`,
  );

  const transport = await loadNodeWasmTransport("{}", { modulePath });
  assert.equal(await transport.execute("async"), "async");
  assert.equal(transport.executeSync("sync"), "sync");
  await transport.dispose();
});

test("candidate constructors preserve bindings-core typed errors", async () => {
  const envelope = {
    version: 1,
    ok: false,
    error: {
      code: 3,
      code_name: "MERMAN_OPTIONS_JSON_ERROR",
      kind: "generic",
      capability_id: null,
      message: "unsupported binding options version 99",
    },
  };
  class NativeEngine {
    constructor() {
      throw new Error(JSON.stringify(envelope));
    }
  }
  class WasmEngine {
    constructor() {
      throw envelope;
    }
  }

  for (const create of [
    () => loadNativeTransport("{}", { loadPackage: () => ({ NativeEngine }) }),
    () => loadNodeWasmTransport("{}", {
      modulePath: "candidate:node-wasm",
      loadModule: async () => ({ WasmEngine }),
    }),
  ]) {
    await assert.rejects(create, (error) => {
      assert.ok(error instanceof MermanOperationError);
      assert.equal(error.status, 3);
      assert.equal(error.codeName, "MERMAN_OPTIONS_JSON_ERROR");
      assert.equal(error.kind, "generic");
      assert.equal(error.capabilityId, null);
      return true;
    });
  }
});
