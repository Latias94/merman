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
  jsonStringFieldBytes,
  parseTransportJsonText,
  snapshotTransportJsonValue,
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
const ASCII_EXPECTATION = EXPECTATION_BY_ID.get("ascii");
const REQUIRE_UNAVAILABLE = { requireUnavailable: true };

test("direct JSON string accounting preserves scalar validation and all escape widths", () => {
  const text = String.fromCharCode(...Array.from({ length: 128 }, (_, index) => index))
    + "é北京🧪\u2028\u2029\ufffd";
  const bytes = Buffer.byteLength(text);
  assert.equal(jsonStringFieldBytes(text, "source", bytes), Buffer.byteLength(JSON.stringify(text)) - 2);
  assert.throws(() => jsonStringFieldBytes(text, "source", bytes - 1), /field limit/i);
  for (const surrogate of ["\ud800", "\udc00"]) {
    const invalid = text + surrogate;
    assert.throws(() => jsonStringFieldBytes(invalid, "source", bytes + 3), /isolated UTF-16 surrogate/i);
    assert.throws(() => jsonStringFieldBytes(invalid, "source", bytes + 2), /field limit/i);
  }
});

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

  for (const malformed of ['"\ud800é"', '"é\udc00"']) {
    const exactMalformed = malformed + " ".repeat(limits.max_utf8_bytes - Buffer.byteLength(malformed));
    assert.throws(
      () => parseTransportJsonText(exactMalformed, "boundary", limits),
      /isolated UTF-16 surrogate/i,
    );
    assert.throws(
      () => parseTransportJsonText(`${exactMalformed} `, "boundary", limits),
      /wire limit/i,
    );
  }

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
    escaped: '"\\\u0000\b\t\n\f\r\u001f',
    unicode: "é😀\u2028\u2029",
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

test("bounded JSON encoding checks wire limits without parsing its controlled output", (t) => {
  const value = { values: [null, true, false, -1.25e3, "é😀"] };
  const expected = JSON.stringify(value);
  const parse = t.mock.method(JSON, "parse", () => {
    throw new Error("Encoding must not materialize the JSON output.");
  });

  assert.equal(
    encodeTransportJson(value, "boundary", NODE_TRANSPORT_LIMITS.binding_options),
    expected,
  );
  assert.equal(parse.mock.callCount(), 0);
});

test("bounded JSON encoding retains exact structural and string limits", () => {
  const value = { a: "é😀", b: [null] };
  const expected = JSON.stringify(value);
  const limits = {
    ...NODE_TRANSPORT_LIMITS.binding_options,
    max_depth: 2,
    max_members: 3,
    max_tokens: 6,
    max_string_utf8_bytes: 6,
  };
  assert.equal(encodeTransportJson(value, "boundary", limits), expected);

  for (const [field, message] of [
    ["max_depth", /structural depth limit/i],
    ["max_members", /member-work limit/i],
    ["max_tokens", /token-work limit/i],
    ["max_string_utf8_bytes", /string exceeding.*field limit/i],
  ]) {
    assert.throws(
      () => encodeTransportJson(value, "boundary", {
        ...limits,
        [field]: limits[field] - 1,
      }),
      (error) => {
        assert.ok(error instanceof MermanInvalidTransportError);
        assert.match(error.cause?.message ?? error.message, message);
        return true;
      },
      field,
    );
  }
});

test("bounded JSON snapshots reject member, token, and string work before stringify", (context) => {
  const stringify = context.mock.method(JSON, "stringify");
  for (const [value, overrides, message] of [
    [[null, null], { max_members: 1 }, /member-work limit/i],
    [{ a: null }, { max_tokens: 2 }, /token-work limit/i],
    [{ a: "é😀" }, { max_string_utf8_bytes: 5 }, /field limit/i],
  ]) {
    const callsBefore = stringify.mock.callCount();
    assert.throws(
      () => encodeTransportJson(value, "boundary", {
        ...NODE_TRANSPORT_LIMITS.binding_options,
        ...overrides,
      }),
      (error) => {
        assert.ok(error instanceof MermanInvalidTransportError);
        assert.match(error.cause?.message ?? error.message, message);
        return true;
      },
    );
    assert.equal(stringify.mock.callCount(), callsBefore);
  }
});

test("bounded JSON snapshots give the string field limit priority over isolated surrogates", (context) => {
  const stringify = context.mock.method(JSON, "stringify");
  const limits = { ...NODE_TRANSPORT_LIMITS.binding_options, max_utf8_bytes: 32 };
  for (const value of ["\ud800é", "é\udc00"]) {
    for (const input of [value, { [value]: null }]) {
      for (const [bytes, message] of [
        [5, /isolated UTF-16 surrogate/i],
        [4, /string exceeding the 4-byte field limit/i],
        [1, /string exceeding the 1-byte field limit/i],
      ]) {
        assert.throws(
          () => encodeTransportJson(input, "boundary", { ...limits, max_string_utf8_bytes: bytes }),
          { code: "MERMAN_INVALID_TRANSPORT", message },
        );
      }
    }
  }
  assert.throws(
    () => encodeTransportJson("\u0000é\udc00", "boundary", {
      ...limits,
      max_utf8_bytes: 6,
      max_string_utf8_bytes: 4,
    }),
    { code: "MERMAN_INVALID_TRANSPORT", message: /exceeds the 6-byte wire limit/i },
  );
  assert.equal(stringify.mock.callCount(), 0);
});

test("bounded JSON encoding counts expanded array elements in the member budget", () => {
  const shared = [null, null];
  const value = { first: shared, second: shared };
  const limits = {
    ...NODE_TRANSPORT_LIMITS.binding_options,
    max_members: 6,
    max_tokens: 9,
  };
  assert.equal(encodeTransportJson(value, "boundary", limits), JSON.stringify(value));
  assert.throws(
    () => encodeTransportJson(value, "boundary", { ...limits, max_members: 5 }),
    /member-work limit/i,
  );
  assert.throws(
    () => encodeTransportJson(value, "boundary", { ...limits, max_tokens: 8 }),
    /token-work limit/i,
  );
});

test("binding option snapshots enforce escaped wire bytes before applying product policy", () => {
  const limits = NODE_TRANSPORT_LIMITS.binding_options;
  const value = {
    runtime_policy: "native",
    payload: "\u0000".repeat(Math.floor(limits.max_utf8_bytes / 6)),
  };
  assert.throws(
    () => normalizeBindingOptions(value),
    (error) => {
      assert.ok(error instanceof MermanInvalidTransportError);
      assert.match(error.message, /wire limit/i);
      return true;
    },
  );
  assert.equal(value.runtime_policy, "native");
  assert.throws(
    () => normalizeBindingOptions({ first: undefined, later: NaN }),
    (error) => {
      assert.ok(error instanceof TypeError);
      assert.match(error.message, /bindingOptions\.first is not a JSON wire value/i);
      return true;
    },
  );
});

test("bounded JSON encoding never invokes own or inherited JSON hooks", (context) => {
  let hookCalls = 0;
  const hook = () => {
    hookCalls += 1;
    throw new Error("JSON hooks must not run.");
  };
  for (const value of [
    { toJSON: hook },
    Object.defineProperty({}, "toJSON", { enumerable: true, get: hook }),
    Object.defineProperty([null], "0", { enumerable: true, get: hook }),
  ]) {
    assert.throws(
      () => encodeTransportJson(value, "boundary", NODE_TRANSPORT_LIMITS.binding_options),
      MermanInvalidTransportError,
    );
  }
  for (const prototype of [Object.prototype, Array.prototype]) {
    const descriptor = Object.getOwnPropertyDescriptor(prototype, "toJSON");
    Object.defineProperty(prototype, "toJSON", { configurable: true, get: hook });
    context.after(() => {
      if (descriptor) Object.defineProperty(prototype, "toJSON", descriptor);
      else Reflect.deleteProperty(prototype, "toJSON");
    });
  }
  assert.equal(
    encodeTransportJson({ values: [null] }, "boundary", NODE_TRANSPORT_LIMITS.binding_options),
    '{"values":[null]}',
  );
  assert.equal(hookCalls, 0);
});

test("bounded JSON snapshots are detached, prototype-free, and safe to snapshot again", () => {
  const shared = Object.freeze([Object.freeze({ value: "é😀" })]);
  const input = Object.freeze(Object.defineProperty({ first: shared, second: shared }, "__proto__", {
    enumerable: true,
    value: Object.freeze({ polluted: true }),
  }));
  const snapshot = snapshotTransportJsonValue(
    input, "boundary", NODE_TRANSPORT_LIMITS.binding_options,
  );
  assert.equal(Object.getPrototypeOf(snapshot), null);
  assert.equal(Object.getPrototypeOf(snapshot.first), null);
  assert.equal(Object.getPrototypeOf(snapshot.first[0]), null);
  assert.equal(Array.isArray(snapshot.first), true);
  assert.deepEqual(Reflect.ownKeys(snapshot.first), ["0", "length"]);
  assert.notEqual(snapshot.first, input.first);
  assert.notEqual(snapshot.first, snapshot.second);
  assert.equal(Object.hasOwn(snapshot, "__proto__"), true);
  assert.equal(Object.hasOwn(Object.prototype, "polluted"), false);
  assert.equal(
    encodeTransportJson(snapshot, "boundary", NODE_TRANSPORT_LIMITS.binding_options),
    '{"first":[{"value":"é😀"}],"second":[{"value":"é😀"}],"__proto__":{"polluted":true}}',
  );
  snapshot.first[0].value = "changed";
  assert.equal(input.first[0].value, "é😀");
  assert.equal(snapshot.second[0].value, "é😀");
});

test("a mutable snapshot never bypasses ordinary encoder validation", () => {
  let getterCalls = 0;
  for (const change of [
    (snapshot) => { snapshot.value = new Date(); },
    (snapshot) => { snapshot.self = snapshot; },
    (snapshot) => { snapshot.value = "\ud800"; },
    (snapshot) => { snapshot.value = new Array(1); },
    (snapshot) => { snapshot.toJSON = () => { throw new Error("Must not run."); }; },
    (snapshot) => Object.defineProperty(snapshot, "value", {
      enumerable: true,
      get() {
        getterCalls += 1;
        return null;
      },
    }),
  ]) {
    const snapshot = snapshotTransportJsonValue(
      { value: null }, "boundary", NODE_TRANSPORT_LIMITS.binding_options,
    );
    change(snapshot);
    assert.throws(
      () => encodeTransportJson(snapshot, "boundary", NODE_TRANSPORT_LIMITS.binding_options),
      MermanInvalidTransportError,
    );
  }
  assert.equal(getterCalls, 0);
});

test("ordinary JSON encoding rejects all non-JSON external values without coercion", () => {
  const limits = NODE_TRANSPORT_LIMITS.binding_options;
  const cycle = {};
  cycle.self = cycle;
  const customArray = [null];
  customArray.custom = true;
  const symbolKey = { [Symbol("key")]: null };
  const hidden = Object.defineProperty({}, "hidden", { value: null });
  for (const value of [
    undefined, () => {}, 1n, Symbol("value"), NaN, Infinity, -Infinity,
    new Date(), new Map(), new Set(), /pattern/, new Uint8Array(1),
    cycle, new Array(1), customArray, symbolKey, hidden,
    "\ud800", "\udc00", { "\ud800": null }, [undefined],
  ]) {
    assert.throws(
      () => encodeTransportJson(value, "external", limits),
      MermanInvalidTransportError,
    );
  }
});

test("external JSON text still requires native syntax validation", () => {
  for (const text of ['{"value":true,}', '"\\q"', "true false"]) {
    assert.throws(
      () => parseTransportJsonText(text, "boundary", NODE_TRANSPORT_LIMITS.binding_options),
      /invalid boundary JSON/i,
    );
  }
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

test("ASCII schema 4 metadata preserves unmeasured and empty primary geometry", () => {
  const unmeasured = {
    kind: "ascii",
    schema_version: 4,
    family: "flowchart",
    projection: "structured_text",
    encoding: "plain",
    primary_width: null,
    primary_height: null,
    emitted_width: 9,
    emitted_height: 1,
    width_profile: "unicode",
    layout_profile: null,
    requested_layout_profile: "canonical",
    requested_graph_presentation: "adaptive",
    compact_attempted: false,
    requested_max_width: null,
    primary_overflowed: null,
    emitted_overflowed: false,
    outcome: "fallback",
    fallback_capability: "available",
    fallback_attempted: true,
    fallback_reason: "port_capacity_exceeded",
    trimmed: false,
    lossiness: "presentation_only",
  };
  const empty = {
    ...unmeasured,
    projection: "diagrammatic",
    primary_width: 0,
    primary_height: 0,
    emitted_width: 0,
    emitted_height: 0,
    layout_profile: "canonical",
    requested_graph_presentation: "strict",
    primary_overflowed: false,
    outcome: "empty",
    fallback_attempted: false,
    fallback_reason: null,
    lossiness: "none",
  };
  for (const [plan, text] of [[unmeasured, "N1: Alpha"], [empty, ""]]) {
    const wire = successEnvelope(ASCII_EXPECTATION, text, { metadata: { output_plan: plan } });
    const result = decodeWireResponse(wire, ASCII_EXPECTATION);
    assert.equal(result.data, text);
    assert.equal(result.metadata_json, JSON.parse(wire).result.metadata_json);
    const metadata = JSON.parse(result.metadata_json);
    assert.equal(metadata.version, 1);
    assert.deepEqual(metadata.output_plan, plan);
    assert.equal("overflowed" in metadata.output_plan, false);
  }
});

test("data-only responses require operation identity and omit metadata", () => {
  const envelope = JSON.parse(successEnvelope(SVG_EXPECTATION, "<svg>你好</svg>"));
  delete envelope.result.metadata_json;
  const dataOnly = { includeMetadata: false };
  assert.equal(
    decodeWireResponse(JSON.stringify(envelope), SVG_EXPECTATION, dataOnly),
    "<svg>你好</svg>",
  );
  assert.throws(
    () => decodeWireResponse(JSON.stringify(envelope), SVG_EXPECTATION),
    MermanInvalidTransportError,
  );
  assert.throws(
    () => decodeWireResponse(successEnvelope(), SVG_EXPECTATION, dataOnly),
    /metadata for a data-only operation/i,
  );
  envelope.result.operation_id = "semantic-json";
  assert.throws(
    () => decodeWireResponse(JSON.stringify(envelope), SVG_EXPECTATION, dataOnly),
    /identity/i,
  );
  assert.throws(
    () => decodeWireResponse(errorEnvelope(), PNG_EXPECTATION, {
      ...dataOnly,
      requireUnavailable: true,
    }),
    MermanOperationError,
  );
});

test("data-only and metadata responses preserve literal and escaped Unicode", () => {
  const data = 'é汉😀e\u0301א\u0000"\\\n';
  for (const includeMetadata of [false, true]) {
    const wire = successEnvelope(SVG_EXPECTATION, data, {
      result: includeMetadata ? {} : { metadata_json: undefined },
    });
    const escapedWire = wire.replace(/[\u007f-\uffff]/g, (unit) =>
      `\\u${unit.charCodeAt(0).toString(16).padStart(4, "0")}`,
    );
    for (const text of [wire, escapedWire]) {
      const result = decodeWireResponse(text, SVG_EXPECTATION, { includeMetadata });
      assert.equal(includeMetadata ? result.data : result, data);
      if (includeMetadata) {
        assert.equal(JSON.parse(result.metadata_json).byte_length, Buffer.byteLength(data));
      }
    }
  }
  assert.throws(
    () => decodeWireResponse(successEnvelope(SVG_EXPECTATION, data, {
      metadata: { byte_length: data.length },
    }), SVG_EXPECTATION),
    /operation metadata does not match/i,
  );
});

test("data-only decoding handles duplicate, nested and escaped keys without selecting token facts", () => {
  const prefix = '{"version":1,"ok":true,"result":{' +
    '"operation_id":"svg","media_type":"image/svg+xml",';
  for (const [members, expected] of [
    ['"data":"first","future":{"data":"nested"},"d\\u0061ta":"\\u00e9\\ud83d\\ude00"', "é😀"],
    ['"d\\u0061ta":"first","future":[{"data":"nested"}],"data":"last"', "last"],
    ['"data":null,"data":17,"d\\u0061ta":"last"', "last"],
  ]) {
    assert.equal(
      decodeWireResponse(`${prefix}${members}}}`, SVG_EXPECTATION, { includeMetadata: false }),
      expected,
    );
  }
  assert.throws(
    () => decodeWireResponse(`${prefix}"data":"first","data":null}}`, SVG_EXPECTATION, {
      includeMetadata: false,
    }),
    /invalid result envelope/i,
  );
});

test("data-only responses reject invalid Unicode in every string token position", () => {
  const prefix = '{"version":1,"ok":true,"result":{' +
    '"operation_id":"svg","media_type":"image/svg+xml",';
  for (const members of [
    '"data":"\\ud800","data":"ok"',
    '"data":"ok","d\\u0061ta":"\\udc00"',
    '"data":"\\ud800\\u0041"',
    '"data":"ok","future":{"data":"\\ud800"}',
    '"data":"ok","future":["\\udc00"]',
    '"data":"ok","\\ud800":0',
  ]) {
    assert.throws(
      () => decodeWireResponse(`${prefix}${members}}}`, SVG_EXPECTATION, {
        includeMetadata: false,
      }),
      { code: "MERMAN_INVALID_TRANSPORT", message: /isolated JSON surrogate escape/i },
    );
  }
  for (const unit of [0xd800, 0xdc00]) {
    assert.throws(
      () => decodeWireResponse(`${prefix}"data":"${String.fromCharCode(unit)}"}}`, SVG_EXPECTATION, {
        includeMetadata: false,
      }),
      { code: "MERMAN_INVALID_TRANSPORT", message: /isolated UTF-16 surrogate/i },
    );
  }
});

test("data-only responses preserve invalid-result, identity and metadata error order", () => {
  for (const [result, message] of [
    [{ data: null, operation_id: "semantic-json" }, /invalid result envelope/i],
    [{ operation_id: "x".repeat(NODE_TRANSPORT_FIELD_LIMITS.operation_id_utf8_bytes + 1) },
      /response operation_id exceeds.*field limit/i],
    [{ media_type: "x".repeat(NODE_TRANSPORT_FIELD_LIMITS.media_type_utf8_bytes + 1) },
      /response media_type exceeds.*field limit/i],
    [{ operation_id: "semantic-json" }, /result identity/i],
    [{ metadata_json: 0 }, /metadata for a data-only operation/i],
  ]) {
    assert.throws(
      () => decodeWireResponse(successEnvelope(SVG_EXPECTATION, "ok", { result }), SVG_EXPECTATION, {
        includeMetadata: false,
      }),
      { code: "MERMAN_INVALID_TRANSPORT", message },
    );
  }
  const inexactVersion = successEnvelope(SVG_EXPECTATION, "ok", {
    result: { operation_id: "semantic-json" },
  }).replace('"version":1,', '"version":1.0000000000000001,');
  assert.throws(
    () => decodeWireResponse(inexactVersion, SVG_EXPECTATION, { includeMetadata: false }),
    /exact JSON-safe integers/i,
  );
});

test("inherited data retains exact validation before identity and metadata checks", { concurrency: false }, () => {
  const descriptor = Object.getOwnPropertyDescriptor(Object.prototype, "data");
  const ownWire = successEnvelope(SVG_EXPECTATION, "own", {
    result: { metadata_json: undefined },
  });
  for (const [data, message] of [
    ["é😀", null],
    [String.fromCharCode(0xd800), /response data contains an isolated UTF-16 surrogate/i],
    [String.fromCharCode(0xdc00), /response data contains an isolated UTF-16 surrogate/i],
    ["x".repeat(NODE_TRANSPORT_FIELD_LIMITS.data_utf8_bytes + 1),
      /response data exceeds.*field limit/i],
  ]) {
    for (const includeMetadata of [false, true]) {
      const result = { data: undefined };
      if (message) result.operation_id = "semantic-json";
      else if (!includeMetadata) result.metadata_json = undefined;
      const wire = successEnvelope(SVG_EXPECTATION, data, { result });
      try {
        Object.defineProperty(Object.prototype, "data", {
          configurable: true, writable: true, value: data,
        });
        if (message) {
          assert.throws(
            () => decodeWireResponse(wire, SVG_EXPECTATION, { includeMetadata }),
            { code: "MERMAN_INVALID_TRANSPORT", message },
          );
        } else {
          const decoded = decodeWireResponse(wire, SVG_EXPECTATION, { includeMetadata });
          assert.equal(includeMetadata ? decoded.data : decoded, data);
        }
        assert.equal(decodeWireResponse(ownWire, SVG_EXPECTATION, { includeMetadata: false }), "own");
      } finally {
        if (descriptor) Object.defineProperty(Object.prototype, "data", descriptor);
        else Reflect.deleteProperty(Object.prototype, "data");
      }
    }
  }
});

test("inherited result objects cannot bypass data Unicode or field validation", { concurrency: false }, () => {
  const descriptor = Object.getOwnPropertyDescriptor(Object.prototype, "result");
  for (const [data, message] of [
    ["é😀", null],
    [String.fromCharCode(0xd800), /response data contains an isolated UTF-16 surrogate/i],
    ["x".repeat(NODE_TRANSPORT_FIELD_LIMITS.data_utf8_bytes + 1),
      /response data exceeds.*field limit/i],
  ]) {
    const result = JSON.parse(successEnvelope(SVG_EXPECTATION, data)).result;
    delete result.metadata_json;
    if (message) result.operation_id = "semantic-json";
    try {
      Object.defineProperty(Object.prototype, "result", {
        configurable: true, writable: true, value: result,
      });
      const decode = () => decodeWireResponse('{"version":1,"ok":true}', SVG_EXPECTATION, {
        includeMetadata: false,
      });
      if (message) assert.throws(decode, { code: "MERMAN_INVALID_TRANSPORT", message });
      else assert.equal(decode(), data);
    } finally {
      if (descriptor) Object.defineProperty(Object.prototype, "result", descriptor);
      else Reflect.deleteProperty(Object.prototype, "result");
    }
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

  const diagnostic = {
    code: "merman.ascii.width_overflow",
    span: null,
    field: null,
    diagram_type: "flowchart-v2",
    requested_max_width: 10,
    actual_width: 42,
    width_profile: "unicode",
    fallback_reason: null,
  };
  const diagnosticError = {
    code: 6,
    codeName: "MERMAN_RENDER_ERROR",
    kind: "generic",
    capabilityId: null,
    details: { diagnostic },
    message: "ASCII output exceeds the requested width",
  };
  assert.throws(
    () => decodeWireResponse(errorEnvelope(diagnosticError), SVG_EXPECTATION),
    (error) => {
      assert.ok(error instanceof MermanOperationError);
      assert.deepEqual(error.diagnosticDetails, diagnostic);
      return true;
    },
  );
  assert.throws(
    () =>
      decodeWireResponse(
        errorEnvelope({
          ...diagnosticError,
          details: {
            diagnostic: {
              code: diagnostic.code,
              span: diagnostic.span,
              field: diagnostic.field,
              diagram_type: diagnostic.diagram_type,
            },
          },
        }),
        SVG_EXPECTATION,
      ),
    (error) => {
      assert.ok(error instanceof MermanOperationError);
      assert.deepEqual(error.diagnosticDetails, {
        code: diagnostic.code,
        span: diagnostic.span,
        field: diagnostic.field,
        diagram_type: diagnostic.diagram_type,
      });
      return true;
    },
  );
  for (const [invalidDiagnostic, expectedMessage] of [
    [{ ...diagnostic, requested_max_width: -1 }, /invalid diagnostic error details/i],
    [{ ...diagnostic, actual_width: 42.5 }, /exact JSON-safe integers/i],
    [{ ...diagnostic, width_profile: 7 }, /invalid diagnostic error details/i],
    [{ ...diagnostic, fallback_reason: false }, /invalid diagnostic error details/i],
    [
      { ...diagnostic, requested_max_width: "9007199254740992" },
      /invalid diagnostic error details/i,
    ],
  ]) {
    assert.throws(
      () =>
        decodeWireResponse(
          errorEnvelope({
            ...diagnosticError,
            details: { diagnostic: invalidDiagnostic },
          }),
          SVG_EXPECTATION,
        ),
      expectedMessage,
    );
  }
  const inexactDiagnosticEnvelope = errorEnvelope(diagnosticError).replace(
    '"requested_max_width":10',
    '"requested_max_width":10.0000000000000001',
  );
  assert.throws(
    () => decodeWireResponse(inexactDiagnosticEnvelope, SVG_EXPECTATION),
    /exact JSON-safe integers/i,
  );

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
  const limit = NODE_TRANSPORT_FIELD_LIMITS.data_utf8_bytes;
  const exactData = "é😀".repeat(Math.floor(limit / 6)) + "x".repeat(limit % 6);
  assert.equal(Buffer.byteLength(exactData), limit);
  for (const includeMetadata of [false, true]) {
    const result = includeMetadata ? {} : { metadata_json: undefined };
    const decoded = decodeWireResponse(
      successEnvelope(SVG_EXPECTATION, exactData, { result }),
      SVG_EXPECTATION,
      { includeMetadata },
    );
    assert.equal(includeMetadata ? decoded.data : decoded, exactData);
    assert.throws(
      () => decodeWireResponse(
        successEnvelope(SVG_EXPECTATION, `${exactData}x`, { result }),
        SVG_EXPECTATION,
        { includeMetadata },
      ),
      /response contains a string exceeding.*field limit/i,
    );
  }
  const oversizedToken = JSON.stringify(`${exactData}x`);
  for (const members of [
    `"data":${oversizedToken},"data":"ok"`,
    `"data":"ok","future":[{"data":${oversizedToken}}]`,
  ]) {
    assert.throws(
      () => decodeWireResponse(
        '{"version":1,"ok":true,"result":{' +
          `"operation_id":"svg","media_type":"image/svg+xml",${members}}}`,
        SVG_EXPECTATION,
        { includeMetadata: false },
      ),
      /response contains a string exceeding.*field limit/i,
    );
  }
});

test("data-only and metadata responses retain exact and plus-one wire limits", () => {
  const limits = NODE_TRANSPORT_LIMITS.response;
  const data = "é😀";
  for (const includeMetadata of [false, true]) {
    const future = ["x".repeat(limits.max_string_utf8_bytes), ""];
    const result = includeMetadata ? { future } : { future, metadata_json: undefined };
    const baseWire = successEnvelope(SVG_EXPECTATION, data, { result });
    future[1] = "x".repeat(limits.max_utf8_bytes - Buffer.byteLength(baseWire));
    const exactWire = successEnvelope(SVG_EXPECTATION, data, { result });
    assert.equal(Buffer.byteLength(exactWire), limits.max_utf8_bytes);
    const decoded = decodeWireResponse(exactWire, SVG_EXPECTATION, { includeMetadata });
    assert.equal(includeMetadata ? decoded.data : decoded, data);
    assert.throws(
      () => decodeWireResponse(`${exactWire} `, SVG_EXPECTATION, { includeMetadata }),
      /response exceeds.*wire limit/i,
    );
  }
});

test("binding option normalization rejects non-JSON host values without invoking accessors", () => {
  const cycle = {};
  cycle.self = cycle;
  assert.throws(
    () => normalizeBindingOptions(cycle),
    { name: "TypeError", message: /cyclic references/i },
  );

  let exactDepth = {};
  let cursor = exactDepth;
  for (let depth = 1; depth < NODE_TRANSPORT_LIMITS.binding_options.max_depth; depth += 1) {
    cursor.next = {};
    cursor = cursor.next;
  }
  assert.doesNotThrow(() => normalizeBindingOptions(exactDepth));
  cursor.next = {};
  assert.throws(
    () => normalizeBindingOptions(exactDepth),
    { name: "RangeError", message: /structural depth limit/i },
  );

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
    { name: "MermanInvalidTransportError", message: /member-work limit/i },
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

test("SVG theme outcome survives the generic result metadata projection", () => {
  const themeOutcome = {
    mode: "merman_native", preset_id: "studio", appearance: "dark", notices: [],
  };
  const result = decodeWireResponse(
    successEnvelope(SVG_EXPECTATION, "<svg />", { metadata: { theme_outcome: themeOutcome } }),
    SVG_EXPECTATION,
  );
  assert.deepEqual(JSON.parse(result.metadata_json).theme_outcome, themeOutcome);
});

test("additive operation error details survive the public error projection", () => {
  const details = { future_details: { retained: true } };
  assert.throws(
    () => decodeWireResponse(errorEnvelope({
      code: 1,
      codeName: "MERMAN_INVALID_ARGUMENT",
      kind: "generic",
      capabilityId: null,
      message: "invalid resolve-theme request",
      details,
    }), EXPECTATION_BY_ID.get("resolve-theme")),
    (error) => {
      assert.ok(error instanceof MermanOperationError);
      assert.deepEqual(error.details, details);
      return true;
    },
  );
});
