export interface BenchmarkStatistics {
  readonly coefficientOfVariation: number;
  readonly count: number;
  readonly max: number;
  readonly mean: number;
  readonly median: number;
  readonly min: number;
  readonly p95: number;
}
export class BenchmarkStatisticsError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BenchmarkStatisticsError";
  }
}

export function calculateBenchmarkStatistics(
  values: readonly number[]
): BenchmarkStatistics {
  if (values.length === 0) {
    throw new BenchmarkStatisticsError(
      "Benchmark statistics require at least one observation."
    );
  }
  if (values.some((value) => !Number.isFinite(value) || value < 0)) {
    throw new BenchmarkStatisticsError(
      "Benchmark observations must be finite nonnegative numbers."
    );
  }

  const sorted = [...values].sort((left, right) => left - right);
  const sum = sorted.reduce((total, value) => total + value, 0);
  const mean = sum / sorted.length;
  const middle = Math.floor(sorted.length / 2);
  const median =
    sorted.length % 2 === 0
      ? (sorted[middle - 1] + sorted[middle]) / 2
      : sorted[middle];
  const p95 = sorted[Math.max(0, Math.ceil(sorted.length * 0.95) - 1)];
  const variance =
    sorted.reduce((total, value) => total + (value - mean) ** 2, 0) /
    sorted.length;

  return Object.freeze({
    count: sorted.length,
    median,
    p95,
    min: sorted[0],
    max: sorted[sorted.length - 1],
    mean,
    coefficientOfVariation: mean === 0 ? 0 : Math.sqrt(variance) / mean,
  });
}

export function calculateMedianRatio(
  denominator: BenchmarkStatistics | null,
  numerator: BenchmarkStatistics | null
): number | null {
  if (!denominator || !numerator || denominator.median === 0) return null;
  const ratio = numerator.median / denominator.median;
  return Number.isFinite(ratio) ? ratio : null;
}
