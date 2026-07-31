import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  MermanNodeEngine,
  createNodeEngine,
  normalizeBindingOptions,
} from "../src/engine.mjs";
import {
  MermanDisposedError,
  MermanInvalidTransportError,
  MermanOperationError,
  MermanQueueSaturatedError,
} from "../src/errors.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const PACKAGE_VERSION = JSON.parse(
  readFileSync(path.join(nodeRoot, "package-surfaces.json"), "utf8"),
).version;

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

function success(svg = "<svg />") {
  return JSON.stringify({
    version: 1,
    ok: true,
    result: {
      operation_id: "svg",
      media_type: "image/svg+xml",
      data: svg,
      metadata_json: JSON.stringify({
        version: 1,
        operation_id: "svg",
        media_type: "image/svg+xml",
        runtime_policy: "deterministic",
        byte_length: Buffer.byteLength(svg),
      }),
    },
  });
}

function jsonSuccess(operationId, value) {
  const data = JSON.stringify(value);
  return JSON.stringify({
    version: 1,
    ok: true,
    result: {
      operation_id: operationId,
      media_type: "application/json",
      data,
      metadata_json: JSON.stringify({
        version: 1,
        operation_id: operationId,
        media_type: "application/json",
        runtime_policy: "deterministic",
        byte_length: Buffer.byteLength(data),
      }),
    },
  });
}

function failure({ kind, capabilityId = null }) {
  return JSON.stringify({
    version: 1,
    ok: false,
    error: {
      code: 7,
      code_name: "MERMAN_UNSUPPORTED_OPERATION",
      kind,
      capability_id: capabilityId,
      message:
        kind === "unknown-operation"
          ? "unknown operation `bitmap`"
          : `operation requires missing capability \`${capabilityId}\``,
    },
  });
}

function runtimeCatalog(overrides = {}) {
  return {
    schema_version: 1,
    transport_api_version: 1,
    package_version: PACKAGE_VERSION,
    capabilities: {
      capability_ids: ["layout-cytoscape", "layout-elk", "math", "svg"],
      output_ids: ["svg"],
      operation_ids: ["layout-json", "semantic-json", "svg", "svg-plan-json"],
      system_adapter_ids: [],
      text_measurement: {
        protocol_version: 3,
        provider_ids: ["vendored"],
      },
    },
    output_contracts: [{
      id: "svg",
      media_type: "image/svg+xml",
      system_fonts: null,
      embedded_images: null,
    }],
    registry: { diagram_family_count: 35 },
    resources: {
      general_binding_default_profile: "interactive",
      cli_default_profile: "trusted-native",
      limits: [{
        id: "max_source_bytes",
        phase: "source",
        description: "Maximum source bytes.",
        overridable: true,
        hard_cap: false,
      }],
      profiles: [
        {
          id: "interactive",
          purpose: "Interactive rendering.",
          trust_assumption: "Cooperative input.",
          recommended_binding_default: true,
          limits: { max_source_bytes: 1024 },
        },
        {
          id: "trusted-native",
          purpose: "Trusted rendering.",
          trust_assumption: "Trusted input.",
          recommended_binding_default: false,
          limits: { max_source_bytes: null },
        },
      ],
    },
    ...overrides,
  };
}

test("runtime catalog accepts additive fields within schema 1", async () => {
  const catalog = runtimeCatalog();
  catalog.future_root_metadata = true;
  catalog.capabilities.future_capability_metadata = true;
  catalog.output_contracts[0].future_output_metadata = true;
  catalog.registry.future_registry_metadata = true;
  catalog.resources.future_resource_metadata = true;
  const factory = transportFactory({
    runtimeCatalogJson() {
      return JSON.stringify(catalog);
    },
  });

  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });
  assert.deepEqual(engine.runtimeCatalog.capabilities.capability_ids, [
    "layout-cytoscape",
    "layout-elk",
    "math",
    "svg",
  ]);
  assert.equal(engine.runtimeCatalog.output_contracts[0].future_output_metadata, true);
  await engine.dispose();
});

test("runtime catalog validates output contracts and preserves additive nested fields", async () => {
  const catalog = runtimeCatalog();
  catalog.capabilities.capability_ids = [
    ...catalog.capabilities.capability_ids,
    "png",
  ].sort();
  catalog.capabilities.output_ids = ["png", "svg"];
  catalog.capabilities.operation_ids = [
    ...catalog.capabilities.operation_ids,
    "png",
  ].sort();
  catalog.output_contracts = [
    {
      id: "png",
      media_type: "image/png",
      system_fonts: {
        source_id: "host-system",
        discovery: "first-use",
        cache_scope: "process-global",
        host_dependent: true,
        caller_configurable: false,
        resource_bounded: false,
        future_font_metadata: true,
      },
      embedded_images: {
        source_ids: ["data-url"],
        filesystem_access: false,
        network_access: false,
        caller_configurable: false,
        limits: {
          max_bytes_per_image: 16 * 1024 * 1024,
          max_total_bytes: 32 * 1024 * 1024,
          max_pixels_per_image: 16 * 1024 * 1024,
          max_total_pixels: 32 * 1024 * 1024,
          future_limit_metadata: true,
        },
        future_image_metadata: true,
      },
    },
    catalog.output_contracts[0],
  ];
  const factory = transportFactory({
    runtimeCatalogJson() {
      return JSON.stringify(catalog);
    },
  });

  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });
  assert.deepEqual(
    engine.runtimeCatalog.output_contracts.map(({ id }) => id),
    catalog.capabilities.output_ids,
  );
  assert.equal(
    engine.runtimeCatalog.output_contracts[0].system_fonts.future_font_metadata,
    true,
  );
  assert.equal(
    engine.runtimeCatalog.output_contracts[0].embedded_images.limits.future_limit_metadata,
    true,
  );
  await engine.dispose();
});

test("runtime catalog output contracts fail closed on ID and nested-shape drift", async () => {
  const nativeContract = {
    id: "png",
    media_type: "image/png",
    system_fonts: {
      source_id: "host-system",
      discovery: "first-use",
      cache_scope: "process-global",
      host_dependent: true,
      caller_configurable: false,
      resource_bounded: false,
    },
    embedded_images: {
      source_ids: ["data-url"],
      filesystem_access: false,
      network_access: false,
      caller_configurable: false,
      limits: {
        max_bytes_per_image: 16 * 1024 * 1024,
        max_total_bytes: 32 * 1024 * 1024,
        max_pixels_per_image: 16 * 1024 * 1024,
        max_total_pixels: 32 * 1024 * 1024,
      },
    },
  };
  const invalidCatalogs = [];

  const missingContracts = runtimeCatalog();
  delete missingContracts.output_contracts;
  invalidCatalogs.push(missingContracts);

  const missingOutputId = runtimeCatalog();
  missingOutputId.output_contracts = [];
  invalidCatalogs.push(missingOutputId);

  const extraOutputContract = runtimeCatalog();
  extraOutputContract.output_contracts.push(nativeContract);
  invalidCatalogs.push(extraOutputContract);

  const malformedFonts = runtimeCatalog();
  malformedFonts.output_contracts[0] = structuredClone(nativeContract);
  malformedFonts.capabilities.capability_ids = [
    ...malformedFonts.capabilities.capability_ids,
    "png",
  ].sort();
  malformedFonts.capabilities.output_ids = ["png"];
  malformedFonts.capabilities.operation_ids = [
    ...malformedFonts.capabilities.operation_ids.filter((id) => id !== "svg"),
    "png",
  ].sort();
  delete malformedFonts.output_contracts[0].system_fonts.discovery;
  invalidCatalogs.push(malformedFonts);

  const malformedImages = structuredClone(malformedFonts);
  malformedImages.output_contracts[0] = structuredClone(nativeContract);
  malformedImages.output_contracts[0].embedded_images.filesystem_access = "false";
  invalidCatalogs.push(malformedImages);

  const malformedImageLimits = structuredClone(malformedFonts);
  malformedImageLimits.output_contracts[0] = structuredClone(nativeContract);
  delete malformedImageLimits.output_contracts[0].embedded_images.limits.max_total_pixels;
  invalidCatalogs.push(malformedImageLimits);

  const malformedSourceIds = structuredClone(malformedFonts);
  malformedSourceIds.output_contracts[0] = structuredClone(nativeContract);
  malformedSourceIds.output_contracts[0].embedded_images.source_ids = [
    "future-source",
    "data-url",
  ];
  invalidCatalogs.push(malformedSourceIds);

  for (const catalog of invalidCatalogs) {
    const factory = transportFactory({
      runtimeCatalogJson() {
        return JSON.stringify(catalog);
      },
    });
    await assert.rejects(
      createNodeEngine({}, { loadTransport: factory.loadTransport }),
      MermanInvalidTransportError,
    );
  }
});

function transportFactory(overrides = {}) {
  const calls = [];
  const createdWith = [];
  const transport = {
    async execute(requestJson) {
      calls.push(JSON.parse(requestJson));
      return success();
    },
    executeSync(requestJson) {
      calls.push(JSON.parse(requestJson));
      return success();
    },
    runtimeCatalogJson() {
      return JSON.stringify(runtimeCatalog());
    },
    async dispose() {},
    ...overrides,
  };
  return {
    calls,
    createdWith,
    loadTransport: async (optionsJson) => {
      createdWith.push(JSON.parse(optionsJson));
      return transport;
    },
    transport,
  };
}

test("default construction is explicit deterministic interactive policy", async () => {
  const factory = transportFactory();
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });

  assert.deepEqual(factory.createdWith, [
    {
      version: 1,
      runtime_policy: "deterministic",
      resources: { profile: "interactive" },
    },
  ]);

  const rendered = engine.renderSvg("flowchart TD\nA --> B");
  assert.ok(rendered instanceof Promise);
  assert.equal(await rendered, "<svg />");
  assert.deepEqual(factory.calls, [
    {
      operation_id: "svg",
      source: "flowchart TD\nA --> B",
      uri: null,
    },
  ]);
  await engine.dispose();
});

test("generic operations preserve request-local options JSON", async () => {
  const factory = transportFactory();
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });
  const optionsJson = JSON.stringify({
    resources: { limits: { max_source_bytes: 4096 } },
  });

  await engine.executeOperation({
    operationId: "svg",
    source: "flowchart TD\nA --> B",
    optionsJson,
  });

  assert.deepEqual(factory.calls, [
    {
      operation_id: "svg",
      source: "flowchart TD\nA --> B",
      uri: null,
      options_json: optionsJson,
    },
  ]);
  await engine.dispose();
});

test("generic operations invoke descriptor-owned SVG planning", async () => {
  const plan = {
    schema_version: 1,
    planned_operation_id: "svg",
    diagram_type: "flowchart-v2",
    required_capability_ids: [],
    missing_capability_ids: [],
    ready: true,
  };
  const factory = transportFactory();
  factory.transport.execute = async (requestJson) => {
    const request = JSON.parse(requestJson);
    factory.calls.push(request);
    return jsonSuccess(request.operation_id, plan);
  };
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });

  const result = await engine.executeOperation({
    operationId: "svg-plan-json",
    source: "flowchart TD\nA --> B",
  });

  assert.equal(result.operation_id, "svg-plan-json");
  assert.equal(result.media_type, "application/json");
  assert.deepEqual(JSON.parse(result.data), plan);
  assert.deepEqual(factory.calls, [
    {
      operation_id: "svg-plan-json",
      source: "flowchart TD\nA --> B",
      uri: null,
    },
  ]);
  await engine.dispose();
});

test("generic operations reject superseded request fields", async () => {
  const factory = transportFactory();
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });

  assert.throws(
    () => engine.executeOperationSync({
      operationId: "svg",
      source: "flowchart TD\nA --> B",
      formatOptions: { version: 1 },
    }),
    /unknown operation request field `formatOptions`/i,
  );
  assert.deepEqual(factory.calls, []);
  await engine.dispose();
});

test("construction validates and exposes the loaded artifact runtime catalog", async () => {
  const factory = transportFactory();
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });

  assert.deepEqual(engine.runtimeCatalog, runtimeCatalog());
  const callerCopy = engine.runtimeCatalog;
  callerCopy.capabilities.capability_ids.length = 0;
  assert.deepEqual(engine.runtimeCatalog, runtimeCatalog());
  await engine.dispose();

  const malformed = transportFactory({
    runtimeCatalogJson() {
      return JSON.stringify(runtimeCatalog({
        capabilities: {
          ...runtimeCatalog().capabilities,
          operation_ids: ["semantic-json"],
        },
      }));
    },
  });
  await assert.rejects(
    createNodeEngine({}, { loadTransport: malformed.loadTransport }),
    MermanInvalidTransportError,
  );
});

test("construction disposes every unusable transport exactly once", async () => {
  const cases = [
    {
      name: "transport shape",
      options: {},
      overrides: { executeSync: undefined },
      expected: MermanInvalidTransportError,
    },
    {
      name: "runtime catalog",
      options: {},
      overrides: {
        runtimeCatalogJson() {
          return JSON.stringify(runtimeCatalog({ schema_version: 2 }));
        },
      },
      expected: MermanInvalidTransportError,
    },
    {
      name: "engine construction",
      options: { concurrency: 0 },
      overrides: {},
      expected: RangeError,
    },
  ];

  for (const item of cases) {
    let disposeCalls = 0;
    const factory = transportFactory({
      ...item.overrides,
      async dispose() {
        disposeCalls += 1;
      },
    });
    await assert.rejects(
      createNodeEngine(item.options, { loadTransport: factory.loadTransport }),
      item.expected,
      item.name,
    );
    assert.equal(disposeCalls, 1, item.name);
  }
});

test("construction cleanup never obscures the primary transport failure", async () => {
  const primary = new Error("runtime catalog probe failed");
  let disposeCalls = 0;
  const factory = transportFactory({
    runtimeCatalogJson() {
      throw primary;
    },
    async dispose() {
      disposeCalls += 1;
      throw new Error("dispose failed");
    },
  });

  await assert.rejects(
    createNodeEngine({}, { loadTransport: factory.loadTransport }),
    (error) => error === primary,
  );
  assert.equal(disposeCalls, 1);
});

test("runtime catalog validates text measurement and resource local relations", async () => {
  const invalidCatalogs = [];

  const missingProtocol = runtimeCatalog();
  delete missingProtocol.capabilities.text_measurement.protocol_version;
  invalidCatalogs.push(missingProtocol);

  const missingVendored = runtimeCatalog();
  missingVendored.capabilities.text_measurement.provider_ids = ["host-callback"];
  invalidCatalogs.push(missingVendored);

  const uncallableHostCallback = runtimeCatalog();
  uncallableHostCallback.capabilities.text_measurement.provider_ids = [
    "host-callback",
    "vendored",
  ];
  invalidCatalogs.push(uncallableHostCallback);

  const measurementWithoutSvg = runtimeCatalog();
  measurementWithoutSvg.capabilities.capability_ids =
    measurementWithoutSvg.capabilities.capability_ids.filter((id) => id !== "svg");
  measurementWithoutSvg.capabilities.output_ids = [];
  measurementWithoutSvg.capabilities.operation_ids = [
    "semantic-json",
  ];
  invalidCatalogs.push(measurementWithoutSvg);

  const malformedLimit = runtimeCatalog();
  malformedLimit.resources.limits = [{ id: "max-source-bytes" }];
  invalidCatalogs.push(malformedLimit);

  const malformedProfile = runtimeCatalog();
  malformedProfile.resources.profiles = [{
    id: "interactive",
    purpose: "Interactive rendering.",
    trust_assumption: "Untrusted input.",
    recommended_binding_default: true,
    limits: { max_source_bytes: -1 },
  }];
  invalidCatalogs.push(malformedProfile);

  const missingProfileLimit = runtimeCatalog();
  missingProfileLimit.resources.profiles[0].limits = {};
  invalidCatalogs.push(missingProfileLimit);

  const unknownDefaultProfile = runtimeCatalog();
  unknownDefaultProfile.resources.general_binding_default_profile = "missing";
  invalidCatalogs.push(unknownDefaultProfile);

  const duplicateLimit = runtimeCatalog();
  duplicateLimit.resources.limits.push({ ...duplicateLimit.resources.limits[0] });
  invalidCatalogs.push(duplicateLimit);

  for (const catalog of invalidCatalogs) {
    const factory = transportFactory({
      runtimeCatalogJson() {
        return JSON.stringify(catalog);
      },
    });
    await assert.rejects(
      createNodeEngine({}, { loadTransport: factory.loadTransport }),
      MermanInvalidTransportError,
    );
  }
});

test("public TypeScript declarations cover the generic operation API", () => {
  const declarations = readFileSync(path.join(nodeRoot, "src", "index.d.ts"), "utf8");
  for (const method of [
    "dispose",
    "executeOperation",
    "executeOperationSync",
    "renderSvg",
    "renderSvgSync",
  ]) {
    assert.equal(typeof MermanNodeEngine.prototype[method], "function", method);
    assert.match(declarations, new RegExp(`\\b${method}\\s*\\(`));
  }
  assert.match(declarations, /\breadonly runtimeCatalog:/);
  assert.match(declarations, /\boptionsJson\?: string;/);
  assert.match(declarations, /provider_ids:\s*\["vendored"\]/);
  assert.match(declarations, /output_contracts:\s*MermanRuntimeOutputContract\[\]/);
  assert.match(declarations, /system_fonts:\s*MermanRuntimeSystemFontContract\s*\|\s*null/);
  assert.match(
    declarations,
    /embedded_images:\s*MermanRuntimeEmbeddedImageContract\s*\|\s*null/,
  );
  assert.match(declarations, /limits:\s*MermanRuntimeResourceLimit\[\]/);
  assert.match(declarations, /profiles:\s*MermanRuntimeResourceProfile\[\]/);
  assert.doesNotMatch(declarations, /\b(?:limits|output_contracts|profiles):\s*unknown\[\]/);
  assert.doesNotMatch(declarations, /\bformatOptions\??:/);
});

test("binding options preserve the shared profile vocabulary and reject host measurement", () => {
  assert.deepEqual(
    normalizeBindingOptions({
      resources: { profile: "trusted-native" },
      fixed_today: "2026-07-23",
    }),
    {
      version: 1,
      runtime_policy: "deterministic",
      resources: { profile: "trusted-native" },
      fixed_today: "2026-07-23",
    },
  );

  assert.throws(
    () => normalizeBindingOptions({ textMeasurer: () => ({ width: 1 }) }),
    /text measurement callbacks are not supported/i,
  );
  assert.throws(
    () => normalizeBindingOptions({ resources: { profile: "default" } }),
    /unknown resource profile `default`/i,
  );
});

test("typed unknown-operation and missing-capability errors survive the JS boundary", async () => {
  for (const expected of [
    { operationId: "bitmap", kind: "unknown-operation", capabilityId: null },
    { operationId: "png", kind: "missing-capability", capabilityId: "png" },
  ]) {
    const factory = transportFactory({
      async execute() {
        return failure({ kind: expected.kind, capabilityId: expected.capabilityId });
      },
    });
    const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });
    await assert.rejects(
      engine.executeOperation({
        operationId: expected.operationId,
        source: "flowchart TD\nA",
      }),
      (error) => {
        assert.ok(error instanceof MermanOperationError);
        assert.equal(error.kind, expected.kind);
        assert.equal(error.capabilityId, expected.capabilityId);
        assert.equal(error.codeName, "MERMAN_UNSUPPORTED_OPERATION");
        return true;
      },
    );
    await engine.dispose();
  }
});

test("queue admission is bounded and dispose drains only executing work", async () => {
  const started = deferred();
  const release = deferred();
  let executions = 0;
  let transportDisposed = false;
  const factory = transportFactory({
    async execute() {
      executions += 1;
      started.resolve();
      await release.promise;
      return success(`<svg data-execution="${executions}" />`);
    },
    async dispose() {
      transportDisposed = true;
    },
  });
  const engine = await createNodeEngine(
    { concurrency: 1, maxQueue: 1 },
    { loadTransport: factory.loadTransport },
  );

  const active = engine.renderSvg("flowchart TD\nA");
  await started.promise;
  const queued = engine.renderSvg("flowchart TD\nB");
  await assert.rejects(
    engine.renderSvg("flowchart TD\nC"),
    MermanQueueSaturatedError,
  );

  const disposing = engine.dispose();
  await assert.rejects(queued, MermanDisposedError);
  assert.equal(transportDisposed, false);
  release.resolve();
  assert.match(await active, /data-execution="1"/);
  await disposing;
  assert.equal(transportDisposed, true);
  assert.equal(executions, 1);
  await assert.rejects(engine.renderSvg("flowchart TD\nD"), MermanDisposedError);
});

test("AbortSignal cancels queued work but never claims to preempt executing work", async () => {
  const started = deferred();
  const release = deferred();
  let executions = 0;
  const factory = transportFactory({
    async execute() {
      executions += 1;
      started.resolve();
      await release.promise;
      return success();
    },
  });
  const engine = await createNodeEngine(
    { concurrency: 1, maxQueue: 1 },
    { loadTransport: factory.loadTransport },
  );

  const executingAbort = new AbortController();
  const active = engine.renderSvg("flowchart TD\nA", {
    signal: executingAbort.signal,
  });
  await started.promise;
  executingAbort.abort();

  const queuedAbort = new AbortController();
  const queued = engine.renderSvg("flowchart TD\nB", {
    signal: queuedAbort.signal,
  });
  queuedAbort.abort();
  await assert.rejects(queued, (error) => error?.name === "AbortError");

  const replacement = engine.renderSvg("flowchart TD\nC");
  release.resolve();
  assert.equal(await active, "<svg />");
  assert.equal(await replacement, "<svg />");
  assert.equal(executions, 2);
  await engine.dispose();
});

test("renderSvgSync is explicit and refuses lifecycle races", async () => {
  const factory = transportFactory();
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });
  assert.equal(engine.renderSvgSync("flowchart TD\nA"), "<svg />");
  await engine.dispose();
  assert.throws(() => engine.renderSvgSync("flowchart TD\nB"), MermanDisposedError);
});
