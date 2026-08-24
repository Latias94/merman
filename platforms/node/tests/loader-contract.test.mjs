import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

import { loadNativeTransport } from "../src/candidates/native.mjs";
import {
  loadNodeWasmTransport,
  nodeWasmModuleSpecifier,
} from "../src/candidates/wasm.mjs";
import {
  assertRuntimePackageVersion,
  loadNativeBinding,
  nodeLoaderPackageVersion,
  nativePackageName,
  resolveNodeTarget,
} from "../src/native-loader.mjs";
import {
  MermanDisposedError,
  MermanInvalidTransportError,
  MermanMissingPlatformPackageError,
  MermanNativeLoadError,
  MermanOperationError,
  MermanUnsupportedTargetError,
  NODE_TRANSPORT_LIMITS,
  NODE_WIRE_CONTRACT,
} from "../src/errors.mjs";
import { CAPABILITY_DESCRIPTOR_DIGEST } from "../src/generated/capability-surface.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function transportIdentity(transportKind, overrides = {}) {
  return {
    schema_version: 1,
    package_id: NODE_WIRE_CONTRACT.package_id,
    artifact_id: NODE_WIRE_CONTRACT.artifact_id,
    package_version: nodeLoaderPackageVersion(),
    transport_kind: transportKind,
    transport_api_version: NODE_WIRE_CONTRACT.transport_api_version,
    binding_result_payload_version: NODE_WIRE_CONTRACT.binding_result_payload_version,
    capability_descriptor_digest: CAPABILITY_DESCRIPTOR_DIGEST,
    wire_contract: NODE_WIRE_CONTRACT,
    ...overrides,
  };
}

function transportIdentityJson(transportKind, overrides = {}) {
  return JSON.stringify(transportIdentity(transportKind, overrides));
}

const glibcReport = { header: { glibcVersionRuntime: "2.39" }, sharedObjects: [] };
const muslReport = {
  header: {},
  sharedObjects: ["/lib/ld-musl-x86_64.so.1"],
};

test("Node WASM module specifiers classify paths before generic URL schemes", () => {
  assert.equal(
    nodeWasmModuleSpecifier(String.raw`F:\repo\binding.js`),
    "file:///F:/repo/binding.js",
  );
  assert.equal(
    nodeWasmModuleSpecifier("C:/repo/binding.js"),
    "file:///C:/repo/binding.js",
  );
  assert.equal(
    nodeWasmModuleSpecifier(String.raw`C:\repo\diagram #1?.js`),
    "file:///C:/repo/diagram%20%231%3F.js",
  );
  assert.equal(
    nodeWasmModuleSpecifier("file:///opt/merman/binding.js"),
    "file:///opt/merman/binding.js",
  );
  assert.equal(
    nodeWasmModuleSpecifier("https://example.invalid/binding.js"),
    "https://example.invalid/binding.js",
  );

  const relative = nodeWasmModuleSpecifier("fixtures/binding.js", { cwd: nodeRoot });
  assert.equal(
    path.relative(nodeRoot, fileURLToPath(relative)).split(path.sep).join("/"),
    "fixtures/binding.js",
  );
  const posixPath = "/opt/merman/binding.js";
  assert.equal(
    nodeWasmModuleSpecifier(posixPath),
    pathToFileURL(path.resolve(posixPath)).href,
  );
});

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

test("the native loader diagnoses installed packages that fail dynamic loading", () => {
  const loaderFailure = Object.assign(
    new Error("/lib64/libm.so.6: version `GLIBC_2.35' not found"),
    { code: "ERR_DLOPEN_FAILED" },
  );
  assert.throws(
    () =>
      loadNativeBinding({
        platform: "linux",
        arch: "x64",
        report: glibcReport,
        loadPackage: () => {
          throw loaderFailure;
        },
      }),
    (error) => {
      assert.ok(error instanceof MermanNativeLoadError);
      assert.equal(error.code, "MERMAN_NATIVE_LOAD_ERROR");
      assert.equal(error.packageName, "@mermanjs/node-linux-x64-gnu");
      assert.equal(error.target, "linux-x64-gnu");
      assert.equal(error.cause, loaderFailure);
      assert.match(error.message, /ABI|shared-library/i);
      assert.match(error.message, /node-wasm/);
      return true;
    },
  );
});

test("the native loader reads its expected version from its own package manifest", () => {
  const manifest = JSON.parse(readFileSync(path.join(nodeRoot, "package.json"), "utf8"));
  assert.equal(nodeLoaderPackageVersion(), manifest.version);
});

test("the native loader accepts and caches an exact-version runtime catalog", async () => {
  let catalogReads = 0;
  class NativeEngine {
    execute() {}
    executeSync() {}
    metadataJson(id) {
      return JSON.stringify({ id });
    }
    runtimeCatalogJson() {
      catalogReads += 1;
      return JSON.stringify({ package_version: nodeLoaderPackageVersion() });
    }
  }

  const transport = await loadNativeTransport("{}", {
    loadPackage: () => ({
      NativeEngine,
      transportIdentityJson: () => transportIdentityJson("napi"),
    }),
  });
  assert.equal(catalogReads, 1);
  assert.equal(
    JSON.parse(transport.runtimeCatalogJson()).package_version,
    nodeLoaderPackageVersion(),
  );
  assert.equal(catalogReads, 1);
  await transport.dispose();
});

test("the native version preflight accepts bounded catalog text only", () => {
  assert.throws(
    () => assertRuntimePackageVersion({
      package_version: nodeLoaderPackageVersion(),
    }),
    MermanInvalidTransportError,
  );

  const catalog = JSON.stringify({ package_version: nodeLoaderPackageVersion() });
  const exactCatalog = catalog + " ".repeat(
    NODE_TRANSPORT_LIMITS.runtime_catalog.max_utf8_bytes - Buffer.byteLength(catalog),
  );
  assert.equal(assertRuntimePackageVersion(exactCatalog), exactCatalog);
  assert.throws(
    () => assertRuntimePackageVersion(`${exactCatalog} `),
    /wire limit/i,
  );
});

test("candidate identity is a strict transport and contract preflight", async () => {
  const invalidIdentities = [
    transportIdentity("napi", { package_version: "0.0.0-stale" }),
    transportIdentity("wasm", {}),
    transportIdentity("napi", { transport_api_version: 99 }),
    transportIdentity("napi", { binding_result_payload_version: 99 }),
    transportIdentity("napi", { capability_descriptor_digest: "sha256:stale" }),
    transportIdentity("napi", {
      wire_contract: { ...NODE_WIRE_CONTRACT, artifact_id: "future-artifact" },
    }),
    { ...transportIdentity("napi"), package_id: "@mermanjs/other" },
  ];
  for (const identity of invalidIdentities) {
    let constructed = false;
    class NativeEngine {
      constructor() {
        constructed = true;
      }
    }
    await assert.rejects(
      loadNativeTransport("{}", {
        loadPackage: () => ({
          NativeEngine,
          transportIdentityJson: () => JSON.stringify(identity),
        }),
      }),
      MermanInvalidTransportError,
    );
    assert.equal(constructed, false);
  }

  for (const identityJson of [
    null,
    () => transportIdentity("wasm"),
    () => transportIdentityJson("wasm", { package_version: "0.0.0-stale" }),
    () => transportIdentityJson("napi"),
  ]) {
    let constructed = false;
    class WasmEngine {
      constructor() {
        constructed = true;
      }
    }
    await assert.rejects(
      loadNodeWasmTransport("{}", {
        modulePath: "candidate:node-wasm",
        loadModule: async () => ({
          WasmEngine,
          ...(identityJson === null ? {} : { transportIdentityJson: identityJson }),
        }),
      }),
      MermanInvalidTransportError,
    );
    assert.equal(constructed, false);
  }

  const identityJson = transportIdentityJson("napi");
  const exactIdentityJson = identityJson + " ".repeat(
    NODE_TRANSPORT_LIMITS.identity.max_utf8_bytes - Buffer.byteLength(identityJson),
  );
  assert.equal(
    Buffer.byteLength(exactIdentityJson),
    NODE_TRANSPORT_LIMITS.identity.max_utf8_bytes,
  );
  let exactConstructed = false;
  class ExactNativeEngine {
    constructor() {
      exactConstructed = true;
    }
    execute() {
      return "{}";
    }
    executeSync() {
      return "{}";
    }
    metadataJson() {
      return "{}";
    }
    runtimeCatalogJson() {
      return JSON.stringify({ package_version: nodeLoaderPackageVersion() });
    }
  }
  const exactTransport = await loadNativeTransport("{}", {
    loadPackage: () => ({
      NativeEngine: ExactNativeEngine,
      transportIdentityJson: () => exactIdentityJson,
    }),
  });
  assert.equal(exactConstructed, true);
  await exactTransport.dispose();

  await assert.rejects(
    loadNativeTransport("{}", {
      loadPackage: () => ({
        NativeEngine: ExactNativeEngine,
        transportIdentityJson: () => `${exactIdentityJson} `,
      }),
    }),
    /wire limit/i,
  );
});

test("candidate identity export failures become typed invalid-transport errors", async () => {
  for (const transportKind of ["napi", "wasm"]) {
    const identityFailure = new Error(`${transportKind} identity failed`);
    const readIdentity = () => {
      throw identityFailure;
    };
    const load = transportKind === "napi"
      ? () => loadNativeTransport("{}", {
        loadPackage: () => ({
          NativeEngine: class NativeEngine {},
          transportIdentityJson: readIdentity,
        }),
      })
      : () => loadNodeWasmTransport("{}", {
        modulePath: "candidate:node-wasm",
        loadModule: async () => ({
          WasmEngine: class WasmEngine {},
          transportIdentityJson: readIdentity,
        }),
      });
    await assert.rejects(load, (error) => {
      assert.ok(error instanceof MermanInvalidTransportError);
      assert.match(error.message, /identity preflight failed/i);
      assert.equal(error.cause, identityFailure);
      return true;
    });
  }
});

test("the native loader rejects a stale binary runtime catalog", async () => {
  let disposed = false;
  class NativeEngine {
    execute() {}
    executeSync() {}
    metadataJson(id) {
      return JSON.stringify({ id });
    }
    runtimeCatalogJson() {
      return JSON.stringify({ package_version: "0.0.0-stale" });
    }
    dispose() {
      disposed = true;
    }
  }

  await assert.rejects(
    loadNativeTransport("{}", {
      loadPackage: () => ({
        NativeEngine,
        transportIdentityJson: () => transportIdentityJson("napi"),
      }),
    }),
    (error) => {
      assert.ok(error instanceof MermanInvalidTransportError);
      assert.match(error.message, /runtime package version.*loader package/i);
      assert.match(error.message, /0\.0\.0-stale/);
      assert.match(error.message, new RegExp(nodeLoaderPackageVersion().replaceAll(".", "\\.")));
      return true;
    },
  );
  assert.equal(disposed, true);
});

test("the Node WASM loader rejects and disposes a stale runtime catalog", async () => {
  let disposeCalls = 0;
  class WasmEngine {
    execute() {}
    executeSync() {}
    metadataJson(id) {
      return JSON.stringify({ id });
    }
    runtimeCatalogJson() {
      return JSON.stringify({ package_version: "0.0.0-stale" });
    }
    dispose() {
      disposeCalls += 1;
    }
  }

  await assert.rejects(
    loadNodeWasmTransport("{}", {
      modulePath: "candidate:node-wasm",
      loadModule: async () => ({
        WasmEngine,
        transportIdentityJson: () => transportIdentityJson("wasm"),
      }),
    }),
    /runtime package version.*loader package/i,
  );
  assert.equal(disposeCalls, 1);
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
  transportIdentityJson() {
    return ${JSON.stringify(transportIdentityJson("wasm"))};
  },
  WasmEngine: class WasmEngine {
    execute(value) { return value; }
    executeSync(value) { return value; }
    metadataJson(id) { return JSON.stringify({ id }); }
    runtimeCatalogJson() {
      return JSON.stringify({ package_version: ${JSON.stringify(nodeLoaderPackageVersion())} });
    }
  },
};
`,
  );

  const transport = await loadNodeWasmTransport("{}", { modulePath });
  assert.equal(await transport.execute("async"), "async");
  assert.equal(transport.executeSync("sync"), "sync");
  await transport.dispose();
});

test("candidate wrappers dispose owned engines once and fail closed afterward", async () => {
  for (const transportKind of ["napi", "wasm"]) {
    let disposeCalls = 0;
    let executeArgs = null;
    let executeSyncArgs = null;
    class CandidateEngine {
      execute(...args) {
        executeArgs = args;
        const [value] = args;
        return value;
      }
      executeSync(...args) {
        executeSyncArgs = args;
        const [value] = args;
        return value;
      }
      metadataJson(id) {
        return JSON.stringify({ id });
      }
      runtimeCatalogJson() {
        return JSON.stringify({ package_version: nodeLoaderPackageVersion() });
      }
      dispose() {
        disposeCalls += 1;
      }
    }
    const transport = transportKind === "napi"
      ? await loadNativeTransport("{}", {
        loadPackage: () => ({
          NativeEngine: CandidateEngine,
          transportIdentityJson: () => transportIdentityJson("napi"),
        }),
      })
      : await loadNodeWasmTransport("{}", {
        modulePath: "candidate:node-wasm",
        loadModule: async () => ({
          WasmEngine: CandidateEngine,
          transportIdentityJson: () => transportIdentityJson("wasm"),
        }),
      });

    const signal = new AbortController().signal;
    assert.equal(await transport.execute("request", signal, 25), "request");
    assert.deepEqual(
      executeArgs,
      transportKind === "napi" ? ["request", signal, 25] : ["request", 25],
    );
    assert.equal(transport.executeSync("sync-request", 12), "sync-request");
    assert.deepEqual(executeSyncArgs, ["sync-request", 12]);
    await transport.dispose();
    await transport.dispose();
    assert.equal(disposeCalls, 1);
    assert.throws(() => transport.execute("request"), MermanDisposedError);
    assert.throws(() => transport.executeSync("request"), MermanDisposedError);
    assert.throws(() => transport.metadataJson("supported-diagrams"), MermanDisposedError);
  }
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
      throw new Error(JSON.stringify(envelope));
    }
  }

  for (const create of [
    () => loadNativeTransport("{}", {
      loadPackage: () => ({
        NativeEngine,
        transportIdentityJson: () => transportIdentityJson("napi"),
      }),
    }),
    () => loadNodeWasmTransport("{}", {
      modulePath: "candidate:node-wasm",
      loadModule: async () => ({
        WasmEngine,
        transportIdentityJson: () => transportIdentityJson("wasm"),
      }),
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
