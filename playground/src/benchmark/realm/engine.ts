import type { CompareRenderPayload } from "../../runtime/realm/channel-protocol.ts";
import type { BenchmarkFailureStage } from "../protocol.ts";
import type { BenchmarkTraceMark } from "../trace.ts";

export interface BenchmarkEngineContext {
  readonly mark: (event: BenchmarkTraceMark) => void;
  readonly payload: CompareRenderPayload;
  readonly resourceUrl: string | null;
}

export interface BenchmarkEngineSession {
  readonly version: string;
  dispose(): void;
  render(): string | Promise<string>;
}

export interface BenchmarkEngineAdapter {
  initialize(context: BenchmarkEngineContext): Promise<BenchmarkEngineSession>;
}

export class BenchmarkEngineError extends Error {
  readonly cause: unknown;
  readonly stage: BenchmarkFailureStage;

  constructor(stage: BenchmarkFailureStage, cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause));
    this.name = "BenchmarkEngineError";
    this.cause = cause;
    this.stage = stage;
  }
}

export async function runBenchmarkEngineStage<T>(
  stage: BenchmarkFailureStage,
  run: () => T | Promise<T>
): Promise<T> {
  try {
    return await run();
  } catch (error) {
    if (error instanceof BenchmarkEngineError) throw error;
    throw new BenchmarkEngineError(stage, error);
  }
}

export async function runObservedBenchmarkEngineStage<T>(
  stage: BenchmarkFailureStage,
  run: () => T | Promise<T>,
  markEnd: () => void
): Promise<T> {
  try {
    return await runBenchmarkEngineStage(stage, run);
  } finally {
    markEnd();
  }
}
