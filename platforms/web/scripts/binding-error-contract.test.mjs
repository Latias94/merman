import assert from "node:assert/strict";
import test from "node:test";

import {
  isBindingErrorPayload,
  isBindingStatusCodeName,
} from "../dist/public-catalog.js";

test("binding error contract accepts operation statuses and cancellation-only details", () => {
  assert.equal(isBindingStatusCodeName("MERMAN_BUSY"), true);
  assert.equal(isBindingStatusCodeName("MERMAN_CANCELLED"), true);

  const cancellationError = {
    version: 1,
    ok: false,
    code: 12,
    code_name: "MERMAN_CANCELLED",
    kind: "generic",
    capability_id: null,
    details: {
      cancellation: {
        reason: "deadline_exceeded",
        phase: "admission",
      },
    },
    message: "operation cancelled during admission: deadline exceeded",
  };

  assert.equal(isBindingErrorPayload(cancellationError), true);
  assert.equal(
    isBindingErrorPayload({
      ...cancellationError,
      details: {
        cancellation: {
          ...cancellationError.details.cancellation,
          phase: undefined,
        },
      },
    }),
    false,
  );

  assert.equal(
    isBindingErrorPayload({
      ...cancellationError,
      details: {
        diagnostic: {
          code: "parse",
          span: { start: -1, end: 0, kind: "exact" },
          field: null,
          diagram_type: null,
        },
      },
      code_name: "MERMAN_PARSE_ERROR",
    }),
    false,
  );
  assert.equal(
    isBindingErrorPayload({
      ...cancellationError,
      details: {
        cancellation: {
          reason: "requested",
          phase: "admission",
        },
      },
      code_name: "MERMAN_PARSE_ERROR",
    }),
    false,
  );
  assert.equal(
    isBindingErrorPayload({
      ...cancellationError,
      details: {
        cancellation: {
          reason: "not-a-reason",
          phase: "admission",
        },
      },
    }),
    false,
  );
  assert.equal(
    isBindingErrorPayload({
      ...cancellationError,
      code: 5,
    }),
    false,
  );
  assert.equal(
    isBindingErrorPayload({
      ...cancellationError,
      kind: "missing-capability",
      capability_id: "svg-png",
    }),
    false,
  );
  assert.equal(
    isBindingErrorPayload({
      ...cancellationError,
      capability_id: "svg-png",
    }),
    false,
  );
  assert.equal(
    isBindingErrorPayload({
      ...cancellationError,
      details: {
        ...cancellationError.details,
        resource: {
          cause: "ceiling",
          limit_id: "max_svg_bytes",
          phase: "render",
          actual: 2,
          max: 1,
          profile: "constrained",
        },
      },
    }),
    false,
  );

  assert.equal(
    isBindingErrorPayload({
      version: 1,
      ok: false,
      code: 5,
      code_name: "MERMAN_PARSE_ERROR",
      kind: "generic",
      capability_id: null,
      details: {
        future_context: {
          source: "future-schema",
        },
      },
      message: "future additive details remain forward compatible",
    }),
    true,
  );
});
