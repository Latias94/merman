import assert from "node:assert/strict";
import test from "node:test";

import {
  BenchmarkScheduleError,
  createBalancedBenchmarkSchedule,
} from "./schedule.ts";

test("balanced schedule is deterministic and contains equal AB and BA blocks", () => {
  const first = createBalancedBenchmarkSchedule(12, 0xdecafbad);
  const second = createBalancedBenchmarkSchedule(12, 0xdecafbad);

  assert.deepEqual(first, second);
  assert.equal(first.blocks.length, 12);
  assert.equal(
    first.blocks.filter((block) => block.order.join(",") === "merman,mermaid")
      .length,
    6
  );
  assert.equal(
    first.blocks.filter((block) => block.order.join(",") === "mermaid,merman")
      .length,
    6
  );
  assert.deepEqual(
    first.blocks.map((block) => block.index),
    Array.from({ length: 12 }, (_, index) => index)
  );
});
test("different seeds alter order without changing balance", () => {
  const first = createBalancedBenchmarkSchedule(20, 1);
  const second = createBalancedBenchmarkSchedule(20, 2);

  assert.notDeepEqual(
    first.blocks.map((block) => block.order),
    second.blocks.map((block) => block.order)
  );
  for (const schedule of [first, second]) {
    const mermanFirst = schedule.blocks.filter(
      (block) => block.order[0] === "merman"
    ).length;
    assert.equal(mermanFirst, schedule.blocks.length / 2);
  }
});

test("schedule rejects unbalanced counts and invalid seeds", () => {
  for (const [iterations, seed] of [
    [0, 1],
    [3, 1],
    [1002, 1],
    [2, -1],
    [2, 2 ** 32],
    [2, 1.5],
  ] as const) {
    assert.throws(
      () => createBalancedBenchmarkSchedule(iterations, seed),
      BenchmarkScheduleError
    );
  }
});
