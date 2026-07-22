import { createStore, type StoreApi } from "zustand/vanilla";
import {
  UNAVAILABLE_DIAGRAM_DETECTION,
  type DiagramDetectionFacts,
} from "@mermanjs/web";

import type {
  MermanDomainFacade,
  MermanRenderOptions,
} from "./merman-core.ts";
import {
  mermaidExternalRequirementsFor,
} from "./mermaid-requirements.ts";
import type {
  MermaidRealmController,
  MermaidRealmRenderResult,
  MermaidRealmRenderSuccess,
} from "./mermaid-realm-controller.ts";
import type {
  CompareFailureStage,
  RealmViewport,
} from "./realm/channel-protocol.ts";
import { projectError, type ErrorProjection } from "./error-projection.ts";

export interface RenderCoordinatorInput {
  readonly configJson: string;
  readonly facade: MermanDomainFacade | null;
  readonly options: MermanRenderOptions;
  readonly source: string;
  readonly theme: string;
}

export interface FrozenRenderSnapshot {
  readonly compareEnabled: boolean;
  readonly configJson: string;
  readonly diagnosticsEnabled: boolean;
  readonly key: string;
  readonly mermanVersion: string;
  readonly options: Readonly<MermanRenderOptions>;
  readonly requestId: number;
  readonly source: string;
  readonly theme: string;
  readonly viewport: RealmViewport | null;
}

export interface MermanRenderSuccess {
  readonly ascii: string | null;
  readonly asciiError: ErrorProjection | null;
  readonly engine: "merman";
  readonly presentedAt: number | null;
  readonly renderTimeMs: number;
  readonly status: "success";
  readonly svg: string;
}

export interface MermaidRenderSuccess extends MermaidRealmRenderSuccess {
  readonly engine: "mermaid";
  readonly presentedAt: number | null;
}

export interface MermanRenderFailure {
  readonly detail: string | null;
  readonly engine: "merman";
  readonly message: string;
  readonly stage: "render" | "svg-validation";
  readonly status: "failure";
}

export interface MermaidRenderFailure {
  readonly detail: string | null;
  readonly engine: "mermaid";
  readonly message: string;
  readonly stage: CompareFailureStage;
  readonly status: "failure";
}

export type EngineRenderFailure =
  | MermanRenderFailure
  | MermaidRenderFailure;

export interface DiagnosticArtifact {
  readonly elapsedMs: number | null;
  readonly error: string | null;
  readonly errorDetail: string | null;
  readonly json: string | null;
}

export interface RenderDiagnostics {
  readonly layout: DiagnosticArtifact;
  readonly parse: DiagnosticArtifact;
}

export type MermanBatchResult = MermanRenderSuccess | MermanRenderFailure;
export type MermaidBatchResult = MermaidRenderSuccess | MermaidRenderFailure;

interface CompletedBatchBase {
  readonly actionsEnabled: true;
  readonly detection: DiagramDetectionFacts;
  readonly diagnostics: RenderDiagnostics | null;
  readonly publishedAt: number;
  readonly snapshot: FrozenRenderSnapshot;
}

export type RenderSuccessState = CompletedBatchBase &
  (
    | {
        readonly status: "success";
        readonly merman: MermanRenderSuccess;
        readonly mermaid: null;
      }
    | {
        readonly status: "success";
        readonly merman: MermanRenderSuccess;
        readonly mermaid: MermaidRenderSuccess;
      }
  );

export type RenderPartialState = CompletedBatchBase &
  (
    | {
        readonly status: "partial";
        readonly merman: MermanRenderSuccess;
        readonly mermaid: MermaidRenderFailure;
      }
    | {
        readonly status: "partial";
        readonly merman: MermanRenderFailure;
        readonly mermaid: MermaidRenderSuccess;
      }
  );

export type RenderFailedState = CompletedBatchBase &
  (
    | {
        readonly status: "failed";
        readonly merman: MermanRenderFailure;
        readonly mermaid: null;
      }
    | {
        readonly status: "failed";
        readonly merman: MermanRenderFailure;
        readonly mermaid: MermaidRenderFailure;
      }
  );

export type CompletedRenderBatch =
  | RenderSuccessState
  | RenderPartialState
  | RenderFailedState;

export type RenderCoordinatorState =
  | { readonly status: "empty"; readonly actionsEnabled: false }
  | {
      readonly status: "pending";
      readonly actionsEnabled: false;
      readonly snapshot: FrozenRenderSnapshot;
    }
  | {
      readonly status: "updating";
      readonly actionsEnabled: false;
      readonly previous: CompletedRenderBatch;
      readonly snapshot: FrozenRenderSnapshot;
    }
  | CompletedRenderBatch;

export interface RenderCoordinator {
  readonly store: Pick<
    StoreApi<RenderCoordinatorState>,
    "getInitialState" | "getState" | "subscribe"
  >;
  dispose(): void;
  markPresented(
    requestId: number,
    engine: "merman" | "mermaid",
    at: number
  ): void;
  pause(): Promise<() => void>;
  refresh(): void;
  resume(): void;
  setCompareEnabled(enabled: boolean): void;
  setDiagnosticsEnabled(enabled: boolean): void;
  setInput(input: RenderCoordinatorInput): void;
  suspend(): void;
}

export interface RenderCoordinatorOptions {
  readonly compare: MermaidRealmController;
  readonly compareViewport: RealmViewport;
  readonly debounceMs?: number;
  readonly now?: () => number;
  readonly validateSvg: (svg: string) => void;
}

interface ScheduledRequest {
  readonly facade: MermanDomainFacade;
  readonly identity: RequestIdentity;
  readonly scheduledAt: number;
  readonly snapshot: FrozenRenderSnapshot;
}

interface RequestIdentity {
  readonly compareEnabled: boolean;
  readonly configJson: string;
  readonly diagnosticsEnabled: boolean;
  readonly diagramFont: MermanRenderOptions["diagramFont"];
  readonly hostThemePreset: MermanRenderOptions["hostThemePreset"];
  readonly pipeline: MermanRenderOptions["pipeline"];
  readonly source: string;
  readonly textMeasurementMode: MermanRenderOptions["textMeasurementMode"];
  readonly theme: string;
  readonly viewportHeight: number | null;
  readonly viewportWidth: number | null;
}

const EMPTY_STATE: RenderCoordinatorState = Object.freeze({
  status: "empty",
  actionsEnabled: false,
});
export function createRenderCoordinator({
  compare,
  compareViewport,
  debounceMs = 300,
  now = () => performance.now(),
  validateSvg,
}: RenderCoordinatorOptions): RenderCoordinator {
  const store = createStore<RenderCoordinatorState>(() => EMPTY_STATE);
  let disposed = false;
  let suspended = false;
  let pauseCount = 0;
  let compareEnabled = false;
  let diagnosticsEnabled = false;
  let requestSequence = 0;
  let currentInput: RenderCoordinatorInput | null = null;
  let latest: ScheduledRequest | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let active: Promise<void> | null = null;

  const replaceState = (state: RenderCoordinatorState) => {
    store.setState(state, true);
  };
  const clearTimer = () => {
    if (timer === null) return;
    clearTimeout(timer);
    timer = null;
  };
  const previousCompleted = (): CompletedRenderBatch | null => {
    const state = store.getState();
    if (isCompletedRenderState(state)) return state;
    return state.status === "updating" ? state.previous : null;
  };

  const scheduleCurrent = (force: boolean, immediate = false) => {
    if (disposed || !currentInput) return;
    const { facade, source } = currentInput;
    if (!facade || !source.trim()) {
      requestSequence += 1;
      latest = null;
      clearTimer();
      replaceState(EMPTY_STATE);
      return;
    }

    const identity = requestIdentity(
      currentInput,
      compareEnabled,
      diagnosticsEnabled,
      compareEnabled ? compareViewport : null
    );
    if (
      !force &&
      latest !== null &&
      sameRequestIdentity(latest.identity, identity) &&
      latest.facade === facade
    ) {
      return;
    }

    requestSequence += 1;
    const snapshot: FrozenRenderSnapshot = Object.freeze({
      compareEnabled,
      configJson: currentInput.configJson,
      diagnosticsEnabled,
      key: `render-${requestSequence}`,
      mermanVersion: facade.packageVersion,
      options: Object.freeze({ ...currentInput.options }),
      requestId: requestSequence,
      source,
      theme: currentInput.theme,
      viewport: compareEnabled ? Object.freeze({ ...compareViewport }) : null,
    });
    latest = {
      facade,
      identity,
      scheduledAt: now(),
      snapshot,
    };
    const previous = previousCompleted();
    replaceState(
      previous
        ? {
            status: "updating",
            actionsEnabled: false,
            previous,
            snapshot,
          }
        : { status: "pending", actionsEnabled: false, snapshot }
    );
    scheduleLatest(immediate);
  };

  const scheduleLatest = (immediate: boolean) => {
    clearTimer();
    if (
      disposed ||
      suspended ||
      pauseCount > 0 ||
      active ||
      !latest
    ) {
      return;
    }
    const remaining = immediate
      ? 0
      : Math.max(0, debounceMs - (now() - latest.scheduledAt));
    timer = setTimeout(() => {
      timer = null;
      const request = latest;
      if (!request || disposed || suspended || pauseCount > 0) return;
      const execution = execute(request)
        .then((completed) => {
          if (
            !disposed &&
            !suspended &&
            pauseCount === 0 &&
            latest?.snapshot.requestId === request.snapshot.requestId
          ) {
            replaceState(completed);
          }
        })
        .finally(() => {
          if (active === execution) active = null;
          if (
            latest &&
            latest.snapshot.requestId !== request.snapshot.requestId
          ) {
            scheduleLatest(false);
          }
        });
      active = execution;
    }, remaining);
  };

  const execute = async (
    request: ScheduledRequest
  ): Promise<CompletedRenderBatch> => {
    const { facade, snapshot } = request;
    const detection = detectDiagram(facade, snapshot);
    const externalRequirements = mermaidExternalRequirementsFor(detection);
    const comparePromise = renderCompare(
      compare,
      snapshot,
      externalRequirements
    );
    const merman = renderMerman(facade, snapshot, detection, validateSvg);
    const diagnostics = snapshot.diagnosticsEnabled
      ? collectDiagnostics(facade, snapshot, now)
      : null;
    const compareResult = await comparePromise;
    const mermaid = compareResult ? toMermaidBatchResult(compareResult) : null;
    return classifyBatch(
      snapshot,
      detection,
      diagnostics,
      merman,
      mermaid,
      now()
    );
  };

  const setInput = (input: RenderCoordinatorInput) => {
    currentInput = {
      ...input,
      options: { ...input.options },
    };
    scheduleCurrent(false);
  };
  const setCompareEnabled = (enabled: boolean) => {
    if (compareEnabled === enabled) return;
    compareEnabled = enabled;
    scheduleCurrent(true, true);
  };
  const setDiagnosticsEnabled = (enabled: boolean) => {
    if (diagnosticsEnabled === enabled) return;
    diagnosticsEnabled = enabled;
    scheduleCurrent(true, true);
  };
  const refresh = () => {
    scheduleCurrent(true, true);
  };
  const pause = async (): Promise<(() => void)> => {
    pauseCount += 1;
    clearTimer();
    await active;
    let released = false;
    return () => {
      if (released) return;
      released = true;
      pauseCount = Math.max(0, pauseCount - 1);
      if (pauseCount === 0) scheduleLatest(true);
    };
  };
  const suspend = () => {
    if (disposed || suspended) return;
    suspended = true;
    clearTimer();
    compare.reset();
  };
  const resume = () => {
    if (disposed || !suspended) return;
    suspended = false;
    scheduleCurrent(true, true);
  };
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    clearTimer();
    latest = null;
    currentInput = null;
    compare.dispose();
    replaceState(EMPTY_STATE);
  };
  const markPresented = (
    requestId: number,
    engine: "merman" | "mermaid",
    at: number
  ) => {
    if (!Number.isFinite(at)) return;
    const state = store.getState();
    if (
      !isCompletedRenderState(state) ||
      state.snapshot.requestId !== requestId
    ) {
      return;
    }
    const artifact = state[engine];
    if (
      !artifact ||
      artifact.status !== "success" ||
      artifact.presentedAt !== null
    ) {
      return;
    }
    replaceState({
      ...state,
      [engine]: { ...artifact, presentedAt: at },
    } as CompletedRenderBatch);
  };

  return {
    store,
    dispose,
    markPresented,
    pause,
    refresh,
    resume,
    setCompareEnabled,
    setDiagnosticsEnabled,
    setInput,
    suspend,
  };
}

function detectDiagram(
  facade: MermanDomainFacade,
  snapshot: FrozenRenderSnapshot
): DiagramDetectionFacts {
  try {
    return facade.detectDiagram(
      snapshot.source,
      snapshot.theme,
      snapshot.configJson,
      snapshot.options
    );
  } catch {
    return UNAVAILABLE_DIAGRAM_DETECTION;
  }
}

function renderCompare(
  compare: MermaidRealmController,
  snapshot: FrozenRenderSnapshot,
  externalRequirements: ReturnType<typeof mermaidExternalRequirementsFor>
): Promise<MermaidRealmRenderResult | null> {
  if (!snapshot.compareEnabled) return Promise.resolve(null);
  if (!snapshot.viewport) {
    return Promise.resolve({
      status: "failure",
      stage: "presentation",
      message: "Compare viewport is unavailable.",
      detail: null,
    });
  }
  return compare
    .render({
      source: snapshot.source,
      theme: snapshot.theme,
      configJson: snapshot.configJson,
      diagramFont: snapshot.options.diagramFont ?? "trebuchet",
      externalRequirements,
      viewport: snapshot.viewport,
    })
    .catch((error) => {
      const projection = projectError(error);
      return {
        status: "failure" as const,
        stage: "protocol" as const,
        message: projection.summary,
        detail: projection.detail,
      };
    });
}

function renderMerman(
  facade: MermanDomainFacade,
  snapshot: FrozenRenderSnapshot,
  detection: DiagramDetectionFacts,
  validateSvg: (svg: string) => void
): MermanBatchResult {
  let result;
  try {
    result = facade.render(
      snapshot.source,
      snapshot.theme,
      snapshot.configJson,
      snapshot.options
    );
  } catch (error) {
    return mermanFailure("render", error);
  }
  if (result.status === "failure") {
    return projectedMermanFailure("render", result.error);
  }
  try {
    validateSvg(result.svg);
  } catch (error) {
    return mermanFailure("svg-validation", error);
  }
  const diagramType =
    detection.status === "available" ? detection.diagramType : null;
  let ascii: string | null = null;
  let asciiError: ErrorProjection | null = null;
  try {
    if (
      diagramType &&
      facade
        .getAsciiSupportedDiagrams()
        .some((candidate) => candidate === diagramType)
    ) {
      const asciiResult = facade.renderAscii(
        snapshot.source,
        snapshot.theme,
        snapshot.configJson
      );
      if (asciiResult.status === "success") {
        ascii = asciiResult.ascii;
      } else {
        asciiError = normalizeErrorProjection(asciiResult.error);
      }
    }
  } catch (error) {
    ascii = null;
    asciiError = projectError(error);
  }
  return {
    status: "success",
    engine: "merman",
    svg: result.svg,
    ascii,
    asciiError,
    renderTimeMs: result.renderTime,
    presentedAt: null,
  };
}

function toMermaidBatchResult(
  result: MermaidRealmRenderResult
): MermaidBatchResult {
  if (result.status === "failure") {
    return mermaidFailure(result.stage, result.message, result.detail ?? null);
  }
  return {
    ...result,
    engine: "mermaid",
    presentedAt: null,
  };
}

function collectDiagnostics(
  facade: MermanDomainFacade,
  snapshot: FrozenRenderSnapshot,
  now: () => number
): RenderDiagnostics {
  return {
    parse: collectDiagnostic(
      () =>
        facade.parseJson(
          snapshot.source,
          snapshot.theme,
          snapshot.configJson,
          snapshot.options
        ),
      now
    ),
    layout: collectDiagnostic(
      () =>
        facade.layoutJson(
          snapshot.source,
          snapshot.theme,
          snapshot.configJson,
          snapshot.options
        ),
      now
    ),
  };
}

function collectDiagnostic(
  operation: () => string,
  now: () => number
): DiagnosticArtifact {
  const startedAt = now();
  try {
    return {
      json: formatDiagnosticJson(operation()),
      error: null,
      errorDetail: null,
      elapsedMs: now() - startedAt,
    };
  } catch (error) {
    const projection = projectError(error);
    return {
      json: null,
      error: projection.summary,
      errorDetail: projection.detail,
      elapsedMs: now() - startedAt,
    };
  }
}

function formatDiagnosticJson(rawJson: string): string {
  try {
    return `${JSON.stringify(JSON.parse(rawJson), null, 2)}\n`;
  } catch {
    return rawJson;
  }
}

function classifyBatch(
  snapshot: FrozenRenderSnapshot,
  detection: DiagramDetectionFacts,
  diagnostics: RenderDiagnostics | null,
  merman: MermanBatchResult,
  mermaid: MermaidBatchResult | null,
  publishedAt: number
): CompletedRenderBatch {
  const base = {
    actionsEnabled: true as const,
    detection,
    diagnostics,
    publishedAt,
    snapshot,
  };
  if (!mermaid) {
    return merman.status === "success"
      ? { ...base, status: "success", merman, mermaid: null }
      : { ...base, status: "failed", merman, mermaid: null };
  }
  if (merman.status === "success" && mermaid.status === "success") {
    return { ...base, status: "success", merman, mermaid };
  }
  if (merman.status === "failure" && mermaid.status === "failure") {
    return { ...base, status: "failed", merman, mermaid };
  }
  if (merman.status === "success" && mermaid.status === "failure") {
    return { ...base, status: "partial", merman, mermaid };
  }
  if (merman.status === "failure" && mermaid.status === "success") {
    return { ...base, status: "partial", merman, mermaid };
  }
  throw new Error("Render batch classification is not exhaustive.");
}

function mermanFailure(
  stage: MermanRenderFailure["stage"],
  error: unknown
): MermanRenderFailure {
  const projection = projectError(error);
  return {
    status: "failure",
    engine: "merman",
    stage,
    message: projection.summary,
    detail: projection.detail,
  };
}

function projectedMermanFailure(
  stage: MermanRenderFailure["stage"],
  error: unknown
): MermanRenderFailure {
  const projection = normalizeErrorProjection(error);
  return {
    status: "failure",
    engine: "merman",
    stage,
    message: projection.summary,
    detail: projection.detail,
  };
}

function normalizeErrorProjection(error: unknown): ErrorProjection {
  try {
    if (error && typeof error === "object") {
      const candidate = error as { detail?: unknown; summary?: unknown };
      const summary = candidate.summary;
      const detail = candidate.detail;
      if (
        typeof summary === "string" &&
        (detail === null || typeof detail === "string")
      ) {
        return {
          summary,
          detail,
        };
      }
    }
  } catch {
    // Fall through to the defensive projector for hostile realm values.
  }
  return projectError(error);
}

function mermaidFailure(
  stage: MermaidRenderFailure["stage"],
  message: string,
  detail: string | null
): MermaidRenderFailure {
  return { status: "failure", engine: "mermaid", stage, message, detail };
}

function requestIdentity(
  input: RenderCoordinatorInput,
  compareEnabled: boolean,
  diagnosticsEnabled: boolean,
  viewport: RealmViewport | null
): RequestIdentity {
  return {
    compareEnabled,
    configJson: input.configJson,
    diagnosticsEnabled,
    diagramFont: input.options.diagramFont,
    hostThemePreset: input.options.hostThemePreset,
    pipeline: input.options.pipeline,
    source: input.source,
    textMeasurementMode: input.options.textMeasurementMode,
    theme: input.theme,
    viewportHeight: viewport?.height ?? null,
    viewportWidth: viewport?.width ?? null,
  };
}

function sameRequestIdentity(
  left: RequestIdentity,
  right: RequestIdentity
): boolean {
  return (
    left.source === right.source &&
    left.theme === right.theme &&
    left.configJson === right.configJson &&
    left.hostThemePreset === right.hostThemePreset &&
    left.textMeasurementMode === right.textMeasurementMode &&
    left.diagramFont === right.diagramFont &&
    left.pipeline === right.pipeline &&
    left.compareEnabled === right.compareEnabled &&
    left.diagnosticsEnabled === right.diagnosticsEnabled &&
    left.viewportWidth === right.viewportWidth &&
    left.viewportHeight === right.viewportHeight
  );
}

export function isCompletedRenderState(
  state: RenderCoordinatorState
): state is CompletedRenderBatch {
  return (
    state.status === "success" ||
    state.status === "partial" ||
    state.status === "failed"
  );
}
