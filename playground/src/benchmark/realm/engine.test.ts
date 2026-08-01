import assert from "node:assert/strict";
import test from "node:test";

import {
  BenchmarkEngineError,
  runObservedBenchmarkEngineStage,
} from "./engine.ts";

test("observed engine stage runs its end mark once and preserves mark failure", async () => {
  const timeout = new Error("stage timeout");
  let marks = 0;

  await assert.rejects(
    runObservedBenchmarkEngineStage(
      "engine-import",
      async () => "loaded",
      () => {
        marks += 1;
        throw timeout;
      }
    ),
    (error: unknown) => error === timeout
  );
  assert.equal(marks, 1);
});

test("observed engine stage wraps operation failure before its single end mark", async () => {
  const cause = new Error("import failed");
  let marks = 0;

  await assert.rejects(
    runObservedBenchmarkEngineStage(
      "engine-import",
      async () => {
        throw cause;
      },
      () => {
        marks += 1;
      }
    ),
    (error: unknown) =>
      error instanceof BenchmarkEngineError &&
      error.stage === "engine-import" &&
      error.cause === cause
  );
  assert.equal(marks, 1);
});
