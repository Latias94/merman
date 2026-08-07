import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  assertRuntimeOwnerEvidence,
  expectedWebOperationIds,
  expectedWebOutputIds,
} from "./wasm-build/runtime-evidence.mjs";

describe("WASM runtime owner evidence", () => {
  const evidence = {
    runtime_capability_ids: ["analysis", "svg"],
    runtime_output_ids: ["svg"],
  };
  const expectedOperationIds = expectedWebOperationIds(
    evidence.runtime_capability_ids,
  );
  const expectedOutputIds = expectedWebOutputIds(evidence.runtime_capability_ids);

  it("derives outputs from explicit operation relationships", () => {
    assert.deepEqual(expectedOutputIds, ["svg"]);
    assert.deepEqual(expectedWebOutputIds(["ascii"]), ["ascii"]);
    assert.deepEqual(expectedWebOutputIds(["analysis"]), []);
  });

  it("accepts exact runtime capability and output IDs", () => {
    assert.doesNotThrow(() =>
      assertRuntimeOwnerEvidence(
        {
          capability_ids: ["analysis", "svg"],
          operation_ids: expectedOperationIds,
          output_ids: expectedOutputIds,
        },
        evidence,
      ),
    );
  });

  it("rejects extra actual outputs even when they are operation IDs", () => {
    assert.throws(
      () =>
        assertRuntimeOwnerEvidence(
          {
            capability_ids: ["analysis", "svg"],
            operation_ids: expectedOperationIds,
            output_ids: ["ascii", "svg"],
          },
          evidence,
        ),
      /runtime output IDs/i,
    );
  });

  it("rejects missing or reordered expected outputs", () => {
    assert.throws(
      () =>
        assertRuntimeOwnerEvidence(
          {
            capability_ids: ["analysis", "svg"],
            operation_ids: expectedOperationIds,
            output_ids: [],
          },
          evidence,
        ),
      /runtime output IDs/i,
    );
    assert.throws(
      () =>
        assertRuntimeOwnerEvidence(
          {
            capability_ids: ["analysis", "svg"],
            operation_ids: expectedOperationIds,
            output_ids: ["svg", "ascii"],
          },
          {
            ...evidence,
            runtime_output_ids: ["ascii", "svg"],
          },
        ),
      /runtime output IDs/i,
    );
  });

  it("rejects missing or extra runtime operations derived from canonical capabilities", () => {
    for (const operationIds of [
      expectedOperationIds.slice(1),
      [...expectedOperationIds, "unknown-operation"],
    ]) {
      assert.throws(
        () =>
          assertRuntimeOwnerEvidence(
            {
              capability_ids: ["analysis", "svg"],
              operation_ids: operationIds,
              output_ids: expectedOutputIds,
            },
            evidence,
          ),
        /runtime operation IDs/i,
      );
    }
  });
});
