import assert from "node:assert/strict";
import test from "node:test";

import { BENCHMARK_BUDGETS } from "../runtime/realm/channel-protocol.ts";
import { createBenchmarkSampleBudget } from "./sample-budget.ts";

test("sample budget accepts exact warmup and iteration limits", () => {
  const budget = createBenchmarkSampleBudget();
  for (let index = 0; index < BENCHMARK_BUDGETS.maxWarmups; index += 1) {
    budget.accept("warmup");
  }
  for (let index = 0; index < BENCHMARK_BUDGETS.maxIterations; index += 1) {
    budget.accept("measured");
  }
});

test("sample budget rejects one warmup beyond the limit", () => {
  const budget = createBenchmarkSampleBudget();
  for (let index = 0; index < BENCHMARK_BUDGETS.maxWarmups; index += 1) {
    budget.accept("warmup");
  }
  assert.throws(() => budget.accept("warmup"), /protocol budget/);
});

test("sample budget rejects one measured sample beyond the limit", () => {
  const budget = createBenchmarkSampleBudget();
  for (let index = 0; index < BENCHMARK_BUDGETS.maxIterations; index += 1) {
    budget.accept("measured");
  }
  assert.throws(() => budget.accept("measured"), /protocol budget/);
});
