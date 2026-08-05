import assert from "node:assert/strict";
import test from "node:test";
import { runInNewContext } from "node:vm";

import { projectError } from "./error-projection.ts";

test("projects binding, cyclic, hostile, and cross-realm failures", () => {
  const binding = projectError({
    version: 1,
    ok: false,
    code: 5,
    code_name: "MERMAN_PARSE_ERROR",
    kind: "parse",
    capability_id: null,
    message: "Expected a diagram statement.",
  });
  assert.equal(binding.summary, "Expected a diagram statement.");
  assert.doesNotMatch(binding.summary, /\[object Object\]/);
  assert.match(binding.detail ?? "", /"code_name": "MERMAN_PARSE_ERROR"/);

  const cyclic: { message: string; self?: unknown } = {
    message: "Structured failure",
  };
  cyclic.self = cyclic;
  const projected = projectError(cyclic);
  assert.equal(projected.summary, "Structured failure");
  assert.match(projected.detail ?? "", /\[circular\]/);

  const opaque = new Proxy(
    {},
    {
      get() {
        throw new Error("unreadable getter");
      },
      ownKeys() {
        throw new Error("unreadable keys");
      },
    }
  );
  assert.doesNotThrow(() => projectError(opaque));
  assert.equal(projectError(opaque).detail, '"[unreadable object]"');

  const hostilePrototype = new Proxy(
    {},
    {
      getPrototypeOf() {
        throw new Error("unreadable prototype");
      },
    }
  );
  assert.deepEqual(projectError(hostilePrototype), {
    summary: "Unexpected error.",
    detail: '"[unreadable error]"',
  });

  const parserError = Object.assign(new Error("Parse error on line 2"), {
    hash: {
      expected: ["NODE_TEXT"],
      loc: { first_column: 4, first_line: 2 },
      token: "INVALID",
    },
  });
  const parserProjection = projectError(parserError);
  assert.equal(parserProjection.summary, "Parse error on line 2");
  assert.match(parserProjection.detail ?? "", /"token": "INVALID"/);

  const crossRealmError = runInNewContext(
    'Object.assign(new Error("Cross-realm Merman failure."), { code: "MERMAN_CROSS_REALM" })'
  );
  const crossRealmProjection = projectError(crossRealmError);
  assert.equal(crossRealmProjection.summary, "Cross-realm Merman failure.");
  assert.match(crossRealmProjection.detail ?? "", /MERMAN_CROSS_REALM/);
});

test("preserves and bounds already-projected payloads", () => {
  const original = {
    summary: "Already projected.",
    detail: '{"stage":"render"}',
  };
  const projected = projectError(original);
  assert.deepEqual(projected, original);
  assert.equal(Object.isFrozen(projected), true);

  const oversized = projectError({
    summary: "s".repeat(9_001),
    detail: "d".repeat(9_001),
  });
  assert.match(oversized.summary, /\[truncated\]$/);
  assert.match(oversized.detail ?? "", /\[truncated\]$/);
  assert.ok((oversized.detail?.length ?? 0) < 9_001);
});
