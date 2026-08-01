import { BENCHMARK_BUDGETS } from "../runtime/realm/channel-protocol.ts";
import type { BenchmarkEngine } from "./trace.ts";

export interface BenchmarkScheduleBlock {
  readonly index: number;
  readonly order: readonly [BenchmarkEngine, BenchmarkEngine];
}
export interface BalancedBenchmarkSchedule {
  readonly blocks: readonly BenchmarkScheduleBlock[];
  readonly seed: number;
}

export class BenchmarkScheduleError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BenchmarkScheduleError";
  }
}

const AB = Object.freeze(["merman", "mermaid"] as const);
const BA = Object.freeze(["mermaid", "merman"] as const);

export function createBalancedBenchmarkSchedule(
  iterations: number,
  seed: number
): BalancedBenchmarkSchedule {
  if (
    !Number.isSafeInteger(iterations) ||
    iterations < 2 ||
    iterations > BENCHMARK_BUDGETS.maxIterations ||
    iterations % 2 !== 0
  ) {
    throw new BenchmarkScheduleError(
      `Benchmark iterations must be an even integer from 2 to ${BENCHMARK_BUDGETS.maxIterations}.`
    );
  }
  if (
    !Number.isSafeInteger(seed) ||
    seed < 0 ||
    seed > 0xffff_ffff
  ) {
    throw new BenchmarkScheduleError(
      "Benchmark seed must be an unsigned 32-bit integer."
    );
  }

  const orders: Array<readonly [BenchmarkEngine, BenchmarkEngine]> = [];
  for (let index = 0; index < iterations / 2; index += 1) {
    orders.push(AB, BA);
  }
  shuffleInPlace(orders, createUint32Random(seed));

  return Object.freeze({
    seed,
    blocks: Object.freeze(
      orders.map((order, index) => Object.freeze({ index, order }))
    ),
  });
}

export function createUint32Random(seed: number): () => number {
  let state = (seed ^ 0x9e37_79b9) >>> 0;
  if (state === 0) state = 0x6d2b_79f5;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 0x1_0000_0000;
  };
}

export function shuffleInPlace<T>(values: T[], random: () => number): void {
  for (let index = values.length - 1; index > 0; index -= 1) {
    const target = Math.floor(random() * (index + 1));
    [values[index], values[target]] = [values[target]!, values[index]!];
  }
}
