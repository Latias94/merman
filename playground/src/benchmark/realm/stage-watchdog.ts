import {
  REALM_BUDGETS,
  RealmProtocolError,
} from "../../runtime/realm/channel-protocol.ts";
import type { BenchmarkFailureStage } from "../protocol.ts";
import type {
  BenchmarkEngine,
  BenchmarkSampleMode,
  BenchmarkTraceMark,
} from "../trace.ts";

export interface BenchmarkProgressGate {
  assertComplete(): void;
  isEmpty(): boolean;
  observe(event: BenchmarkTraceMark): void;
}

export interface BenchmarkProgressContract {
  readonly engine: BenchmarkEngine;
  readonly mode: BenchmarkSampleMode;
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

  constructor(stage: BenchmarkFailureStage) {
    super(`Benchmark stage ${stage} exceeded its time budget.`);
    this.name = "BenchmarkStageTimeoutError";
    this.stage = stage;
  }
}

const STAGE_START_EVENTS = Object.freeze({
  fonts_wait_start: "fonts",
  adapter_import_start: "adapter-import",
  engine_import_start: "engine-import",
  resource_acquire_start: "resource-acquire",
  register_start: "register",
  initialize_start: "initialize",
  render_start: "render",
  budgeted_svg_ready: "presentation",
} as const satisfies Partial<Record<BenchmarkTraceMark, BenchmarkFailureStage>>);

const STAGE_END_EVENTS = Object.freeze({
  fonts_wait_end: "fonts",
  adapter_import_end: "adapter-import",
  engine_import_end: "engine-import",
  resource_acquire_end: "resource-acquire",
  register_end: "register",
  initialize_end: "initialize",
  budgeted_svg_ready: "render",
  isolated_presentation_ready: "presentation",
} as const satisfies Partial<Record<BenchmarkTraceMark, BenchmarkFailureStage>>);

const WARM_PROGRESS = Object.freeze([
  "fonts_wait_start",
  "fonts_wait_end",
  "render_start",
  "budgeted_svg_ready",
  "isolated_dom_inserted",
  "isolated_layout_box_ready",
  "isolated_presentation_ready",
] as const satisfies readonly BenchmarkTraceMark[]);

const COLD_COMMON_PROGRESS = Object.freeze([
  "adapter_import_start",
  "adapter_import_end",
  "engine_import_start",
  "engine_import_end",
  "initialize_start",
  "initialize_end",
] as const satisfies readonly BenchmarkTraceMark[]);

export function createBenchmarkProgressGate(
  contract: BenchmarkProgressContract
): BenchmarkProgressGate {
  const seen = new Set<BenchmarkTraceMark>();
  const isCold = contract.mode === "realm-cold";
  const requireSeen = (
    event: BenchmarkTraceMark,
    dependency: BenchmarkTraceMark
  ) => {
    if (!seen.has(dependency)) {
      throw new RealmProtocolError(
        `Benchmark progress ${event} requires ${dependency}.`
      );
    }
  };
  const requireCold = (event: BenchmarkTraceMark) => {
    if (!isCold) {
      throw new RealmProtocolError(
        `Benchmark progress ${event} is forbidden for warm samples.`
      );
    }
  };
  const requireEngine = (
    event: BenchmarkTraceMark,
    engine: BenchmarkEngine
  ) => {
    if (contract.engine !== engine) {
      throw new RealmProtocolError(
        `Benchmark progress ${event} is forbidden for ${contract.engine}.`
      );
    }
  };

  return Object.freeze({
    assertComplete() {
      const required: readonly BenchmarkTraceMark[] = isCold
        ? [
            ...WARM_PROGRESS,
            ...COLD_COMMON_PROGRESS,
            ...(contract.engine === "merman"
              ? ([
                  "resource_acquire_start",
                  "resource_acquire_end",
                ] as const)
              : (["register_start", "register_end"] as const)),
          ]
        : WARM_PROGRESS;
      if (required.some((event) => !seen.has(event))) {
        throw new RealmProtocolError("Benchmark progress is incomplete.");
      }
    },

    isEmpty() {
      return seen.size === 0;
    },

    observe(event: BenchmarkTraceMark) {
      if (seen.has(event)) {
        throw new RealmProtocolError(
          `Benchmark progress event ${event} was observed twice.`
        );
      }

      switch (event) {
        case "fonts_wait_start":
          if (seen.size > 0) {
            throw new RealmProtocolError(
              "Benchmark progress fonts_wait_start must be first."
            );
          }
          break;
        case "adapter_import_start":
          requireCold(event);
          requireSeen(event, "fonts_wait_start");
          if (seen.has("fonts_wait_end")) {
            throw new RealmProtocolError(
              "Benchmark progress adapter_import_start must overlap the font wait."
            );
          }
          break;
        case "fonts_wait_end":
          requireSeen(event, "fonts_wait_start");
          if (isCold) requireSeen(event, "adapter_import_start");
          break;
        case "adapter_import_end":
          requireCold(event);
          requireSeen(event, "adapter_import_start");
          break;
        case "engine_import_start":
          requireCold(event);
          requireSeen(event, "fonts_wait_end");
          requireSeen(event, "adapter_import_end");
          break;
        case "resource_acquire_start":
          requireCold(event);
          requireEngine(event, "merman");
          requireSeen(event, "engine_import_start");
          break;
        case "engine_import_end":
          requireCold(event);
          requireSeen(event, "engine_import_start");
          if (contract.engine === "merman") {
            requireSeen(event, "resource_acquire_start");
          }
          break;
        case "resource_acquire_end":
          requireCold(event);
          requireEngine(event, "merman");
          requireSeen(event, "resource_acquire_start");
          break;
        case "register_start":
          requireCold(event);
          requireEngine(event, "mermaid");
          requireSeen(event, "engine_import_end");
          break;
        case "register_end":
          requireCold(event);
          requireEngine(event, "mermaid");
          requireSeen(event, "register_start");
          break;
        case "initialize_start":
          requireCold(event);
          requireSeen(event, "fonts_wait_end");
          requireSeen(event, "adapter_import_end");
          requireSeen(event, "engine_import_end");
          requireSeen(
            event,
            contract.engine === "merman"
              ? "resource_acquire_end"
              : "register_end"
          );
          break;
        case "initialize_end":
          requireCold(event);
          requireSeen(event, "initialize_start");
          break;
        case "render_start":
          requireSeen(event, "fonts_wait_end");
          if (isCold) requireSeen(event, "initialize_end");
          break;
        case "budgeted_svg_ready":
          requireSeen(event, "render_start");
          break;
        case "isolated_dom_inserted":
          requireSeen(event, "budgeted_svg_ready");
          break;
        case "isolated_layout_box_ready":
          requireSeen(event, "isolated_dom_inserted");
          break;
        case "isolated_presentation_ready":
          requireSeen(event, "isolated_layout_box_ready");
          break;
      }

      seen.add(event);
    },
  });
}

export function createBenchmarkStageWatchdog(
  onTimeout: (stage: BenchmarkFailureStage) => void,
  timer: BenchmarkStageTimer = BROWSER_TIMER,
  timeoutMs: number = REALM_BUDGETS.stageTimeoutMs
): BenchmarkStageWatchdog {
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    throw new RangeError("Benchmark stage timeout must be finite and positive.");
  }
  const timers = new Map<BenchmarkFailureStage, ActiveStageTimer>();
  const clear = (stage: BenchmarkFailureStage) => {
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
      const ending = STAGE_END_EVENTS[event as keyof typeof STAGE_END_EVENTS];
      if (ending) {
        const active = clear(ending);
        if (active && timer.now() - active.startedAt > timeoutMs) {
          throw new BenchmarkStageTimeoutError(ending);
        }
      }
      const starting = STAGE_START_EVENTS[event as keyof typeof STAGE_START_EVENTS];
      if (!starting) return;
      clear(starting);
      const startedAt = timer.now();
      let handle: unknown;
      handle = timer.set(() => {
        if (timers.get(starting)?.handle !== handle) return;
        timers.delete(starting);
        onTimeout(starting);
      }, timeoutMs);
      timers.set(
        starting,
        { handle, startedAt }
      );
    },
  });
}
