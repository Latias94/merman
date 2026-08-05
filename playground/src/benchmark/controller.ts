import { createStore, type StoreApi } from "zustand/vanilla";

import {
  BENCHMARK_PROTOCOL_VERSION,
} from "./protocol.ts";
import {
  BENCHMARK_TRACE_SCHEMA_VERSION,
  type BenchmarkEngine,
} from "./trace.ts";
import {
  benchmarkIntentMode,
  benchmarkIntentPurpose,
  createBenchmarkSamplePlan,
  isBenchmarkAggregationIntent,
  isBenchmarkInputBindingIntent,
  type BenchmarkSampleIntent,
  type BenchmarkSamplePlan,
  type BenchmarkSamplePurpose,
} from "./sample-plan.ts";
import {
  BENCHMARK_REPORT_SCHEMA_VERSION,
  buildBenchmarkReport,
  projectBenchmarkRealmSample,
  projectBenchmarkTransportFailure,
  rejectBenchmarkRecordedSample,
  type BenchmarkDetectionSnapshot,
  type BenchmarkEnvironment,
  type BenchmarkEnvironmentTransition,
  type BenchmarkFrozenInput,
  type BenchmarkRecordedFailureDetail,
  type BenchmarkRecordedSample,
  type BenchmarkReport,
  type BenchmarkTerminalStatus,
} from "./report.ts";
import type { BenchmarkDocumentLifecycle } from "./document-lifecycle.ts";
import type {
  BenchmarkSampleInput,
  BrowserBenchmarkRealmSession,
} from "./realm/controller.ts";
import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  validateCompareRenderPayload,
  type CompareRenderPayload,
  type RealmViewport,
} from "../runtime/realm/channel-protocol.ts";
import { projectError } from "../runtime/error-projection.ts";

interface BenchmarkRunRequestBase {
  readonly detection: BenchmarkDetectionSnapshot;
  readonly iterations: number;
  readonly payload: CompareRenderPayload;
  readonly seed?: number;
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
}

export type BenchmarkRunRequest = BenchmarkRunRequestBase &
  (
    | Readonly<{ mode: "realm-cold"; warmups?: 0 }>
    | Readonly<{ mode: "warm"; warmups: number }>
  );

export interface ValidatedBenchmarkRunRequest {
  readonly input: BenchmarkFrozenInput;
  readonly payload: CompareRenderPayload;
  readonly plan: BenchmarkSamplePlan;
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
}

export interface BenchmarkControllerDependencies {
  clearTimer(handle: unknown): void;
  createRealm(
    engine: BenchmarkEngine,
    viewport: RealmViewport,
    signal: AbortSignal
  ): Promise<BrowserBenchmarkRealmSession>;
  createSeed(): number;
  createToken(): string;
  dateNow(): number;
  getEnvironment(): BenchmarkEnvironment;
  readonly lifecycle: BenchmarkDocumentLifecycle;
  now(): number;
  pauseCoordinator(): Promise<() => void>;
  setTimer(callback: () => void, timeoutMs: number): unknown;
}

export interface BenchmarkProgress {
  readonly blockIndex: number | null;
  readonly completed: number;
  readonly engine: BenchmarkEngine | null;
  readonly purpose: BenchmarkSamplePurpose | null;
  readonly stage: "pausing" | "creating-realm" | "sampling";
  readonly total: number;
}

export interface BenchmarkRetainedReport {
  readonly report: BenchmarkReport;
  readonly stale: boolean;
}

export interface BenchmarkCancellationNotice {
  readonly report: BenchmarkReport;
}

export type BenchmarkControllerState =
  | {
      readonly cancellation: null;
      readonly report: null;
      readonly retained: null;
      readonly stale: false;
      readonly status: "idle";
    }
  | {
      readonly activeRunId: string;
      readonly cancellation: null;
      readonly progress: BenchmarkProgress;
      readonly report: null;
      readonly retained: BenchmarkRetainedReport | null;
      readonly stale: boolean;
      readonly status: "running";
    }
  | {
      readonly cancellation: BenchmarkCancellationNotice | null;
      readonly report: BenchmarkReport;
      readonly retained: BenchmarkRetainedReport;
      readonly stale: boolean;
      readonly status: BenchmarkTerminalStatus;
    };

export interface BenchmarkController {
  readonly store: Pick<
    StoreApi<BenchmarkControllerState>,
    "getInitialState" | "getState" | "subscribe"
  >;
  cancel(reason?: string): void;
  dispose(): void;
  markStale(): void;
  start(request: BenchmarkRunRequest): BenchmarkRunHandle;
}

export interface BenchmarkRunHandle {
  readonly completion: Promise<BenchmarkReport>;
  readonly runId: string;
}

interface ActiveBenchmarkRun {
  readonly abort: AbortController;
  readonly completion: PromiseWithResolvers<BenchmarkReport>;
  readonly environment: BenchmarkEnvironment;
  readonly inputId: string;
  readonly request: ValidatedBenchmarkRunRequest;
  readonly runId: string;
  readonly runToken: string;
  readonly samples: BenchmarkRecordedSample[];
  readonly sessions: Set<BrowserBenchmarkRealmSession>;
  readonly startedAt: string;
  readonly startedAtMs: number;
  readonly transitions: BenchmarkEnvironmentTransition[];
  listenerCleanup: (() => void) | null;
  releaseCoordinator: (() => void) | null;
  runTimer: unknown | null;
  settled: boolean;
}

const IDLE_STATE: BenchmarkControllerState = Object.freeze({
  cancellation: null,
  status: "idle",
  report: null,
  retained: null,
  stale: false,
});

export function validateBenchmarkRunRequest(
  request: BenchmarkRunRequest,
  generatedSeed: number
): ValidatedBenchmarkRunRequest {
  if (request.mode !== "realm-cold" && request.mode !== "warm") {
    throw new RealmProtocolError("Benchmark mode is invalid.");
  }
  if (
    request.mode === "realm-cold" &&
    request.warmups !== undefined &&
    request.warmups !== 0
  ) {
    throw new RealmProtocolError(
      "Fresh-runtime benchmark requests cannot contain warmups."
    );
  }
  const seed = request.seed ?? generatedSeed;
  const validatedPayload = validateCompareRenderPayload(request.payload);
  const payload = Object.freeze({
    ...validatedPayload,
    externalRequirements: Object.freeze(
      validatedPayload.externalRequirements
    ),
    viewport: Object.freeze(validatedPayload.viewport),
  });
  const detection = validateDetection(request.detection);
  const versions = Object.freeze({
    merman: validateVersion(request.versions.merman, "Merman"),
    mermaid: validateVersion(request.versions.mermaid, "Mermaid"),
  });
  const plan = createBenchmarkSamplePlan(
    request.mode === "warm"
      ? {
          iterations: request.iterations,
          mode: "warm",
          seed,
          warmups: request.warmups,
        }
      : {
          iterations: request.iterations,
          mode: "realm-cold",
          seed,
        }
  );
  return Object.freeze({
    payload,
    plan,
    versions,
    input: Object.freeze({
      ...payload,
      detection,
    }),
  });
}

export function createBenchmarkController(
  dependencies: BenchmarkControllerDependencies
): BenchmarkController {
  const store = createStore<BenchmarkControllerState>(() => IDLE_STATE);
  let active: ActiveBenchmarkRun | null = null;
  let disposed = false;
  let runSequence = 0;

  const replaceState = (state: BenchmarkControllerState) => {
    store.setState(state, true);
  };

  const settle = (
    run: ActiveBenchmarkRun,
    status: BenchmarkTerminalStatus,
    terminalError: BenchmarkRecordedFailureDetail | null
  ): BenchmarkReport | null => {
    if (run.settled) return null;
    run.settled = true;
    run.abort.abort();
    if (run.runTimer !== null) {
      dependencies.clearTimer(run.runTimer);
      run.runTimer = null;
    }
    for (const session of run.sessions) session.dispose();
    run.sessions.clear();
    run.listenerCleanup?.();
    run.listenerCleanup = null;
    run.releaseCoordinator?.();
    run.releaseCoordinator = null;

    const endedAtMs = dependencies.now();
    const endedAtWallMs = dependencies.dateNow();
    const report = buildBenchmarkReport(
      {
        schemaVersion: BENCHMARK_REPORT_SCHEMA_VERSION,
        protocols: {
          benchmark: BENCHMARK_PROTOCOL_VERSION,
          realm: REALM_PROTOCOL_VERSION,
          trace: BENCHMARK_TRACE_SCHEMA_VERSION,
        },
        run: {
          id: run.runId,
          startedAt: run.startedAt,
          endedAt: new Date(endedAtWallMs).toISOString(),
          durationMs: Math.max(0, endedAtMs - run.startedAtMs),
        },
        input: run.request.input,
        plan: run.request.plan,
        versions: {
          expected: run.request.versions,
          observed: observedVersions(run.samples),
        },
        environment: run.environment,
        transitions: Object.freeze([...run.transitions]),
        samples: Object.freeze([...run.samples]),
        terminalError,
      },
      status
    );
    const runningState = store.getState();
    if (runningState.status !== "running") {
      throw new RealmProtocolError("Benchmark run state was lost before settlement.");
    }
    const stale = runningState.stale;
    const previousRetained = runningState.retained;
    if (active === run) active = null;
    const retained =
      status === "cancelled" && previousRetained
        ? previousRetained
        : createRetainedReport(report, stale);
    replaceState({
      cancellation:
        status === "cancelled" && previousRetained
          ? Object.freeze({ report })
          : null,
      report,
      retained,
      stale: retained.stale,
      status,
    });
    run.completion.resolve(report);
    return report;
  };

  const invalidate = (
    run: ActiveBenchmarkRun,
    transition: BenchmarkEnvironmentTransition
  ) => {
    if (run.settled) return;
    run.transitions.push(Object.freeze(transition));
    settle(
      run,
      "invalidated",
      transportFailure(
        "environment",
        `Benchmark environment changed during ${transition.kind}.`
      )
    );
  };

  const installLifecycleListeners = (run: ActiveBenchmarkRun): (() => void) =>
    dependencies.lifecycle.subscribe((signal) => {
      if (signal.kind === "resume" || signal.kind === "pageshow") return;
      invalidate(run, {
        ...signal,
        atMs: elapsed(run, dependencies.now()),
      });
    });

  const createSession = async (
    run: ActiveBenchmarkRun,
    engine: BenchmarkEngine
  ): Promise<BrowserBenchmarkRealmSession> => {
    if (run.sessions.size >= run.request.plan.budget.maxLiveRealms) {
      throw new RealmProtocolError(
        "Benchmark controller exceeded its live realm budget."
      );
    }
    const session = await dependencies.createRealm(
      engine,
      run.request.input.viewport,
      run.abort.signal
    );
    if (run.settled) {
      session.dispose();
      throw new RunAlreadySettledError();
    }
    run.sessions.add(session);
    return session;
  };

  const disposeSession = (
    run: ActiveBenchmarkRun,
    session: BrowserBenchmarkRealmSession
  ) => {
    session.dispose();
    run.sessions.delete(session);
  };

  const sampleInput = (
    run: ActiveBenchmarkRun,
    intent: BenchmarkSampleIntent
  ): BenchmarkSampleInput => {
    const identity = {
      runId: run.runId,
      runToken: run.runToken,
      inputId: run.inputId,
      engine: intent.engine,
      sampleId: intent.sampleId,
    } as const;
    return isBenchmarkInputBindingIntent(intent)
      ? Object.freeze({
          ...identity,
          intentKind: intent.kind,
          payload: run.request.payload,
        })
      : Object.freeze({ ...identity, intentKind: intent.kind });
  };

  const transportIdentity = (
    run: ActiveBenchmarkRun,
    intent: BenchmarkSampleIntent
  ) =>
    Object.freeze({
      requestId: intent.sampleId,
      runId: run.runId,
    });

  const recordTransportFailure = (
    run: ActiveBenchmarkRun,
    intent: BenchmarkSampleIntent,
    error: unknown,
    stage: string,
    realmCreation = null as BrowserBenchmarkRealmSession["creationEvidence"] | null
  ) => {
    run.samples.push(
      projectBenchmarkTransportFailure(
        intent,
        transportIdentity(run, intent),
        error,
        stage,
        realmCreation
      )
    );
  };

  const sample = async (
    run: ActiveBenchmarkRun,
    session: BrowserBenchmarkRealmSession,
    input: BenchmarkSampleInput,
    intent: BenchmarkSampleIntent
  ): Promise<boolean> => {
    if (dependencies.lifecycle.getVisibilityState() !== "visible") {
      invalidate(run, {
        atMs: elapsed(run, dependencies.now()),
        kind: "visibility-hidden",
        visibilityState: dependencies.lifecycle.getVisibilityState(),
      });
      return false;
    }
    updateProgress(
      run,
      "sampling",
      intent.engine,
      benchmarkIntentPurpose(intent),
      isBenchmarkAggregationIntent(intent) ? intent.blockIndex : null
    );
    try {
      const result = await session.sample(input);
      if (run.settled) return false;
      const realmCreation =
        benchmarkIntentMode(intent) === "realm-cold"
          ? session.creationEvidence
          : null;
      let projected = projectBenchmarkRealmSample(
        intent,
        result,
        realmCreation
      );
      if (
        projected.outcome === "success" &&
        projected.version !== run.request.versions[intent.engine]
      ) {
        projected = rejectBenchmarkRecordedSample(
          projected,
          transportFailure(
            "version",
            `${engineLabel(intent.engine)} returned ${projected.version}; expected ${run.request.versions[intent.engine]}.`
          )
        );
      }
      run.samples.push(projected);
      updateProgress(
        run,
        "sampling",
        intent.engine,
        benchmarkIntentPurpose(intent),
        isBenchmarkAggregationIntent(intent) ? intent.blockIndex : null
      );
      return projected.outcome === "success";
    } catch (error) {
      if (run.settled || error instanceof RunAlreadySettledError) return false;
      recordTransportFailure(
        run,
        intent,
        error,
        "transport",
        benchmarkIntentMode(intent) === "realm-cold"
          ? session.creationEvidence
          : null
      );
      return false;
    }
  };

  const executePlan = async (run: ActiveBenchmarkRun) => {
    const sessions = new Map<string, BrowserBenchmarkRealmSession>();
    for (const intent of run.request.plan.samples) {
      if (run.settled) return;
      const input = sampleInput(run, intent);
      let session = sessions.get(intent.sessionId) ?? null;
      if (intent.session !== "reuse") {
        updateProgress(
          run,
          "creating-realm",
          intent.engine,
          benchmarkIntentPurpose(intent),
          isBenchmarkAggregationIntent(intent) ? intent.blockIndex : null
        );
        try {
          session = await createSession(run, intent.engine);
        } catch (error) {
          if (!run.settled && !(error instanceof RunAlreadySettledError)) {
            recordTransportFailure(run, intent, error, "realm-create");
          }
          if (intent.session !== "single-use") return;
          continue;
        }
        if (intent.session === "open-reused") {
          sessions.set(intent.sessionId, session);
        }
      }
      if (!session) {
        throw new RealmProtocolError(
          `Benchmark session ${intent.sessionId} is missing.`
        );
      }

      const succeeded = await sample(run, session, input, intent);
      if (intent.session === "single-use") {
        disposeSession(run, session);
      } else if (!succeeded) {
        return;
      }
    }
  };

  const execute = async (run: ActiveBenchmarkRun) => {
    try {
      const release = once(await dependencies.pauseCoordinator());
      if (run.settled) {
        release();
        return;
      }
      run.releaseCoordinator = release;
      if (dependencies.lifecycle.getVisibilityState() !== "visible") {
        invalidate(run, {
          atMs: elapsed(run, dependencies.now()),
          kind: "visibility-hidden",
          visibilityState: dependencies.lifecycle.getVisibilityState(),
        });
        return;
      }
      await executePlan(run);
      if (run.settled) return;

      const hasErrors =
        run.samples.some((candidate) => candidate.outcome === "failure") ||
        run.samples.length !== run.request.plan.samples.length;
      settle(
        run,
        hasErrors ? "complete-with-errors" : "success",
        null
      );
    } catch (error) {
      if (run.settled) return;
      settle(
        run,
        "failed",
        transportFailure(
          run.releaseCoordinator ? "controller" : "coordinator-pause",
          error
        )
      );
    }
  };

  const start = (request: BenchmarkRunRequest): BenchmarkRunHandle => {
    if (disposed) {
      throw new RealmProtocolError("Benchmark controller is disposed.");
    }
    if (active) {
      throw new RealmProtocolError("A benchmark run is already active.");
    }
    if (dependencies.lifecycle.getVisibilityState() !== "visible") {
      throw new RealmProtocolError(
        "Benchmark cannot start while the document is hidden."
      );
    }

    const validated = validateBenchmarkRunRequest(
      request,
      request.seed ?? dependencies.createSeed()
    );

    runSequence += 1;
    const startedAtMs = dependencies.now();
    const startedAtWallMs = dependencies.dateNow();
    const total = validated.plan.budget.totalSamples;
    const current: ActiveBenchmarkRun = {
      abort: new AbortController(),
      completion: Promise.withResolvers<BenchmarkReport>(),
      environment: Object.freeze({ ...dependencies.getEnvironment() }),
      inputId: `input-${runSequence}-${validated.plan.seed.toString(16)}`,
      request: validated,
      runId: `run-${runSequence}-${validated.plan.seed.toString(16)}`,
      runToken: dependencies.createToken(),
      samples: [],
      sessions: new Set(),
      startedAt: new Date(startedAtWallMs).toISOString(),
      startedAtMs,
      transitions: [
        Object.freeze({
          atMs: 0,
          kind: "start",
          visibilityState: "visible",
        }),
      ],
      listenerCleanup: null,
      releaseCoordinator: null,
      runTimer: null,
      settled: false,
    };
    active = current;
    current.listenerCleanup = installLifecycleListeners(current);
    current.runTimer = dependencies.setTimer(() => {
      settle(
        current,
        "failed",
        transportFailure("timeout", "Benchmark controller run timed out.")
      );
    }, REALM_BUDGETS.runTimeoutMs);
    const previousState = store.getState();
    const retained =
      previousState.status === "idle" ? null : previousState.retained;
    replaceState({
      activeRunId: current.runId,
      cancellation: null,
      status: "running",
      report: null,
      retained,
      stale: false,
      progress: {
        stage: "pausing",
        completed: 0,
        total,
        engine: null,
        purpose: null,
        blockIndex: null,
      },
    });
    void execute(current);
    return Object.freeze({
      completion: current.completion.promise,
      runId: current.runId,
    });
  };

  return {
    store,
    start,
    cancel(reason = "user") {
      if (!active) return;
      settle(
        active,
        "cancelled",
        transportFailure("cancelled", `Benchmark was cancelled (${reason}).`)
      );
    },
    markStale() {
      const state = store.getState();
      if (state.status === "idle" || state.stale) return;
      if (state.status === "running") {
        replaceState({
          ...state,
          stale: true,
        });
        return;
      }
      replaceState({
        ...state,
        retained: Object.freeze({ ...state.retained, stale: true }),
        stale: true,
      });
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      if (active) {
        settle(
          active,
          "cancelled",
          transportFailure("disposed", "Benchmark controller was disposed.")
        );
      }
    },
  };

  function updateProgress(
    run: ActiveBenchmarkRun,
    stage: BenchmarkProgress["stage"],
    engine: BenchmarkEngine | null,
    purpose: BenchmarkSamplePurpose | null,
    blockIndex: number | null
  ) {
    if (run.settled || active !== run) return;
    const state = store.getState();
    if (state.status !== "running") return;
    replaceState({
      activeRunId: state.activeRunId,
      cancellation: null,
      status: "running",
      report: null,
      retained: state.retained,
      stale: state.stale,
      progress: {
        stage,
        completed: run.samples.length,
        total: run.request.plan.budget.totalSamples,
        engine,
        purpose,
        blockIndex,
      },
    });
  }
}

function createRetainedReport(
  report: BenchmarkReport,
  stale: boolean,
): BenchmarkRetainedReport {
  return Object.freeze({ report, stale });
}

function validateDetection(
  detection: BenchmarkDetectionSnapshot
): BenchmarkDetectionSnapshot {
  if (detection.status !== "available" && detection.status !== "unavailable") {
    throw new RealmProtocolError("Benchmark detection status is invalid.");
  }
  if (
    detection.validity !== "valid" &&
    detection.validity !== "recoverable-invalid" &&
    detection.validity !== "unknown"
  ) {
    throw new RealmProtocolError("Benchmark detection validity is invalid.");
  }
  const values = [
    detection.diagramType,
    detection.syntaxId,
    detection.effectiveLayoutId,
  ];
  if (
    values.some(
      (value) =>
        value !== null &&
        (typeof value !== "string" || value.length === 0 || value.length > 256)
    )
  ) {
    throw new RealmProtocolError("Benchmark detection facts are invalid.");
  }
  if (
    detection.status === "available" &&
    (values.some((value) => value === null) || detection.validity === "unknown")
  ) {
    throw new RealmProtocolError(
      "Available benchmark detection facts must be complete."
    );
  }
  if (
    detection.status === "unavailable" &&
    (values.some((value) => value !== null) || detection.validity !== "unknown")
  ) {
    throw new RealmProtocolError(
      "Unavailable benchmark detection facts cannot retain stale values."
    );
  }
  return Object.freeze({ ...detection });
}

function validateVersion(value: string, engine: string): string {
  if (typeof value !== "string" || value.length === 0 || value.length > 256) {
    throw new RealmProtocolError(`${engine} benchmark version is invalid.`);
  }
  return value;
}

function observedVersions(
  samples: readonly BenchmarkRecordedSample[]
): Readonly<Record<BenchmarkEngine, readonly string[]>> {
  const observed: Record<BenchmarkEngine, Set<string>> = {
    merman: new Set(),
    mermaid: new Set(),
  };
  for (const sample of samples) {
    if (sample.version) observed[sample.engine].add(sample.version);
  }
  return Object.freeze({
    merman: Object.freeze([...observed.merman].sort()),
    mermaid: Object.freeze([...observed.mermaid].sort()),
  });
}

function elapsed(run: ActiveBenchmarkRun, now: number): number {
  return Math.max(0, now - run.startedAtMs);
}

function transportFailure(
  stage: string,
  error: unknown
): BenchmarkRecordedFailureDetail {
  const projection = projectError(error);
  return {
    detail: projection.detail,
    kind: "transport",
    message: projection.summary,
    stage,
  };
}

function engineLabel(engine: BenchmarkEngine): string {
  return engine === "merman" ? "Merman" : "Mermaid";
}

function once(callback: () => void): () => void {
  let called = false;
  return () => {
    if (called) return;
    called = true;
    callback();
  };
}

class RunAlreadySettledError extends Error {
  constructor() {
    super("Benchmark run is already settled.");
    this.name = "RunAlreadySettledError";
  }
}
