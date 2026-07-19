import { createStore, type StoreApi } from "zustand/vanilla";

import {
  BENCHMARK_PROTOCOL_VERSION,
  type BenchmarkSampleRole,
} from "./protocol.ts";
import {
  BENCHMARK_TRACE_SCHEMA_VERSION,
  type BenchmarkEngine,
  type BenchmarkSampleMode,
} from "./trace.ts";
import {
  createBalancedBenchmarkSchedule,
  type BalancedBenchmarkSchedule,
} from "./schedule.ts";
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
  type BenchmarkSampleMetadata,
  type BenchmarkSamplePurpose,
  type BenchmarkTerminalStatus,
} from "./report.ts";
import type {
  BenchmarkSampleInput,
  BrowserBenchmarkRealmSession,
} from "./realm/controller.ts";
import {
  BENCHMARK_BUDGETS,
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  validateCompareRenderPayload,
  type CompareRenderPayload,
  type RealmViewport,
} from "../runtime/realm/channel-protocol.ts";

export interface BenchmarkRunRequest {
  readonly detection: BenchmarkDetectionSnapshot;
  readonly iterations: number;
  readonly mode: BenchmarkSampleMode;
  readonly payload: CompareRenderPayload;
  readonly seed?: number;
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
  readonly warmups: number;
}

export interface ValidatedBenchmarkRunRequest {
  readonly input: BenchmarkFrozenInput;
  readonly iterations: number;
  readonly mode: BenchmarkSampleMode;
  readonly schedule: BalancedBenchmarkSchedule;
  readonly seed: number;
  readonly versions: Readonly<Record<BenchmarkEngine, string>>;
  readonly warmups: number;
}

export interface BenchmarkLifecycleTarget {
  addEventListener(type: string, listener: (event: unknown) => void): void;
  removeEventListener(type: string, listener: (event: unknown) => void): void;
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
  readonly documentTarget: BenchmarkLifecycleTarget;
  getEnvironment(): BenchmarkEnvironment;
  getVisibilityState(): string;
  now(): number;
  pauseCoordinator(): Promise<() => void>;
  setTimer(callback: () => void, timeoutMs: number): unknown;
  readonly windowTarget: BenchmarkLifecycleTarget;
}

export interface BenchmarkProgress {
  readonly blockIndex: number | null;
  readonly completed: number;
  readonly engine: BenchmarkEngine | null;
  readonly purpose: BenchmarkSamplePurpose | null;
  readonly stage: "pausing" | "creating-realm" | "sampling" | "cleanup";
  readonly total: number;
}

export type BenchmarkControllerState =
  | {
      readonly report: null;
      readonly stale: false;
      readonly status: "idle";
    }
  | {
      readonly progress: BenchmarkProgress;
      readonly report: null;
      readonly stale: boolean;
      readonly status: "running";
    }
  | {
      readonly report: BenchmarkReport;
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
  run(request: BenchmarkRunRequest): Promise<BenchmarkReport>;
}

interface ActiveBenchmarkRun {
  readonly abort: AbortController;
  readonly completion: PromiseWithResolvers<BenchmarkReport>;
  readonly environment: BenchmarkEnvironment;
  readonly request: ValidatedBenchmarkRunRequest;
  readonly runId: string;
  readonly runToken: string;
  readonly samples: BenchmarkRecordedSample[];
  readonly sessions: Set<BrowserBenchmarkRealmSession>;
  readonly startedAt: string;
  readonly startedAtMs: number;
  readonly transitions: BenchmarkEnvironmentTransition[];
  readonly total: number;
  completed: number;
  listenerCleanup: (() => void) | null;
  releaseCoordinator: (() => void) | null;
  runTimer: unknown | null;
  requestSequence: number;
  settled: boolean;
}

const IDLE_STATE: BenchmarkControllerState = Object.freeze({
  status: "idle",
  report: null,
  stale: false,
});

export function validateBenchmarkRunRequest(
  request: BenchmarkRunRequest,
  generatedSeed: number
): ValidatedBenchmarkRunRequest {
  const seed = request.seed ?? generatedSeed;
  const schedule = createBalancedBenchmarkSchedule(request.iterations, seed);
  if (!Number.isSafeInteger(request.warmups) || request.warmups < 0) {
    throw new RealmProtocolError(
      "Benchmark warmups must be a nonnegative integer."
    );
  }
  if (request.mode !== "realm-cold" && request.mode !== "warm") {
    throw new RealmProtocolError("Benchmark mode is invalid.");
  }
  if (request.mode === "realm-cold" && request.warmups !== 0) {
    throw new RealmProtocolError(
      "Realm-cold benchmark runs cannot contain warm samples."
    );
  }
  if (
    request.mode === "warm" &&
    request.warmups + 1 > BENCHMARK_BUDGETS.maxWarmups
  ) {
    throw new RealmProtocolError(
      "Benchmark warmups exceed the per-realm protocol budget."
    );
  }
  const retainedSamples =
    request.mode === "realm-cold"
      ? request.iterations * 2
      : (request.iterations + request.warmups + 1) * 2;
  if (retainedSamples > BENCHMARK_BUDGETS.maxRetainedSamples) {
    throw new RealmProtocolError(
      "Benchmark run exceeds the retained sample budget."
    );
  }

  const payload = validateCompareRenderPayload(request.payload);
  const detection = validateDetection(request.detection);
  const versions = Object.freeze({
    merman: validateVersion(request.versions.merman, "Merman"),
    mermaid: validateVersion(request.versions.mermaid, "Mermaid"),
  });
  return Object.freeze({
    mode: request.mode,
    iterations: request.iterations,
    warmups: request.warmups,
    seed,
    schedule,
    versions,
    input: Object.freeze({
      ...payload,
      externalRequirements: Object.freeze(payload.externalRequirements),
      viewport: Object.freeze(payload.viewport),
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
    updateProgress(run, "cleanup", null, null, null);
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
          seed: run.request.seed,
          mode: run.request.mode,
          iterations: run.request.iterations,
          warmups: run.request.warmups,
          startedAt: run.startedAt,
          endedAt: new Date(endedAtWallMs).toISOString(),
          durationMs: Math.max(0, endedAtMs - run.startedAtMs),
        },
        input: run.request.input,
        schedule: run.request.schedule,
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
    const stale = store.getState().stale;
    if (active === run) active = null;
    replaceState({ status, report, stale });
    run.completion.resolve(report);
    return report;
  };

  const invalidate = (
    run: ActiveBenchmarkRun,
    transition: BenchmarkEnvironmentTransition
  ) => {
    if (run.settled) return;
    run.transitions.push(Object.freeze(transition));
    settle(run, "invalidated", {
      kind: "transport",
      stage: "environment",
      message: `Benchmark environment changed during ${transition.kind}.`,
    });
  };

  const installLifecycleListeners = (run: ActiveBenchmarkRun): (() => void) => {
    const onVisibilityChange = () => {
      const visibilityState = dependencies.getVisibilityState();
      if (visibilityState === "visible") return;
      invalidate(run, {
        atMs: elapsed(run, dependencies.now()),
        kind: "visibility-hidden",
        visibilityState,
      });
    };
    const onPageHide = (event: unknown) => {
      invalidate(run, {
        atMs: elapsed(run, dependencies.now()),
        kind: "pagehide",
        persisted: readPersisted(event),
        visibilityState: dependencies.getVisibilityState(),
      });
    };
    const onFreeze = () => {
      invalidate(run, {
        atMs: elapsed(run, dependencies.now()),
        kind: "freeze",
        visibilityState: dependencies.getVisibilityState(),
      });
    };
    dependencies.documentTarget.addEventListener(
      "visibilitychange",
      onVisibilityChange
    );
    dependencies.documentTarget.addEventListener("freeze", onFreeze);
    dependencies.windowTarget.addEventListener("pagehide", onPageHide);
    return () => {
      dependencies.documentTarget.removeEventListener(
        "visibilitychange",
        onVisibilityChange
      );
      dependencies.documentTarget.removeEventListener("freeze", onFreeze);
      dependencies.windowTarget.removeEventListener("pagehide", onPageHide);
    };
  };

  const createSession = async (
    run: ActiveBenchmarkRun,
    engine: BenchmarkEngine
  ): Promise<BrowserBenchmarkRealmSession> => {
    if (run.sessions.size >= BENCHMARK_BUDGETS.maxLiveRealms) {
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

  const nextSample = (
    run: ActiveBenchmarkRun,
    engine: BenchmarkEngine,
    mode: BenchmarkSampleMode,
    role: BenchmarkSampleRole
  ): BenchmarkSampleInput => {
    run.requestSequence += 1;
    const payload: CompareRenderPayload = {
      source: run.request.input.source,
      configJson: run.request.input.configJson,
      theme: run.request.input.theme,
      diagramFont: run.request.input.diagramFont,
      externalRequirements: run.request.input.externalRequirements,
      viewport: run.request.input.viewport,
    };
    return Object.freeze({
      runId: run.runId,
      runToken: run.runToken,
      requestId: `${run.runId}-sample-${run.requestSequence}`,
      engine,
      mode,
      role,
      payload,
    });
  };

  const recordTransportFailure = (
    run: ActiveBenchmarkRun,
    metadata: BenchmarkSampleMetadata,
    input: BenchmarkSampleInput,
    error: unknown,
    stage: string,
    realmCreation = null as BrowserBenchmarkRealmSession["creationEvidence"] | null
  ) => {
    run.samples.push(
      projectBenchmarkTransportFailure(
        metadata,
        input,
        error,
        stage,
        realmCreation
      )
    );
    run.completed += 1;
  };

  const sample = async (
    run: ActiveBenchmarkRun,
    session: BrowserBenchmarkRealmSession,
    input: BenchmarkSampleInput,
    metadata: BenchmarkSampleMetadata
  ): Promise<boolean> => {
    if (dependencies.getVisibilityState() !== "visible") {
      invalidate(run, {
        atMs: elapsed(run, dependencies.now()),
        kind: "visibility-hidden",
        visibilityState: dependencies.getVisibilityState(),
      });
      return false;
    }
    updateProgress(
      run,
      "sampling",
      input.engine,
      metadata.purpose,
      metadata.blockIndex
    );
    try {
      const result = await session.sample(input);
      if (run.settled) return false;
      const realmCreation =
        input.mode === "realm-cold" ? session.creationEvidence : null;
      let projected = projectBenchmarkRealmSample(
        metadata,
        result,
        realmCreation
      );
      if (
        projected.outcome === "success" &&
        projected.version !== run.request.versions[input.engine]
      ) {
        projected = rejectBenchmarkRecordedSample(projected, {
          kind: "transport",
          stage: "version",
          message: `${engineLabel(input.engine)} returned ${projected.version}; expected ${run.request.versions[input.engine]}.`,
        });
      }
      run.samples.push(projected);
      run.completed += 1;
      updateProgress(
        run,
        "sampling",
        input.engine,
        metadata.purpose,
        metadata.blockIndex
      );
      return projected.outcome === "success";
    } catch (error) {
      if (run.settled || error instanceof RunAlreadySettledError) return false;
      recordTransportFailure(
        run,
        metadata,
        input,
        error,
        "transport",
        input.mode === "realm-cold" ? session.creationEvidence : null
      );
      return false;
    }
  };

  const coldAttempt = async (
    run: ActiveBenchmarkRun,
    engine: BenchmarkEngine,
    metadata: BenchmarkSampleMetadata
  ) => {
    const input = nextSample(run, engine, "realm-cold", "measured");
    updateProgress(
      run,
      "creating-realm",
      engine,
      metadata.purpose,
      metadata.blockIndex
    );
    let session: BrowserBenchmarkRealmSession | null = null;
    try {
      session = await createSession(run, engine);
      await sample(run, session, input, metadata);
    } catch (error) {
      if (!run.settled && !(error instanceof RunAlreadySettledError)) {
        recordTransportFailure(run, metadata, input, error, "realm-create");
      }
    } finally {
      if (session) disposeSession(run, session);
    }
  };

  const executeCold = async (run: ActiveBenchmarkRun) => {
    for (const block of run.request.schedule.blocks) {
      for (let orderIndex = 0; orderIndex < block.order.length; orderIndex += 1) {
        if (run.settled) return;
        await coldAttempt(run, block.order[orderIndex], {
          blockIndex: block.index,
          orderIndex,
          purpose: "measured",
        });
      }
    }
  };

  const executeWarm = async (run: ActiveBenchmarkRun) => {
    const sessions = new Map<BenchmarkEngine, BrowserBenchmarkRealmSession>();
    const initializationOrder = run.request.schedule.blocks[0].order;
    for (let orderIndex = 0; orderIndex < initializationOrder.length; orderIndex += 1) {
      if (run.settled) return;
      const engine = initializationOrder[orderIndex];
      const metadata: BenchmarkSampleMetadata = {
        blockIndex: null,
        orderIndex,
        purpose: "setup",
      };
      const input = nextSample(run, engine, "realm-cold", "warmup");
      updateProgress(run, "creating-realm", engine, "setup", null);
      let session: BrowserBenchmarkRealmSession;
      try {
        session = await createSession(run, engine);
      } catch (error) {
        if (!run.settled && !(error instanceof RunAlreadySettledError)) {
          recordTransportFailure(run, metadata, input, error, "realm-create");
        }
        return;
      }
      sessions.set(engine, session);
      if (!(await sample(run, session, input, metadata))) return;
    }

    for (let round = 0; round < run.request.warmups; round += 1) {
      const order = run.request.schedule.blocks[round % run.request.schedule.blocks.length]
        .order;
      for (let orderIndex = 0; orderIndex < order.length; orderIndex += 1) {
        if (run.settled) return;
        const engine = order[orderIndex];
        const session = sessions.get(engine);
        if (!session) throw new RealmProtocolError("Warm benchmark realm is missing.");
        const metadata: BenchmarkSampleMetadata = {
          blockIndex: null,
          orderIndex,
          purpose: "warmup",
        };
        const input = nextSample(run, engine, "warm", "warmup");
        if (!(await sample(run, session, input, metadata))) return;
      }
    }

    for (const block of run.request.schedule.blocks) {
      for (let orderIndex = 0; orderIndex < block.order.length; orderIndex += 1) {
        if (run.settled) return;
        const engine = block.order[orderIndex];
        const session = sessions.get(engine);
        if (!session) throw new RealmProtocolError("Warm benchmark realm is missing.");
        const metadata: BenchmarkSampleMetadata = {
          blockIndex: block.index,
          orderIndex,
          purpose: "measured",
        };
        const input = nextSample(run, engine, "warm", "measured");
        if (!(await sample(run, session, input, metadata))) return;
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
      if (dependencies.getVisibilityState() !== "visible") {
        invalidate(run, {
          atMs: elapsed(run, dependencies.now()),
          kind: "visibility-hidden",
          visibilityState: dependencies.getVisibilityState(),
        });
        return;
      }
      if (run.request.mode === "realm-cold") {
        await executeCold(run);
      } else {
        await executeWarm(run);
      }
      if (run.settled) return;

      const measuredSuccesses = run.samples.filter(
        (candidate) =>
          candidate.purpose === "measured" && candidate.outcome === "success"
      ).length;
      const hasErrors =
        run.samples.some((candidate) => candidate.outcome === "failure") ||
        measuredSuccesses !== run.request.iterations * 2;
      settle(
        run,
        hasErrors ? "complete-with-errors" : "success",
        null
      );
    } catch (error) {
      if (run.settled) return;
      settle(run, "failed", {
        kind: "transport",
        stage: run.releaseCoordinator ? "controller" : "coordinator-pause",
        message: boundedErrorMessage(error),
      });
    }
  };

  const run = (request: BenchmarkRunRequest): Promise<BenchmarkReport> => {
    if (disposed) {
      return Promise.reject(new RealmProtocolError("Benchmark controller is disposed."));
    }
    if (active) {
      return Promise.reject(
        new RealmProtocolError("A benchmark run is already active.")
      );
    }
    if (dependencies.getVisibilityState() !== "visible") {
      return Promise.reject(
        new RealmProtocolError(
          "Benchmark cannot start while the document is hidden."
        )
      );
    }

    let validated: ValidatedBenchmarkRunRequest;
    try {
      validated = validateBenchmarkRunRequest(
        request,
        request.seed ?? dependencies.createSeed()
      );
    } catch (error) {
      return Promise.reject(error);
    }

    runSequence += 1;
    const startedAtMs = dependencies.now();
    const startedAtWallMs = dependencies.dateNow();
    const total =
      validated.mode === "realm-cold"
        ? validated.iterations * 2
        : (validated.iterations + validated.warmups + 1) * 2;
    const current: ActiveBenchmarkRun = {
      abort: new AbortController(),
      completion: Promise.withResolvers<BenchmarkReport>(),
      environment: Object.freeze({ ...dependencies.getEnvironment() }),
      request: validated,
      runId: `run-${runSequence}-${validated.seed.toString(16)}`,
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
      total,
      completed: 0,
      listenerCleanup: null,
      releaseCoordinator: null,
      runTimer: null,
      requestSequence: 0,
      settled: false,
    };
    active = current;
    current.listenerCleanup = installLifecycleListeners(current);
    current.runTimer = dependencies.setTimer(() => {
      settle(current, "failed", {
        kind: "transport",
        stage: "timeout",
        message: "Benchmark controller run timed out.",
      });
    }, REALM_BUDGETS.runTimeoutMs);
    replaceState({
      status: "running",
      report: null,
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
    return current.completion.promise;
  };

  return {
    store,
    run,
    cancel(reason = "user") {
      if (!active) return;
      settle(active, "cancelled", {
        kind: "transport",
        stage: "cancelled",
        message: `Benchmark was cancelled (${reason}).`,
      });
    },
    markStale() {
      const state = store.getState();
      if (state.status === "idle" || state.stale) return;
      replaceState({ ...state, stale: true });
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      if (active) {
        settle(active, "cancelled", {
          kind: "transport",
          stage: "disposed",
          message: "Benchmark controller was disposed.",
        });
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
    const stale = store.getState().stale;
    replaceState({
      status: "running",
      report: null,
      stale,
      progress: {
        stage,
        completed: run.completed,
        total: run.total,
        engine,
        purpose,
        blockIndex,
      },
    });
  }
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

function readPersisted(event: unknown): boolean {
  return Boolean(
    event &&
      typeof event === "object" &&
      "persisted" in event &&
      (event as { persisted?: unknown }).persisted === true
  );
}

function elapsed(run: ActiveBenchmarkRun, now: number): number {
  return Math.max(0, now - run.startedAtMs);
}

function boundedErrorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return message.slice(0, 8_192);
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
