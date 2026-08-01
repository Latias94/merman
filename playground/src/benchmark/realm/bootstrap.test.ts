import assert from "node:assert/strict";
import test from "node:test";

import { RealmProtocolError } from "../../runtime/realm/channel-protocol.ts";
import { validateBenchmarkEngineModule } from "./bootstrap.ts";

const adapter = Object.freeze({
  async initialize() {
    throw new Error("test adapter is never initialized");
  },
});

test("benchmark engine module contracts follow the verified artifact identity", () => {
  assert.doesNotThrow(() =>
    validateBenchmarkEngineModule({ benchmarkEngineAdapter: adapter }, "benchmark-merman")
  );
  assert.doesNotThrow(() =>
    validateBenchmarkEngineModule(
      {
        benchmarkEngineAdapter: adapter,
        renderWithMermaid() {},
      },
      "mermaid"
    )
  );

  assert.throws(
    () => validateBenchmarkEngineModule({ benchmarkEngineAdapter: adapter }, "mermaid"),
    RealmProtocolError
  );
  assert.throws(
    () =>
      validateBenchmarkEngineModule(
        {
          benchmarkEngineAdapter: adapter,
          renderWithMermaid() {},
        },
        "benchmark-merman"
      ),
    RealmProtocolError
  );
});
