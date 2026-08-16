import assert from "node:assert/strict";
import test from "node:test";

import {
  CANONICAL_BENCHMARK_SCREEN_AVAILABLE_WIDTH,
  createCanonicalBenchmarkPayload,
} from "./input.ts";

test("benchmark payload owns one host-independent render environment", () => {
  const payload = createCanonicalBenchmarkPayload({
    source: "flowchart TD\nA-->B",
    configJson: "{}",
    theme: "default",
    diagramFont: "trebuchet",
    externalRequirements: { externalDiagrams: [], layoutModules: [] },
  });

  assert.equal(CANONICAL_BENCHMARK_SCREEN_AVAILABLE_WIDTH, 800);
  assert.deepEqual(payload, {
    source: "flowchart TD\nA-->B",
    configJson: "{}",
    theme: "default",
    diagramFont: "trebuchet",
    externalRequirements: { externalDiagrams: [], layoutModules: [] },
    screenAvailableWidth: 800,
    viewport: { width: 800, height: 600 },
  });
});
