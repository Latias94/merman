import assert from "node:assert/strict";
import test from "node:test";

import {
  benchmarkIntentPurpose,
  createBenchmarkSamplePlan,
  isBenchmarkAggregationIntent,
} from "./sample-plan.ts";

test("cold plan materializes balanced measured samples and single-use realms", () => {
  const plan = createBenchmarkSamplePlan({
    iterations: 6,
    mode: "realm-cold",
    seed: 42,
  });

  assert.equal(plan.samples.length, 12);
  assert.deepEqual(plan.budget, {
    maxLiveRealms: 1,
    measuredSamples: 12,
    realmCreations: 12,
    setupSamples: 0,
    totalSamples: 12,
    warmupSamples: 0,
  });
  assert.equal(
    plan.blocks.filter((block) => block.order[0] === "merman").length,
    3
  );
  assert.equal(
    plan.blocks.filter((block) => block.order[0] === "mermaid").length,
    3
  );
  assert.ok(plan.samples.every((sample) => sample.kind === "cold-measured"));
  assert.ok(plan.sessions.every((session) => session.sampleIds.length === 1));
});

test("warm plan owns setup, warmup, measured order, reuse, and exact budget", () => {
  const plan = createBenchmarkSamplePlan({
    iterations: 4,
    mode: "warm",
    seed: 7,
    warmups: 3,
  });

  assert.deepEqual(
    plan.samples.map((sample) => benchmarkIntentPurpose(sample)),
    [
      "setup",
      "setup",
      "warmup",
      "warmup",
      "warmup",
      "warmup",
      "warmup",
      "warmup",
      "measured",
      "measured",
      "measured",
      "measured",
      "measured",
      "measured",
      "measured",
      "measured",
    ]
  );
  assert.deepEqual(plan.budget, {
    maxLiveRealms: 2,
    measuredSamples: 8,
    realmCreations: 2,
    setupSamples: 2,
    totalSamples: 16,
    warmupSamples: 6,
  });
  assert.equal(plan.sessions.length, 2);
  assert.ok(
    plan.samples
      .filter(isBenchmarkAggregationIntent)
      .every((sample) => sample.aggregateKey === `${sample.blockIndex}:${sample.engine}`)
  );
});

test("sample plans are deterministic, deeply immutable, and seed-sensitive", () => {
  const options = { iterations: 6, mode: "warm", seed: 42, warmups: 2 } as const;
  const first = createBenchmarkSamplePlan(options);
  const second = createBenchmarkSamplePlan(options);
  const different = createBenchmarkSamplePlan({ ...options, seed: 43 });

  assert.deepEqual(first, second);
  assert.notDeepEqual(first.blocks, different.blocks);
  assert.equal(Object.isFrozen(first), true);
  assert.equal(Object.isFrozen(first.blocks), true);
  assert.equal(Object.isFrozen(first.blocks[0]), true);
  assert.equal(Object.isFrozen(first.blocks[0].order), true);
  assert.equal(Object.isFrozen(first.samples), true);
  assert.equal(Object.isFrozen(first.samples[0]), true);
  assert.equal(Object.isFrozen(first.sessions), true);
  assert.equal(Object.isFrozen(first.sessions[0]), true);
  assert.equal(Object.isFrozen(first.sessions[0].sampleIds), true);
  assert.equal(Object.isFrozen(first.budget), true);
});

test("sample plan rejects illegal options before any work is emitted", () => {
  for (const options of [
    { iterations: 3, mode: "realm-cold", seed: 1 },
    { iterations: 2, mode: "realm-cold", seed: -1 },
    { iterations: 2, mode: "warm", seed: 1, warmups: -1 },
  ] as const) {
    assert.throws(() => createBenchmarkSamplePlan(options));
  }
});
