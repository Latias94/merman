import { preloadMermaid, renderMermaidSvg } from "@/src/lib/mermaid-renderer";

export type BenchEngine = "merman" | "mermaid";

export interface BenchSample {
  elapsedMs: number;
  svgLength: number;
}

export interface BenchEngineStats {
  engine: BenchEngine;
  samples: BenchSample[];
  errorCount: number;
  medianMs: number | null;
  p95Ms: number | null;
  minMs: number | null;
  maxMs: number | null;
  meanMs: number | null;
}

export interface BenchRunResult {
  warmupIterations: number;
  measureIterations: number;
  results: BenchEngineStats[];
}

export interface MermanRenderResult {
  svg: string | null;
  error: string | null;
}

export type MermanRenderFn = (
  source: string,
  theme: string,
  configJson: string
) => MermanRenderResult;

export interface BenchRunOptions {
  source: string;
  theme: string;
  configJson: string;
  engines: BenchEngine[];
  warmupIterations: number;
  measureIterations: number;
  renderMerman: MermanRenderFn;
  signal?: AbortSignal;
}

export async function runLocalRenderBench({
  source,
  theme,
  configJson,
  engines,
  warmupIterations,
  measureIterations,
  renderMerman,
  signal,
}: BenchRunOptions): Promise<BenchRunResult> {
  const results: BenchEngineStats[] = [];

  for (const engine of engines) {
    throwIfAborted(signal);
    await prepareEngine(engine);
    throwIfAborted(signal);
    await runWarmup({
      engine,
      source,
      theme,
      configJson,
      iterations: warmupIterations,
      renderMerman,
      signal,
    });
    results.push(
      await runMeasured({
        engine,
        source,
        theme,
        configJson,
        iterations: measureIterations,
        renderMerman,
        signal,
      })
    );
  }

  return {
    warmupIterations,
    measureIterations,
    results,
  };
}

async function prepareEngine(engine: BenchEngine) {
  if (engine === "mermaid") {
    await preloadMermaid();
  }
}

async function runWarmup({
  iterations,
  ...options
}: RunLoopOptions & { iterations: number }) {
  for (let i = 0; i < iterations; i++) {
    throwIfAborted(options.signal);
    await renderOnce(options);
    await yieldToBrowser(i);
  }
}

async function runMeasured({
  iterations,
  ...options
}: RunLoopOptions & { iterations: number }): Promise<BenchEngineStats> {
  const samples: BenchSample[] = [];
  let errorCount = 0;

  for (let i = 0; i < iterations; i++) {
    throwIfAborted(options.signal);
    const result = await renderOnce(options);
    if (result.error) {
      errorCount += 1;
    } else {
      samples.push({
        elapsedMs: result.elapsedMs,
        svgLength: result.svgLength,
      });
    }
    await yieldToBrowser(i);
  }

  return {
    engine: options.engine,
    samples,
    errorCount,
    ...summarize(samples.map((sample) => sample.elapsedMs)),
  };
}

interface RunLoopOptions {
  engine: BenchEngine;
  source: string;
  theme: string;
  configJson: string;
  renderMerman: MermanRenderFn;
  signal?: AbortSignal;
}

async function renderOnce({
  engine,
  source,
  theme,
  configJson,
  renderMerman,
}: Omit<RunLoopOptions, "signal">): Promise<{
  elapsedMs: number;
  svgLength: number;
  error: string | null;
}> {
  if (engine === "mermaid") {
    const result = await renderMermaidSvg(source, theme, configJson);
    return {
      elapsedMs: result.renderTime,
      svgLength: result.svg?.length ?? 0,
      error: result.error,
    };
  }

  const startedAt = performance.now();
  const result = renderMerman(source, theme, configJson);
  return {
    elapsedMs: performance.now() - startedAt,
    svgLength: result.svg?.length ?? 0,
    error: result.error,
  };
}

function summarize(values: number[]): Omit<BenchEngineStats, "engine" | "samples" | "errorCount"> {
  if (values.length === 0) {
    return {
      medianMs: null,
      p95Ms: null,
      minMs: null,
      maxMs: null,
      meanMs: null,
    };
  }

  const sorted = [...values].sort((a, b) => a - b);
  const sum = values.reduce((acc, value) => acc + value, 0);
  return {
    medianMs: percentile(sorted, 0.5),
    p95Ms: percentile(sorted, 0.95),
    minMs: sorted[0],
    maxMs: sorted[sorted.length - 1],
    meanMs: sum / values.length,
  };
}

function percentile(sortedValues: number[], percentileValue: number): number {
  if (sortedValues.length === 1) {
    return sortedValues[0];
  }

  const index = (sortedValues.length - 1) * percentileValue;
  const lower = Math.floor(index);
  const upper = Math.ceil(index);
  if (lower === upper) {
    return sortedValues[lower];
  }
  const weight = index - lower;
  return sortedValues[lower] * (1 - weight) + sortedValues[upper] * weight;
}

async function yieldToBrowser(iteration: number) {
  if (iteration % 5 !== 4) return;
  await new Promise<void>((resolve) => {
    window.setTimeout(resolve, 0);
  });
}

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new DOMException("Benchmark cancelled.", "AbortError");
  }
}
