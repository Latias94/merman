import assert from "node:assert/strict";
import test from "node:test";

import {
  BenchmarkStatisticsError,
  calculateBenchmarkStatistics,
  calculateMedianRatio,
} from "./statistics.ts";

test("statistics report exact descriptive values without a stability verdict", () => {
  assert.deepEqual(calculateBenchmarkStatistics([1, 2, 3, 4, 10]), {
    count: 5,
    median: 3,
    p95: 10,
    min: 1,
    max: 10,
    mean: 4,
    coefficientOfVariation: Math.sqrt(10) / 4,
  });
});

test("statistics handle singleton and all-zero vectors", () => {
  assert.deepEqual(calculateBenchmarkStatistics([7]), {
    count: 1,
    median: 7,
    p95: 7,
    min: 7,
    max: 7,
    mean: 7,
    coefficientOfVariation: 0,
  });
  assert.equal(
    calculateBenchmarkStatistics([0, 0]).coefficientOfVariation,
    0
  );
});

test("statistics reject empty, negative, and non-finite observations", () => {
  for (const values of [[], [-1], [Number.NaN], [Number.POSITIVE_INFINITY]]) {
    assert.throws(
      () => calculateBenchmarkStatistics(values),
      BenchmarkStatisticsError
    );
  }
});

test("median ratio is fail-closed for missing or zero-denominator sets", () => {
  const merman = calculateBenchmarkStatistics([2, 4]);
  const mermaid = calculateBenchmarkStatistics([6, 8]);
  assert.equal(calculateMedianRatio(merman, mermaid), 7 / 3);
  assert.equal(calculateMedianRatio(null, mermaid), null);
  assert.equal(
    calculateMedianRatio(calculateBenchmarkStatistics([0, 0]), mermaid),
    null
  );
});
