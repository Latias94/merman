import assert from "node:assert/strict";
import test from "node:test";

import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  advanceCompareOperationStage,
  assertEncodedMessageBudget,
  assertRealmInitBudget,
  createOneTimeRealmInitGate,
  createRealmToken,
  realmEngineArtifactSourceBytes,
  utf8ByteLength,
  validateCompareRenderRequest,
  validateCompareRenderResponse,
  validateRealmHello,
  validateRealmEngineArtifact,
  validateRealmInit,
  validateRealmReady,
} from "./channel-protocol.ts";

const BOOT_NONCE = "b".repeat(43);
const IDENTITY = {
  kind: "compare" as const,
  realmId: "realm-1",
  realmToken: "t".repeat(43),
};
const ENGINE_ARTIFACT = {
  schemaVersion: 1 as const,
  id: "mermaid" as const,
  bytes: 17,
  sha256: "a".repeat(64),
  resourceUrl: null,
  source: "export default 1;",
};
const ENGINE_IDENTITY = {
  schemaVersion: ENGINE_ARTIFACT.schemaVersion,
  id: ENGINE_ARTIFACT.id,
  bytes: ENGINE_ARTIFACT.bytes,
  sha256: ENGINE_ARTIFACT.sha256,
};

test("one-time handshake messages bind boot and port identities", () => {
  const hello = {
    type: "realm-hello",
    protocol: REALM_PROTOCOL_VERSION,
    kind: IDENTITY.kind,
    realmId: IDENTITY.realmId,
    bootNonce: BOOT_NONCE,
  };
  assert.deepEqual(
    validateRealmHello(hello, {
      kind: IDENTITY.kind,
      realmId: IDENTITY.realmId,
      bootNonce: BOOT_NONCE,
    }),
    hello
  );

  const init = {
    type: "realm-init",
    protocol: REALM_PROTOCOL_VERSION,
    ...IDENTITY,
    bootNonce: BOOT_NONCE,
    engineArtifact: ENGINE_ARTIFACT,
  };
  assert.deepEqual(
    validateRealmInit(init, {
      kind: IDENTITY.kind,
      realmId: IDENTITY.realmId,
      bootNonce: BOOT_NONCE,
    }, ENGINE_IDENTITY),
    init
  );

  const ready = {
    type: "realm-ready",
    protocol: REALM_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence: 0,
    viewport: { width: 800, height: 600 },
  };
  assert.deepEqual(validateRealmReady(ready, IDENTITY), ready);
});

test("compare progress must visit every declared stage in order", () => {
  let stageIndex = -1;
  stageIndex = advanceCompareOperationStage(stageIndex, "fonts");
  assert.equal(stageIndex, 0);
  assert.throws(
    () => advanceCompareOperationStage(stageIndex, "load"),
    /advance exactly one stage/
  );
  assert.throws(
    () => advanceCompareOperationStage(stageIndex, "fonts"),
    /advance exactly one stage/
  );

  for (const stage of [
    "adapter-import",
    "load",
    "register",
    "initialize",
    "render",
    "svg-budget",
    "presentation",
  ] as const) {
    stageIndex = advanceCompareOperationStage(stageIndex, stage);
  }
  assert.equal(stageIndex, 7);
});

test("handshake validation rejects foreign identity and schema drift", () => {
  const hello = {
    type: "realm-hello",
    protocol: REALM_PROTOCOL_VERSION,
    kind: IDENTITY.kind,
    realmId: IDENTITY.realmId,
    bootNonce: BOOT_NONCE,
  };
  for (const invalid of [
    { ...hello, bootNonce: "x".repeat(43) },
    { ...hello, protocol: REALM_PROTOCOL_VERSION + 1 },
    { ...hello, extra: true },
  ]) {
    assert.throws(
      () =>
        validateRealmHello(invalid, {
          kind: IDENTITY.kind,
          realmId: IDENTITY.realmId,
          bootNonce: BOOT_NONCE,
        }),
      RealmProtocolError
    );
  }
});

test("engine artifact validation binds identity, bytes, and resource authority", () => {
  assert.deepEqual(
    validateRealmEngineArtifact(ENGINE_ARTIFACT, ENGINE_IDENTITY),
    ENGINE_ARTIFACT
  );
  for (const invalid of [
    { ...ENGINE_ARTIFACT, id: "benchmark-merman" },
    { ...ENGINE_ARTIFACT, bytes: ENGINE_ARTIFACT.bytes + 1 },
    { ...ENGINE_ARTIFACT, source: `${ENGINE_ARTIFACT.source}x` },
    { ...ENGINE_ARTIFACT, sha256: "A".repeat(64) },
    { ...ENGINE_ARTIFACT, resourceUrl: "https://example.test/engine.wasm" },
    { ...ENGINE_ARTIFACT, extra: true },
  ]) {
    assert.throws(
      () => validateRealmEngineArtifact(invalid, ENGINE_IDENTITY),
      RealmProtocolError
    );
  }

  const merman = {
    ...ENGINE_ARTIFACT,
    id: "benchmark-merman" as const,
    resourceUrl: "https://play.test/merman_wasm_bg.wasm",
  };
  assert.equal(
    validateRealmEngineArtifact(merman, {
      ...ENGINE_IDENTITY,
      id: "benchmark-merman",
    }).resourceUrl,
    merman.resourceUrl
  );
});

test("validated engine artifacts own one reusable UTF-8 byte buffer", () => {
  const artifact = validateRealmEngineArtifact(
    ENGINE_ARTIFACT,
    ENGINE_IDENTITY,
  );
  const first = realmEngineArtifactSourceBytes(artifact);
  const second = realmEngineArtifactSourceBytes(artifact);

  assert.equal(first, second);
  assert.equal(first.byteLength, ENGINE_ARTIFACT.bytes);
  assert.equal(new TextDecoder().decode(first), ENGINE_ARTIFACT.source);
});

test("one-time realm init gate rejects missing ports and replay", () => {
  const boot = {
    kind: IDENTITY.kind,
    realmId: IDENTITY.realmId,
    bootNonce: BOOT_NONCE,
  };
  const init = {
    type: "realm-init",
    protocol: REALM_PROTOCOL_VERSION,
    ...IDENTITY,
    bootNonce: BOOT_NONCE,
    engineArtifact: ENGINE_ARTIFACT,
  };
  assert.throws(
    () => createOneTimeRealmInitGate(boot, ENGINE_IDENTITY).consume(init, 0),
    /transfer one port/
  );

  const gate = createOneTimeRealmInitGate(boot, ENGINE_IDENTITY);
  assert.deepEqual(gate.consume(init, 1), init);
  assert.throws(() => gate.consume(init, 1), /INIT was replayed/);
});

test("protocol budgets count UTF-8 bytes and accept exact boundaries", () => {
  assert.equal(utf8ByteLength("A"), 1);
  assert.equal(utf8ByteLength("é"), 2);
  for (const value of [
    "💡",
    "\ud800",
    "\udc00",
    `${"a".repeat(15_999)}💡tail`,
  ]) {
    assert.equal(utf8ByteLength(value), new TextEncoder().encode(value).byteLength);
  }

  const source = "s".repeat(REALM_BUDGETS.sourceBytes);
  const configJson = "c".repeat(REALM_BUDGETS.configBytes);
  assert.equal(
    validateCompareRenderRequest(
      renderRequest({ source, configJson }),
      IDENTITY,
      1
    ).payload.source.length,
    source.length
  );

  const svg = "s".repeat(REALM_BUDGETS.svgBytes);
  const response = validateCompareRenderResponse(
      renderResponse({ svg }),
      IDENTITY,
      1,
      "request-1"
    );
  assert.equal(response.type, "render-success");
  assert.equal(response.type === "render-success" ? response.svg.length : 0, svg.length);
});

test("protocol budgets reject one byte beyond each public limit", () => {
  assert.throws(
    () =>
      validateCompareRenderRequest(
        renderRequest({
          source: "s".repeat(REALM_BUDGETS.sourceBytes + 1),
        }),
        IDENTITY,
        1
      ),
    RealmProtocolError
  );
  assert.throws(
    () =>
      validateCompareRenderRequest(
        renderRequest({
          configJson: "c".repeat(REALM_BUDGETS.configBytes + 1),
        }),
        IDENTITY,
        1
      ),
    RealmProtocolError
  );
  assert.throws(
    () =>
      validateCompareRenderResponse(
        renderResponse({ svg: "s".repeat(REALM_BUDGETS.svgBytes + 1) }),
        IDENTITY,
        1,
        "request-1"
      ),
    RealmProtocolError
  );
  assert.throws(
    () => assertEncodedMessageBudget("m".repeat(REALM_BUDGETS.messageBytes + 1)),
    RealmProtocolError
  );
});

test("compare input validates the controlled browser screen width", () => {
  assert.equal(
    validateCompareRenderRequest(renderRequest(), IDENTITY, 1).payload
      .screenAvailableWidth,
    1512,
  );
  for (const screenAvailableWidth of [0, -1, Number.NaN, 16_385]) {
    const request = renderRequest() as ReturnType<typeof renderRequest>;
    request.payload.screenAvailableWidth = screenAvailableWidth;
    assert.throws(
      () => validateCompareRenderRequest(request, IDENTITY, 1),
      RealmProtocolError,
    );
  }
});

test("realm initialization reserves a separate verified-engine budget", () => {
  const generatedEngine = "e".repeat(REALM_BUDGETS.messageBytes + 1);
  assert.throws(
    () => assertEncodedMessageBudget(generatedEngine),
    RealmProtocolError
  );
  assert.doesNotThrow(() => assertRealmInitBudget(generatedEngine));
  assert.throws(
    () => assertRealmInitBudget("e".repeat(REALM_BUDGETS.realmInitBytes + 1)),
    (error: unknown) =>
      error instanceof RealmProtocolError &&
      "resource" in error &&
      error.resource === "engineArtifact"
  );
});

test("request validation rejects replayed or foreign envelopes", () => {
  for (const invalid of [
    { ...renderRequest(), protocol: REALM_PROTOCOL_VERSION + 1 },
    { ...renderRequest(), realmToken: "foreign" },
    { ...renderRequest(), realmId: "foreign" },
    { ...renderRequest(), sequence: 2 },
    { ...renderRequest(), requestId: "" },
  ]) {
    assert.throws(
      () => validateCompareRenderRequest(invalid, IDENTITY, 1),
      RealmProtocolError
    );
  }
});

test("response validation binds sequence and request id", () => {
  for (const invalid of [
    { ...renderResponse(), sequence: 2 },
    { ...renderResponse(), requestId: "request-2" },
    { ...renderResponse(), renderTimeMs: Number.NaN },
    { ...renderResponse(), presentationTimeMs: -1 },
  ]) {
    assert.throws(
      () =>
        validateCompareRenderResponse(
          invalid,
          IDENTITY,
          1,
          "request-1"
        ),
      RealmProtocolError
    );
  }
});

test("failure responses preserve structured engine detail", () => {
  const response = {
    type: "render-failure",
    protocol: REALM_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence: 1,
    requestId: "request-1",
    stage: "render",
    message: "Parse error on line 2",
    detail: JSON.stringify({
      name: "Error",
      hash: { token: "INVALID", loc: { first_line: 2 } },
    }),
  };

  assert.deepEqual(
    validateCompareRenderResponse(response, IDENTITY, 1, "request-1"),
    response
  );
  assert.throws(
    () =>
      validateCompareRenderResponse(
        { ...response, detail: { token: "INVALID" } },
        IDENTITY,
        1,
        "request-1"
      ),
    RealmProtocolError
  );
});

test("realm tokens use browser entropy and do not repeat", () => {
  const first = createRealmToken();
  const second = createRealmToken();
  assert.match(first, /^[A-Za-z0-9_-]{43}$/);
  assert.notEqual(first, second);
});

function renderRequest(
  overrides: Partial<{
    source: string;
    configJson: string;
  }> = {}
) {
  return {
    type: "render",
    protocol: REALM_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence: 1,
    requestId: "request-1",
    payload: {
      source: overrides.source ?? "flowchart TD\nA-->B",
      configJson: overrides.configJson ?? "{}",
      theme: "default",
      diagramFont: "trebuchet",
      externalRequirements: { externalDiagrams: [], layoutModules: [] },
      screenAvailableWidth: 1512,
      viewport: { width: 800, height: 600 },
    },
  };
}

function renderResponse(overrides: Partial<{ svg: string }> = {}) {
  return {
    type: "render-success",
    protocol: REALM_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence: 1,
    requestId: "request-1",
    svg: overrides.svg ?? '<svg xmlns="http://www.w3.org/2000/svg" />',
    prepareTimeMs: 1,
    renderTimeMs: 2,
    presentationTimeMs: 3,
    version: "11.16.0",
  };
}
