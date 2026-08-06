import {
  REALM_BUDGETS,
  RealmProtocolError,
} from "../../runtime/realm/channel-protocol.ts";
import {
  benchmarkPhasePath,
  type BenchmarkEngine,
  type BenchmarkFailureStage,
  type BenchmarkTimedPhase,
  type BenchmarkTraceMark,
} from "../phase-contract.ts";
import {
  benchmarkIntentModeFromKind,
  type BenchmarkSampleIntentKind,
} from "../sample-plan.ts";

export interface BenchmarkProgressGate {
  assertComplete(): void;
  isEmpty(): boolean;
  observe(event: BenchmarkTraceMark): void;
}

export interface BenchmarkProgressContract {
  readonly engine: BenchmarkEngine;
  readonly intentKind: BenchmarkSampleIntentKind;
}

export interface BenchmarkStageWatchdog {
  dispose(): void;
  observe(event: BenchmarkTraceMark): void;
}

export interface BenchmarkStageTimer {
  clear(handle: unknown): void;
  now(): number;
  set(callback: () => void, timeoutMs: number): unknown;
}

const BROWSER_TIMER: BenchmarkStageTimer = {
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
  now: () => performance.now(),
  set: (callback, timeoutMs) => setTimeout(callback, timeoutMs),
};

interface ActiveStageTimer {
  readonly handle: unknown;
  readonly startedAt: number;
}

export class BenchmarkStageTimeoutError extends Error {
  readonly stage: BenchmarkFailureStage;

  constructor(stage: BenchmarkTimedPhase) {
    super(`Benchmark stage ${stage} exceeded its time budget.`);
    this.name = "BenchmarkStageTimeoutError";
    this.stage = stage;
  }
}

export function createBenchmarkProgressGate(
  contract: BenchmarkProgressContract
): BenchmarkProgressGate {
  const path = benchmarkPhasePath(
    contract.engine,
    benchmarkIntentModeFromKind(contract.intentKind)
  );
  const seen = new Set<BenchmarkTraceMark>();
  return Object.freeze({
    assertComplete() {
      try {
        path.assertSuccess(seen);
      } catch (error) {
        throw protocolError(error);
      }
    },
    isEmpty() {
      return seen.size === 0;
    },
    observe(event: BenchmarkTraceMark) {
      try {
        path.assertNext(seen, event);
      } catch (error) {
        throw protocolError(error);
      }
      seen.add(event);
    },
  });
}

export function createBenchmarkStageWatchdog(
  contract: BenchmarkProgressContract,
  onTimeout: (stage: BenchmarkFailureStage) => void,
  timer: BenchmarkStageTimer = BROWSER_TIMER,
  timeoutMs: number = REALM_BUDGETS.stageTimeoutMs
): BenchmarkStageWatchdog {
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    throw new RangeError("Benchmark stage timeout must be finite and positive.");
  }
  const path = benchmarkPhasePath(
    contract.engine,
    benchmarkIntentModeFromKind(contract.intentKind)
  );
  const timers = new Map<BenchmarkTimedPhase, ActiveStageTimer>();
  const clear = (stage: BenchmarkTimedPhase) => {
    const active = timers.get(stage);
    if (active) timer.clear(active.handle);
    timers.delete(stage);
    return active ?? null;
  };
  return Object.freeze({
    dispose() {
      for (const active of timers.values()) timer.clear(active.handle);
      timers.clear();
    },
    observe(event: BenchmarkTraceMark) {
      const transition = path.rule(event)?.watchdog;
      if (!transition) {
        throw protocolError(
          new Error(
            `Benchmark progress ${event} is forbidden for ${path.mode} ${path.engine}.`
          )
        );
      }
      if (transition.complete) {
        const active = clear(transition.complete);
        if (active && timer.now() - active.startedAt > timeoutMs) {
          throw new BenchmarkStageTimeoutError(transition.complete);
        }
      }
      if (!transition.start) return;
      clear(transition.start);
      const stage = transition.start;
      const startedAt = timer.now();
      let handle: unknown;
      handle = timer.set(() => {
        if (timers.get(stage)?.handle !== handle) return;
        timers.delete(stage);
        onTimeout(stage);
      }, timeoutMs);
      timers.set(stage, { handle, startedAt });
    },
  });
}

function protocolError(error: unknown): RealmProtocolError {
  return error instanceof Error
    ? new RealmProtocolError(error.message)
    : new RealmProtocolError("Benchmark phase contract failed.");
}
