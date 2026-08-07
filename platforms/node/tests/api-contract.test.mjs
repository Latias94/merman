import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  MermanEngine,
  createNodeEngine,
  normalizeBindingOptions,
} from "../src/engine.mjs";
import * as publicApi from "../src/index.mjs";
import {
  MermanDisposedError,
  MermanInvalidTransportError,
  MermanOperationError,
  MermanQueueSaturatedError,
  NODE_TRANSPORT_FIELD_LIMITS,
  NODE_TRANSPORT_LIMITS,
  NODE_WIRE_CONTRACT,
  parseTransportJsonText,
  parseRuntimeCatalogJsonText,
} from "../src/errors.mjs";
import {
  BINDING_OPTION_GROUP_SPECS,
  BINDING_OPERATION_EXPECTATIONS,
  METADATA_SPECS,
  TEXT_MEASUREMENT_PROTOCOL_VERSION,
} from "../src/generated/binding-contract.mjs";
import { NODE_BINDING_OPERATIONS } from "../src/generated/capability-surface.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const PACKAGE_VERSION = JSON.parse(
  readFileSync(path.join(nodeRoot, "package-surfaces.json"), "utf8"),
).version;

function optionGroupIds(capabilityIds, { usesSvgPipeline = false } = {}) {
  const capabilities = new Set(capabilityIds);
  return BINDING_OPTION_GROUP_SPECS
    .filter(
      (spec) =>
        spec.always_available ||
        (spec.requires_svg_pipeline && usesSvgPipeline) ||
        spec.any_capability_ids.some((id) => capabilities.has(id)),
    )
    .map((spec) => spec.id);
}

function metadataIds(capabilityIds) {
  const capabilities = new Set(capabilityIds);
  return METADATA_SPECS
    .filter(
      (spec) =>
        spec.required_capability_id === null ||
        capabilities.has(spec.required_capability_id),
    )
    .map((spec) => spec.id);
}

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

function operationSuccess(expectation, data = "payload", additions = {}) {
  const metadata = {
    version: expectation.metadata_schema_version,
    operation_id: expectation.operation_id,
    media_type: expectation.media_type,
    runtime_policy: "deterministic",
    byte_length: Buffer.byteLength(data),
    ...additions.metadata,
  };
  return JSON.stringify({
    version: 1,
    ok: true,
    result: {
      operation_id: expectation.operation_id,
      media_type: expectation.media_type,
      data,
      metadata_json: JSON.stringify(metadata),
      ...additions.result,
    },
    ...additions.envelope,
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

test("operation errors preserve structured resource details", () => {
  const resource = {
    cause: "arithmetic_overflow",
    limit_id: "max_embedded_image_bytes",
    phase: "embedded_image_decode",
    actual: 5,
    max: 4,
    profile: "constrained",
  };
  const error = new MermanOperationError({
    code: 10,
    code_name: "MERMAN_RESOURCE_LIMIT_EXCEEDED",
    kind: "generic",
    capability_id: null,
    details: { resource },
    message: "embedded image is too large",
  });

  assert.deepEqual(error.resourceDetails, resource);
});

test("runtime catalog lossless integer scanning covers every known numeric path", () => {
  const unsafe = "9007199254740991.1";
  const catalogs = [
    `{"schema_version":${unsafe}}`,
    `{"transport_api_version":${unsafe}}`,
    `{"options_schema_versions":[${unsafe}]}`,
    `{"payload_schemas":[{"version":${unsafe}}]}`,
    `{"capabilities":{"text_measurement":{"protocol_version":${unsafe}}}}`,
    `{"constructor_service_contracts":[{"resource_limits":[{"value":${unsafe}}]}]}`,
    `{"output_contracts":[{"embedded_images":{"limits":{"max_bytes_per_image":${unsafe}}}}]}`,
    `{"output_contracts":[{"embedded_images":{"limits":{"max_total_bytes":${unsafe}}}}]}`,
    `{"output_contracts":[{"embedded_images":{"limits":{"max_pixels_per_image":${unsafe}}}}]}`,
    `{"output_contracts":[{"embedded_images":{"limits":{"max_total_pixels":${unsafe}}}}]}`,
    `{"registry":{"diagram_family_count":${unsafe}}}`,
    `{"resources":{"limits":[{"minimum_value":${unsafe}}]}}`,
    `{"resources":{"profiles":[{"limits":{"future_limit":${unsafe}}}]}}`,
  ];
  for (const catalog of catalogs) {
    assert.throws(
      () => parseRuntimeCatalogJsonText(catalog),
      /known integer fields as exact JSON-safe integers/i,
    );
  }

  const additive = parseRuntimeCatalogJsonText(
    `{"future_ratio":0.5,"future_nested":{"values":[${unsafe}]}}`,
  );
  assert.equal(additive.future_ratio, 0.5);
  assert.equal(additive.future_nested.values.length, 1);
});

function runtimeCatalog(overrides = {}) {
  return {
    schema_version: 1,
    transport_api_version: 1,
    package_version: PACKAGE_VERSION,
    options_schema_versions: [2],
    payload_schemas: [
      { id: "binding-result", version: 1 },
      { id: "operation-metadata", version: 1 },
    ],
    metadata_ids: metadataIds(["layout-cytoscape", "layout-elk", "math", "svg"]),
    option_group_ids: optionGroupIds(
      ["layout-cytoscape", "layout-elk", "math", "svg"],
      { usesSvgPipeline: true },
    ),
    constructor_service_ids: [],
    constructor_service_contracts: [],
    capabilities: {
      capability_ids: ["layout-cytoscape", "layout-elk", "math", "svg"],
      output_ids: ["svg"],
      operation_ids: ["layout-json", "semantic-json", "svg", "svg-plan-json"],
      system_adapter_ids: [],
      text_measurement: {
        protocol_version: TEXT_MEASUREMENT_PROTOCOL_VERSION,
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
        minimum_value: 1,
        operation_ids: ["layout-json", "semantic-json", "svg", "svg-plan-json"],
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
  catalog.future_ratio = 0.5;
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
  assert.equal(engine.runtimeCatalog.future_ratio, 0.5);
  await engine.dispose();
});

test("runtime catalog identifiers enforce the shared exact UTF-8 field ceiling", async () => {
  const exactCatalog = runtimeCatalog();
  exactCatalog.capabilities.capability_ids.push(
    "z".repeat(NODE_TRANSPORT_FIELD_LIMITS.capability_id_utf8_bytes),
  );
  exactCatalog.capabilities.capability_ids.sort();
  const exactFactory = transportFactory({
    runtimeCatalogJson: () => JSON.stringify(exactCatalog),
  });
  const engine = await createNodeEngine({}, { loadTransport: exactFactory.loadTransport });
  await engine.dispose();

  const oversizedCatalog = structuredClone(exactCatalog);
  oversizedCatalog.capabilities.capability_ids[oversizedCatalog.capabilities.capability_ids.length - 1] +=
    "z";
  const oversizedFactory = transportFactory({
    runtimeCatalogJson: () => JSON.stringify(oversizedCatalog),
  });
  await assert.rejects(
    createNodeEngine({}, { loadTransport: oversizedFactory.loadTransport }),
    /field limit/i,
  );
});

test("runtime catalog preserves unknown future output contracts without weakening the known profile", async () => {
  const catalog = runtimeCatalog();
  catalog.capabilities.capability_ids = [
    ...catalog.capabilities.capability_ids,
    "future-image",
  ].sort();
  catalog.capabilities.output_ids = ["future-image", "svg"];
  catalog.capabilities.operation_ids = [
    ...catalog.capabilities.operation_ids,
    "future-render",
  ].sort();
  catalog.option_group_ids = [
    ...catalog.option_group_ids,
    "future-raster",
  ].sort();
  catalog.output_contracts = [
    {
      id: "future-image",
      media_type: "image/future",
      system_fonts: {
        source_id: "future-font-source",
        discovery: "first-use",
        cache_scope: "process-global",
        host_dependent: true,
        caller_configurable: false,
        resource_bounded: false,
        future_font_metadata: true,
      },
      embedded_images: {
        source_ids: ["future-data-url"],
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
  catalog.resources.limits[0].operation_ids = [
    ...catalog.resources.limits[0].operation_ids,
    "future-render",
  ].sort();
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
  const futureContract = {
    id: "future-image",
    media_type: "image/future",
    system_fonts: {
      source_id: "future-font-source",
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
  const addFutureOutput = (catalog, contract = futureContract) => {
    catalog.capabilities.capability_ids.push("future-image");
    catalog.capabilities.capability_ids.sort();
    catalog.capabilities.output_ids.unshift("future-image");
    catalog.capabilities.operation_ids.push("future-render");
    catalog.capabilities.operation_ids.sort();
    catalog.output_contracts.unshift(contract);
    catalog.resources.limits[0].operation_ids.push("future-render");
    catalog.resources.limits[0].operation_ids.sort();
    return catalog;
  };
  const invalidCatalogs = [];

  const missingContracts = runtimeCatalog();
  delete missingContracts.output_contracts;
  invalidCatalogs.push(missingContracts);

  const missingOutputId = runtimeCatalog();
  missingOutputId.output_contracts = [];
  invalidCatalogs.push(missingOutputId);

  const extraOutputContract = runtimeCatalog();
  extraOutputContract.output_contracts.unshift(futureContract);
  invalidCatalogs.push(extraOutputContract);

  const knownMediaDrift = runtimeCatalog();
  knownMediaDrift.output_contracts[0].media_type = "image/png";
  invalidCatalogs.push(knownMediaDrift);

  const knownEnvironmentDrift = runtimeCatalog();
  knownEnvironmentDrift.output_contracts[0].system_fonts = {
    source_id: "future-font-source",
    discovery: "first-use",
    cache_scope: "process-global",
    host_dependent: true,
    caller_configurable: false,
    resource_bounded: false,
  };
  invalidCatalogs.push(knownEnvironmentDrift);

  const malformedFonts = addFutureOutput(runtimeCatalog(), structuredClone(futureContract));
  delete malformedFonts.output_contracts[0].system_fonts.discovery;
  invalidCatalogs.push(malformedFonts);

  const malformedImages = structuredClone(malformedFonts);
  malformedImages.output_contracts[0] = structuredClone(futureContract);
  malformedImages.output_contracts[0].embedded_images.filesystem_access = "false";
  invalidCatalogs.push(malformedImages);

  const malformedImageLimits = structuredClone(malformedFonts);
  malformedImageLimits.output_contracts[0] = structuredClone(futureContract);
  delete malformedImageLimits.output_contracts[0].embedded_images.limits.max_total_pixels;
  invalidCatalogs.push(malformedImageLimits);

  const malformedSourceIds = structuredClone(malformedFonts);
  malformedSourceIds.output_contracts[0] = structuredClone(futureContract);
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
    metadataJson(id) {
      return JSON.stringify({ id, future_metadata: true });
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
      version: 2,
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

test("operation URI policy normalizes empty values and rejects forbidden identifiers", async () => {
  const factory = transportFactory();
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });

  await engine.executeOperation({
    operationId: "svg",
    source: "flowchart TD\nA --> B",
    uri: "",
  });
  assert.deepEqual(factory.calls, [
    {
      operation_id: "svg",
      source: "flowchart TD\nA --> B",
      uri: null,
    },
  ]);

  assert.throws(
    () => engine.executeOperation({
      operationId: "svg",
      source: "flowchart TD\nA --> B",
      uri: "file:///diagram.mmd",
    }),
    /does not accept a uri/i,
  );
  assert.equal(factory.calls.length, 1);
  await engine.dispose();
});

test("operation request fields accept exact UTF-8 ceilings and reject plus one", async () => {
  const factory = transportFactory({
    async execute(requestJson) {
      const request = JSON.parse(requestJson);
      factory.calls.push(request);
      if (request.operation_id === "document-analysis-json") {
        return failure({ kind: "missing-capability", capabilityId: "analysis" });
      }
      return success();
    },
  });
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });

  const exactSource = "x".repeat(NODE_TRANSPORT_FIELD_LIMITS.source_utf8_bytes);
  assert.equal(await engine.renderSvg(exactSource), "<svg />");
  assert.throws(
    () => engine.renderSvg(`${exactSource}x`),
    /field limit/i,
  );

  const exactOptionsJson = "{}" + " ".repeat(
    NODE_TRANSPORT_FIELD_LIMITS.options_json_utf8_bytes - 2,
  );
  assert.equal(
    await engine.renderSvg("flowchart TD\nA", { optionsJson: exactOptionsJson }),
    "<svg />",
  );
  assert.throws(
    () => engine.renderSvg("flowchart TD\nA", { optionsJson: `${exactOptionsJson} ` }),
    /field limit|wire limit/i,
  );

  const exactUri = `file:///${"x".repeat(
    NODE_TRANSPORT_FIELD_LIMITS.uri_utf8_bytes - Buffer.byteLength("file:///"),
  )}`;
  await assert.rejects(
    engine.executeOperation({
      operationId: "document-analysis-json",
      source: "flowchart TD\nA",
      uri: exactUri,
    }),
    MermanOperationError,
  );
  assert.throws(
    () => engine.executeOperation({
      operationId: "document-analysis-json",
      source: "flowchart TD\nA",
      uri: "",
    }),
    /requires a non-empty uri/i,
  );
  assert.throws(
    () => engine.executeOperation({
      operationId: "document-analysis-json",
      source: "flowchart TD\nA",
      uri: `${exactUri}x`,
    }),
    /field limit/i,
  );

  assert.equal(factory.calls.length, 3);
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

test("named metadata and SVG-plan helpers preserve text JSON payloads", async () => {
  const plan = {
    schema_version: 1,
    planned_operation_id: "svg",
    ready: true,
    future_plan_metadata: { source: "descriptor" },
  };
  const metadataCalls = [];
  const factory = transportFactory({
    async execute(requestJson) {
      const request = JSON.parse(requestJson);
      factory.calls.push(request);
      return jsonSuccess(request.operation_id, plan);
    },
    executeSync(requestJson) {
      const request = JSON.parse(requestJson);
      factory.calls.push(request);
      return jsonSuccess(request.operation_id, plan);
    },
    metadataJson(id) {
      metadataCalls.push(id);
      return JSON.stringify({ id, future_metadata: { preserved: true } });
    },
  });
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });

  assert.equal(
    engine.metadataJson("supported-diagrams"),
    JSON.stringify({
      id: "supported-diagrams",
      future_metadata: { preserved: true },
    }),
  );
  assert.equal(await engine.svgPlanJson("flowchart TD\nA --> B"), JSON.stringify(plan));
  assert.equal(engine.svgPlanJsonSync("flowchart TD\nA --> B"), JSON.stringify(plan));
  assert.deepEqual(metadataCalls, ["supported-diagrams"]);
  assert.deepEqual(factory.calls.map(({ operation_id }) => operation_id), [
    "svg-plan-json",
    "svg-plan-json",
  ]);
  await engine.dispose();
  assert.throws(
    () => engine.metadataJson("supported-diagrams"),
    MermanDisposedError,
  );
});

test("metadata helper rejects non-text, oversized, unadvertised, and typed-error responses", async () => {
  const directObjectFactory = transportFactory({
    metadataJson(id) {
      return { id };
    },
  });
  const directObjectEngine = await createNodeEngine(
    {},
    { loadTransport: directObjectFactory.loadTransport },
  );
  assert.throws(
    () => directObjectEngine.metadataJson("supported-diagrams"),
    MermanInvalidTransportError,
  );
  assert.throws(
    () => directObjectEngine.metadataJson("future-metadata"),
    /not callable/i,
  );
  await directObjectEngine.dispose();

  const exactMetadata = "{}" + " ".repeat(
    NODE_TRANSPORT_LIMITS.metadata.max_utf8_bytes - 2,
  );
  const boundaryFactory = transportFactory({
    metadataJson() {
      return exactMetadata;
    },
  });
  const boundaryEngine = await createNodeEngine(
    {},
    { loadTransport: boundaryFactory.loadTransport },
  );
  assert.equal(boundaryEngine.metadataJson("supported-diagrams"), exactMetadata);
  boundaryFactory.transport.metadataJson = () => `${exactMetadata} `;
  assert.throws(
    () => boundaryEngine.metadataJson("supported-diagrams"),
    /wire limit/i,
  );
  await boundaryEngine.dispose();

  const typedErrorFactory = transportFactory({
    metadataJson() {
      throw new Error(failure({ kind: "unknown-operation" }));
    },
  });
  const typedErrorEngine = await createNodeEngine(
    {},
    { loadTransport: typedErrorFactory.loadTransport },
  );
  assert.throws(
    () => typedErrorEngine.metadataJson("supported-diagrams"),
    MermanOperationError,
  );
  await typedErrorEngine.dispose();
});

test("generic results preserve the candidate operation metadata JSON", async () => {
  const factory = transportFactory();
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });

  const result = await engine.executeOperation({
    operationId: "svg",
    source: "flowchart TD\nA --> B",
  });

  assert.equal(typeof result.metadata_json, "string");
  assert.deepEqual(JSON.parse(result.metadata_json), {
    version: 1,
    operation_id: "svg",
    media_type: "image/svg+xml",
    runtime_policy: "deterministic",
    byte_length: 7,
  });
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

  const malformedLimit = runtimeCatalog();
  malformedLimit.resources.limits = [{ id: "max-source-bytes" }];
  invalidCatalogs.push(malformedLimit);

  const staleOptionsSchema = runtimeCatalog();
  staleOptionsSchema.options_schema_versions = [1];
  invalidCatalogs.push(staleOptionsSchema);

  const malformedPayloadSchema = runtimeCatalog();
  malformedPayloadSchema.payload_schemas[0].version = 0;
  invalidCatalogs.push(malformedPayloadSchema);

  const unavailableLimitOperation = runtimeCatalog();
  unavailableLimitOperation.resources.limits[0].operation_ids = ["future-operation"];
  invalidCatalogs.push(unavailableLimitOperation);

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

  const duplicateProfile = runtimeCatalog();
  duplicateProfile.resources.profiles.push({ ...duplicateProfile.resources.profiles[0] });
  invalidCatalogs.push(duplicateProfile);

  const nonrecommendedDefault = runtimeCatalog();
  nonrecommendedDefault.resources.general_binding_default_profile = "trusted-native";
  invalidCatalogs.push(nonrecommendedDefault);

  const multipleRecommendedProfiles = runtimeCatalog();
  multipleRecommendedProfiles.resources.profiles[1].recommended_binding_default = true;
  invalidCatalogs.push(multipleRecommendedProfiles);

  const hardCapOverridable = runtimeCatalog();
  hardCapOverridable.resources.limits[0].hard_cap = true;
  invalidCatalogs.push(hardCapOverridable);

  const hardCapUnbounded = runtimeCatalog();
  hardCapUnbounded.resources.limits[0].hard_cap = true;
  hardCapUnbounded.resources.limits[0].overridable = false;
  invalidCatalogs.push(hardCapUnbounded);

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

test("runtime catalog follows descriptor-owned SVG compiled prerequisites", async () => {
  const descriptor = JSON.parse(
    readFileSync(
      path.join(repositoryRoot, "capabilities", "feature-surface-v1.json"),
      "utf8",
    ),
  );
  const pipelineOperations = descriptor.binding_operations.filter((operation) =>
    operation.compiled_prerequisites.includes("svg")
  );
  assert.deepEqual(
    NODE_BINDING_OPERATIONS,
    descriptor.binding_operations
      .map(({ id, compiled_prerequisites }) => ({ id, compiled_prerequisites }))
      .sort((left, right) => left.id.localeCompare(right.id)),
  );
  assert.notEqual(pipelineOperations.length, 0);
  const catalog = runtimeCatalog();
  const capabilityIds = new Set(catalog.capabilities.capability_ids);
  const outputIds = new Set(catalog.capabilities.output_ids);
  for (const operationId of catalog.capabilities.operation_ids) {
    const operation = descriptor.binding_operations.find(({ id }) => id === operationId);
    assert.ok(operation, operationId);
    assert.equal(
      operation.compiled_prerequisites.every((id) => capabilityIds.has(id)),
      true,
      operationId,
    );
    if (operation.output !== null) {
      assert.equal(outputIds.has(operation.output), true, operationId);
    }
  }

  const factory = transportFactory({
    runtimeCatalogJson() {
      return JSON.stringify(catalog);
    },
  });
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });
  assert.deepEqual(
    engine.runtimeCatalog.capabilities.capability_ids,
    NODE_WIRE_CONTRACT.artifact.capability_ids,
  );
  await engine.dispose();

  const missingCompiledPrerequisite = runtimeCatalog();
  missingCompiledPrerequisite.capabilities.capability_ids =
    missingCompiledPrerequisite.capabilities.capability_ids.filter((id) => id !== "svg");
  const missingProviderFactory = transportFactory({
    runtimeCatalogJson() {
      return JSON.stringify(missingCompiledPrerequisite);
    },
  });
  await assert.rejects(
    createNodeEngine({}, { loadTransport: missingProviderFactory.loadTransport }),
    /missing implied capability|known capabilities do not match the Node artifact contract/i,
  );
});

test("schema-1 catalog extensions validate strictly and preserve open discovery", async () => {
  const additive = runtimeCatalog();
  additive.payload_schemas.splice(1, 0, { id: "future-payload", version: 9 });
  additive.metadata_ids.splice(1, 0, "future-metadata");
  additive.option_group_ids.push("future-option-group");
  additive.option_group_ids.sort();
  additive.constructor_service_ids = ["future-constructor-service"];
  additive.constructor_service_contracts = [{
    id: "future-constructor-service",
    provided_text_measurement_provider_ids: ["future-provider"],
    resource_limits: [{
      id: "future-constructor-limit",
      phase: "future-construction",
      unit: "items",
      description: "Future constructor-owned resource ceiling.",
      value: 0,
      future_limit_metadata: true,
    }],
    future_service_metadata: true,
  }];
  additive.capabilities.text_measurement.provider_ids = ["future-provider", "vendored"];
  additive.future_root = { preserved: true };
  const additiveFactory = transportFactory({
    runtimeCatalogJson() {
      return JSON.stringify(additive);
    },
  });
  const additiveEngine = await createNodeEngine(
    {},
    { loadTransport: additiveFactory.loadTransport },
  );
  assert.deepEqual(additiveEngine.runtimeCatalog.payload_schemas, additive.payload_schemas);
  assert.deepEqual(additiveEngine.runtimeCatalog.option_group_ids, additive.option_group_ids);
  assert.deepEqual(
    additiveEngine.runtimeCatalog.capabilities.text_measurement.provider_ids,
    additive.capabilities.text_measurement.provider_ids,
  );
  assert.deepEqual(
    additiveEngine.runtimeCatalog.constructor_service_ids,
    additive.constructor_service_ids,
  );
  assert.deepEqual(
    additiveEngine.runtimeCatalog.constructor_service_contracts,
    additive.constructor_service_contracts,
  );
  assert.deepEqual(additiveEngine.runtimeCatalog.future_root, { preserved: true });
  assert.throws(
    () => additiveEngine.metadataJson("future-metadata"),
    /not callable through this SDK/i,
  );
  await additiveEngine.dispose();

  const futureOutput = runtimeCatalog();
  futureOutput.capabilities.capability_ids.push("future-image");
  futureOutput.capabilities.capability_ids.sort();
  futureOutput.capabilities.output_ids.unshift("future-image");
  futureOutput.capabilities.operation_ids.push("future-render");
  futureOutput.capabilities.operation_ids.sort();
  futureOutput.option_group_ids.push("future-image-options");
  futureOutput.option_group_ids.sort();
  futureOutput.output_contracts.unshift({
    id: "future-image",
    media_type: "image/future",
    system_fonts: null,
    embedded_images: null,
    future_output_contract: true,
  });
  futureOutput.resources.limits[0].operation_ids.push("future-render");
  futureOutput.resources.limits[0].operation_ids.sort();
  const futureOutputFactory = transportFactory({
    runtimeCatalogJson() {
      return JSON.stringify(futureOutput);
    },
  });
  const futureOutputEngine = await createNodeEngine(
    {},
    { loadTransport: futureOutputFactory.loadTransport },
  );
  assert.deepEqual(futureOutputEngine.runtimeCatalog.capabilities.output_ids, [
    "future-image",
    "svg",
  ]);
  assert.equal(
    futureOutputEngine.runtimeCatalog.output_contracts[0].future_output_contract,
    true,
  );
  assert.throws(
    () => futureOutputEngine.executeOperationSync({
      operationId: "future-render",
      source: "flowchart TD\nA --> B",
    }),
    /not callable through this SDK version/i,
  );
  await futureOutputEngine.dispose();

  const legacy = runtimeCatalog();
  delete legacy.option_group_ids;
  delete legacy.constructor_service_ids;
  delete legacy.constructor_service_contracts;
  const legacyFactory = transportFactory({
    runtimeCatalogJson() {
      return JSON.stringify(legacy);
    },
  });
  const legacyEngine = await createNodeEngine({}, { loadTransport: legacyFactory.loadTransport });
  assert.deepEqual(legacyEngine.runtimeCatalog.option_group_ids, []);
  assert.deepEqual(legacyEngine.runtimeCatalog.constructor_service_ids, []);
  assert.deepEqual(legacyEngine.runtimeCatalog.constructor_service_contracts, []);
  await legacyEngine.dispose();

  const hiddenKnownOperation = runtimeCatalog();
  hiddenKnownOperation.capabilities.operation_ids = hiddenKnownOperation.capabilities.operation_ids
    .filter((id) => id !== "svg-plan-json");
  hiddenKnownOperation.resources.limits[0].operation_ids =
    hiddenKnownOperation.resources.limits[0].operation_ids
      .filter((id) => id !== "svg-plan-json");
  const hiddenOperationFactory = transportFactory({
    runtimeCatalogJson() {
      return JSON.stringify(hiddenKnownOperation);
    },
  });
  await assert.rejects(
    createNodeEngine({}, { loadTransport: hiddenOperationFactory.loadTransport }),
    /known operations do not match the Node artifact contract/i,
  );

  const metadataSubset = runtimeCatalog();
  metadataSubset.metadata_ids = ["supported-diagrams"];
  const metadataSubsetFactory = transportFactory({
    runtimeCatalogJson() {
      return JSON.stringify(metadataSubset);
    },
  });
  await assert.rejects(
    createNodeEngine({}, { loadTransport: metadataSubsetFactory.loadTransport }),
    /known metadata ids do not match the Node artifact contract/i,
  );

  const invalidCatalogs = [];
  const missingRequiredPayload = runtimeCatalog();
  missingRequiredPayload.payload_schemas = [{ id: "binding-result", version: 1 }];
  invalidCatalogs.push(missingRequiredPayload);

  const wrongRequiredPayloadVersion = runtimeCatalog();
  wrongRequiredPayloadVersion.payload_schemas[1].version = 2;
  invalidCatalogs.push(wrongRequiredPayloadVersion);

  const missingKnownOptionGroup = runtimeCatalog();
  missingKnownOptionGroup.option_group_ids = missingKnownOptionGroup.option_group_ids.filter(
    (id) => id !== "svg",
  );
  invalidCatalogs.push(missingKnownOptionGroup);

  const unavailableKnownOptionGroup = runtimeCatalog();
  unavailableKnownOptionGroup.option_group_ids.push("lint");
  unavailableKnownOptionGroup.option_group_ids.sort();
  invalidCatalogs.push(unavailableKnownOptionGroup);

  const unavailableKnownMetadata = runtimeCatalog();
  unavailableKnownMetadata.metadata_ids.unshift("ascii-capabilities");
  invalidCatalogs.push(unavailableKnownMetadata);

  const missingImpliedCapability = runtimeCatalog();
  missingImpliedCapability.capabilities.capability_ids = ["layout-elk"];
  missingImpliedCapability.capabilities.output_ids = [];
  missingImpliedCapability.capabilities.operation_ids = ["semantic-json"];
  missingImpliedCapability.capabilities.text_measurement = null;
  invalidCatalogs.push(missingImpliedCapability);

  const unsupportedKnownService = runtimeCatalog();
  unsupportedKnownService.constructor_service_ids = ["host-text-measurement"];
  unsupportedKnownService.constructor_service_contracts = [{
    id: "host-text-measurement",
    provided_text_measurement_provider_ids: ["host-callback"],
    resource_limits: [],
  }];
  unsupportedKnownService.capabilities.text_measurement.provider_ids = [
    "host-callback",
    "vendored",
  ];
  invalidCatalogs.push(unsupportedKnownService);

  const missingServiceContracts = runtimeCatalog();
  delete missingServiceContracts.constructor_service_contracts;
  invalidCatalogs.push(missingServiceContracts);

  const orphanServiceContracts = runtimeCatalog();
  delete orphanServiceContracts.constructor_service_ids;
  invalidCatalogs.push(orphanServiceContracts);

  const mismatchedServiceContracts = runtimeCatalog();
  mismatchedServiceContracts.constructor_service_ids = ["future-service"];
  mismatchedServiceContracts.constructor_service_contracts = [{
    id: "different-future-service",
    provided_text_measurement_provider_ids: [],
    resource_limits: [],
  }];
  invalidCatalogs.push(mismatchedServiceContracts);

  const malformedServiceLimit = runtimeCatalog();
  malformedServiceLimit.constructor_service_ids = ["future-service"];
  malformedServiceLimit.constructor_service_contracts = [{
    id: "future-service",
    provided_text_measurement_provider_ids: [],
    resource_limits: [{
      id: "future-limit",
      phase: "construction",
      unit: "items",
      description: "Future limit.",
      value: -1,
    }],
  }];
  invalidCatalogs.push(malformedServiceLimit);

  const unsafeServiceLimit = runtimeCatalog();
  unsafeServiceLimit.constructor_service_ids = ["future-service"];
  unsafeServiceLimit.constructor_service_contracts = [{
    id: "future-service",
    provided_text_measurement_provider_ids: [],
    resource_limits: [{
      id: "future-limit",
      phase: "construction",
      unit: "items",
      description: "Future limit.",
      value: Number.MAX_SAFE_INTEGER + 1,
    }],
  }];
  invalidCatalogs.push(unsafeServiceLimit);

  const duplicateProviderOwner = runtimeCatalog();
  duplicateProviderOwner.constructor_service_ids = ["future-service-a", "future-service-b"];
  duplicateProviderOwner.constructor_service_contracts = [
    {
      id: "future-service-a",
      provided_text_measurement_provider_ids: ["future-provider"],
      resource_limits: [],
    },
    {
      id: "future-service-b",
      provided_text_measurement_provider_ids: ["future-provider"],
      resource_limits: [],
    },
  ];
  duplicateProviderOwner.capabilities.text_measurement.provider_ids = [
    "future-provider",
    "vendored",
  ];
  invalidCatalogs.push(duplicateProviderOwner);

  const pipelineProviderOwnedByService = runtimeCatalog();
  pipelineProviderOwnedByService.constructor_service_ids = ["future-service"];
  pipelineProviderOwnedByService.constructor_service_contracts = [{
    id: "future-service",
    provided_text_measurement_provider_ids: ["vendored"],
    resource_limits: [],
  }];
  invalidCatalogs.push(pipelineProviderOwnedByService);

  const invalidSystemFontIdentifier = runtimeCatalog();
  invalidSystemFontIdentifier.output_contracts[0].system_fonts = {
    source_id: "Bad ID",
    discovery: "first-use",
    cache_scope: "process-global",
    host_dependent: true,
    caller_configurable: false,
    resource_bounded: false,
  };
  invalidCatalogs.push(invalidSystemFontIdentifier);

  const invalidResourceLimitIdentifier = runtimeCatalog();
  invalidResourceLimitIdentifier.resources.limits[0].id = "Bad ID";
  invalidResourceLimitIdentifier.resources.profiles[0].limits = { "Bad ID": 1024 };
  invalidResourceLimitIdentifier.resources.profiles[1].limits = { "Bad ID": null };
  invalidCatalogs.push(invalidResourceLimitIdentifier);

  const invalidResourceProfileIdentifier = runtimeCatalog();
  invalidResourceProfileIdentifier.resources.profiles[0].id = "Bad ID";
  invalidResourceProfileIdentifier.resources.general_binding_default_profile = "Bad ID";
  invalidCatalogs.push(invalidResourceProfileIdentifier);

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

test("runtime catalog transport boundary accepts JSON text only", async () => {
  const factory = transportFactory({
    runtimeCatalogJson() {
      return runtimeCatalog();
    },
  });

  await assert.rejects(
    createNodeEngine({}, { loadTransport: factory.loadTransport }),
    MermanInvalidTransportError,
  );

  const catalogJson = JSON.stringify(runtimeCatalog());
  const exactCatalog = catalogJson + " ".repeat(
    NODE_TRANSPORT_LIMITS.runtime_catalog.max_utf8_bytes - Buffer.byteLength(catalogJson),
  );
  const boundaryFactory = transportFactory({
    runtimeCatalogJson() {
      return exactCatalog;
    },
  });
  const engine = await createNodeEngine({}, { loadTransport: boundaryFactory.loadTransport });
  await engine.dispose();

  boundaryFactory.transport.runtimeCatalogJson = () => `${exactCatalog} `;
  await assert.rejects(
    createNodeEngine({}, { loadTransport: boundaryFactory.loadTransport }),
    /wire limit/i,
  );

  const denseUnknownValueCount = 400_000;
  const denseUnknownCatalog = `${catalogJson.slice(0, -1)},"future_values":[${"0,".repeat(
    denseUnknownValueCount - 1,
  )}0]}`;
  assert.ok(
    Buffer.byteLength(denseUnknownCatalog) <
      NODE_TRANSPORT_LIMITS.runtime_catalog.max_utf8_bytes,
  );
  const denseUnknownFactory = transportFactory({
    runtimeCatalogJson() {
      return denseUnknownCatalog;
    },
  });
  await assert.rejects(
    createNodeEngine({}, { loadTransport: denseUnknownFactory.loadTransport }),
    /member-work|token-work/i,
  );

  const roundedUnsafeInteger = JSON.stringify(runtimeCatalog()).replace(
    '"diagram_family_count":35',
    '"diagram_family_count":9007199254740991.1',
  );
  const roundedUnsafeIntegerFactory = transportFactory({
    runtimeCatalogJson() {
      return roundedUnsafeInteger;
    },
  });
  await assert.rejects(
    createNodeEngine({}, { loadTransport: roundedUnsafeIntegerFactory.loadTransport }),
    /exact JSON-safe integers/i,
  );
});

test("public TypeScript declarations cover the generic operation API", () => {
  const declarations = readFileSync(path.join(nodeRoot, "src", "index.d.ts"), "utf8");
  for (const method of [
    "dispose",
    "executeOperation",
    "executeOperationSync",
    "metadataJson",
    "renderSvg",
    "renderSvgSync",
    "svgPlanJson",
    "svgPlanJsonSync",
  ]) {
    assert.equal(typeof MermanEngine.prototype[method], "function", method);
    assert.match(declarations, new RegExp(`\\b${method}\\s*\\(`));
  }
  assert.equal(publicApi.MermanEngine, MermanEngine);
  assert.equal(publicApi.MermanInvalidTransportError, MermanInvalidTransportError);
  assert.equal("MermanNodeEngine" in publicApi, false);
  assert.doesNotMatch(declarations, /\bMermanNodeEngine\b/);
  assert.match(
    declarations,
    /export declare class MermanEngine\s*{[^}]*private constructor\(\);/s,
  );
  assert.doesNotMatch(declarations, /"deterministic"\s*\|\s*"native"/);
  assert.match(declarations, /class MermanInvalidTransportError extends MermanError/);
  assert.match(declarations, /\breadonly runtimeCatalog:/);
  assert.match(declarations, /\boptionsJson\?: string;/);
  assert.match(declarations, /provider_ids:\s*string\[\]/);
  assert.match(declarations, /options_schema_versions:\s*number\[\]/);
  assert.match(declarations, /option_group_ids:\s*string\[\]/);
  assert.match(declarations, /constructor_service_ids:\s*string\[\]/);
  assert.match(
    declarations,
    /constructor_service_contracts:\s*MermanRuntimeConstructorServiceContract\[\]/,
  );
  assert.match(declarations, /operation_ids:\s*string\[\]/);
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

test("MermanEngine construction is factory-only", async () => {
  const factoryOnly = (construct) => {
    assert.throws(construct, (error) => {
      assert.ok(error instanceof MermanInvalidTransportError);
      assert.match(error.message, /createNodeEngine/);
      return true;
    });
  };

  factoryOnly(() => new publicApi.MermanEngine());
  factoryOnly(
    () => new publicApi.MermanEngine(
      transportFactory().transport,
      runtimeCatalog(),
      { concurrency: 1, maxQueue: 1 },
    ),
  );

  const factory = transportFactory();
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });
  assert.ok(engine instanceof MermanEngine);
  assert.ok(engine instanceof publicApi.MermanEngine);
  await engine.dispose();
});

test("binding options preserve the shared profile vocabulary and reject host measurement", () => {
  assert.deepEqual(
    normalizeBindingOptions({
      resources: { profile: "trusted-native" },
      fixed_today: "2026-07-23",
    }),
    {
      version: 2,
      runtime_policy: "deterministic",
      resources: { profile: "trusted-native" },
      fixed_today: "2026-07-23",
    },
  );

  assert.throws(
    () => normalizeBindingOptions({ version: 1 }),
    /unsupported binding options schema version `1`; expected 2/i,
  );
  assert.throws(
    () => normalizeBindingOptions({ runtime_policy: "native" }),
    /runtime_policy.*deterministic/i,
  );
  for (const textMeasurement of ["vendored", "parity", "deterministic"]) {
    assert.equal(
      normalizeBindingOptions({
        environment: { text_measurement: textMeasurement },
      }).environment.text_measurement,
      textMeasurement,
    );
  }
  assert.throws(
    () => normalizeBindingOptions({ textMeasurer: () => ({ width: 1 }) }),
    /not a JSON wire value/i,
  );
  assert.throws(
    () => normalizeBindingOptions({ resources: { profile: "default" } }),
    /unknown resource profile `default`/i,
  );
});

test("capability-gated errors survive while advertised-operation contradictions fail closed", async () => {
  for (const expected of [
    { kind: "missing-capability", capabilityId: "png", operationId: "png" },
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

  const contradictory = transportFactory({
    async execute() {
      return failure({ kind: "unknown-operation", capabilityId: null });
    },
  });
  const engine = await createNodeEngine({}, { loadTransport: contradictory.loadTransport });
  await assert.rejects(
    engine.executeOperation({ operationId: "svg", source: "flowchart TD\nA" }),
    MermanInvalidTransportError,
  );
  await engine.dispose();
});

test("generic execution covers the complete 13-operation matrix", async () => {
  const expectations = new Map(
    BINDING_OPERATION_EXPECTATIONS.map((expectation) => [
      expectation.operation_id,
      expectation,
    ]),
  );
  const advertised = new Set(NODE_WIRE_CONTRACT.artifact.operation_ids);
  const factory = transportFactory({
    async execute(requestJson) {
      const request = JSON.parse(requestJson);
      factory.calls.push(request);
      const expectation = expectations.get(request.operation_id);
      assert.ok(expectation, request.operation_id);
      if (!advertised.has(request.operation_id)) {
        assert.ok(expectation.unavailable);
        return failure({
          kind: expectation.unavailable.error_kind,
          capabilityId: expectation.unavailable.capability_id,
        });
      }
      const data =
        expectation.media_type === "image/svg+xml"
          ? "<svg />"
          : JSON.stringify({ operation_id: request.operation_id });
      return operationSuccess(expectation, data);
    },
  });
  const engine = await createNodeEngine({}, { loadTransport: factory.loadTransport });

  for (const expectation of BINDING_OPERATION_EXPECTATIONS) {
    const request = {
      operationId: expectation.operation_id,
      source: "flowchart TD\nA --> B",
    };
    if (expectation.requires_uri) request.uri = "file:///fixture.mmd";
    if (advertised.has(expectation.operation_id)) {
      const result = await engine.executeOperation(request);
      assert.equal(result.operation_id, expectation.operation_id);
      assert.equal(result.media_type, expectation.media_type);
    } else {
      await assert.rejects(
        engine.executeOperation(request),
        (error) => {
          assert.ok(error instanceof MermanOperationError);
          assert.equal(error.kind, expectation.unavailable.error_kind);
          assert.equal(error.capabilityId, expectation.unavailable.capability_id);
          return true;
        },
      );
    }
  }

  assert.deepEqual(
    factory.calls.map(({ operation_id }) => operation_id).sort(),
    BINDING_OPERATION_EXPECTATIONS.map(({ operation_id }) => operation_id).sort(),
  );
  assert.equal(
    factory.calls.filter(({ uri }) => uri !== null).length,
    BINDING_OPERATION_EXPECTATIONS.filter(({ requires_uri }) => requires_uri).length,
  );
  await engine.dispose();
});

test("direct transport execution failures normalize across async and sync calls", async () => {
  const typedFactory = transportFactory({
    async execute() {
      throw new Error(failure({ kind: "unknown-operation" }));
    },
    executeSync() {
      throw new Error(failure({ kind: "unknown-operation" }));
    },
  });
  const typedEngine = await createNodeEngine(
    {},
    { loadTransport: typedFactory.loadTransport },
  );
  const request = {
    operationId: "svg",
    source: "flowchart TD\nA",
  };
  const assertTypedError = (error) => {
    assert.ok(error instanceof MermanOperationError);
    assert.equal(error.kind, "unknown-operation");
    assert.equal(error.codeName, "MERMAN_UNSUPPORTED_OPERATION");
    return true;
  };
  await assert.rejects(typedEngine.executeOperation(request), assertTypedError);
  assert.throws(() => typedEngine.executeOperationSync(request), assertTypedError);
  await typedEngine.dispose();

  const transportFailure = new Error("transport failed before returning a wire envelope");
  const plainFactory = transportFactory({
    async execute() {
      throw transportFailure;
    },
    executeSync() {
      throw transportFailure;
    },
  });
  const plainEngine = await createNodeEngine(
    {},
    { loadTransport: plainFactory.loadTransport },
  );
  const assertTransportError = (error) => {
    assert.ok(error instanceof MermanInvalidTransportError);
    assert.equal(error.message, "Merman operation failed.");
    assert.equal(error.cause, transportFailure);
    return true;
  };
  await assert.rejects(plainEngine.executeOperation(request), assertTransportError);
  assert.throws(() => plainEngine.executeOperationSync(request), assertTransportError);
  await plainEngine.dispose();
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
  assert.equal(engine.dispose(), disposing);
  await assert.rejects(queued, MermanDisposedError);
  assert.equal(transportDisposed, false);
  release.resolve();
  assert.match(await active, /data-execution="1"/);
  await disposing;
  assert.equal(transportDisposed, true);
  assert.equal(executions, 1);
  await assert.rejects(engine.renderSvg("flowchart TD\nD"), MermanDisposedError);
  assert.throws(
    () => engine.executeOperationSync({ operationId: "svg", source: "flowchart TD\nD" }),
    MermanDisposedError,
  );
  assert.equal(engine.dispose(), disposing);
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
