import assert from "node:assert/strict";
import test from "node:test";

import * as webApi from "../dist/index.js";
import * as coreRuntime from "../dist/runtime-core.js";
import * as editorRuntimeBindings from "../dist/runtime-editor.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";

if (typeof globalThis.window === "undefined") globalThis.window = {};
if (typeof globalThis.document === "undefined") globalThis.document = {};

const nativeSessions = [];
let descriptorCalls = 0;
const coreTestImplementation = {
  getMerman: coreRuntime.getMerman,
  initMerman: coreRuntime.initMerman,
  isMermanInitialized: coreRuntime.isMermanInitialized,
  packageVersion: coreRuntime.packageVersion,
  runtimeCatalog: coreRuntime.runtimeCatalog,
  transportApiVersion: coreRuntime.transportApiVersion,
};
const editorTestImplementation = {
  ...coreTestImplementation,
  createEditorSession: editorRuntimeBindings.createEditorSession,
};

class FakeNativeEditorSession {
  constructor(source, version, uri, optionsJson) {
    this.source = source;
    this.version = version;
    this.uri = uri ?? "file:///merman/untitled.mmd";
    this.optionsJson = optionsJson;
    this.freeCalls = 0;
    nativeSessions.push(this);
  }

  update(source, version) {
    this.source = source;
    this.version = version;
  }

  diagnostics() {
    return { version: 1, diagnostics: [] };
  }

  diagramDetection() {
    return {
      status: "available",
      validity: "valid",
      diagramType: "flowchart",
      syntaxId: "flowchart-v2",
      effectiveLayoutId: "dagre",
    };
  }

  codeActions() {
    return [];
  }

  completions(line, character) {
    return { isIncomplete: false, items: [{ label: `${line}:${character}` }] };
  }

  hover() {
    return null;
  }

  documentSymbols() {
    return [];
  }

  workspaceSymbols(query) {
    return [{ name: query }];
  }

  definition() {
    return null;
  }

  references() {
    return [];
  }

  prepareRename() {
    return null;
  }

  rename() {
    return null;
  }

  semanticTokens() {
    return new Uint32Array([0, 0, 1, 0, 0]);
  }

  free() {
    this.freeCalls += 1;
    if (this.throwOnFree) {
      throw new Error("synthetic native free failure");
    }
  }
}

await webApi.initMerman({
  loader: async () => ({
    default: async () => {},
    packageVersion: () => "0.8.0-alpha.4",
    transportApiVersion: () => 3,
    runtimeCatalog: runtimeCatalogFixture,
    EditorSession: FakeNativeEditorSession,
    editorSemanticTokenDescriptor() {
      descriptorCalls += 1;
      return runtimeDescriptor(webApi.SEMANTIC_TOKEN_DESCRIPTOR);
    },
  }),
});

test("a native free failure still seals the browser editor session", () => {
  const session = webApi.createEditorSession("flowchart TD", 1);
  const native = nativeSessions.at(-1);
  native.throwOnFree = true;

  assert.throws(() => session.dispose(), /synthetic native free failure/);
  assert.equal(native.freeCalls, 1);
  assert.throws(() => session.diagnostics(), /editor session is disposed/i);
  session.dispose();
  assert.equal(native.freeCalls, 1);
});

test("editor sessions retain their creating surface runtime", async () => {
  const descriptorCounts = { editor: 0, full: 0 };
  const editorRuntime = bindSurfaceRuntime(
    async () =>
      surfaceModule(() => {
        descriptorCounts.editor += 1;
      }),
    editorTestImplementation,
  );
  const fullRuntime = bindSurfaceRuntime(
    async () =>
      surfaceModule(() => {
        descriptorCounts.full += 1;
      }),
    editorTestImplementation,
  );
  await editorRuntime.initMerman();
  const editorSession = editorRuntime.createEditorSession("flowchart TD", 1);
  await fullRuntime.initMerman();
  const fullSession = fullRuntime.createEditorSession("flowchart TD", 1);

  editorSession.semanticTokens();
  editorSession.semanticTokens();
  fullSession.semanticTokens();
  fullSession.semanticTokens();
  assert.deepEqual(descriptorCounts, { editor: 1, full: 1 });

  editorSession.dispose();
  fullSession.dispose();
});

test("browser editor session owns one native analyzed document", () => {
  const session = webApi.createEditorSession(
    "flowchart TD\nA-->B",
    1,
    "file:///workspace/example.mmd",
    { site_config: { layout: "dagre" } },
  );
  const native = nativeSessions.at(-1);

  assert.equal(native.source, "flowchart TD\nA-->B");
  assert.equal(native.version, 1);
  assert.equal(native.uri, "file:///workspace/example.mmd");
  assert.equal(native.optionsJson, JSON.stringify({ site_config: { layout: "dagre" } }));
  assert.equal(session.version, 1);
  assert.equal(session.uri, "file:///workspace/example.mmd");
  assert.deepEqual(session.diagnostics(), { version: 1, diagnostics: [] });
  assert.equal(session.diagramDetection().diagramType, "flowchart");
  assert.deepEqual(session.completions({ line: 2, character: 7 }), {
    isIncomplete: false,
    items: [{ label: "2:7" }],
  });
  assert.deepEqual(session.workspaceSymbols("Alpha"), [{ name: "Alpha" }]);

  session.update("flowchart TD\nA-->C", 2);
  assert.equal(native.source, "flowchart TD\nA-->C");
  assert.equal(session.version, 2);

  const firstTokens = session.semanticTokens();
  const secondTokens = session.semanticTokens();
  assert.deepEqual([...firstTokens], [0, 0, 1, 0, 0]);
  assert.deepEqual([...secondTokens], [0, 0, 1, 0, 0]);
  assert.equal(descriptorCalls, 1);

  session.dispose();
  session.dispose();
  assert.equal(native.freeCalls, 1);
  for (const access of [
    () => session.version,
    () => session.uri,
    () => session.update("flowchart TD", 3),
    () => session.diagnostics(),
    () => session.diagramDetection(),
    () => session.codeActions(),
    () => session.completions({ line: 0, character: 0 }),
    () => session.hover({ line: 0, character: 0 }),
    () => session.documentSymbols(),
    () => session.workspaceSymbols(""),
    () => session.definition({ line: 0, character: 0 }),
    () => session.references({ line: 0, character: 0 }),
    () => session.prepareRename({ line: 0, character: 0 }),
    () => session.rename({ line: 0, character: 0 }, "B"),
    () => session.semanticTokens(),
  ]) {
    assert.throws(access, /editor session is disposed/i);
  }
});

function editorCapabilities() {
  return {
    capability_ids: ["analysis", "editor"],
    output_ids: [],
    operation_ids: ["analysis-json", "semantic-json"],
    system_adapter_ids: [],
    text_measurement: null,
  };
}

function surfaceModule(recordDescriptorCall) {
  return {
    default: async () => {},
    packageVersion: () => "0.8.0-alpha.4",
    transportApiVersion: () => 3,
    runtimeCatalog: runtimeCatalogFixture,
    EditorSession: FakeNativeEditorSession,
    editorSemanticTokenDescriptor() {
      recordDescriptorCall();
      return runtimeDescriptor(webApi.SEMANTIC_TOKEN_DESCRIPTOR);
    },
  };
}

function runtimeCatalogFixture({ capabilities = editorCapabilities() } = {}) {
  return {
    schema_version: 1,
    transport_api_version: 3,
    package_version: "0.8.0-alpha.4",
    capabilities,
    registry: { diagram_family_count: 0 },
    resources: {
      general_binding_default_profile: "interactive",
      cli_default_profile: "trusted-native",
      limits: [],
      profiles: [],
    },
  };
}

test("runtime catalog rejects malformed shapes and invalid local relations", async () => {
  const cases = [
    [
      () => {
        const catalog = runtimeCatalogFixture();
        catalog.schema_version = 2;
        return catalog;
      },
      /unsupported runtime catalog schema/,
    ],
    [
      () => {
        const catalog = runtimeCatalogFixture();
        delete catalog.resources;
        return catalog;
      },
      /runtime catalog is missing required fields: resources/,
    ],
    [
      () =>
        runtimeCatalogFixture({
          capabilities: {
            capability_ids: ["editor", "analysis"],
            output_ids: [],
            operation_ids: ["analysis-json", "semantic-json"],
            system_adapter_ids: [],
            text_measurement: null,
          },
        }),
      /runtime capability IDs must be sorted and unique/,
    ],
    [
      () =>
        runtimeCatalogFixture({
          capabilities: {
            capability_ids: ["analysis", "editor"],
            output_ids: [],
            operation_ids: ["analysis-json", "analysis-json"],
            system_adapter_ids: [],
            text_measurement: null,
          },
        }),
      /runtime binding operation IDs must be sorted and unique/,
    ],
    [
      () =>
        runtimeCatalogFixture({
          capabilities: {
            capability_ids: ["analysis", "editor"],
            output_ids: ["svg"],
            operation_ids: ["analysis-json", "semantic-json"],
            system_adapter_ids: [],
            text_measurement: null,
          },
        }),
      /runtime output svg is absent from runtime binding operation IDs/,
    ],
    [
      () =>
        runtimeCatalogFixture({
          capabilities: {
            capability_ids: ["analysis", "editor"],
            output_ids: [],
            operation_ids: ["analysis-json", "semantic-json"],
            system_adapter_ids: ["system-clock"],
            text_measurement: null,
          },
        }),
      /system adapter system-clock is absent from runtime capability IDs/,
    ],
  ];

  for (const [catalog, expected] of cases) {
    const runtime = bindSurfaceRuntime(
      async () => ({
        default: async () => {},
        packageVersion: () => "0.8.0-alpha.4",
        transportApiVersion: () => 3,
        runtimeCatalog: catalog,
      }),
      coreTestImplementation,
    );
    await runtime.initMerman();
    assert.throws(() => runtime.runtimeCatalog(), expected);
  }
});

test("runtime catalog accepts unknown future IDs", async () => {
  const futureCatalog = runtimeCatalogFixture({
    capabilities: {
      capability_ids: ["analysis", "editor", "future-capability"],
      output_ids: ["future-output"],
      operation_ids: [
        "analysis-json",
        "future-operation",
        "future-output",
        "semantic-json",
      ],
      system_adapter_ids: [],
      text_measurement: null,
      future_capability_metadata: { version: 1 },
    },
  });
  futureCatalog.future_root_metadata = true;
  futureCatalog.registry.future_registry_metadata = true;
  futureCatalog.resources.future_resource_metadata = true;
  futureCatalog.resources.limits = [
    {
      id: "future-limit",
      phase: "future",
      description: "future limit",
      overridable: false,
      hard_cap: true,
      future_limit_metadata: true,
    },
  ];
  futureCatalog.resources.profiles = [
    {
      id: "future-profile",
      purpose: "future",
      trust_assumption: "future",
      recommended_binding_default: false,
      limits: {},
      future_profile_metadata: true,
    },
  ];
  const runtime = bindSurfaceRuntime(
    async () => ({
      default: async () => {},
      packageVersion: () => "0.8.0-alpha.4",
      transportApiVersion: () => 3,
      runtimeCatalog: () => futureCatalog,
    }),
    coreTestImplementation,
  );
  await runtime.initMerman();

  const catalog = runtime.runtimeCatalog();
  assert.equal(catalog.capabilities.capability_ids.at(-1), "future-capability");
  assert.equal(catalog.resources.limits[0].id, "future-limit");
  assert.equal(catalog.resources.profiles[0].id, "future-profile");
});

test("runtime catalog preserves wasm-bindgen optional and map projections", async () => {
  const catalog = runtimeCatalogFixture({
    capabilities: {
      capability_ids: ["analysis"],
      output_ids: [],
      operation_ids: ["analysis-json", "semantic-json"],
      system_adapter_ids: [],
      text_measurement: undefined,
    },
  });
  catalog.resources.profiles = [
    {
      id: "interactive",
      purpose: "test",
      trust_assumption: "test",
      recommended_binding_default: true,
      limits: new Map([
        ["max_source_bytes", 1024],
        ["max_svg_bytes", undefined],
      ]),
    },
  ];
  const runtime = bindSurfaceRuntime(
    async () => ({
      default: async () => {},
      packageVersion: () => "0.8.0-alpha.4",
      transportApiVersion: () => 3,
      runtimeCatalog: () => catalog,
    }),
    coreTestImplementation,
  );
  await runtime.initMerman();

  const normalized = runtime.runtimeCatalog();
  assert.equal(normalized.capabilities.text_measurement, null);
  assert.deepEqual(normalized.resources.profiles[0].limits, {
    max_source_bytes: 1024,
    max_svg_bytes: null,
  });
});

test("resource options preserve wrapper placement and stricter caller limits", () => {
  const resources = { profile: "constrained" };
  assert.deepEqual(
    webApi.withResourceOptions(
      {
        analysis: {
          site_config: { theme: "dark" },
          resources: { limits: { max_source_bytes: 4096 } },
        },
      },
      resources,
    ),
    {
      analysis: {
        site_config: { theme: "dark" },
        resources: {
          profile: "constrained",
          limits: { max_source_bytes: 4096 },
        },
      },
    },
  );
  assert.deepEqual(
    webApi.withResourceOptions({ parse: { suppress_errors: true } }, resources),
    {
      parse: { suppress_errors: true },
      resources,
    },
  );
});

test("resource options retain ceiling overrides while tightening another limit", () => {
  const resources = {
    profile: "constrained",
    limits: { max_source_bytes: 4096 },
  };
  assert.deepEqual(
    webApi.withResourceOptions(
      {
        resources: {
          limits: { max_model_items: 1000 },
        },
      },
      resources,
    ),
    {
      resources: {
        profile: "constrained",
        limits: {
          max_source_bytes: 4096,
          max_model_items: 1000,
        },
      },
    },
  );
  assert.throws(
    () =>
      webApi.withResourceOptions(
        {
          resources: {
            limits: { max_source_bytes: 8192 },
          },
        },
        resources,
      ),
    /loosen the transport ceiling/,
  );
});

test("resource options reject looser policy and ambiguous wrappers", () => {
  assert.throws(
    () =>
      webApi.withResourceOptions(
        { resources: { profile: "trusted-native" } },
        { profile: "constrained" },
      ),
    /loosen the transport ceiling/,
  );
  assert.throws(
    () =>
      webApi.withResourceOptions(
        {
          analysis: {},
          merman: {},
        },
        { profile: "constrained" },
      ),
    /must not contain both analysis and merman wrappers/,
  );
  assert.throws(
    () =>
      webApi.withResourceOptions(
        {
          resources: { profile: "constrained" },
          analysis: {},
        },
        { profile: "constrained" },
      ),
    /must not mix top-level resources/,
  );
  assert.throws(
    () =>
      webApi.withResourceOptions(
        { analysis: null },
        { profile: "constrained" },
      ),
    /analysis wrapper must be an object/,
  );
});

test("Web transport API version rejects invalid module reports", async () => {
  const runtime = bindSurfaceRuntime(
    async () => ({
      default: async () => {},
      transportApiVersion: () => 0,
    }),
    coreTestImplementation,
  );
  await runtime.initMerman();
  assert.throws(
    () => runtime.transportApiVersion(),
    /invalid Web transport API version/
  );
});

function runtimeDescriptor(descriptor) {
  return {
    schemaVersion: descriptor.schemaVersion,
    digest: descriptor.digest,
    tokenTypes: descriptor.tokenTypes.map(({ id, code, lspName, lspIndex }) => ({
      id,
      code,
      lspName,
      lspIndex,
    })),
    modifiers: descriptor.modifiers.map(({ id, index, bit, lspName, lspIndex }) => ({
      id,
      index,
      bit,
      lspName,
      lspIndex,
    })),
    packed: {
      encoding: descriptor.packed.encoding,
      wordWidthBits: descriptor.packed.wordWidthBits,
      recordWidth: descriptor.packed.recordWidth,
      fieldOrder: [...descriptor.packed.fieldOrder],
    },
    validTypeCodeMax: descriptor.validTypeCodeMax,
    validModifierMask: descriptor.validModifierMask,
  };
}
