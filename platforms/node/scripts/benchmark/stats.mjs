export function summarize(samples) {
  if (!Array.isArray(samples) || samples.length === 0) {
    throw new Error("cannot summarize an empty sample distribution");
  }
  const sorted = [...samples].sort((left, right) => left - right);
  const total = sorted.reduce((sum, value) => sum + value, 0);
  return {
    count: sorted.length,
    min_ms: sorted[0],
    p50_ms: percentile(sorted, 0.5),
    p95_ms: percentile(sorted, 0.95),
    max_ms: sorted.at(-1),
    mean_ms: total / sorted.length,
  };
}

function percentile(sorted, probability) {
  const index = (sorted.length - 1) * probability;
  const lower = Math.floor(index);
  const upper = Math.ceil(index);
  if (lower === upper) return sorted[lower];
  return sorted[lower] + (sorted[upper] - sorted[lower]) * (index - lower);
}
