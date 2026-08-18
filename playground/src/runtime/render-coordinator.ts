import { createStore, type StoreApi } from "zustand/vanilla";
import {
  UNAVAILABLE_DIAGRAM_DETECTION,
  type DiagramDetectionFacts,
  type DiagramType,
  type SvgPlanResult,
} from "@mermanjs/web";

import type {
  MermanDomainFacade,
  MermanRenderFailureStage,
} from "./merman-core.ts";
import {
  freezeRenderOperation,
  type FreezeRenderOperationInput,
  type FrozenRenderOperation,
} from "./merman-operation-input.ts";
import type { WorkspaceSnapshot } from "../lib/workspace-snapshot.ts";
import { MERMAID_JS_VERSION } from "./mermaid-requirements.ts";
import { mermaidExternalRequirementsFor } from "./mermaid-requirements.ts";
import type {
  MermaidRealmController,
  MermaidRealmRenderResult,
  MermaidRealmRenderSuccess,
} from "./mermaid-realm-controller.ts";
import type { CompareFailureStage } from "./realm/channel-protocol.ts";
import type { CapturedRenderViewport } from "./render-viewport.ts";
import { projectError, type ErrorProjection } from "./error-projection.ts";
import { isAsciiSupported } from "../lib/ascii-support.ts";
import {
  assertNavigableInlineSvgArtifact,
  type NavigableInlineSvg,
} from "./render-artifact.ts";

export interface RenderCoordinatorInput {
  readonly facade: MermanDomainFacade | null;
  readonly renderViewport: Readonly<CapturedRenderViewport>;
  readonly workspace: Readonly<WorkspaceSnapshot>;
}

export interface FrozenRenderSnapshot {
  readonly operation: FrozenRenderOperation;
  readonly publicationId: RenderPublicationId;
}

export interface ScheduledRenderSnapshot {
  readonly publicationId: RenderPublicationId;
}

declare const RENDER_PUBLICATION_ID: unique symbol;
export type RenderPublicationId = number & {
  readonly [RENDER_PUBLICATION_ID]: "RenderPublicationId";
};

export interface MermanRenderSuccess {
  readonly artifact: NavigableInlineSvg;
  readonly engine: "merman";
  readonly presentedAt: number | null;
  readonly renderTimeMs: number;
  readonly status: "success";
}

export interface MermaidRenderSuccess extends MermaidRealmRenderSuccess {
  readonly engine: "mermaid";
  readonly presentedAt: number | null;
}

export interface MermanRenderFailure {
  readonly detail: string | null;
  readonly engine: "merman";
  readonly message: string;
  readonly stage: MermanRenderFailureStage;
  readonly status: "failure";
}

export interface MermaidRenderFailure {
  readonly detail: string | null;
  readonly engine: "mermaid";
  readonly message: string;
  readonly stage: CompareFailureStage;
  readonly status: "failure";
}

export type EngineRenderFailure = MermanRenderFailure | MermaidRenderFailure;

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

export type MermanAsciiBatchResult =
  | {
      readonly artifact: string;
      readonly status: "success";
    }
  | {
      readonly error: ErrorProjection;
      readonly status: "failure";
    }
  | {
      readonly diagramType: DiagramType;
      readonly status: "unsupported";
    }
  | {
      readonly reason: "diagram-detection-unavailable";
      readonly status: "unavailable";
    };

interface CompletedBatchBase {
  readonly ascii: MermanAsciiBatchResult | null;
  readonly detection: DiagramDetectionFacts;
  readonly diagnostics: RenderDiagnostics | null;
  readonly publishedAt: number;
  readonly snapshot: FrozenRenderSnapshot;
  readonly svgPlan: SvgPlanResult | null;
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
  RenderSuccessState | RenderPartialState | RenderFailedState;

export type RenderCoordinatorState =
  | { readonly status: "empty" }
  | {
      readonly status: "pending";
      readonly snapshot: ScheduledRenderSnapshot;
    }
  | {
      readonly status: "updating";
      readonly previous: CompletedRenderBatch;
      readonly snapshot: ScheduledRenderSnapshot;
    }
  | CompletedRenderBatch;

export interface RenderCoordinator {
  readonly store: Pick<
    StoreApi<RenderCoordinatorState>,
    "getInitialState" | "getState" | "subscribe"
  >;
  dispose(): void;
  markPresented(
    publicationId: RenderPublicationId,
    engine: "merman" | "mermaid",
    at: number,
  ): void;
  pause(): Promise<() => void>;
  refresh(): void;
  resume(): void;
  setFeatures(features: RenderFeatures): void;
  setInput(input: RenderCoordinatorInput): void;
  suspend(): void;
}

export interface RenderFeatures {
  readonly asciiEnabled: boolean;
  readonly compareEnabled: boolean;
  readonly diagnosticsEnabled: boolean;
}

export interface RenderCoordinatorOptions {
  readonly compare: MermaidRealmController;
  readonly debounceMs?: number;
  readonly freezeOperation?: (
    input: FreezeRenderOperationInput,
  ) => FrozenRenderOperation;
  readonly now?: () => number;
}

interface ScheduledRequest {
  readonly facade: MermanDomainFacade;
  readonly operationInput: FreezeRenderOperationInput;
  readonly publicationId: RenderPublicationId;
  readonly scheduledAt: number;
}

interface ActiveRequest {
  readonly facade: MermanDomainFacade;
  readonly snapshot: FrozenRenderSnapshot;
}

const EMPTY_STATE: RenderCoordinatorState = Object.freeze({
  status: "empty",
});
export function createRenderCoordinator({
  compare,
  debounceMs = 300,
  freezeOperation = freezeRenderOperation,
  now = () => performance.now(),
}: RenderCoordinatorOptions): RenderCoordinator {
  const store = createStore<RenderCoordinatorState>(() => EMPTY_STATE);
  let disposed = false;
  let suspended = false;
  let pauseCount = 0;
  let asciiEnabled = false;
  let compareEnabled = false;
  let diagnosticsEnabled = false;
  let requestSequence = 0;
  let currentInput: RenderCoordinatorInput | null = null;
  let latest: ScheduledRequest | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let active: Promise<void> | null = null;
  let activeRequest: ActiveRequest | null = null;

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
  const cancelActiveCompare = (): boolean => {
    if (activeRequest?.snapshot.operation.compareEnabled) {
      activeRequest = null;
      compare.reset();
      return true;
    }
    return false;
  };

  const scheduleCurrent = (force: boolean, immediate = false) => {
    if (disposed || !currentInput) return;
    const { facade, renderViewport, workspace } = currentInput;
    if (!facade || !workspace.code.trim()) {
      cancelActiveCompare();
      requestSequence += 1;
      latest = null;
      clearTimer();
      replaceState(EMPTY_STATE);
      return;
    }

    const operationInput = freezeScheduledOperationInput({
      asciiEnabled,
      compareEnabled,
      diagnosticsEnabled,
      layoutEnvironment: renderViewport.layoutEnvironment,
      versions: {
        merman: facade.packageVersion,
        mermaid: MERMAID_JS_VERSION,
      },
      viewport: compareEnabled ? renderViewport.viewport : null,
      workspace,
    });
    if (
      !force &&
      latest !== null &&
      sameScheduledOperationInput(latest.operationInput, operationInput) &&
      latest.facade === facade
    ) {
      return;
    }

    cancelActiveCompare();
    requestSequence += 1;
    const publicationId = requestSequence as RenderPublicationId;
    const snapshot: ScheduledRenderSnapshot = Object.freeze({
      publicationId,
    });
    latest = {
      facade,
      operationInput,
      publicationId,
      scheduledAt: now(),
    };
    const previous = previousCompleted();
    replaceState(
      previous
        ? {
            status: "updating",
            previous,
            snapshot,
          }
        : { status: "pending", snapshot },
    );
    scheduleLatest(immediate);
  };

  const scheduleLatest = (immediate: boolean) => {
    clearTimer();
    if (disposed || suspended || pauseCount > 0 || active || !latest) {
      return;
    }
    const remaining = immediate
      ? 0
      : Math.max(0, debounceMs - (now() - latest.scheduledAt));
    timer = setTimeout(() => {
      timer = null;
      const request = latest;
      if (!request || disposed || suspended || pauseCount > 0) return;
      const activeRequestForExecution: ActiveRequest = Object.freeze({
        facade: request.facade,
        snapshot: Object.freeze({
          operation: freezeOperation(request.operationInput),
          publicationId: request.publicationId,
        }),
      });
      activeRequest = activeRequestForExecution;
      const execution = execute(activeRequestForExecution)
        .then((completed) => {
          if (
            !disposed &&
            !suspended &&
            pauseCount === 0 &&
            latest?.publicationId === request.publicationId
          ) {
            replaceState(completed);
          }
        })
        .finally(() => {
          if (active === execution) {
            active = null;
            activeRequest = null;
          }
          if (
            latest &&
            latest.publicationId !== request.publicationId
          ) {
            scheduleLatest(false);
          }
        });
      active = execution;
    }, remaining);
  };

  const execute = async (
    request: ActiveRequest,
  ): Promise<CompletedRenderBatch> => {
    const { facade, snapshot } = request;
    const operation = snapshot.operation;
    const detection = detectDiagram(facade, operation);
    const svgPlan = collectSvgPlan(facade, operation);
    const externalRequirements = mermaidExternalRequirementsFor(detection);
    const comparePromise = renderCompare(
      compare,
      operation,
      externalRequirements,
    );
    const merman = renderMerman(facade, operation);
    const ascii = operation.asciiEnabled
      ? renderMermanAscii(facade, operation, detection)
      : null;
    const diagnostics = operation.diagnosticsEnabled
      ? collectDiagnostics(facade, operation, now)
      : null;
    const compareResult = await comparePromise;
    const mermaid = compareResult
      ? toMermaidBatchResult(compareResult, operation)
      : null;
    return classifyBatch(
      snapshot,
      detection,
      diagnostics,
      svgPlan,
      merman,
      ascii,
      mermaid,
      now(),
    );
  };

  const setInput = (input: RenderCoordinatorInput) => {
    currentInput = input;
    scheduleCurrent(false);
  };
  const setFeatures = (features: RenderFeatures) => {
    const shouldSchedule =
      compareEnabled !== features.compareEnabled ||
      diagnosticsEnabled !== features.diagnosticsEnabled ||
      (!asciiEnabled && features.asciiEnabled);
    if (
      asciiEnabled === features.asciiEnabled &&
      compareEnabled === features.compareEnabled &&
      diagnosticsEnabled === features.diagnosticsEnabled
    ) {
      return;
    }
    asciiEnabled = features.asciiEnabled;
    compareEnabled = features.compareEnabled;
    diagnosticsEnabled = features.diagnosticsEnabled;
    if (!shouldSchedule) return;
    scheduleCurrent(true, true);
  };
  const refresh = () => {
    scheduleCurrent(true, true);
  };
  const pause = async (): Promise<() => void> => {
    pauseCount += 1;
    clearTimer();
    cancelActiveCompare();
    try {
      await active;
    } catch (error) {
      pauseCount = Math.max(0, pauseCount - 1);
      if (pauseCount === 0) scheduleLatest(true);
      throw error;
    }
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
    if (!cancelActiveCompare()) compare.reset();
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
    publicationId: RenderPublicationId,
    engine: "merman" | "mermaid",
    at: number,
  ) => {
    if (!Number.isFinite(at)) return;
    const state = store.getState();
    if (
      !isCompletedRenderState(state) ||
      state.snapshot.publicationId !== publicationId
    ) {
      return;
    }
    if (engine === "merman") {
      if (
        state.merman.status !== "success" ||
        state.merman.presentedAt !== null
      ) {
        return;
      }
      replaceState(
        classifyBatch(
          state.snapshot,
          state.detection,
          state.diagnostics,
          state.svgPlan,
          Object.freeze({ ...state.merman, presentedAt: at }),
          state.ascii,
          state.mermaid,
          state.publishedAt,
        ),
      );
      return;
    }
    if (
      !state.mermaid ||
      state.mermaid.status !== "success" ||
      state.mermaid.presentedAt !== null
    ) {
      return;
    }
    replaceState(
      classifyBatch(
        state.snapshot,
        state.detection,
        state.diagnostics,
        state.svgPlan,
        state.merman,
        state.ascii,
        Object.freeze({ ...state.mermaid, presentedAt: at }),
        state.publishedAt,
      ),
    );
  };

  return {
    store,
    dispose,
    markPresented,
    pause,
    refresh,
    resume,
    setFeatures,
    setInput,
    suspend,
  };
}

function freezeScheduledOperationInput({
  asciiEnabled,
  compareEnabled,
  diagnosticsEnabled,
  layoutEnvironment,
  versions,
  viewport,
  workspace,
}: FreezeRenderOperationInput): FreezeRenderOperationInput {
  return Object.freeze({
    asciiEnabled,
    compareEnabled,
    diagnosticsEnabled,
    layoutEnvironment: Object.freeze({ ...layoutEnvironment }),
    versions: Object.freeze({ ...versions }),
    viewport: viewport ? Object.freeze({ ...viewport }) : null,
    workspace: Object.freeze({ ...workspace }),
  });
}

function sameScheduledOperationInput(
  left: FreezeRenderOperationInput,
  right: FreezeRenderOperationInput,
): boolean {
  return (
    left.asciiEnabled === right.asciiEnabled &&
    left.compareEnabled === right.compareEnabled &&
    left.diagnosticsEnabled === right.diagnosticsEnabled &&
    left.layoutEnvironment.containerWidth ===
      right.layoutEnvironment.containerWidth &&
    left.layoutEnvironment.containerHeight ===
      right.layoutEnvironment.containerHeight &&
    (left.layoutEnvironment.screenAvailableWidth ?? null) ===
      (right.layoutEnvironment.screenAvailableWidth ?? null) &&
    left.versions.merman === right.versions.merman &&
    left.versions.mermaid === right.versions.mermaid &&
    (left.viewport?.width ?? null) === (right.viewport?.width ?? null) &&
    (left.viewport?.height ?? null) === (right.viewport?.height ?? null) &&
    left.workspace.code === right.workspace.code &&
    left.workspace.mermaidConfig === right.workspace.mermaidConfig &&
    left.workspace.diagramTheme === right.workspace.diagramTheme &&
    left.workspace.presentationThemePresetId ===
      right.workspace.presentationThemePresetId &&
    left.workspace.presentationProfileId ===
      right.workspace.presentationProfileId &&
    left.workspace.svgPipeline === right.workspace.svgPipeline &&
    left.workspace.textMeasurementMode ===
      right.workspace.textMeasurementMode &&
    left.workspace.diagramFont === right.workspace.diagramFont
  );
}

function collectSvgPlan(
  facade: MermanDomainFacade,
  operation: FrozenRenderOperation,
): SvgPlanResult | null {
  if (!operation.presentationProfileId) {
    return null;
  }

  try {
    return freezeSvgPlan(facade.svgPlan(operation));
  } catch {
    return null;
  }
}

function detectDiagram(
  facade: MermanDomainFacade,
  operation: FrozenRenderOperation,
): DiagramDetectionFacts {
  try {
    return freezeDetection(facade.detectDiagram(operation));
  } catch {
    return UNAVAILABLE_DIAGRAM_DETECTION;
  }
}

function renderCompare(
  compare: MermaidRealmController,
  operation: FrozenRenderOperation,
  externalRequirements: ReturnType<typeof mermaidExternalRequirementsFor>,
): Promise<MermaidRealmRenderResult | null> {
  if (!operation.compareEnabled) return Promise.resolve(null);
  if (!operation.viewport) {
    return Promise.resolve({
      status: "failure",
      stage: "presentation",
      message: "Compare viewport is unavailable.",
      detail: null,
    });
  }
  const screenAvailableWidth =
    operation.layoutEnvironment.screenAvailableWidth;
  if (screenAvailableWidth === undefined) {
    return Promise.resolve({
      status: "failure",
      stage: "presentation",
      message: "Compare screen width is unavailable.",
      detail: null,
    });
  }
  let result: Promise<MermaidRealmRenderResult>;
  try {
    result = compare.render({
      source: operation.source,
      theme: operation.theme,
      configJson: operation.configJson,
      diagramFont: operation.diagramFont,
      externalRequirements,
      screenAvailableWidth,
      viewport: operation.viewport,
    });
  } catch (error) {
    const projection = projectError(error);
    return Promise.resolve({
      status: "failure",
      stage: "protocol",
      message: projection.summary,
      detail: projection.detail,
    });
  }
  return result.catch((error) => {
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
  operation: FrozenRenderOperation,
): MermanBatchResult {
  let result;
  try {
    result = facade.render(operation);
  } catch (error) {
    return mermanFailure("render", error);
  }
  if (result.status === "failure") {
    return mermanFailure(result.stage, result.error);
  }
  try {
    assertNavigableInlineSvgArtifact(result.artifact);
  } catch (error) {
    return mermanFailure("svg-validation", error);
  }
  return Object.freeze({
    status: "success",
    engine: "merman",
    artifact: result.artifact,
    renderTimeMs: result.renderTime,
    presentedAt: null,
  });
}

function renderMermanAscii(
  facade: MermanDomainFacade,
  operation: FrozenRenderOperation,
  detection: DiagramDetectionFacts,
): MermanAsciiBatchResult {
  if (operation.configurationError) {
    return Object.freeze({
      status: "failure",
      error: operation.configurationError,
    });
  }
  if (detection.status !== "available") {
    return Object.freeze({
      status: "unavailable",
      reason: "diagram-detection-unavailable",
    });
  }
  try {
    if (
      !isAsciiSupported(
        detection.diagramType,
        facade.getAsciiSupportedDiagrams(),
      )
    ) {
      return Object.freeze({
        status: "unsupported",
        diagramType: detection.diagramType,
      });
    }
    const result = facade.renderAscii(operation);
    if (result.status === "success") {
      return Object.freeze({ status: "success", artifact: result.ascii });
    }
    return Object.freeze({
      status: "failure",
      error: projectError(result.error),
    });
  } catch (error) {
    return Object.freeze({ status: "failure", error: projectError(error) });
  }
}

function toMermaidBatchResult(
  result: MermaidRealmRenderResult,
  operation: FrozenRenderOperation,
): MermaidBatchResult {
  if (result.status === "failure") {
    return mermaidFailure(result.stage, result.message, result.detail ?? null);
  }
  if (result.version !== operation.versions.mermaid) {
    return mermaidFailure(
      "protocol",
      `Mermaid realm version mismatch: expected ${operation.versions.mermaid}, received ${result.version}.`,
      null,
    );
  }
  return Object.freeze({
    ...result,
    engine: "mermaid",
    presentedAt: null,
  });
}

function collectDiagnostics(
  facade: MermanDomainFacade,
  operation: FrozenRenderOperation,
  now: () => number,
): RenderDiagnostics {
  return Object.freeze({
    parse: collectDiagnostic(() => facade.parseJson(operation), now),
    layout: collectDiagnostic(() => facade.layoutJson(operation), now),
  });
}

function collectDiagnostic(
  operation: () => string,
  now: () => number,
): DiagnosticArtifact {
  const startedAt = now();
  try {
    return Object.freeze({
      json: formatDiagnosticJson(operation()),
      error: null,
      errorDetail: null,
      elapsedMs: now() - startedAt,
    });
  } catch (error) {
    const projection = projectError(error);
    return Object.freeze({
      json: null,
      error: projection.summary,
      errorDetail: projection.detail,
      elapsedMs: now() - startedAt,
    });
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
  svgPlan: SvgPlanResult | null,
  merman: MermanBatchResult,
  ascii: MermanAsciiBatchResult | null,
  mermaid: MermaidBatchResult | null,
  publishedAt: number,
): CompletedRenderBatch {
  const base = {
    ascii,
    detection,
    diagnostics,
    publishedAt,
    snapshot,
    svgPlan,
  };
  if (!mermaid) {
    return merman.status === "success"
      ? Object.freeze({ ...base, status: "success", merman, mermaid: null })
      : Object.freeze({ ...base, status: "failed", merman, mermaid: null });
  }
  if (merman.status === "success" && mermaid.status === "success") {
    return Object.freeze({ ...base, status: "success", merman, mermaid });
  }
  if (merman.status === "failure" && mermaid.status === "failure") {
    return Object.freeze({ ...base, status: "failed", merman, mermaid });
  }
  if (merman.status === "success" && mermaid.status === "failure") {
    return Object.freeze({ ...base, status: "partial", merman, mermaid });
  }
  if (merman.status === "failure" && mermaid.status === "success") {
    return Object.freeze({ ...base, status: "partial", merman, mermaid });
  }
  throw new Error("Render batch classification is not exhaustive.");
}

function mermanFailure(
  stage: MermanRenderFailure["stage"],
  error: unknown,
): MermanRenderFailure {
  const projection = projectError(error);
  return Object.freeze({
    status: "failure",
    engine: "merman",
    stage,
    message: projection.summary,
    detail: projection.detail,
  });
}

function mermaidFailure(
  stage: MermaidRenderFailure["stage"],
  message: string,
  detail: string | null,
): MermaidRenderFailure {
  return Object.freeze({
    status: "failure",
    engine: "mermaid",
    stage,
    message,
    detail,
  });
}

function freezeDetection(
  detection: DiagramDetectionFacts,
): DiagramDetectionFacts {
  return Object.freeze({ ...detection });
}

function freezeSvgPlan(plan: SvgPlanResult): SvgPlanResult {
  const presentationAspects = plan.presentation_aspects.map((aspect) =>
    Object.freeze({ ...aspect }),
  );
  const requiredCapabilityIds = [...plan.required_capability_ids];
  const missingCapabilityIds = [...plan.missing_capability_ids];
  Object.freeze(presentationAspects);
  Object.freeze(requiredCapabilityIds);
  Object.freeze(missingCapabilityIds);
  return Object.freeze({
    ...plan,
    presentation_aspects: presentationAspects,
    required_capability_ids: requiredCapabilityIds,
    missing_capability_ids: missingCapabilityIds,
  });
}

export function isCompletedRenderState(
  state: RenderCoordinatorState,
): state is CompletedRenderBatch {
  return (
    state.status === "success" ||
    state.status === "partial" ||
    state.status === "failed"
  );
}
