import assert from "node:assert/strict";
import test from "node:test";

import { normalizeBindingOptions } from "../src/engine.mjs";
import {
  MermanInvalidTransportError,
  MermanOperationError,
  NODE_TRANSPORT_FIELD_LIMITS,
  NODE_TRANSPORT_LIMITS,
  assertUtf8Field,
  decodeWireResponse,
  encodeTransportJson,
  parseTransportJsonText,
} from "../src/errors.mjs";
import { BINDING_OPERATION_EXPECTATIONS } from "../src/generated/binding-contract.mjs";

const EXPECTATION_BY_ID = new Map(
  BINDING_OPERATION_EXPECTATIONS.map((expectation) => [
    expectation.operation_id,
    expectation,
  ]),
);
const SVG_EXPECTATION = EXPECTATION_BY_ID.get("svg");
const PNG_EXPECTATION = EXPECTATION_BY_ID.get("png");
const REQUIRE_UNAVAILABLE = { requireUnavailable: true };

function successEnvelope(
  expectation = SVG_EXPECTATION,
  data = "<svg />",
  { envelope = {}, result = {}, metadata = {} } = {},
) {
  return JSON.stringify({
    version: 1,
    ok: true,
    result: {
      operation_id: expectation.operation_id,
      media_type: expectation.media_type,
      data,
      metadata_json: JSON.stringify({
        version: expectation.metadata_schema_version,
        operation_id: expectation.operation_id,
        media_type: expectation.media_type,
        runtime_policy: "deterministic",
        byte_length: Buffer.byteLength(data),
        ...metadata,
      }),
      ...result,
    },
    ...envelope,
  });
}

function errorEnvelope(
  {
    code = 7,
    codeName = "MERMAN_UNSUPPORTED_OPERATION",
    kind = "missing-capability",
    capabilityId = "png",
    message = "operation requires missing capability `png`",
    details,
    future = {},
  } = {},
) {
  return JSON.stringify({
    version: 1,
    ok: false,
    error: {
      code,
      code_name: codeName,
      kind,
      capability_id: capabilityId,
      message,
      ...(details === undefined ? {} : { details }),
      ...future,
    },
  });
}

function nestedArrayJson(depth) {
  return `${"[".repeat(depth)}0${"]".repeat(depth)}`;
}

function repeatedMemberObjectJson(count) {
  return `{${'"a":0,'.repeat(count - 1)}"a":0}`;
}

function repeatedTokenArrayJson(tokenCount) {
  return `[${"0,".repeat(tokenCount - 2)}0]`;
}

test("bounded JSON preparse enforces exact and plus-one document limits", () => {
  const limits = NODE_TRANSPORT_LIMITS.binding_options;

  const exactBytes = "{}" + " ".repeat(limits.max_utf8_bytes - 2);
  assert.deepEqual(parseTransportJsonText(exactBytes, "boundary", limits), {});
  assert.throws(
    () => parseTransportJsonText(`${exactBytes} `, "boundary", limits),
    /wire limit/i,
  );

  assert.doesNotThrow(() =>
    parseTransportJsonText(nestedArrayJson(limits.max_depth), "boundary", limits),
  );
  assert.throws(
    () =>
      parseTransportJsonText(
        nestedArrayJson(limits.max_depth + 1),
        "boundary",
        limits,
      ),
    /structural depth limit/i,
  );

  assert.doesNotThrow(() =>
    parseTransportJsonText(
      repeatedMemberObjectJson(limits.max_members),
      "boundary",
      limits,
    ),
  );
  assert.throws(
    () =>
      parseTransportJsonText(
        repeatedMemberObjectJson(limits.max_members + 1),
        "boundary",
        limits,
      ),
    /member-work limit/i,
  );

  assert.doesNotThrow(() =>
    parseTransportJsonText(
      repeatedTokenArrayJson(limits.max_tokens),
      "boundary",
      limits,
    ),
  );
  assert.throws(
    () =>
      parseTransportJsonText(
        repeatedTokenArrayJson(limits.max_tokens + 1),
        "boundary",
        limits,
      ),
    /token-work limit/i,
  );

  const requestLimits = NODE_TRANSPORT_LIMITS.request;
  assert.doesNotThrow(() =>
    parseTransportJsonText(
      JSON.stringify("x".repeat(requestLimits.max_string_utf8_bytes)),
      "boundary",
      requestLimits,
    ),
  );
  assert.throws(
    () =>
      parseTransportJsonText(
        JSON.stringify("x".repeat(requestLimits.max_string_utf8_bytes + 1)),
        "boundary",
        requestLimits,
      ),
    /string exceeding.*field limit/i,
  );
});

test("bounded JSON encoding rejects shared-value amplification before stringify", () => {
  const limits = NODE_TRANSPORT_LIMITS.binding_options;
  const shared = "x".repeat(Math.floor(limits.max_utf8_bytes * 0.6));
  let stringifyReached = false;

  assert.throws(
    () =>
      encodeTransportJson(
        {
          first: shared,
          second: shared,
          probe: {
            toJSON() {
              stringifyReached = true;
              return null;
            },
          },
        },
        "binding options",
        limits,
      ),
    /wire limit/i,
  );
  assert.equal(stringifyReached, false);
});

test("bounded JSON encoding counts escaped and Unicode output bytes exactly", () => {
  const value = {
    escaped: '"\\\u0000\n',
    unicode: "é😀",
  };
  const expected = JSON.stringify(value);
  const exactBytes = Buffer.byteLength(expected);
  const limits = {
    ...NODE_TRANSPORT_LIMITS.binding_options,
    max_utf8_bytes: exactBytes,
  };

  assert.equal(encodeTransportJson(value, "boundary", limits), expected);
  assert.throws(
    () =>
      encodeTransportJson(value, "boundary", {
        ...limits,
        max_utf8_bytes: exactBytes - 1,
      }),
    /wire limit/i,
  );
});

test("bounded JSON preparse matches Rust finite-number semantics", () => {
  const limits = NODE_TRANSPORT_LIMITS.binding_options;
  assert.equal(
    parseTransportJsonText('{"value":1e308}', "finite number", limits).value,
    1e308,
  );
  assert.throws(
    () => parseTransportJsonText('{"value":1e309}', "non-finite number", limits),
    /finite JSON range/i,
  );
});

test("text-only and field boundaries reject host objects and invalid Unicode", () => {
  assert.throws(
    () => parseTransportJsonText({}, "response", NODE_TRANSPORT_LIMITS.response),
    /must be JSON text/i,
  );
  assert.throws(
    () => decodeWireResponse(JSON.parse(successEnvelope()), SVG_EXPECTATION),
    /must be JSON text/i,
  );
  assert.throws(
    () =>
      parseTransportJsonText(
        `{"value":"${String.fromCharCode(0xd800)}"}`,
        "response",
        NODE_TRANSPORT_LIMITS.response,
      ),
    /isolated UTF-16 surrogate/i,
  );
  assert.throws(
    () =>
      parseTransportJsonText(
        '{"value":"\\ud800"}',
        "response",
        NODE_TRANSPORT_LIMITS.response,
      ),
    /isolated JSON surrogate escape/i,
  );

  const uriLimit = NODE_TRANSPORT_FIELD_LIMITS.uri_utf8_bytes;
  assert.equal(assertUtf8Field("x".repeat(uriLimit), "uri", uriLimit), uriLimit);
  assert.throws(
    () => assertUtf8Field("x".repeat(uriLimit + 1), "uri", uriLimit),
    /field limit/i,
  );
});

test("success envelopes are strict while preserving unknown future fields", () => {
  const valid = decodeWireResponse(
    successEnvelope(SVG_EXPECTATION, "<svg />", {
      envelope: { future_envelope: true },
      result: { future_result: { preserved: true } },
      metadata: { future_metadata: [1, 2, 3] },
    }),
    SVG_EXPECTATION,
  );
  assert.equal(valid.future_result.preserved, true);

  const malformed = [
    { ...JSON.parse(successEnvelope()), error: JSON.parse(errorEnvelope()).error },
    { version: 1, ok: true },
    { version: 1, ok: "true", result: {} },
    { ...JSON.parse(successEnvelope()), version: 99 },
    JSON.parse(successEnvelope(SVG_EXPECTATION, "<svg />", {
      result: { operation_id: "semantic-json" },
    })),
    JSON.parse(successEnvelope(SVG_EXPECTATION, "<svg />", {
      result: { media_type: "application/json" },
    })),
    JSON.parse(successEnvelope(SVG_EXPECTATION, "<svg />", {
      metadata: { version: 99 },
    })),
    JSON.parse(successEnvelope(SVG_EXPECTATION, "<svg />", {
      metadata: { operation_id: "semantic-json" },
    })),
    JSON.parse(successEnvelope(SVG_EXPECTATION, "<svg />", {
      metadata: { media_type: "application/json" },
    })),
    JSON.parse(successEnvelope(SVG_EXPECTATION, "<svg />", {
      metadata: { byte_length: 999 },
    })),
  ];
  for (const envelope of malformed) {
    assert.throws(
      () => decodeWireResponse(JSON.stringify(envelope), SVG_EXPECTATION),
      MermanInvalidTransportError,
    );
  }
});

test("success metadata integer tokens and deterministic policy remain exact", () => {
  const exactMetadata =
    '{"version":1e0,"operation_id":"svg","media_type":"image/svg+xml",' +
    '"runtime_policy":"deterministic","byte_length":1e0}';
  const envelopeWith = (metadataJson) => JSON.stringify({
    version: 1,
    ok: true,
    result: {
      operation_id: "svg",
      media_type: "image/svg+xml",
      data: "x",
      metadata_json: metadataJson,
    },
  });
  assert.equal(
    decodeWireResponse(envelopeWith(exactMetadata), SVG_EXPECTATION).data,
    "x",
  );

  const roundedMetadata = exactMetadata.replace("1e0}", "1.0000000000000001}");
  assert.throws(
    () => decodeWireResponse(envelopeWith(roundedMetadata), SVG_EXPECTATION),
    /exact JSON-safe integers/i,
  );

  for (const runtimePolicy of ["native", "future-policy"]) {
    assert.throws(
      () =>
        decodeWireResponse(
          successEnvelope(SVG_EXPECTATION, "x", {
            metadata: { runtime_policy: runtimePolicy },
          }),
          SVG_EXPECTATION,
        ),
      MermanInvalidTransportError,
    );
  }
});

test("error envelopes enforce discriminants, known relations, and capability identity", () => {
  assert.throws(
    () =>
      decodeWireResponse(
        successEnvelope(PNG_EXPECTATION, "not-a-binary-payload"),
        PNG_EXPECTATION,
        REQUIRE_UNAVAILABLE,
      ),
    /does not advertise/i,
  );

  assert.throws(
    () =>
      decodeWireResponse(
        errorEnvelope({ future: { future_error_metadata: true } }),
        PNG_EXPECTATION,
        REQUIRE_UNAVAILABLE,
      ),
    (error) => {
      assert.ok(error instanceof MermanOperationError);
      assert.equal(error.kind, "missing-capability");
      assert.equal(error.capabilityId, "png");
      return true;
    },
  );

  const malformed = [
    { ...JSON.parse(errorEnvelope()), result: {} },
    { version: 1, ok: false },
    JSON.parse(errorEnvelope({ kind: "unknown-operation", capabilityId: "png" })),
    JSON.parse(errorEnvelope({ capabilityId: "jpeg" })),
    JSON.parse(errorEnvelope({ kind: "future-error" })),
    JSON.parse(errorEnvelope({ code: 0 })),
    JSON.parse(errorEnvelope({
      code: 1,
      codeName: "MERMAN_INTERNAL_ERROR",
      kind: "generic",
      capabilityId: null,
    })),
    JSON.parse(errorEnvelope({
      code: 1,
      codeName: "MERMAN_INVALID_ARGUMENT",
      kind: "reentrant-call",
      capabilityId: "svg",
    })),
    JSON.parse(errorEnvelope({
      code: 5,
      codeName: "MERMAN_PARSE_ERROR",
      kind: "generic",
      capabilityId: null,
    })),
  ];
  for (const envelope of malformed) {
    assert.throws(
      () =>
        decodeWireResponse(
          JSON.stringify(envelope),
          PNG_EXPECTATION,
          REQUIRE_UNAVAILABLE,
        ),
      MermanInvalidTransportError,
    );
  }

  assert.throws(
    () =>
      decodeWireResponse(
        errorEnvelope({
          code: 1,
          codeName: "MERMAN_INVALID_ARGUMENT",
          kind: "reentrant-call",
          capabilityId: null,
        }),
        SVG_EXPECTATION,
      ),
    MermanOperationError,
  );

  const iconDetails = {
    icon_registry: {
      kind_id: "resource_limit_exceeded",
      pack_index: 0,
      registration_name: "core",
    },
  };
  assert.throws(
    () =>
      decodeWireResponse(
        errorEnvelope({ details: iconDetails }),
        PNG_EXPECTATION,
        REQUIRE_UNAVAILABLE,
      ),
    MermanOperationError,
  );
  for (const icon_registry of [
    { ...iconDetails.icon_registry, kind_id: "" },
    { ...iconDetails.icon_registry, pack_index: -1 },
    { ...iconDetails.icon_registry, registration_name: 1 },
  ]) {
    assert.throws(
      () =>
        decodeWireResponse(
          errorEnvelope({ details: { icon_registry } }),
          PNG_EXPECTATION,
          REQUIRE_UNAVAILABLE,
        ),
      MermanInvalidTransportError,
    );
  }

  assert.throws(
    () =>
      decodeWireResponse(
        errorEnvelope({
          future: {
            future_error_metadata: "x".repeat(
              NODE_TRANSPORT_LIMITS.error.max_string_utf8_bytes + 1,
            ),
          },
        }),
        PNG_EXPECTATION,
        REQUIRE_UNAVAILABLE,
      ),
    /field limit/i,
  );

  const resourceEnvelope = errorEnvelope({
    details: {
      resource: {
        limit_id: "max_source_bytes",
        phase: "source",
        actual: 5,
        max: 4,
        profile: "interactive",
      },
    },
  }).replace('"actual":5', '"actual":5.0000000000000001');
  assert.throws(
    () => decodeWireResponse(resourceEnvelope, PNG_EXPECTATION, REQUIRE_UNAVAILABLE),
    /exact JSON-safe integers/i,
  );

  const losslessResourceEnvelope = errorEnvelope({
    code: 10,
    codeName: "MERMAN_RESOURCE_LIMIT_EXCEEDED",
    kind: "generic",
    capabilityId: null,
    details: {
      resource: {
        cause: "arithmetic_overflow",
        limit_id: "max_layout_work_units",
        phase: "layout_model",
        actual: "18446744073709551615",
        max: 800_000,
        profile: "interactive",
      },
    },
  });
  assert.throws(
    () => decodeWireResponse(losslessResourceEnvelope, SVG_EXPECTATION),
    (error) => {
      assert.ok(error instanceof MermanOperationError);
      assert.deepEqual(error.resourceDetails, {
        cause: "arithmetic_overflow",
        limit_id: "max_layout_work_units",
        phase: "layout_model",
        actual: "18446744073709551615",
        max: 800_000,
        profile: "interactive",
      });
      return true;
    },
  );

  for (const actual of [
    "5",
    "09007199254740992",
    "18446744073709551616",
    "-9007199254740992",
  ]) {
    assert.throws(
      () =>
        decodeWireResponse(
          errorEnvelope({
            code: 10,
            codeName: "MERMAN_RESOURCE_LIMIT_EXCEEDED",
            kind: "generic",
            capabilityId: null,
            details: {
              resource: {
                cause: "arithmetic_overflow",
                limit_id: "max_layout_work_units",
                phase: "layout_model",
                actual,
                max: 800_000,
                profile: "interactive",
              },
            },
          }),
          SVG_EXPECTATION,
        ),
      /invalid resource error details/i,
    );
  }

  const maxMessage = "x".repeat(NODE_TRANSPORT_FIELD_LIMITS.error_message_utf8_bytes);
  assert.throws(
    () =>
      decodeWireResponse(
        errorEnvelope({ message: maxMessage }),
        PNG_EXPECTATION,
        REQUIRE_UNAVAILABLE,
      ),
    MermanOperationError,
  );
  assert.throws(
    () =>
      decodeWireResponse(
        errorEnvelope({ message: `${maxMessage}x` }),
        PNG_EXPECTATION,
        REQUIRE_UNAVAILABLE,
      ),
    /field limit/i,
  );
});

test("response data field accepts its exact UTF-8 ceiling and rejects plus one", () => {
  const exactData = "x".repeat(NODE_TRANSPORT_FIELD_LIMITS.data_utf8_bytes);
  const result = decodeWireResponse(
    successEnvelope(SVG_EXPECTATION, exactData),
    SVG_EXPECTATION,
  );
  assert.equal(result.data.length, exactData.length);
  assert.throws(
    () =>
      decodeWireResponse(
        successEnvelope(SVG_EXPECTATION, `${exactData}x`),
        SVG_EXPECTATION,
      ),
    /field limit/i,
  );
});

test("binding option normalization rejects non-JSON host values without invoking accessors", () => {
  const cycle = {};
  cycle.self = cycle;
  assert.throws(() => normalizeBindingOptions(cycle), /cyclic references/i);

  let exactDepth = {};
  let cursor = exactDepth;
  for (let depth = 1; depth < NODE_TRANSPORT_LIMITS.binding_options.max_depth; depth += 1) {
    cursor.next = {};
    cursor = cursor.next;
  }
  assert.doesNotThrow(() => normalizeBindingOptions(exactDepth));
  cursor.next = {};
  assert.throws(() => normalizeBindingOptions(exactDepth), /structural depth limit/i);

  for (const value of [new Date(), new Map(), Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.throws(
      () => normalizeBindingOptions({ future_value: value }),
      /plain objects|finite JSON number/i,
    );
  }
  for (const value of [undefined, () => {}, 1n, Symbol("future")]) {
    assert.throws(
      () => normalizeBindingOptions({ future_value: value }),
      /not a JSON wire value/i,
    );
  }

  const sparse = [];
  sparse.length = 1;
  assert.throws(() => normalizeBindingOptions({ sparse }), /must not contain holes/i);

  const oversizedSparse = [];
  oversizedSparse.length = NODE_TRANSPORT_LIMITS.binding_options.max_members;
  assert.throws(
    () => normalizeBindingOptions({ oversizedSparse }),
    /member-work limit/i,
  );

  let getterCalled = false;
  const accessor = {};
  Object.defineProperty(accessor, "value", {
    enumerable: true,
    get() {
      getterCalled = true;
      return 1;
    },
  });
  assert.throws(
    () => normalizeBindingOptions({ accessor }),
    /enumerable data property/i,
  );
  assert.equal(getterCalled, false);

  const accessorArray = [0];
  Object.defineProperty(accessorArray, "0", {
    enumerable: true,
    get() {
      getterCalled = true;
      return 1;
    },
  });
  assert.throws(
    () => normalizeBindingOptions({ accessorArray }),
    /enumerable data property/i,
  );
  assert.equal(getterCalled, false);
});
