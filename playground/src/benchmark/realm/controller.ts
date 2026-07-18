import { assertSafeSvgForDom } from "@mermanjs/web/svg-safety";

import {
  BENCHMARK_PROTOCOL_VERSION,
  validateBenchmarkSampleProgress,
  validateBenchmarkSampleRequest,
  validateBenchmarkSampleResponse,
  type BenchmarkSampleRequest,
  type BenchmarkSampleFailure,
  type BenchmarkSampleResponse,
  type BenchmarkSampleSuccess,
} from "../protocol.ts";
import {
  createAuthenticatedBrowserRealmChannel,
  type AuthenticatedBrowserRealmChannel,
  type BrowserRealmChannelOptions,
} from "../../runtime/realm/browser-realm-channel.ts";
import { createBenchmarkSampleBudget } from "../sample-budget.ts";
import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  RealmTimeoutError,
  isRealmMessageType,
  utf8ByteLength,
  validateRealmFatal,
  type RealmViewport,
} from "../../runtime/realm/channel-protocol.ts";
import {
  createBenchmarkProgressGate,
  createBenchmarkStageWatchdog,
  type BenchmarkProgressGate,
  type BenchmarkStageWatchdog,
  type BenchmarkStageTimer,
} from "./stage-watchdog.ts";

export type BenchmarkSampleInput = Pick<
  BenchmarkSampleRequest,
  | "engine"
  | "mode"
  | "payload"
  | "requestId"
  | "role"
  | "runId"
  | "runToken"
>;

export interface BrowserBenchmarkRealmSession {
  dispose(): void;
  sample(input: BenchmarkSampleInput): Promise<BenchmarkRealmSampleResult>;
}

export interface BenchmarkRealmSampleSuccess
  extends Omit<BenchmarkSampleSuccess, "svg"> {
  readonly svgBytes: number;
}

export type BenchmarkRealmSampleResult =
  | BenchmarkSampleFailure
  | BenchmarkRealmSampleSuccess;

export interface BenchmarkRealmSessionDependencies {
  clearTimer(handle: unknown): void;
  createChannel(
    options: BrowserRealmChannelOptions
  ): Promise<AuthenticatedBrowserRealmChannel>;
  getVisibilityState(): string;
  now(): number;
  readonly realmUrl: URL;
  setTimer(callback: () => void, timeoutMs: number): unknown;
  validateSvg(svg: string): void;
}

interface PendingSample {
  readonly expected: BenchmarkSampleInput;
  readonly progressGate: BenchmarkProgressGate;
  readonly reject: (error: unknown) => void;
  readonly resolve: (response: BenchmarkRealmSampleResult) => void;
  readonly stageWatchdog: BenchmarkStageWatchdog;
  progressTimer: unknown | null;
}

export async function createBrowserBenchmarkRealmSession(
  initialViewport: RealmViewport,
  signal: AbortSignal
): Promise<BrowserBenchmarkRealmSession> {
  return createBenchmarkRealmSession(initialViewport, signal, {
    clearTimer: (handle) =>
      clearTimeout(handle as ReturnType<typeof setTimeout>),
    createChannel: createAuthenticatedBrowserRealmChannel,
    getVisibilityState: () => document.visibilityState,
    now: () => performance.now(),
    realmUrl: new URL(
      `${import.meta.env.BASE_URL}benchmark.html`,
      window.location.origin
    ),
    setTimer: (callback, timeoutMs) => setTimeout(callback, timeoutMs),
    validateSvg: assertSafeSvgForDom,
  });
}

export async function createBenchmarkRealmSession(
  initialViewport: RealmViewport,
  signal: AbortSignal,
  dependencies: BenchmarkRealmSessionDependencies
): Promise<BrowserBenchmarkRealmSession> {
  if (dependencies.getVisibilityState() !== "visible") {
    throw new RealmProtocolError(
      "Benchmark realm cannot start while the document is hidden."
    );
  }

  let disposed = false;
  let transportAvailable = false;
  let incomingSequence = 0;
  let outgoingSequence = 0;
  let pending: PendingSample | null = null;
  let runTimer: unknown | null = null;
  let channelRef: AuthenticatedBrowserRealmChannel | null = null;
  const sampleBudget = createBenchmarkSampleBudget();
  const stageTimer: BenchmarkStageTimer = {
    clear: dependencies.clearTimer,
    now: dependencies.now,
    set: dependencies.setTimer,
  };

  const cleanupPending = (current: PendingSample) => {
    if (current.progressTimer !== null) {
      dependencies.clearTimer(current.progressTimer);
      current.progressTimer = null;
    }
    current.stageWatchdog.dispose();
  };

  const rejectPending = (error: unknown) => {
    if (!pending) return;
    const current = pending;
    pending = null;
    cleanupPending(current);
    current.reject(error);
  };
  const onTransportFailure = (error: Error) => {
    disposed = true;
    transportAvailable = false;
    if (runTimer !== null) dependencies.clearTimer(runTimer);
    runTimer = null;
    if (channelRef) channelRef.port.onmessage = null;
    rejectPending(error);
  };
  const channel = await dependencies.createChannel({
    kind: "benchmark",
    realmUrl: dependencies.realmUrl,
    initialViewport,
    signal,
    label: "Benchmark realm",
    title: "Merman Benchmark Realm",
    onFailure: onTransportFailure,
  });
  channelRef = channel;
  transportAvailable = true;
  const { identity, port } = channel;

  const dispose = () => {
    if (disposed) return;
    disposed = true;
    transportAvailable = false;
    if (runTimer !== null) dependencies.clearTimer(runTimer);
    runTimer = null;
    rejectPending(new RealmProtocolError("Benchmark realm was disposed."));
    port.onmessage = null;
    channel.dispose();
  };
  const poison = (error: unknown) => channel.poison(error);
  const armProgressTimer = (current: PendingSample) => {
    if (current.progressTimer !== null) {
      dependencies.clearTimer(current.progressTimer);
    }
    let handle: unknown;
    handle = dependencies.setTimer(() => {
      if (pending !== current || current.progressTimer !== handle) return;
      current.progressTimer = null;
      poison(new RealmTimeoutError("Benchmark progress timed out."));
    }, REALM_BUDGETS.stageTimeoutMs);
    current.progressTimer = handle;
  };

  port.onmessage = (event) => {
    try {
      const expectedSequence = incomingSequence + 1;
      if (isRealmMessageType(event.data, "realm-fatal")) {
        const fatal = validateRealmFatal(
          event.data,
          identity,
          expectedSequence
        );
        incomingSequence = expectedSequence;
        throw new RealmProtocolError(fatal.message);
      }
      if (!pending) {
        throw new RealmProtocolError(
          "Benchmark realm sent an unsolicited response."
        );
      }
      const current = pending;
      if (isRealmMessageType(event.data, "benchmark-progress")) {
        const progress = validateBenchmarkSampleProgress(
          event.data,
          identity,
          expectedSequence,
          current.expected
        );
        current.progressGate.observe(progress.event);
        current.stageWatchdog.observe(progress.event);
        incomingSequence = expectedSequence;
        armProgressTimer(current);
        return;
      }
      const response = validateBenchmarkSampleResponse(
        event.data,
        identity,
        expectedSequence,
        current.expected
      );
      if (response.type === "benchmark-sample-success") {
        current.progressGate.assertComplete();
      } else if (
        response.trace === null &&
        !current.progressGate.isEmpty()
      ) {
        throw new RealmProtocolError(
          "Pre-clock benchmark failure cannot contain progress."
        );
      }
      if (response.type === "benchmark-sample-success") {
        dependencies.validateSvg(response.svg);
      }
      const projected = projectSample(response);
      incomingSequence = expectedSequence;
      pending = null;
      cleanupPending(current);
      if (response.type === "benchmark-sample-failure") {
        dispose();
      }
      current.resolve(projected);
    } catch (error) {
      poison(error);
    }
  };

  return {
    dispose,
    sample(input) {
      if (disposed || !transportAvailable) {
        return Promise.reject(
          new RealmProtocolError("Benchmark realm is not ready.")
        );
      }
      if (dependencies.getVisibilityState() !== "visible") {
        return Promise.reject(
          new RealmProtocolError(
            "Benchmark sample cannot start while the document is hidden."
          )
        );
      }
      if (pending) {
        return Promise.reject(
          new RealmProtocolError("Benchmark realm already has active work.")
        );
      }
      const nextSequence = outgoingSequence + 1;
      let request: BenchmarkSampleRequest;
      try {
        request = validateBenchmarkSampleRequest(
          {
            type: "benchmark-sample",
            protocol: REALM_PROTOCOL_VERSION,
            benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
            ...identity,
            sequence: nextSequence,
            ...input,
          },
          identity,
          nextSequence
        );
        sampleBudget.accept(request.role);
      } catch (error) {
        return Promise.reject(error);
      }
      outgoingSequence = nextSequence;
      runTimer ??= dependencies.setTimer(() => {
        poison(new RealmTimeoutError("Benchmark realm run timed out."));
      }, REALM_BUDGETS.runTimeoutMs);
      return new Promise<BenchmarkRealmSampleResult>((resolve, reject) => {
        let current: PendingSample;
        const progressGate = createBenchmarkProgressGate(request);
        const stageWatchdog = createBenchmarkStageWatchdog((stage) => {
          if (pending !== current) return;
          poison(
            new RealmTimeoutError(
              `Benchmark parent watchdog timed out during ${stage}.`
            )
          );
        }, stageTimer);
        current = {
          expected: request,
          progressGate,
          resolve,
          reject,
          stageWatchdog,
          progressTimer: null,
        };
        pending = current;
        armProgressTimer(current);
        try {
          port.postMessage(request);
        } catch (error) {
          poison(error);
        }
      });
    },
  };
}

function projectSample(
  response: BenchmarkSampleResponse
): BenchmarkRealmSampleResult {
  if (response.type === "benchmark-sample-failure") {
    return Object.freeze(response);
  }
  const { svg, ...evidence } = response;
  return Object.freeze({ ...evidence, svgBytes: utf8ByteLength(svg) });
}
