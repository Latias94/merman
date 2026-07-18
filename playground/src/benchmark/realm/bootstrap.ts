import { assertSafeSvgForDom } from "@mermanjs/web/svg-safety";

import {
  BENCHMARK_PROTOCOL_VERSION,
  validateBenchmarkSampleProgress,
  validateBenchmarkSampleRequest,
  validateBenchmarkSampleResponse,
  type BenchmarkFailureStage,
  type BenchmarkResourceObservation,
  type BenchmarkSampleRequest,
  type BenchmarkSampleResponse,
} from "../protocol.ts";
import {
  BENCHMARK_TRACE_SCHEMA_VERSION,
  createBenchmarkTraceRecorder,
  type BenchmarkEngine,
  type BenchmarkTraceMark,
  type BenchmarkTraceRecorder,
} from "../trace.ts";
import { createBenchmarkSampleBudget } from "../sample-budget.ts";
import {
  BENCHMARK_BUDGETS,
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  assertEncodedMessageBudget,
  assertRealmSvgBudget,
  createOneTimeRealmInitGate,
  isRealmMessageType,
  validateRealmHello,
  type CompareRenderPayload,
  type RealmBootIdentity,
  type RealmIdentity,
} from "../../runtime/realm/channel-protocol.ts";
import {
  BenchmarkEngineError,
  type BenchmarkEngineAdapter,
  type BenchmarkEngineSession,
} from "./engine.ts";
import {
  BenchmarkStageTimeoutError,
  createBenchmarkProgressGate,
  createBenchmarkStageWatchdog,
  type BenchmarkStageWatchdog,
} from "./stage-watchdog.ts";
import "./benchmark-realm.css";

void startBenchmarkRealm();

interface FrozenRealmInput {
  readonly engine: BenchmarkEngine;
  readonly payloadJson: string;
  readonly runId: string;
  readonly runToken: string;
}

interface SampleEvidence {
  readonly resourceError: string | null;
  readonly resources: readonly BenchmarkResourceObservation[];
  readonly trace: ReturnType<BenchmarkTraceRecorder["finish"]>;
}

type RealmResponseBody<T> = T extends BenchmarkSampleResponse
  ? Omit<T, keyof RealmIdentity | "sequence">
  : never;

type BenchmarkRealmResponse = RealmResponseBody<BenchmarkSampleResponse>;

async function startBenchmarkRealm(): Promise<void> {
  const boot = readBootIdentity();
  if (window.parent === window) {
    throw new RealmProtocolError("Benchmark realm must run inside an iframe.");
  }

  const initGate = createOneTimeRealmInitGate(boot);
  const onInit = (event: MessageEvent) => {
    if (
      event.origin !== window.location.origin ||
      event.source !== window.parent ||
      !isRealmMessageType(event.data, "realm-init")
    ) {
      return;
    }
    let init;
    try {
      init = initGate.consume(event.data, event.ports.length);
    } catch {
      for (const port of event.ports) port.close();
      window.removeEventListener("message", onInit);
      return;
    }
    window.removeEventListener("message", onInit);
    window.history.replaceState(
      null,
      "",
      `${window.location.pathname}${window.location.search}`
    );
    servePort(event.ports[0], init);
  };

  window.addEventListener("message", onInit);
  window.parent.postMessage(
    validateRealmHello(
      {
        type: "realm-hello",
        protocol: REALM_PROTOCOL_VERSION,
        ...boot,
      },
      boot
    ),
    window.location.origin
  );
}

function servePort(port: MessagePort, init: RealmIdentity): void {
  const identity: RealmIdentity = {
    kind: init.kind,
    realmId: init.realmId,
    realmToken: init.realmToken,
  };
  const host = document.getElementById("presentation-host");
  if (!(host instanceof HTMLElement)) {
    port.close();
    return;
  }

  let incomingSequence = 0;
  let outgoingSequence = 0;
  let active = false;
  let closed = false;
  let poisoned = false;
  let frozen: FrozenRealmInput | null = null;
  let engineSession: BenchmarkEngineSession | null = null;
  let runTimer: ReturnType<typeof setTimeout> | null = null;
  let stageWatchdog: BenchmarkStageWatchdog | null = null;
  let activeTimeout: ((error: SampleTimeoutError) => void) | null = null;
  const sampleBudget = createBenchmarkSampleBudget();

  const disposeEngine = () => {
    engineSession?.dispose();
    engineSession = null;
    host.replaceChildren();
  };
  const close = () => {
    if (closed) return;
    closed = true;
    active = false;
    if (runTimer !== null) clearTimeout(runTimer);
    runTimer = null;
    stageWatchdog?.dispose();
    stageWatchdog = null;
    activeTimeout = null;
    disposeEngine();
    port.onmessage = null;
    port.onmessageerror = null;
    port.close();
  };
  const post = (message: unknown) => {
    if (closed) return;
    assertEncodedMessageBudget(message);
    port.postMessage(message);
  };
  const postProgress = (
    request: BenchmarkSampleRequest,
    event: BenchmarkTraceMark
  ) => {
    const nextSequence = outgoingSequence + 1;
    const progress = validateBenchmarkSampleProgress(
      {
        type: "benchmark-progress",
        protocol: REALM_PROTOCOL_VERSION,
        benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
        ...identity,
        sequence: nextSequence,
        runId: request.runId,
        runToken: request.runToken,
        requestId: request.requestId,
        engine: request.engine,
        mode: request.mode,
        role: request.role,
        traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
        event,
      },
      identity,
      nextSequence,
      request
    );
    outgoingSequence = nextSequence;
    post(progress);
  };
  const fatal = (error: unknown) => {
    if (closed) return;
    poisoned = true;
    outgoingSequence += 1;
    post({
      type: "realm-fatal",
      protocol: REALM_PROTOCOL_VERSION,
      ...identity,
      sequence: outgoingSequence,
      message: boundedErrorMessage(error),
    });
    queueMicrotask(close);
  };

  const onDocumentExit = () => close();
  window.addEventListener("pagehide", onDocumentExit, { once: true });
  port.onmessageerror = () => fatal("Benchmark realm could not clone a message.");
  port.onmessage = (event) => {
    if (closed) return;
    if (active || poisoned) {
      fatal("Benchmark realm received work in a terminal or active state.");
      return;
    }

    const expectedSequence = incomingSequence + 1;
    let request: BenchmarkSampleRequest;
    try {
      request = validateBenchmarkSampleRequest(
        event.data,
        identity,
        expectedSequence
      );
      assertRequestState(request, frozen, engineSession, host);
      sampleBudget.accept(request.role);
    } catch (error) {
      fatal(error);
      return;
    }
    incomingSequence = expectedSequence;
    active = true;
    if (frozen === null) {
      frozen = freezeInput(request);
    }

    const timeout = Promise.withResolvers<never>();
    const rejectTimeout = (error: SampleTimeoutError) => timeout.reject(error);
    activeTimeout = rejectTimeout;
    const progressGate = createBenchmarkProgressGate(request);
    stageWatchdog = createBenchmarkStageWatchdog((stage) => {
      activeTimeout?.(new SampleTimeoutError(stage));
    });
    runTimer ??= setTimeout(() => {
      const timeoutError = new SampleTimeoutError("run");
      if (activeTimeout) {
        activeTimeout(timeoutError);
      } else {
        fatal(timeoutError);
      }
    }, REALM_BUDGETS.runTimeoutMs);

    void executeSample(
      request,
      host,
      engineSession,
      (event) => {
        progressGate.observe(event);
        stageWatchdog?.observe(event);
        postProgress(request, event);
      },
      timeout.promise
    )
      .then(({ response, session }) => {
        if (closed) {
          session?.dispose();
          return;
        }
        stageWatchdog?.dispose();
        stageWatchdog = null;
        if (activeTimeout === rejectTimeout) activeTimeout = null;
        engineSession = session;
        if (response.type === "benchmark-sample-success") {
          progressGate.assertComplete();
        } else if (response.trace === null && !progressGate.isEmpty()) {
          throw new RealmProtocolError(
            "Pre-clock benchmark failure cannot contain progress."
          );
        }
        outgoingSequence += 1;
        const validated = validateBenchmarkSampleResponse(
          { ...response, sequence: outgoingSequence, ...identity },
          identity,
          outgoingSequence,
          request
        );
        post(validated);
        active = false;
        if (validated.type === "benchmark-sample-failure") {
          poisoned = true;
          if (runTimer !== null) clearTimeout(runTimer);
          runTimer = null;
          disposeEngine();
        }
      })
      .catch(fatal);
  };
  port.start();
  post({
    type: "realm-ready",
    protocol: REALM_PROTOCOL_VERSION,
    ...identity,
    sequence: 0,
    viewport: { width: window.innerWidth, height: window.innerHeight },
  });
}

async function executeSample(
  request: BenchmarkSampleRequest,
  host: HTMLElement,
  existingSession: BenchmarkEngineSession | null,
  onTraceEvent: (event: BenchmarkTraceMark) => void,
  timeout: Promise<never>
): Promise<{
  response: BenchmarkRealmResponse;
  session: BenchmarkEngineSession | null;
}> {
  if (document.visibilityState !== "visible") {
    return {
      response: failureResponse(
        request,
        "environment",
        "Benchmark realm is not visible.",
        null,
        [],
        null,
        null
      ),
      session: null,
    };
  }

  assertPresentationHost(host, request.payload);
  let clockOrigin: number | null = null;
  const now = () => {
    const value = performance.now();
    clockOrigin ??= value;
    return value;
  };
  const recorder = createBenchmarkTraceRecorder(now);
  const mark = (event: BenchmarkTraceMark) => {
    const offset = recorder.mark(event);
    onTraceEvent(event);
    return offset;
  };
  const waitFor = <T>(operation: T | PromiseLike<T>): Promise<T> =>
    Promise.race([Promise.resolve(operation), timeout]);
  let stage: BenchmarkFailureStage = "fonts";
  let session = existingSession;

  try {
    mark("fonts_wait_start");
    const fontsReady = Promise.resolve(document.fonts.ready).then(
      () => mark("fonts_wait_end"),
      (error) => {
        mark("fonts_wait_end");
        throw new SampleStageError("fonts", error);
      }
    );

    let adapterReady: Promise<BenchmarkEngineAdapter | null>;
    if (request.mode === "realm-cold") {
      stage = "adapter-import";
      mark("adapter_import_start");
      adapterReady = loadAdapter(request.engine).then(
        (adapter) => {
          mark("adapter_import_end");
          return adapter;
        },
        (error) => {
          mark("adapter_import_end");
          throw new SampleStageError("adapter-import", error);
        }
      );
    } else {
      adapterReady = Promise.resolve(null);
    }

    const [fontsResult, adapterResult] = await waitFor(
      Promise.allSettled([fontsReady, adapterReady])
    );
    if (fontsResult.status === "rejected") throw fontsResult.reason;
    if (adapterResult.status === "rejected") throw adapterResult.reason;
    const adapter = adapterResult.value;
    if (adapter) {
      stage = "initialize";
      session = await waitFor(
        adapter.initialize({ mark, payload: request.payload })
      );
    }
    if (!session) {
      throw new SampleStageError(
        "protocol",
        "Warm benchmark sample has no initialized engine."
      );
    }

    stage = "render";
    mark("render_start");
    const svg = await waitFor(session.render());
    stage = "svg-validation";
    assertRealmSvgBudget(svg);
    assertSafeSvgForDom(svg);
    mark("safe_svg_ready");

    stage = "presentation";
    assertPresentationHost(host, request.payload);
    host.innerHTML = svg;
    mark("dom_inserted");
    const svgElement = host.querySelector("svg");
    if (!(svgElement instanceof SVGSVGElement)) {
      throw new Error("Benchmark engine did not return an SVG root element.");
    }
    const rect = svgElement.getBoundingClientRect();
    if (!isNonEmptyRect(rect)) {
      throw new Error("Benchmark SVG has no finite non-empty layout box.");
    }
    mark("layout_box_ready");
    await waitFor(nextAnimationFrame());
    mark("presentation_ready");

    const evidence = finishEvidence(
      recorder,
      clockOrigin ?? performance.now(),
      false
    );
    host.replaceChildren();
    return {
      response: {
        type: "benchmark-sample-success",
        protocol: REALM_PROTOCOL_VERSION,
        benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
        runId: request.runId,
        runToken: request.runToken,
        requestId: request.requestId,
        engine: request.engine,
        mode: request.mode,
        role: request.role,
        traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
        trace: evidence.trace,
        resources: evidence.resources,
        resourceError: evidence.resourceError,
        svg,
        version: session.version,
      },
      session,
    };
  } catch (error) {
    host.replaceChildren();
    const evidence = finishEvidence(
      recorder,
      clockOrigin ?? performance.now(),
      true
    );
    const failureStage =
      error instanceof SampleTimeoutError ||
      error instanceof BenchmarkStageTimeoutError
        ? "timeout"
        : error instanceof BenchmarkEngineError || error instanceof SampleStageError
        ? error.stage
        : stage;
    return {
      response: failureResponse(
        request,
        failureStage,
        boundedErrorMessage(error),
        evidence.trace,
        evidence.resources,
        evidence.resourceError,
        session?.version ?? null
      ),
      session,
    };
  }
}

async function loadAdapter(engine: BenchmarkEngine): Promise<BenchmarkEngineAdapter> {
  const module =
    engine === "merman"
      ? await import("./engines/merman.ts")
      : await import("./engines/mermaid.ts");
  return module.benchmarkEngineAdapter;
}

function failureResponse(
  request: BenchmarkSampleRequest,
  stage: BenchmarkFailureStage,
  message: string,
  trace: ReturnType<BenchmarkTraceRecorder["finish"]> | null,
  resources: readonly BenchmarkResourceObservation[],
  resourceError: string | null,
  version: string | null
): Extract<BenchmarkRealmResponse, { type: "benchmark-sample-failure" }> {
  return {
    type: "benchmark-sample-failure",
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    runId: request.runId,
    runToken: request.runToken,
    requestId: request.requestId,
    engine: request.engine,
    mode: request.mode,
    role: request.role,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace,
    resources,
    resourceError,
    stage,
    message,
    version,
  };
}

function assertRequestState(
  request: BenchmarkSampleRequest,
  frozen: FrozenRealmInput | null,
  session: BenchmarkEngineSession | null,
  host: HTMLElement
): void {
  if (document.visibilityState !== "visible") {
    return;
  }
  assertPresentationHost(host, request.payload);
  if (frozen === null) {
    if (request.mode !== "realm-cold" || session !== null) {
      throw new RealmProtocolError(
        "Benchmark realm must begin with one realm-cold sample."
      );
    }
    return;
  }
  if (
    request.mode !== "warm" ||
    session === null ||
    request.engine !== frozen.engine ||
    request.runId !== frozen.runId ||
    request.runToken !== frozen.runToken ||
    JSON.stringify(request.payload) !== frozen.payloadJson
  ) {
    throw new RealmProtocolError("Benchmark warm sample changed frozen realm input.");
  }
}

function freezeInput(request: BenchmarkSampleRequest): FrozenRealmInput {
  return Object.freeze({
    engine: request.engine,
    payloadJson: JSON.stringify(request.payload),
    runId: request.runId,
    runToken: request.runToken,
  });
}

function assertPresentationHost(
  host: HTMLElement,
  payload: CompareRenderPayload
): void {
  if (!host.isConnected) {
    throw new RealmProtocolError("Benchmark presentation host is detached.");
  }
  if (
    Math.round(window.innerWidth) !== Math.round(payload.viewport.width) ||
    Math.round(window.innerHeight) !== Math.round(payload.viewport.height)
  ) {
    throw new RealmProtocolError("Benchmark realm viewport does not match its input.");
  }
  if (!isNonEmptyRect(host.getBoundingClientRect())) {
    throw new RealmProtocolError(
      "Benchmark presentation host has no finite non-empty layout box."
    );
  }
}

function finishEvidence(
  recorder: BenchmarkTraceRecorder,
  t0: number,
  failure: boolean
): SampleEvidence {
  let resources: readonly BenchmarkResourceObservation[];
  let resourceError: string | null = null;
  try {
    resources = collectResourceObservations(t0);
  } catch (error) {
    resources = Object.freeze([]);
    resourceError = boundedErrorMessage(error);
  }
  const trace = failure ? recorder.finishFailure() : recorder.finish();
  return Object.freeze({ resourceError, resources, trace });
}

function collectResourceObservations(
  t0: number
): readonly BenchmarkResourceObservation[] {
  const entries = performance
    .getEntriesByType("resource")
    .filter((entry): entry is PerformanceResourceTiming =>
      entry instanceof PerformanceResourceTiming && entry.startTime >= t0
    );
  if (entries.length > BENCHMARK_BUDGETS.maxResourceObservations) {
    throw new SampleStageError(
      "protocol",
      "Benchmark resource observations exceed their protocol budget."
    );
  }
  const observations = entries.map((entry) => {
      const extended = entry as PerformanceResourceTiming & {
        deliveryType?: unknown;
        responseStatus?: unknown;
      };
      return Object.freeze({
        name: entry.name,
        initiatorType: entry.initiatorType || "unknown",
        startOffset: Math.max(0, entry.startTime - t0),
        duration: Math.max(0, entry.duration),
        transferSize: finiteOrNull(entry.transferSize),
        encodedBodySize: finiteOrNull(entry.encodedBodySize),
        decodedBodySize: finiteOrNull(entry.decodedBodySize),
        responseStatus: finiteOrNull(extended.responseStatus),
        deliveryType:
          typeof extended.deliveryType === "string" && extended.deliveryType
            ? extended.deliveryType
            : null,
      });
    });
  return Object.freeze(observations);
}

function finiteOrNull(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) && value >= 0
    ? value
    : null;
}

function isNonEmptyRect(rect: DOMRect | DOMRectReadOnly): boolean {
  return (
    Number.isFinite(rect.width) &&
    Number.isFinite(rect.height) &&
    rect.width > 0 &&
    rect.height > 0
  );
}

function nextAnimationFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

class SampleStageError extends Error {
  readonly stage: BenchmarkFailureStage;

  constructor(stage: BenchmarkFailureStage, cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause));
    this.name = "SampleStageError";
    this.stage = stage;
  }
}

class SampleTimeoutError extends Error {
  readonly stage = "timeout" as const;

  constructor(activeStage: BenchmarkFailureStage | "run") {
    super(`Benchmark realm timed out during ${activeStage}.`);
    this.name = "SampleTimeoutError";
  }
}

function readBootIdentity(): RealmBootIdentity {
  const params = new URLSearchParams(window.location.hash.slice(1));
  if (
    params.size !== 4 ||
    params.get("protocol") !== String(REALM_PROTOCOL_VERSION) ||
    params.get("kind") !== "benchmark"
  ) {
    throw new RealmProtocolError("Benchmark realm boot fragment is invalid.");
  }
  const realmId = params.get("realm");
  const bootNonce = params.get("boot");
  if (!realmId || !bootNonce) {
    throw new RealmProtocolError("Benchmark realm boot identity is invalid.");
  }
  const boot: RealmBootIdentity = {
    kind: "benchmark",
    realmId,
    bootNonce,
  };
  validateRealmHello(
    { type: "realm-hello", protocol: REALM_PROTOCOL_VERSION, ...boot },
    boot
  );
  return boot;
}

function boundedErrorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return message.slice(0, Math.floor(REALM_BUDGETS.errorBytes / 4));
}
