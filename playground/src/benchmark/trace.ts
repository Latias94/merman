export const BENCHMARK_TRACE_SCHEMA_VERSION = 1 as const;

export const BENCHMARK_TRACE_EVENT_NAMES = Object.freeze([
  "sample_start",
  "fonts_wait_start",
  "fonts_wait_end",
  "adapter_import_start",
  "adapter_import_end",
  "engine_import_start",
  "engine_import_end",
  "resource_acquire_start",
  "resource_acquire_end",
  "register_start",
  "register_end",
  "initialize_start",
  "initialize_end",
  "render_start",
  "safe_svg_ready",
  "dom_inserted",
  "layout_box_ready",
  "presentation_ready",
  "sample_end",
] as const);

export type BenchmarkEngine = "merman" | "mermaid";
export type BenchmarkSampleMode = "realm-cold" | "warm";
export type BenchmarkTraceOutcome = "success" | "failure";
export type BenchmarkTraceEventName =
  (typeof BENCHMARK_TRACE_EVENT_NAMES)[number];
export type BenchmarkTraceMark = Exclude<
  BenchmarkTraceEventName,
  "sample_start" | "sample_end"
>;

/**
 * The wire shape deliberately contains only the fixed event vector. The
 * protocol envelope carries BENCHMARK_TRACE_SCHEMA_VERSION and sample identity.
 */
export interface BenchmarkRawTrace {
  readonly sample_start: 0;
  readonly fonts_wait_start: number | null;
  readonly fonts_wait_end: number | null;
  readonly adapter_import_start: number | null;
  readonly adapter_import_end: number | null;
  readonly engine_import_start: number | null;
  readonly engine_import_end: number | null;
  readonly resource_acquire_start: number | null;
  readonly resource_acquire_end: number | null;
  readonly register_start: number | null;
  readonly register_end: number | null;
  readonly initialize_start: number | null;
  readonly initialize_end: number | null;
  readonly render_start: number | null;
  readonly safe_svg_ready: number | null;
  readonly dom_inserted: number | null;
  readonly layout_box_ready: number | null;
  readonly presentation_ready: number | null;
  readonly sample_end: number;
}

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
  readonly firstValidSvgMs: number | null;
  readonly firstPresentationReadyMs: number | null;
  readonly warmValidSvgMs: number | null;
  readonly warmPresentationReadyMs: number | null;
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

type PairedEvent = Readonly<{
  end: PairEnd;
  label: string;
  start: PairStart;
}>;

type PairStart =
  | "fonts_wait_start"
  | "adapter_import_start"
  | "engine_import_start"
  | "resource_acquire_start"
  | "register_start"
  | "initialize_start";

type PairEnd =
  | "fonts_wait_end"
  | "adapter_import_end"
  | "engine_import_end"
  | "resource_acquire_end"
  | "register_end"
  | "initialize_end";

type MutableEventVector = Record<BenchmarkTraceEventName, number | null>;

const PAIRS = Object.freeze([
  {
    start: "fonts_wait_start",
    end: "fonts_wait_end",
    label: "fonts_wait",
  },
  {
    start: "adapter_import_start",
    end: "adapter_import_end",
    label: "adapter_import",
  },
  {
    start: "engine_import_start",
    end: "engine_import_end",
    label: "engine_import",
  },
  {
    start: "resource_acquire_start",
    end: "resource_acquire_end",
    label: "resource_acquire",
  },
  { start: "register_start", end: "register_end", label: "register" },
  {
    start: "initialize_start",
    end: "initialize_end",
    label: "initialize",
  },
] as const satisfies readonly PairedEvent[]);

const MARKABLE_EVENTS = new Set<BenchmarkTraceEventName>(
  BENCHMARK_TRACE_EVENT_NAMES.filter(
    (event) => event !== "sample_start" && event !== "sample_end"
  )
);

/**
 * Owns one realm-local clock origin. Calling finish at any failure boundary
 * freezes the completed prefix and records sample_end without fabricating
 * skipped phase events.
 */
export function createBenchmarkTraceRecorder(
  now: () => number = () => performance.now()
): BenchmarkTraceRecorder {
  const t0 = readFiniteClock(now, "sample start");
  const events = createEmptyEventVector();
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
      for (const pair of PAIRS) {
        if ((events[pair.start] === null) !== (events[pair.end] === null)) {
          events[pair.start] = null;
          events[pair.end] = null;
        }
      }
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

  const sampleEnd = expectRequiredOffset(record.sample_end, "sample_end");
  const trace = Object.freeze({
    sample_start: 0,
    fonts_wait_start: expectOffset(
      record.fonts_wait_start,
      "fonts_wait_start"
    ),
    fonts_wait_end: expectOffset(record.fonts_wait_end, "fonts_wait_end"),
    adapter_import_start: expectOffset(
      record.adapter_import_start,
      "adapter_import_start"
    ),
    adapter_import_end: expectOffset(
      record.adapter_import_end,
      "adapter_import_end"
    ),
    engine_import_start: expectOffset(
      record.engine_import_start,
      "engine_import_start"
    ),
    engine_import_end: expectOffset(
      record.engine_import_end,
      "engine_import_end"
    ),
    resource_acquire_start: expectOffset(
      record.resource_acquire_start,
      "resource_acquire_start"
    ),
    resource_acquire_end: expectOffset(
      record.resource_acquire_end,
      "resource_acquire_end"
    ),
    register_start: expectOffset(record.register_start, "register_start"),
    register_end: expectOffset(record.register_end, "register_end"),
    initialize_start: expectOffset(
      record.initialize_start,
      "initialize_start"
    ),
    initialize_end: expectOffset(record.initialize_end, "initialize_end"),
    render_start: expectOffset(record.render_start, "render_start"),
    safe_svg_ready: expectOffset(record.safe_svg_ready, "safe_svg_ready"),
    dom_inserted: expectOffset(record.dom_inserted, "dom_inserted"),
    layout_box_ready: expectOffset(
      record.layout_box_ready,
      "layout_box_ready"
    ),
    presentation_ready: expectOffset(
      record.presentation_ready,
      "presentation_ready"
    ),
    sample_end: sampleEnd,
  } satisfies BenchmarkRawTrace);

  for (const pair of PAIRS) assertPair(trace, pair);
  for (const event of BENCHMARK_TRACE_EVENT_NAMES) {
    const offset = trace[event];
    if (offset !== null && offset > trace.sample_end) {
      throw new BenchmarkTraceError(
        `Benchmark ${event} occurs after sample_end.`
      );
    }
  }

  if (contract.outcome === "success") {
    assertRequiredPair(trace, PAIRS[0]);
  }
  if (contract.mode === "warm") {
    assertWarmApplicability(trace);
  } else {
    assertColdApplicability(trace, contract);
  }

  assertRenderPath(trace, contract);
  return trace;
}

/**
 * Derives only direct endpoint differences. It intentionally has no synthetic
 * total because acquisition and import spans are allowed to overlap.
 */
export function deriveBenchmarkIntervals(
  trace: BenchmarkRawTrace,
  { mode }: Readonly<{ mode: BenchmarkSampleMode }>
): BenchmarkDerivedIntervals {
  if (mode !== "realm-cold" && mode !== "warm") {
    throw new BenchmarkTraceError("Benchmark sample mode is invalid.");
  }

  return Object.freeze({
    adapterImportMs: pairDuration(
      trace.adapter_import_start,
      trace.adapter_import_end
    ),
    engineImportMs: pairDuration(
      trace.engine_import_start,
      trace.engine_import_end
    ),
    resourceAcquisitionMs: pairDuration(
      trace.resource_acquire_start,
      trace.resource_acquire_end
    ),
    registrationMs: pairDuration(trace.register_start, trace.register_end),
    initializationMs: pairDuration(
      trace.initialize_start,
      trace.initialize_end
    ),
    firstValidSvgMs:
      mode === "realm-cold" && trace.safe_svg_ready !== null
        ? trace.safe_svg_ready - trace.sample_start
        : null,
    firstPresentationReadyMs:
      mode === "realm-cold" && trace.presentation_ready !== null
        ? trace.presentation_ready - trace.sample_start
        : null,
    warmValidSvgMs:
      mode === "warm" &&
      trace.render_start !== null &&
      trace.safe_svg_ready !== null
        ? trace.safe_svg_ready - trace.render_start
        : null,
    warmPresentationReadyMs:
      mode === "warm" &&
      trace.render_start !== null &&
      trace.presentation_ready !== null
        ? trace.presentation_ready - trace.render_start
        : null,
  });
}

function assertColdApplicability(
  trace: BenchmarkRawTrace,
  contract: BenchmarkTraceContract
): void {
  const fontsPresent = isPairPresent(trace, PAIRS[0]);
  const adapterPresent = isPairPresent(trace, PAIRS[1]);
  if (fontsPresent && adapterPresent) {
    assertOverlappingPairs(trace, PAIRS[0], PAIRS[1]);
  }

  const enginePresent = isPairPresent(trace, PAIRS[2]);
  if (enginePresent) {
    assertDependency(adapterPresent, "engine_import", "adapter_import");
    assertAtOrBefore(
      trace.adapter_import_end,
      trace.engine_import_start,
      "adapter_import_end",
      "engine_import_start"
    );
  }

  if (contract.engine === "merman") {
    assertForbiddenPair(trace, PAIRS[4]);
    const resourcePresent = isPairPresent(trace, PAIRS[3]);
    if (resourcePresent) {
      assertDependency(adapterPresent, "resource_acquire", "adapter_import");
      assertAtOrBefore(
        trace.adapter_import_end,
        trace.resource_acquire_start,
        "adapter_import_end",
        "resource_acquire_start"
      );
    }
    if (enginePresent && resourcePresent) {
      assertOverlappingPairs(trace, PAIRS[2], PAIRS[3]);
    }
  } else {
    assertForbiddenPair(trace, PAIRS[3]);
    const registerPresent = isPairPresent(trace, PAIRS[4]);
    if (registerPresent) {
      assertDependency(enginePresent, "register", "engine_import");
      assertAtOrBefore(
        trace.engine_import_end,
        trace.register_start,
        "engine_import_end",
        "register_start"
      );
    }
  }

  const initializePresent = isPairPresent(trace, PAIRS[5]);
  if (initializePresent) {
    assertDependency(enginePresent, "initialize", "engine_import");
    if (contract.engine === "merman") {
      assertDependency(
        isPairPresent(trace, PAIRS[3]),
        "initialize",
        "resource_acquire"
      );
      assertAtOrBefore(
        trace.engine_import_end,
        trace.initialize_start,
        "engine_import_end",
        "initialize_start"
      );
      assertAtOrBefore(
        trace.resource_acquire_end,
        trace.initialize_start,
        "resource_acquire_end",
        "initialize_start"
      );
    } else {
      assertDependency(
        isPairPresent(trace, PAIRS[4]),
        "initialize",
        "register"
      );
      assertAtOrBefore(
        trace.register_end,
        trace.initialize_start,
        "register_end",
        "initialize_start"
      );
    }
  }

  if (trace.render_start !== null) {
    assertDependency(initializePresent, "render_start", "initialize");
    assertAtOrBefore(
      trace.initialize_end,
      trace.render_start,
      "initialize_end",
      "render_start"
    );
  }

  if (contract.outcome === "success") {
    assertRequiredPair(trace, PAIRS[1]);
    assertRequiredPair(trace, PAIRS[2]);
    if (contract.engine === "merman") {
      assertRequiredPair(trace, PAIRS[3]);
    } else {
      assertRequiredPair(trace, PAIRS[4]);
    }
    assertRequiredPair(trace, PAIRS[5]);
  }
}

function assertWarmApplicability(trace: BenchmarkRawTrace): void {
  for (const pair of PAIRS.slice(1)) assertForbiddenPair(trace, pair);
}

function assertRenderPath(
  trace: BenchmarkRawTrace,
  contract: BenchmarkTraceContract
): void {
  if (trace.render_start !== null) {
    assertAtOrBefore(
      trace.fonts_wait_end,
      trace.render_start,
      "fonts_wait_end",
      "render_start"
    );
  }

  const points = [
    ["render_start", trace.render_start],
    ["safe_svg_ready", trace.safe_svg_ready],
    ["dom_inserted", trace.dom_inserted],
    ["layout_box_ready", trace.layout_box_ready],
    ["presentation_ready", trace.presentation_ready],
  ] as const;

  for (let index = 1; index < points.length; index += 1) {
    const [previousName, previousValue] = points[index - 1];
    const [name, value] = points[index];
    if (value !== null && previousValue === null) {
      throw new BenchmarkTraceError(
        `Benchmark ${name} requires ${previousName}.`
      );
    }
    if (
      value !== null &&
      previousValue !== null &&
      previousValue > value
    ) {
      throw new BenchmarkTraceError(
        `Benchmark ${previousName} must not occur after ${name}.`
      );
    }
  }

  if (contract.outcome === "success") {
    for (const [name, value] of points) {
      if (value === null) {
        throw new BenchmarkTraceError(
          `Successful benchmark trace requires ${name}.`
        );
      }
    }
  }
}

function assertPair(trace: BenchmarkRawTrace, pair: PairedEvent): void {
  const start = trace[pair.start];
  const end = trace[pair.end];
  if ((start === null) !== (end === null)) {
    throw new BenchmarkTraceError(
      `Benchmark ${pair.label} requires both endpoints or two null values.`
    );
  }
  if (start !== null && end !== null && start > end) {
    throw new BenchmarkTraceError(
      `Benchmark ${pair.start} must not occur after ${pair.end}.`
    );
  }
}

function assertRequiredPair(
  trace: BenchmarkRawTrace,
  pair: PairedEvent
): void {
  if (!isPairPresent(trace, pair)) {
    throw new BenchmarkTraceError(
      `Benchmark trace requires ${pair.label} for this sample.`
    );
  }
}

function assertForbiddenPair(
  trace: BenchmarkRawTrace,
  pair: PairedEvent
): void {
  if (isPairPresent(trace, pair)) {
    throw new BenchmarkTraceError(
      `Benchmark ${pair.label} is forbidden for this engine or mode.`
    );
  }
}

function assertOverlappingPairs(
  trace: BenchmarkRawTrace,
  first: PairedEvent,
  second: PairedEvent
): void {
  assertAtOrBefore(
    trace[first.start],
    trace[second.end],
    first.start,
    second.end
  );
  assertAtOrBefore(
    trace[second.start],
    trace[first.end],
    second.start,
    first.end
  );
}

function assertAtOrBefore(
  earlier: number | null,
  later: number | null,
  earlierName: string,
  laterName: string
): void {
  if (earlier === null || later === null) {
    throw new BenchmarkTraceError(
      `Benchmark ${laterName} requires ${earlierName}.`
    );
  }
  if (earlier > later) {
    throw new BenchmarkTraceError(
      `Benchmark ${earlierName} must not occur after ${laterName}.`
    );
  }
}

function assertDependency(
  present: boolean,
  event: string,
  dependency: string
): void {
  if (!present) {
    throw new BenchmarkTraceError(
      `Benchmark ${event} requires ${dependency}.`
    );
  }
}

function isPairPresent(trace: BenchmarkRawTrace, pair: PairedEvent): boolean {
  return trace[pair.start] !== null;
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
  if (keys.some((key) => typeof key !== "string")) {
    throw new BenchmarkTraceError(
      "Benchmark raw trace contains unexpected fields."
    );
  }
  const actual = (keys as string[]).sort();
  const expected = [...BENCHMARK_TRACE_EVENT_NAMES].sort();
  if (
    actual.length !== expected.length ||
    actual.some((key, index) => key !== expected[index])
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
    safe_svg_ready: null,
    dom_inserted: null,
    layout_box_ready: null,
    presentation_ready: null,
    sample_end: null,
  };
}

function freezeTrace(events: MutableEventVector): BenchmarkRawTrace {
  const sampleEnd = events.sample_end;
  if (sampleEnd === null) {
    throw new BenchmarkTraceError("Benchmark sample_end was not recorded.");
  }
  return Object.freeze({
    sample_start: 0,
    fonts_wait_start: events.fonts_wait_start,
    fonts_wait_end: events.fonts_wait_end,
    adapter_import_start: events.adapter_import_start,
    adapter_import_end: events.adapter_import_end,
    engine_import_start: events.engine_import_start,
    engine_import_end: events.engine_import_end,
    resource_acquire_start: events.resource_acquire_start,
    resource_acquire_end: events.resource_acquire_end,
    register_start: events.register_start,
    register_end: events.register_end,
    initialize_start: events.initialize_start,
    initialize_end: events.initialize_end,
    render_start: events.render_start,
    safe_svg_ready: events.safe_svg_ready,
    dom_inserted: events.dom_inserted,
    layout_box_ready: events.layout_box_ready,
    presentation_ready: events.presentation_ready,
    sample_end: sampleEnd,
  });
}
