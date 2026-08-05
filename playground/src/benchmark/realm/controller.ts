import {
  BENCHMARK_PROTOCOL_VERSION,
  validateBenchmarkSampleProgress,
  validateBenchmarkSampleResponse,
  type BenchmarkInputSampleRequest,
  type BenchmarkSampleRequest,
  type BenchmarkSampleFailure,
  type BenchmarkExpectedSample,
  type BenchmarkReuseSampleRequest,
  type BenchmarkSampleResponse,
  type BenchmarkSampleSuccess,
} from "../protocol.ts";
import {
  createAuthenticatedBrowserRealmChannel,
  type AuthenticatedBrowserRealmChannel,
  type BrowserRealmChannelOptions,
} from "../../runtime/realm/browser-realm-channel.ts";
import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  RealmTimeoutError,
  isRealmMessageType,
  utf8ByteLength,
  validateRealmFatal,
  type RealmViewport,
  type RealmBootIdentity,
  type RealmEngineArtifact,
  type RealmEngineArtifactIdentity,
} from "../../runtime/realm/channel-protocol.ts";
import {
  createBenchmarkProgressGate,
  createBenchmarkStageWatchdog,
  type BenchmarkProgressGate,
  type BenchmarkStageWatchdog,
  type BenchmarkStageTimer,
} from "./stage-watchdog.ts";
import { projectSafeInlineSvg } from "../../runtime/render-artifact.ts";
import type { BenchmarkEngine } from "../trace.ts";
import { benchmarkPhasePath } from "../phase-contract.ts";
import { benchmarkIntentModeFromKind } from "../sample-plan.ts";
import {
  deriveBenchmarkParentPublicationEvidence,
  type BenchmarkParentPublicationEvidence,
} from "../publication.ts";

type BenchmarkSampleIdentityInput = Pick<
  BenchmarkSampleRequest,
  "engine" | "inputId" | "runId" | "runToken" | "sampleId"
>;

export type BenchmarkSampleInput =
  | (BenchmarkSampleIdentityInput &
      Pick<BenchmarkInputSampleRequest, "intentKind" | "payload">)
  | (BenchmarkSampleIdentityInput &
      Pick<BenchmarkReuseSampleRequest, "intentKind">);

export interface BrowserBenchmarkRealmSession {
  readonly creationEvidence: BenchmarkRealmCreationEvidence;
  dispose(): void;
  sample(input: BenchmarkSampleInput): Promise<BenchmarkRealmSampleResult>;
}

export interface BenchmarkRealmCreationEvidence {
  readonly artifact: RealmEngineArtifactIdentity;
  readonly artifactAcquisitionMs: number;
  readonly clockBoundary: "parent-before-sample";
  readonly realmBootstrapMs: number;
  readonly totalMs: number;
}

export interface BenchmarkRealmSampleSuccess
  extends Omit<BenchmarkSampleSuccess, "svg"> {
  readonly parentPublication: BenchmarkParentPublicationEvidence;
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
  createCreationEvidence(): BenchmarkRealmCreationEvidence;
  getVisibilityState(): string;
  readonly engineArtifact: RealmEngineArtifact;
  now(): number;
  readonly realm:
    | {
        readonly createRealmDocument: (identity: RealmBootIdentity) => string;
      }
    | { readonly realmUrl: URL };
  setTimer(callback: () => void, timeoutMs: number): unknown;
}

interface PendingSample {
  dispatchTimer: unknown | null;
  readonly expected: BenchmarkExpectedSample;
  readonly progressGate: BenchmarkProgressGate;
  readonly reject: (error: unknown) => void;
  readonly resolve: (response: BenchmarkRealmSampleResult) => void;
  readonly stageWatchdog: BenchmarkStageWatchdog;
  readonly dispatchedAt: number;
  isolatedPresentationReceivedAt: number | null;
  responseTimer: unknown | null;
}

export async function createBrowserBenchmarkRealmSession(
  engine: BenchmarkEngine,
  initialViewport: RealmViewport,
  signal: AbortSignal
): Promise<BrowserBenchmarkRealmSession> {
  const setupStartedAt = performance.now();
  const opaqueRealm =
    engine === "mermaid"
      ? await import("./opaque-mermaid-artifact.ts")
      : null;
  const mermanRealm =
    engine === "merman" ? await import("./merman-engine-artifact.ts") : null;
  const engineArtifact =
    engine === "mermaid"
      ? await opaqueRealm!.createBenchmarkMermaidEngineArtifact(signal)
      : await mermanRealm!.createMermanBenchmarkEngineArtifact(signal);
  const artifactAcquiredAt = performance.now();
  const bootstrapStartedAt = performance.now();
  return createBenchmarkRealmSession(engine, initialViewport, signal, {
    clearTimer: (handle) =>
      clearTimeout(handle as ReturnType<typeof setTimeout>),
    createChannel: createAuthenticatedBrowserRealmChannel,
    createCreationEvidence() {
      const readyAt = performance.now();
      return {
        artifact: artifactIdentity(engineArtifact),
        artifactAcquisitionMs: artifactAcquiredAt - setupStartedAt,
        clockBoundary: "parent-before-sample",
        realmBootstrapMs: readyAt - bootstrapStartedAt,
        totalMs: readyAt - setupStartedAt,
      };
    },
    getVisibilityState: () => document.visibilityState,
    now: () => performance.now(),
    engineArtifact,
    realm:
      engine === "mermaid"
        ? {
            createRealmDocument:
              opaqueRealm!.createOpaqueMermaidBenchmarkRealmDocument,
          }
        : {
            realmUrl: new URL(
              `${import.meta.env.BASE_URL}benchmark.html`,
              window.location.origin
            ),
          },
    setTimer: (callback, timeoutMs) => setTimeout(callback, timeoutMs),
  });
}

export async function createBenchmarkRealmSession(
  expectedEngine: BenchmarkEngine,
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
  const stageTimer: BenchmarkStageTimer = {
    clear: dependencies.clearTimer,
    now: dependencies.now,
    set: dependencies.setTimer,
  };

  const cleanupPending = (current: PendingSample) => {
    if (current.dispatchTimer !== null) {
      dependencies.clearTimer(current.dispatchTimer);
      current.dispatchTimer = null;
    }
    if (current.responseTimer !== null) {
      dependencies.clearTimer(current.responseTimer);
      current.responseTimer = null;
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
    ...dependencies.realm,
    engineArtifact: dependencies.engineArtifact,
    initialViewport,
    signal,
    label: "Benchmark realm",
    title: "Merman Benchmark Realm",
    onFailure: onTransportFailure,
  });
  channelRef = channel;
  transportAvailable = true;
  const { identity, port } = channel;
  let creationEvidence: BenchmarkRealmCreationEvidence;
  try {
    creationEvidence = validateCreationEvidence(
      dependencies.createCreationEvidence(),
      dependencies.engineArtifact
    );
  } catch (error) {
    transportAvailable = false;
    channel.dispose();
    throw error;
  }

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
  const armResponseTimer = (current: PendingSample) => {
    if (current.responseTimer !== null) return;
    let handle: unknown;
    handle = dependencies.setTimer(() => {
      if (pending !== current || current.responseTimer !== handle) return;
      current.responseTimer = null;
      poison(new RealmTimeoutError("Benchmark response timed out."));
    }, REALM_BUDGETS.stageTimeoutMs);
    current.responseTimer = handle;
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
        const progressReceivedAt = dependencies.now();
        const progress = validateBenchmarkSampleProgress(
          event.data,
          identity,
          expectedSequence,
          current.expected
        );
        current.progressGate.observe(progress.event);
        current.stageWatchdog.observe(progress.event);
        if (current.dispatchTimer !== null) {
          dependencies.clearTimer(current.dispatchTimer);
          current.dispatchTimer = null;
        }
        if (
          benchmarkPhasePath(
            progress.engine,
            benchmarkIntentModeFromKind(progress.intentKind)
          ).rule(progress.event)?.publicationBoundary
        ) {
          current.isolatedPresentationReceivedAt = progressReceivedAt;
          armResponseTimer(current);
        }
        incomingSequence = expectedSequence;
        return;
      }
      const responseReceivedAt = dependencies.now();
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
      const envelopeValidatedAt = dependencies.now();
      let parentPublication: BenchmarkParentPublicationEvidence | null = null;
      if (response.type === "benchmark-sample-success") {
        if (current.isolatedPresentationReceivedAt === null) {
          throw new RealmProtocolError(
            "Successful benchmark response has no isolated presentation receipt."
          );
        }
        projectSafeInlineSvg(response.svg);
        const strictSvgValidatedAt = dependencies.now();
        parentPublication = deriveBenchmarkParentPublicationEvidence({
          dispatchedAt: current.dispatchedAt,
          isolatedPresentationReceivedAt:
            current.isolatedPresentationReceivedAt,
          responseReceivedAt,
          envelopeValidatedAt,
          strictSvgValidatedAt,
        });
      }
      const projected = projectSample(response, parentPublication);
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
    creationEvidence,
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
      if (input.engine !== expectedEngine) {
        return Promise.reject(
          new RealmProtocolError("Benchmark session engine is invalid.")
        );
      }
      const nextSequence = outgoingSequence + 1;
      const request: BenchmarkSampleRequest = Object.freeze({
        type: "benchmark-sample",
        protocol: REALM_PROTOCOL_VERSION,
        benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
        ...identity,
        sequence: nextSequence,
        requestId: input.sampleId,
        ...input,
      });
      outgoingSequence = nextSequence;
      runTimer ??= dependencies.setTimer(() => {
        poison(new RealmTimeoutError("Benchmark realm run timed out."));
      }, REALM_BUDGETS.runTimeoutMs);
      return new Promise<BenchmarkRealmSampleResult>((resolve, reject) => {
        let current: PendingSample;
        const progressGate = createBenchmarkProgressGate(request);
        const stageWatchdog = createBenchmarkStageWatchdog(
          request,
          (stage) => {
            if (pending !== current) return;
            poison(
              new RealmTimeoutError(
                `Benchmark parent watchdog timed out during ${stage}.`
              )
            );
          },
          stageTimer
        );
        const dispatchedAt = dependencies.now();
        current = {
          expected: {
            engine: request.engine,
            intentKind: request.intentKind,
            requestId: request.requestId,
            runId: request.runId,
            runToken: request.runToken,
            sampleId: request.sampleId,
          },
          dispatchTimer: null,
          dispatchedAt,
          isolatedPresentationReceivedAt: null,
          progressGate,
          resolve,
          reject,
          stageWatchdog,
          responseTimer: null,
        };
        pending = current;
        let dispatchHandle: unknown;
        dispatchHandle = dependencies.setTimer(() => {
          if (pending !== current || current.dispatchTimer !== dispatchHandle) {
            return;
          }
          current.dispatchTimer = null;
          poison(new RealmTimeoutError("Benchmark dispatch timed out."));
        }, REALM_BUDGETS.stageTimeoutMs);
        current.dispatchTimer = dispatchHandle;
        try {
          port.postMessage(request);
        } catch (error) {
          poison(error);
        }
      });
    },
  };
}

function artifactIdentity(
  artifact: RealmEngineArtifact
): RealmEngineArtifactIdentity {
  return Object.freeze({
    bytes: artifact.bytes,
    id: artifact.id,
    schemaVersion: artifact.schemaVersion,
    sha256: artifact.sha256,
  });
}

function validateCreationEvidence(
  evidence: BenchmarkRealmCreationEvidence,
  artifact: RealmEngineArtifact
): BenchmarkRealmCreationEvidence {
  if (
    evidence.clockBoundary !== "parent-before-sample" ||
    !sameArtifactIdentity(evidence.artifact, artifact)
  ) {
    throw new RealmProtocolError(
      "Benchmark realm creation evidence identity is invalid."
    );
  }
  for (const [name, value] of [
    ["artifactAcquisitionMs", evidence.artifactAcquisitionMs],
    ["realmBootstrapMs", evidence.realmBootstrapMs],
    ["totalMs", evidence.totalMs],
  ] as const) {
    if (!Number.isFinite(value) || value < 0 || value > REALM_BUDGETS.runTimeoutMs) {
      throw new RealmProtocolError(
        `Benchmark realm creation evidence ${name} is invalid.`
      );
    }
  }
  if (
    evidence.totalMs + Number.EPSILON <
    evidence.artifactAcquisitionMs + evidence.realmBootstrapMs
  ) {
    throw new RealmProtocolError(
      "Benchmark realm creation evidence total is inconsistent."
    );
  }
  return Object.freeze({
    artifact: artifactIdentity(artifact),
    artifactAcquisitionMs: evidence.artifactAcquisitionMs,
    clockBoundary: "parent-before-sample",
    realmBootstrapMs: evidence.realmBootstrapMs,
    totalMs: evidence.totalMs,
  });
}

function sameArtifactIdentity(
  left: RealmEngineArtifactIdentity,
  right: RealmEngineArtifactIdentity
): boolean {
  return (
    left.bytes === right.bytes &&
    left.id === right.id &&
    left.schemaVersion === right.schemaVersion &&
    left.sha256 === right.sha256
  );
}

function projectSample(
  response: BenchmarkSampleResponse,
  parentPublication: BenchmarkParentPublicationEvidence | null
): BenchmarkRealmSampleResult {
  if (response.type === "benchmark-sample-failure") {
    return Object.freeze(response);
  }
  if (parentPublication === null) {
    throw new RealmProtocolError(
      "Successful benchmark sample has no parent publication evidence."
    );
  }
  const { svg, ...evidence } = response;
  return Object.freeze({
    ...evidence,
    parentPublication,
    svgBytes: utf8ByteLength(svg),
  });
}
