import { BENCHMARK_BUDGETS } from "../runtime/realm/channel-protocol.ts";
import type { BenchmarkEngine, BenchmarkSampleMode } from "./trace.ts";

export type BenchmarkPlanOptions =
  | Readonly<{
      iterations: number;
      mode: "realm-cold";
      seed: number;
    }>
  | Readonly<{
      iterations: number;
      mode: "warm";
      seed: number;
      warmups: number;
    }>;

export type BenchmarkPlanBudgetOptions =
  | Readonly<{
      iterations: number;
      mode: "realm-cold";
    }>
  | Readonly<{
      iterations: number;
      mode: "warm";
      warmups: number;
    }>;

export interface BenchmarkMeasuredBlock {
  readonly index: number;
  readonly order: readonly [BenchmarkEngine, BenchmarkEngine];
}

interface BenchmarkSampleIntentBase {
  readonly engine: BenchmarkEngine;
  readonly orderIndex: 0 | 1;
  readonly ordinal: number;
  readonly sampleId: string;
  readonly sessionId: string;
}

export interface BenchmarkColdMeasuredIntent
  extends BenchmarkSampleIntentBase {
  readonly aggregateKey: string;
  readonly blockIndex: number;
  readonly kind: "cold-measured";
  readonly mode: "realm-cold";
  readonly session: "single-use";
}

export interface BenchmarkWarmSetupIntent extends BenchmarkSampleIntentBase {
  readonly kind: "warm-setup";
  readonly mode: "realm-cold";
  readonly session: "open-reused";
}

export interface BenchmarkWarmupIntent extends BenchmarkSampleIntentBase {
  readonly kind: "warmup";
  readonly mode: "warm";
  readonly roundIndex: number;
  readonly session: "reuse";
}

export interface BenchmarkWarmMeasuredIntent
  extends BenchmarkSampleIntentBase {
  readonly aggregateKey: string;
  readonly blockIndex: number;
  readonly kind: "warm-measured";
  readonly mode: "warm";
  readonly session: "reuse";
}

export type BenchmarkSampleIntent =
  | BenchmarkColdMeasuredIntent
  | BenchmarkWarmMeasuredIntent
  | BenchmarkWarmSetupIntent
  | BenchmarkWarmupIntent;

export type BenchmarkSampleIntentKind = BenchmarkSampleIntent["kind"];

export interface BenchmarkSamplePlanBudget {
  readonly maxLiveRealms: 1 | 2;
  readonly measuredSamples: number;
  readonly realmCreations: number;
  readonly setupSamples: number;
  readonly totalSamples: number;
  readonly warmupSamples: number;
}

export interface BenchmarkSessionPlan {
  readonly engine: BenchmarkEngine;
  readonly id: string;
  readonly lifecycle: "reused" | "single-use";
  readonly sampleIds: readonly string[];
}

interface BenchmarkSamplePlanBase {
  readonly blocks: readonly BenchmarkMeasuredBlock[];
  readonly budget: BenchmarkSamplePlanBudget;
  readonly iterations: number;
  readonly samples: readonly BenchmarkSampleIntent[];
  readonly seed: number;
  readonly sessions: readonly BenchmarkSessionPlan[];
}

export interface ColdBenchmarkSamplePlan extends BenchmarkSamplePlanBase {
  readonly mode: "realm-cold";
}

export interface WarmBenchmarkSamplePlan extends BenchmarkSamplePlanBase {
  readonly mode: "warm";
  readonly warmups: number;
}

export type BenchmarkSamplePlan =
  | ColdBenchmarkSamplePlan
  | WarmBenchmarkSamplePlan;

export type BenchmarkSamplePurpose = "measured" | "setup" | "warmup";

export class BenchmarkSamplePlanError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BenchmarkSamplePlanError";
  }
}

const AB = Object.freeze(["merman", "mermaid"] as const);
const BA = Object.freeze(["mermaid", "merman"] as const);

export function createBenchmarkSamplePlan(
  options: BenchmarkPlanOptions
): BenchmarkSamplePlan {
  validateSeed(options.seed);
  const budget = calculateBenchmarkSamplePlanBudget(options);
  const blocks = createMeasuredBlocks(options.iterations, options.seed);
  if (options.mode === "realm-cold") {
    return createColdPlan(options, blocks, budget);
  }
  return createWarmPlan(options, blocks, budget);
}

export function calculateBenchmarkSamplePlanBudget(
  options: BenchmarkPlanBudgetOptions
): BenchmarkSamplePlanBudget {
  if (options.mode !== "realm-cold" && options.mode !== "warm") {
    throw new BenchmarkSamplePlanError("Benchmark mode is invalid.");
  }
  validateIterations(options.iterations);
  if (options.mode === "realm-cold") {
    const totalSamples = options.iterations * 2;
    validateRetainedSamples(totalSamples);
    return freezeBudget({
      maxLiveRealms: 1,
      measuredSamples: totalSamples,
      realmCreations: totalSamples,
      setupSamples: 0,
      totalSamples,
      warmupSamples: 0,
    });
  }
  validateWarmups(options.warmups);
  const totalSamples = 2 + options.warmups * 2 + options.iterations * 2;
  validateRetainedSamples(totalSamples);
  return freezeBudget({
    maxLiveRealms: 2,
    measuredSamples: options.iterations * 2,
    realmCreations: 2,
    setupSamples: 2,
    totalSamples,
    warmupSamples: options.warmups * 2,
  });
}

export function benchmarkIntentPurpose(
  intent: BenchmarkSampleIntent
): BenchmarkSamplePurpose {
  return benchmarkIntentPurposeFromKind(intent.kind);
}

export function benchmarkIntentPurposeFromKind(
  kind: BenchmarkSampleIntentKind
): BenchmarkSamplePurpose {
  switch (kind) {
    case "cold-measured":
    case "warm-measured":
      return "measured";
    case "warm-setup":
      return "setup";
    case "warmup":
      return "warmup";
  }
}

export function benchmarkIntentRole(
  intent: BenchmarkSampleIntent
): "measured" | "warmup" {
  return benchmarkIntentRoleFromKind(intent.kind);
}

export function benchmarkIntentRoleFromKind(
  kind: BenchmarkSampleIntentKind
): "measured" | "warmup" {
  return benchmarkIntentPurposeFromKind(kind) === "measured"
    ? "measured"
    : "warmup";
}

export function benchmarkIntentMode(
  intent: BenchmarkSampleIntent
): BenchmarkSampleMode {
  return benchmarkIntentModeFromKind(intent.kind);
}

export function benchmarkIntentModeFromKind(
  kind: BenchmarkSampleIntentKind
): BenchmarkSampleMode {
  return kind === "cold-measured" || kind === "warm-setup"
    ? "realm-cold"
    : "warm";
}

export function isBenchmarkAggregationIntent(
  intent: BenchmarkSampleIntent
): intent is BenchmarkColdMeasuredIntent | BenchmarkWarmMeasuredIntent {
  return intent.kind === "cold-measured" || intent.kind === "warm-measured";
}

export function isBenchmarkInputBindingIntent(
  intent: BenchmarkSampleIntent
): intent is BenchmarkColdMeasuredIntent | BenchmarkWarmSetupIntent {
  return benchmarkIntentMode(intent) === "realm-cold";
}

export function benchmarkWarmupCount(plan: BenchmarkSamplePlan): number {
  return plan.mode === "warm" ? plan.warmups : 0;
}

export function createUint32Random(seed: number): () => number {
  validateSeed(seed);
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

function createMeasuredBlocks(
  iterations: number,
  seed: number
): readonly BenchmarkMeasuredBlock[] {
  const orders: Array<readonly [BenchmarkEngine, BenchmarkEngine]> = [];
  for (let index = 0; index < iterations / 2; index += 1) {
    orders.push(AB, BA);
  }
  shuffleInPlace(orders, createUint32Random(seed));
  return Object.freeze(
    orders.map((order, index) => Object.freeze({ index, order }))
  );
}

function createColdPlan(
  options: Extract<BenchmarkPlanOptions, { mode: "realm-cold" }>,
  blocks: readonly BenchmarkMeasuredBlock[],
  budget: BenchmarkSamplePlanBudget
): ColdBenchmarkSamplePlan {
  const samples: BenchmarkColdMeasuredIntent[] = [];
  for (const block of blocks) {
    for (const [orderIndex, engine] of block.order.entries()) {
      const ordinal = samples.length;
      samples.push(
        Object.freeze({
          aggregateKey: `${block.index}:${engine}`,
          blockIndex: block.index,
          engine,
          kind: "cold-measured",
          mode: "realm-cold",
          orderIndex: orderIndex as 0 | 1,
          ordinal,
          sampleId: `sample-${ordinal + 1}`,
          session: "single-use",
          sessionId: `session-${ordinal + 1}`,
        })
      );
    }
  }
  const sessions = samples.map((sample) =>
    Object.freeze({
      engine: sample.engine,
      id: sample.sessionId,
      lifecycle: "single-use" as const,
      sampleIds: Object.freeze([sample.sampleId]),
    })
  );
  return Object.freeze({
    blocks,
    budget,
    iterations: options.iterations,
    mode: "realm-cold",
    samples: Object.freeze(samples),
    seed: options.seed,
    sessions: Object.freeze(sessions),
  });
}

function createWarmPlan(
  options: Extract<BenchmarkPlanOptions, { mode: "warm" }>,
  blocks: readonly BenchmarkMeasuredBlock[],
  budget: BenchmarkSamplePlanBudget
): WarmBenchmarkSamplePlan {
  const samples: BenchmarkSampleIntent[] = [];
  const sessionIds: Readonly<Record<BenchmarkEngine, string>> = Object.freeze({
    merman: "session-merman",
    mermaid: "session-mermaid",
  });
  const append = <Intent extends Omit<BenchmarkSampleIntentBase, "ordinal" | "sampleId"> &
    Omit<BenchmarkSampleIntent, keyof BenchmarkSampleIntentBase>>(
    intent: Intent
  ) => {
    const ordinal = samples.length;
    samples.push(
      Object.freeze({
        ...intent,
        ordinal,
        sampleId: `sample-${ordinal + 1}`,
      }) as BenchmarkSampleIntent
    );
  };

  const setupOrder = blocks[0].order;
  for (const [orderIndex, engine] of setupOrder.entries()) {
    append({
      engine,
      kind: "warm-setup",
      mode: "realm-cold",
      orderIndex: orderIndex as 0 | 1,
      session: "open-reused",
      sessionId: sessionIds[engine],
    });
  }
  for (let roundIndex = 0; roundIndex < options.warmups; roundIndex += 1) {
    const order = blocks[roundIndex % blocks.length].order;
    for (const [orderIndex, engine] of order.entries()) {
      append({
        engine,
        kind: "warmup",
        mode: "warm",
        orderIndex: orderIndex as 0 | 1,
        roundIndex,
        session: "reuse",
        sessionId: sessionIds[engine],
      });
    }
  }
  for (const block of blocks) {
    for (const [orderIndex, engine] of block.order.entries()) {
      append({
        aggregateKey: `${block.index}:${engine}`,
        blockIndex: block.index,
        engine,
        kind: "warm-measured",
        mode: "warm",
        orderIndex: orderIndex as 0 | 1,
        session: "reuse",
        sessionId: sessionIds[engine],
      });
    }
  }

  const sessions = (["merman", "mermaid"] as const).map((engine) =>
    Object.freeze({
      engine,
      id: sessionIds[engine],
      lifecycle: "reused" as const,
      sampleIds: Object.freeze(
        samples
          .filter((sample) => sample.engine === engine)
          .map((sample) => sample.sampleId)
      ),
    })
  );
  return Object.freeze({
    blocks,
    budget,
    iterations: options.iterations,
    mode: "warm",
    samples: Object.freeze(samples),
    seed: options.seed,
    sessions: Object.freeze(sessions),
    warmups: options.warmups,
  });
}

function freezeBudget(
  budget: BenchmarkSamplePlanBudget
): BenchmarkSamplePlanBudget {
  return Object.freeze({ ...budget });
}

function validateIterations(iterations: number): void {
  if (
    !Number.isSafeInteger(iterations) ||
    iterations < 2 ||
    iterations > BENCHMARK_BUDGETS.maxIterations ||
    iterations % 2 !== 0
  ) {
    throw new BenchmarkSamplePlanError(
      `Benchmark iterations must be an even integer from 2 to ${BENCHMARK_BUDGETS.maxIterations}.`
    );
  }
}

function validateWarmups(warmups: number): void {
  if (
    !Number.isSafeInteger(warmups) ||
    warmups < 0 ||
    warmups + 1 > BENCHMARK_BUDGETS.maxWarmups
  ) {
    throw new BenchmarkSamplePlanError(
      "Benchmark warmups exceed the per-realm protocol budget."
    );
  }
}

function validateSeed(seed: number): void {
  if (!Number.isSafeInteger(seed) || seed < 0 || seed > 0xffff_ffff) {
    throw new BenchmarkSamplePlanError(
      "Benchmark seed must be an unsigned 32-bit integer."
    );
  }
}

function validateRetainedSamples(total: number): void {
  if (total > BENCHMARK_BUDGETS.maxRetainedSamples) {
    throw new BenchmarkSamplePlanError(
      "Benchmark run exceeds the retained sample budget."
    );
  }
}
