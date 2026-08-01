import {
  BENCHMARK_BUDGETS,
  RealmProtocolError,
} from "../runtime/realm/channel-protocol.ts";
import type { BenchmarkSampleRole } from "./protocol.ts";

export interface BenchmarkSampleBudget {
  accept(role: BenchmarkSampleRole): void;
}

export function createBenchmarkSampleBudget(): BenchmarkSampleBudget {
  let measured = 0;
  let total = 0;
  let warmups = 0;

  return Object.freeze({
    accept(role: BenchmarkSampleRole) {
      const nextTotal = total + 1;
      const nextWarmups = warmups + (role === "warmup" ? 1 : 0);
      const nextMeasured = measured + (role === "measured" ? 1 : 0);
      if (
        nextWarmups > BENCHMARK_BUDGETS.maxWarmups ||
        nextMeasured > BENCHMARK_BUDGETS.maxIterations ||
        nextTotal > BENCHMARK_BUDGETS.maxRetainedSamples
      ) {
        throw new RealmProtocolError(
          "Benchmark sample count exceeds its protocol budget."
        );
      }
      warmups = nextWarmups;
      measured = nextMeasured;
      total = nextTotal;
    },
  });
}
