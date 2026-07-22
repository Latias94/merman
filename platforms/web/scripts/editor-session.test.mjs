import assert from "node:assert/strict";
import test from "node:test";

import * as webApi from "../dist/index.js";
import { bindSurfaceRuntime } from "../dist/surface-runtime.js";

const nativeSessions = [];
let descriptorCalls = 0;

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
    bindingCapabilities: editorCapabilities,
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
  const editorRuntime = bindSurfaceRuntime(async () =>
    surfaceModule(() => {
      descriptorCounts.editor += 1;
    })
  );
  const fullRuntime = bindSurfaceRuntime(async () =>
    surfaceModule(() => {
      descriptorCounts.full += 1;
    })
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
    render: false,
    analysis: true,
    ascii: false,
    core_host: false,
    cytoscape_layout: false,
    elk_layout: false,
    ratex_math: false,
    editor_language: true,
    text_measurement: {
      vendored: false,
      deterministic: false,
      host_callback: false,
      font_assets: false,
    },
  };
}

function surfaceModule(recordDescriptorCall) {
  return {
    default: async () => {},
    bindingCapabilities: editorCapabilities,
    EditorSession: FakeNativeEditorSession,
    editorSemanticTokenDescriptor() {
      recordDescriptorCall();
      return runtimeDescriptor(webApi.SEMANTIC_TOKEN_DESCRIPTOR);
    },
  };
}

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
