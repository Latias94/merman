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
  "budgeted_svg_ready",
  "isolated_dom_inserted",
  "isolated_layout_box_ready",
  "isolated_presentation_ready",
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

export type BenchmarkFailureStage =
  | "environment"
  | "fonts"
  | "adapter-import"
  | "engine-import"
  | "resource-acquire"
  | "register"
  | "initialize"
  | "render"
  | "svg-budget"
  | "presentation"
  | "protocol"
  | "timeout"
  | "disposed";

export type BenchmarkTimedPhase = Exclude<
  BenchmarkFailureStage,
  "disposed" | "environment" | "protocol" | "svg-budget" | "timeout"
>;

export interface BenchmarkWatchdogTransition {
  readonly complete: BenchmarkTimedPhase | null;
  readonly start: BenchmarkTimedPhase | null;
}

export interface BenchmarkPhaseEventRule {
  readonly event: BenchmarkTraceMark;
  readonly label: string;
  readonly phase: BenchmarkTimedPhase | "svg-budget";
  readonly predecessors: readonly BenchmarkTraceMark[];
  readonly publicationBoundary: boolean;
  readonly watchdog: BenchmarkWatchdogTransition;
}

export interface BenchmarkPhaseSpan {
  readonly end: BenchmarkTraceMark;
  readonly label: string;
  readonly phase: BenchmarkTimedPhase;
  readonly start: BenchmarkTraceMark;
}

export interface BenchmarkPhaseBoundary {
  readonly end: BenchmarkTraceMark;
  readonly start: BenchmarkTraceMark;
}

export interface FrozenBenchmarkPhasePath {
  readonly applicableEvents: readonly BenchmarkTraceMark[];
  readonly canonicalSuccessEvents: readonly BenchmarkTraceMark[];
  readonly engine: BenchmarkEngine;
  readonly mode: BenchmarkSampleMode;
  readonly spans: readonly BenchmarkPhaseSpan[];
  readonly timedPhases: readonly BenchmarkTimedPhase[];
  assertNext(
    seen: ReadonlySet<BenchmarkTraceMark>,
    event: BenchmarkTraceMark
  ): void;
  assertSuccess(seen: ReadonlySet<BenchmarkTraceMark>): void;
  boundary(phase: BenchmarkTimedPhase): BenchmarkPhaseBoundary | null;
  dependsOn(
    event: BenchmarkTraceMark,
    predecessor: BenchmarkTraceMark
  ): boolean;
  rule(event: BenchmarkTraceMark): BenchmarkPhaseEventRule | null;
}

export class BenchmarkPhaseContractError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BenchmarkPhaseContractError";
  }
}

type RuleInput = Omit<BenchmarkPhaseEventRule, "event">;

const MARKS = BENCHMARK_TRACE_EVENT_NAMES.filter(
  (event): event is BenchmarkTraceMark =>
    event !== "sample_start" && event !== "sample_end"
);

const SPANS = Object.freeze([
  span("fonts", "fonts_wait_start", "fonts_wait_end", "fonts_wait"),
  span(
    "adapter-import",
    "adapter_import_start",
    "adapter_import_end",
    "adapter_import"
  ),
  span(
    "engine-import",
    "engine_import_start",
    "engine_import_end",
    "engine_import"
  ),
  span(
    "resource-acquire",
    "resource_acquire_start",
    "resource_acquire_end",
    "resource_acquire"
  ),
  span("register", "register_start", "register_end", "register"),
  span(
    "initialize",
    "initialize_start",
    "initialize_end",
    "initialize"
  ),
] as const satisfies readonly BenchmarkPhaseSpan[]);

const PATHS = Object.freeze({
  "realm-cold:merman": createPath("merman", "realm-cold"),
  "realm-cold:mermaid": createPath("mermaid", "realm-cold"),
  "warm:merman": createPath("merman", "warm"),
  "warm:mermaid": createPath("mermaid", "warm"),
});

export function benchmarkPhasePath(
  engine: BenchmarkEngine,
  mode: BenchmarkSampleMode
): FrozenBenchmarkPhasePath {
  const path = PATHS[`${mode}:${engine}`];
  if (!path) {
    throw new BenchmarkPhaseContractError(
      "Benchmark engine or sample mode is invalid."
    );
  }
  return path;
}

const FAILURE_STAGES = new Set<BenchmarkFailureStage>([
  "environment",
  "fonts",
  "adapter-import",
  "engine-import",
  "resource-acquire",
  "register",
  "initialize",
  "render",
  "svg-budget",
  "presentation",
  "protocol",
  "timeout",
  "disposed",
]);

export function isBenchmarkFailureStage(
  value: unknown
): value is BenchmarkFailureStage {
  return (
    typeof value === "string" &&
    FAILURE_STAGES.has(value as BenchmarkFailureStage)
  );
}

function createPath(
  engine: BenchmarkEngine,
  mode: BenchmarkSampleMode
): FrozenBenchmarkPhasePath {
  const rules: Partial<Record<BenchmarkTraceMark, BenchmarkPhaseEventRule>> = {};
  const add = (event: BenchmarkTraceMark, input: RuleInput) => {
    rules[event] = Object.freeze({
      ...input,
      event,
      predecessors: Object.freeze([...input.predecessors]),
      watchdog: Object.freeze({ ...input.watchdog }),
    });
  };
  const transition = (
    start: BenchmarkTimedPhase | null,
    complete: BenchmarkTimedPhase | null
  ): BenchmarkWatchdogTransition => ({ complete, start });

  add("fonts_wait_start", {
    label: "Waiting for fonts",
    phase: "fonts",
    predecessors: [],
    publicationBoundary: false,
    watchdog: transition("fonts", null),
  });
  if (mode === "realm-cold") {
    add("adapter_import_start", {
      label: "Loading benchmark adapter",
      phase: "adapter-import",
      predecessors: ["fonts_wait_start"],
      publicationBoundary: false,
      watchdog: transition("adapter-import", null),
    });
  }
  add("fonts_wait_end", {
    label: "Fonts ready",
    phase: "fonts",
    predecessors:
      mode === "realm-cold"
        ? ["fonts_wait_start", "adapter_import_start"]
        : ["fonts_wait_start"],
    publicationBoundary: false,
    watchdog: transition(null, "fonts"),
  });
  if (mode === "realm-cold") {
    add("adapter_import_end", {
      label: "Benchmark adapter ready",
      phase: "adapter-import",
      predecessors: ["adapter_import_start"],
      publicationBoundary: false,
      watchdog: transition(null, "adapter-import"),
    });
    add("engine_import_start", {
      label: "Loading engine",
      phase: "engine-import",
      predecessors: ["fonts_wait_end", "adapter_import_end"],
      publicationBoundary: false,
      watchdog: transition("engine-import", null),
    });
    if (engine === "merman") {
      add("resource_acquire_start", {
        label: "Acquiring WASM resource",
        phase: "resource-acquire",
        predecessors: ["engine_import_start"],
        publicationBoundary: false,
        watchdog: transition("resource-acquire", null),
      });
    }
    add("engine_import_end", {
      label: "Engine loaded",
      phase: "engine-import",
      predecessors:
        engine === "merman"
          ? ["engine_import_start", "resource_acquire_start"]
          : ["engine_import_start"],
      publicationBoundary: false,
      watchdog: transition(null, "engine-import"),
    });
    if (engine === "merman") {
      add("resource_acquire_end", {
        label: "WASM resource ready",
        phase: "resource-acquire",
        predecessors: ["engine_import_start", "resource_acquire_start"],
        publicationBoundary: false,
        watchdog: transition(null, "resource-acquire"),
      });
    } else {
      add("register_start", {
        label: "Registering Mermaid modules",
        phase: "register",
        predecessors: ["engine_import_end"],
        publicationBoundary: false,
        watchdog: transition("register", null),
      });
      add("register_end", {
        label: "Mermaid modules registered",
        phase: "register",
        predecessors: ["register_start"],
        publicationBoundary: false,
        watchdog: transition(null, "register"),
      });
    }
    add("initialize_start", {
      label: "Initializing engine",
      phase: "initialize",
      predecessors: [
        "fonts_wait_end",
        "adapter_import_end",
        "engine_import_end",
        engine === "merman" ? "resource_acquire_end" : "register_end",
      ],
      publicationBoundary: false,
      watchdog: transition("initialize", null),
    });
    add("initialize_end", {
      label: "Engine initialized",
      phase: "initialize",
      predecessors: ["initialize_start"],
      publicationBoundary: false,
      watchdog: transition(null, "initialize"),
    });
  }
  add("render_start", {
    label: "Rendering SVG",
    phase: "render",
    predecessors:
      mode === "realm-cold" ? ["initialize_end"] : ["fonts_wait_end"],
    publicationBoundary: false,
    watchdog: transition("render", null),
  });
  add("budgeted_svg_ready", {
    label: "Budgeted SVG ready",
    phase: "svg-budget",
    predecessors: ["render_start"],
    publicationBoundary: false,
    watchdog: transition("presentation", "render"),
  });
  add("isolated_dom_inserted", {
    label: "SVG inserted in isolated document",
    phase: "presentation",
    predecessors: ["budgeted_svg_ready"],
    publicationBoundary: false,
    watchdog: transition(null, null),
  });
  add("isolated_layout_box_ready", {
    label: "Isolated layout box ready",
    phase: "presentation",
    predecessors: ["isolated_dom_inserted"],
    publicationBoundary: false,
    watchdog: transition(null, null),
  });
  add("isolated_presentation_ready", {
    label: "Isolated presentation ready",
    phase: "presentation",
    predecessors: ["isolated_layout_box_ready"],
    publicationBoundary: true,
    watchdog: transition(null, "presentation"),
  });

  const eventRules = Object.freeze({ ...rules });
  const applicableEvents = Object.freeze(
    MARKS.filter((event) => eventRules[event] !== undefined)
  );
  const canonicalSuccessEvents = Object.freeze(
    canonicalOrder(engine, mode).filter(
      (event) => eventRules[event] !== undefined
    )
  );
  const spans = Object.freeze(
    SPANS.filter(
      (candidate) =>
        eventRules[candidate.start] !== undefined &&
        eventRules[candidate.end] !== undefined
    )
  );
  const boundaryParts = new Map<
    BenchmarkTimedPhase,
    { start?: BenchmarkTraceMark; end?: BenchmarkTraceMark }
  >();
  for (const event of applicableEvents) {
    const watchdog = eventRules[event]!.watchdog;
    if (watchdog.start) {
      const boundary = boundaryParts.get(watchdog.start) ?? {};
      boundary.start = event;
      boundaryParts.set(watchdog.start, boundary);
    }
    if (watchdog.complete) {
      const boundary = boundaryParts.get(watchdog.complete) ?? {};
      boundary.end = event;
      boundaryParts.set(watchdog.complete, boundary);
    }
  }
  const timedBoundaries = new Map<BenchmarkTimedPhase, BenchmarkPhaseBoundary>();
  for (const [phase, boundary] of boundaryParts) {
    if (!boundary.start || !boundary.end) {
      throw new BenchmarkPhaseContractError(
        `Benchmark phase ${phase} has an incomplete watchdog boundary.`
      );
    }
    timedBoundaries.set(
      phase,
      Object.freeze({ start: boundary.start, end: boundary.end })
    );
  }
  const timedPhases = Object.freeze([...timedBoundaries.keys()]);
  const dependencyClosure = new Map<BenchmarkTraceMark, ReadonlySet<BenchmarkTraceMark>>();
  const collectDependencies = (
    event: BenchmarkTraceMark,
    collected: Set<BenchmarkTraceMark>
  ) => {
    for (const predecessor of eventRules[event]?.predecessors ?? []) {
      if (collected.add(predecessor)) {
        collectDependencies(predecessor, collected);
      }
    }
  };
  for (const event of applicableEvents) {
    const collected = new Set<BenchmarkTraceMark>();
    collectDependencies(event, collected);
    dependencyClosure.set(event, collected);
  }

  return Object.freeze({
    applicableEvents,
    canonicalSuccessEvents,
    engine,
    mode,
    spans,
    timedPhases,
    assertNext(seen: ReadonlySet<BenchmarkTraceMark>, event: BenchmarkTraceMark) {
      const rule = eventRules[event];
      if (!rule) {
        throw new BenchmarkPhaseContractError(
          `Benchmark progress ${event} is forbidden for ${mode} ${engine}.`
        );
      }
      if (seen.has(event)) {
        throw new BenchmarkPhaseContractError(
          `Benchmark progress event ${event} was observed twice.`
        );
      }
      for (const predecessor of rule.predecessors) {
        if (!seen.has(predecessor)) {
          throw new BenchmarkPhaseContractError(
            `Benchmark progress ${event} requires ${predecessor}.`
          );
        }
      }
    },
    assertSuccess(seen: ReadonlySet<BenchmarkTraceMark>) {
      if (
        seen.size !== applicableEvents.length ||
        applicableEvents.some((event) => !seen.has(event))
      ) {
        throw new BenchmarkPhaseContractError(
          "Benchmark progress is incomplete."
        );
      }
    },
    boundary(phase: BenchmarkTimedPhase) {
      return timedBoundaries.get(phase) ?? null;
    },
    dependsOn(event: BenchmarkTraceMark, predecessor: BenchmarkTraceMark) {
      return dependencyClosure.get(event)?.has(predecessor) ?? false;
    },
    rule(event: BenchmarkTraceMark) {
      return eventRules[event] ?? null;
    },
  });
}

function canonicalOrder(
  engine: BenchmarkEngine,
  mode: BenchmarkSampleMode
): readonly BenchmarkTraceMark[] {
  if (mode === "warm") {
    return [
      "fonts_wait_start",
      "fonts_wait_end",
      "render_start",
      "budgeted_svg_ready",
      "isolated_dom_inserted",
      "isolated_layout_box_ready",
      "isolated_presentation_ready",
    ];
  }
  return engine === "merman"
    ? [
        "fonts_wait_start",
        "adapter_import_start",
        "adapter_import_end",
        "fonts_wait_end",
        "engine_import_start",
        "resource_acquire_start",
        "resource_acquire_end",
        "engine_import_end",
        "initialize_start",
        "initialize_end",
        "render_start",
        "budgeted_svg_ready",
        "isolated_dom_inserted",
        "isolated_layout_box_ready",
        "isolated_presentation_ready",
      ]
    : [
        "fonts_wait_start",
        "adapter_import_start",
        "adapter_import_end",
        "fonts_wait_end",
        "engine_import_start",
        "engine_import_end",
        "register_start",
        "register_end",
        "initialize_start",
        "initialize_end",
        "render_start",
        "budgeted_svg_ready",
        "isolated_dom_inserted",
        "isolated_layout_box_ready",
        "isolated_presentation_ready",
      ];
}

function span(
  phase: BenchmarkTimedPhase,
  start: BenchmarkTraceMark,
  end: BenchmarkTraceMark,
  label: string
): BenchmarkPhaseSpan {
  return Object.freeze({ end, label, phase, start });
}
