export const BENCHMARK_TRACE_SCHEMA_VERSION = 1 as const;

import {
  BENCHMARK_TRACE_EVENT_NAMES,
  benchmarkPhasePath,
  type BenchmarkEngine,
  type BenchmarkSampleMode,
  type BenchmarkTraceEventName,
  type BenchmarkTraceMark,
  type BenchmarkTraceOutcome,
  type FrozenBenchmarkPhasePath,
} from "./phase-contract.ts";

export {
  BENCHMARK_TRACE_EVENT_NAMES,
  benchmarkPhasePath,
  type BenchmarkEngine,
  type BenchmarkFailureStage,
  type BenchmarkSampleMode,
  type BenchmarkTraceEventName,
  type BenchmarkTraceMark,
  type BenchmarkTraceOutcome,
} from "./phase-contract.ts";

/**
 * The wire shape deliberately contains only the fixed event vector. The
 * protocol envelope carries BENCHMARK_TRACE_SCHEMA_VERSION and sample identity.
 */
export type BenchmarkRawTrace = Readonly<{
  [Event in BenchmarkTraceEventName]: Event extends "sample_start"
    ? 0
    : Event extends "sample_end"
      ? number
      : number | null;
}>;

export interface BenchmarkTraceContract {
  readonly engine: BenchmarkEngine;
  readonly mode: BenchmarkSampleMode;
  readonly outcome: BenchmarkTraceOutcome;
}

export interface BenchmarkDerivedIntervals {
  readonly adapterImportMs: number | null;
  readonly engineImportMs: number | null;
  readonly resourceAcquisitionMs: number | null;
  readonly registrationMs: number | null;
  readonly initializationMs: number | null;
  readonly firstBudgetedSvgMs: number | null;
  readonly firstIsolatedPresentationMs: number | null;
  readonly warmBudgetedSvgMs: number | null;
  readonly warmIsolatedPresentationMs: number | null;
}

export interface BenchmarkTraceRecorder {
  mark(event: BenchmarkTraceMark): number;
  finish(): BenchmarkRawTrace;
  finishFailure(): BenchmarkRawTrace;
}

export class BenchmarkTraceError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BenchmarkTraceError";
  }
}

type MutableEventVector = {
  -readonly [Event in BenchmarkTraceEventName]: Event extends "sample_start"
    ? 0
    : number | null;
};

const MARKABLE_EVENTS = new Set<BenchmarkTraceMark>(
  BENCHMARK_TRACE_EVENT_NAMES.filter(
    (event): event is BenchmarkTraceMark =>
      event !== "sample_start" && event !== "sample_end"
  )
);
const TRACE_EVENTS = new Set<BenchmarkTraceEventName>(
  BENCHMARK_TRACE_EVENT_NAMES
);

/**
 * Owns one realm-local clock origin. Calling finish at any failure boundary
 * freezes the completed prefix and records sample_end without fabricating
 * skipped phase events.
 */
export function createBenchmarkTraceRecorder(
  now: () => number = () => performance.now()
): BenchmarkTraceRecorder {
  const events = createEmptyEventVector();
  const t0 = readFiniteClock(now, "sample start");
  let lastOffset = 0;
  let finished: BenchmarkRawTrace | null = null;

  const readOffset = (label: string): number => {
    const offset = readFiniteClock(now, label) - t0;
    if (offset < 0 || offset < lastOffset) {
      throw new BenchmarkTraceError(
        `Benchmark clock moved backwards while recording ${label}.`
      );
    }
    lastOffset = offset;
    return offset;
  };

  return Object.freeze({
    mark(event: BenchmarkTraceMark): number {
      if (finished !== null) {
        throw new BenchmarkTraceError("Benchmark trace is already finished.");
      }
      if (!MARKABLE_EVENTS.has(event)) {
        throw new BenchmarkTraceError(`Benchmark trace event ${event} is invalid.`);
      }
      if (events[event] !== null) {
        throw new BenchmarkTraceError(
          `Benchmark trace event ${event} was recorded twice.`
        );
      }
      const offset = readOffset(event);
      events[event] = offset;
      return offset;
    },

    finish(): BenchmarkRawTrace {
      if (finished !== null) return finished;
      events.sample_end = readOffset("sample_end");
      finished = freezeTrace(events);
      return finished;
    },

    finishFailure(): BenchmarkRawTrace {
      if (finished !== null) return finished;
      events.sample_end = readOffset("sample_end");
      finished = freezeTrace(events);
      return finished;
    },
  });
}

/**
 * Reconstructs an immutable trace from untrusted wire data and verifies the
 * engine/mode-specific partial order. Failure traces may stop at any completed
 * phase boundary, while success traces must contain the full applicable path.
 */
export function validateBenchmarkRawTrace(
  value: unknown,
  contract: BenchmarkTraceContract
): BenchmarkRawTrace {
  assertContract(contract);
  const record = expectRecord(value);
  assertExactEventKeys(record);

  const sampleStart = expectRequiredOffset(record.sample_start, "sample_start");
  if (sampleStart !== 0) {
    throw new BenchmarkTraceError("Benchmark sample_start must be exactly 0.");
  }

  const trace = Object.freeze(
    Object.fromEntries(
      BENCHMARK_TRACE_EVENT_NAMES.map((event) => [
        event,
        event === "sample_start"
          ? 0
          : event === "sample_end"
            ? expectRequiredOffset(record[event], event)
            : expectOffset(record[event], event),
      ])
    )
  ) as BenchmarkRawTrace;

  for (const event of BENCHMARK_TRACE_EVENT_NAMES) {
    const offset = trace[event];
    if (offset !== null && offset > trace.sample_end) {
      throw new BenchmarkTraceError(
        `Benchmark ${event} occurs after sample_end.`
      );
    }
  }
  validatePhaseTrace(
    benchmarkPhasePath(contract.engine, contract.mode),
    trace,
    contract.outcome
  );
  return trace;
}

/**
 * Derives only direct endpoint differences. It intentionally has no synthetic
 * total because acquisition and import spans are allowed to overlap.
 */
export function deriveBenchmarkIntervals(
  trace: BenchmarkRawTrace,
  {
    engine,
    mode,
  }: Readonly<{ engine: BenchmarkEngine; mode: BenchmarkSampleMode }>
): BenchmarkDerivedIntervals {
  if (mode !== "realm-cold" && mode !== "warm") {
    throw new BenchmarkTraceError("Benchmark sample mode is invalid.");
  }

  let adapterImportMs: number | null = null;
  let engineImportMs: number | null = null;
  let resourceAcquisitionMs: number | null = null;
  let registrationMs: number | null = null;
  let initializationMs: number | null = null;
  for (const span of benchmarkPhasePath(engine, mode).spans) {
    const duration = pairDuration(trace[span.start], trace[span.end]);
    switch (span.phase) {
      case "adapter-import":
        adapterImportMs = duration;
        break;
      case "engine-import":
        engineImportMs = duration;
        break;
      case "resource-acquire":
        resourceAcquisitionMs = duration;
        break;
      case "register":
        registrationMs = duration;
        break;
      case "initialize":
        initializationMs = duration;
        break;
      case "fonts":
        break;
      default:
        break;
    }
  }
  return Object.freeze({
    adapterImportMs,
    engineImportMs,
    resourceAcquisitionMs,
    registrationMs,
    initializationMs,
    firstBudgetedSvgMs:
      mode === "realm-cold" && trace.budgeted_svg_ready !== null
        ? trace.budgeted_svg_ready - trace.sample_start
        : null,
    firstIsolatedPresentationMs:
      mode === "realm-cold" && trace.isolated_presentation_ready !== null
        ? trace.isolated_presentation_ready - trace.sample_start
        : null,
    warmBudgetedSvgMs:
      mode === "warm" &&
      trace.render_start !== null &&
      trace.budgeted_svg_ready !== null
        ? trace.budgeted_svg_ready - trace.render_start
        : null,
    warmIsolatedPresentationMs:
      mode === "warm" &&
      trace.render_start !== null &&
      trace.isolated_presentation_ready !== null
        ? trace.isolated_presentation_ready - trace.render_start
        : null,
  });
}

function validatePhaseTrace(
  path: FrozenBenchmarkPhasePath,
  trace: BenchmarkRawTrace,
  outcome: BenchmarkTraceOutcome
): void {
  const seen = new Set<BenchmarkTraceMark>();
  for (const event of BENCHMARK_TRACE_EVENT_NAMES) {
    if (event === "sample_start" || event === "sample_end") continue;
    const offset = trace[event];
    const rule = path.rule(event);
    if (!rule) {
      if (offset !== null) {
        throw new BenchmarkTraceError(
          `Benchmark ${event} is forbidden for this engine or mode.`
        );
      }
      continue;
    }
    if (offset === null) continue;
    for (const predecessor of rule.predecessors) {
      const predecessorOffset = trace[predecessor];
      if (predecessorOffset === null) {
        throw new BenchmarkTraceError(
          `Benchmark ${event} requires ${predecessor}.`
        );
      }
      if (predecessorOffset > offset) {
        throw new BenchmarkTraceError(
          `Benchmark ${predecessor} must not occur after ${event}.`
        );
      }
    }
    seen.add(event);
  }

  for (const span of path.spans) {
    const start = trace[span.start];
    const end = trace[span.end];
    if (start === null && end !== null) {
      throw new BenchmarkTraceError(
        `Benchmark ${span.end} requires ${span.start}.`
      );
    }
    if (start !== null && end !== null && start > end) {
      throw new BenchmarkTraceError(
        `Benchmark ${span.start} must not occur after ${span.end}.`
      );
    }
  }

  if (
    outcome === "success" &&
    (seen.size !== path.applicableEvents.length ||
      path.applicableEvents.some((event) => !seen.has(event)))
  ) {
    const missing = path.applicableEvents.find((event) => !seen.has(event));
    throw new BenchmarkTraceError(
      `Successful benchmark trace requires ${missing ?? "the complete phase path"}.`
    );
  }
}

function pairDuration(start: number | null, end: number | null): number | null {
  return start === null || end === null ? null : end - start;
}

function assertContract(contract: BenchmarkTraceContract): void {
  if (contract.engine !== "merman" && contract.engine !== "mermaid") {
    throw new BenchmarkTraceError("Benchmark engine is invalid.");
  }
  if (contract.mode !== "realm-cold" && contract.mode !== "warm") {
    throw new BenchmarkTraceError("Benchmark sample mode is invalid.");
  }
  if (contract.outcome !== "success" && contract.outcome !== "failure") {
    throw new BenchmarkTraceError("Benchmark trace outcome is invalid.");
  }
}

function expectRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new BenchmarkTraceError("Benchmark raw trace must be an object.");
  }
  return value as Record<string, unknown>;
}

function assertExactEventKeys(record: Record<string, unknown>): void {
  const keys = Reflect.ownKeys(record);
  if (
    keys.length !== TRACE_EVENTS.size ||
    keys.some(
      (key) =>
        typeof key !== "string" ||
        !TRACE_EVENTS.has(key as BenchmarkTraceEventName)
    )
  ) {
    throw new BenchmarkTraceError(
      "Benchmark raw trace contains unexpected fields."
    );
  }
}

function expectOffset(value: unknown, name: string): number | null {
  if (value === null) return null;
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw new BenchmarkTraceError(
      `Benchmark ${name} must be a finite non-negative offset or null.`
    );
  }
  return value;
}

function expectRequiredOffset(value: unknown, name: string): number {
  const offset = expectOffset(value, name);
  if (offset === null) {
    throw new BenchmarkTraceError(
      `Benchmark ${name} must be a finite non-negative offset.`
    );
  }
  return offset;
}

function readFiniteClock(now: () => number, label: string): number {
  const value = now();
  if (!Number.isFinite(value)) {
    throw new BenchmarkTraceError(
      `Benchmark clock returned a non-finite value for ${label}.`
    );
  }
  return value;
}

function createEmptyEventVector(): MutableEventVector {
  return {
    sample_start: 0,
    fonts_wait_start: null,
    fonts_wait_end: null,
    adapter_import_start: null,
    adapter_import_end: null,
    engine_import_start: null,
    engine_import_end: null,
    resource_acquire_start: null,
    resource_acquire_end: null,
    register_start: null,
    register_end: null,
    initialize_start: null,
    initialize_end: null,
    render_start: null,
    budgeted_svg_ready: null,
    isolated_dom_inserted: null,
    isolated_layout_box_ready: null,
    isolated_presentation_ready: null,
    sample_end: null,
  };
}

function freezeTrace(events: MutableEventVector): BenchmarkRawTrace {
  const sampleEnd = events.sample_end;
  if (sampleEnd === null) {
    throw new BenchmarkTraceError("Benchmark sample_end was not recorded.");
  }
  return Object.freeze({ ...events, sample_start: 0, sample_end: sampleEnd });
}
