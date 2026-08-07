import { createStore, type StoreApi } from "zustand/vanilla";
import {
  UNAVAILABLE_DIAGRAM_DETECTION,
  type DiagramDetectionFacts,
  type SvgPlanResult,
} from "@mermanjs/web";

import type {
  MermanDomainFacade,
  MermanLayoutEnvironment,
  MermanRenderFailureStage,
} from "./merman-core.ts";
import {
  freezeRenderOperation,
  sameRenderOperation,
  type FrozenRenderOperation,
} from "./merman-operation-input.ts";
import type { WorkspaceSnapshot } from "../lib/workspace-snapshot.ts";
import { MERMAID_JS_VERSION } from "./mermaid-requirements.ts";
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
import {
  assertNavigableInlineSvgArtifact,
  type NavigableInlineSvg,
} from "./render-artifact.ts";

export interface RenderCoordinatorInput {
  readonly facade: MermanDomainFacade | null;
  readonly workspace: Readonly<WorkspaceSnapshot>;
}

export interface FrozenRenderSnapshot {
  readonly operation: FrozenRenderOperation;
  readonly publicationId: RenderPublicationId;
}

declare const RENDER_PUBLICATION_ID: unique symbol;
export type RenderPublicationId = number & {
  readonly [RENDER_PUBLICATION_ID]: "RenderPublicationId";
};

export interface MermanRenderSuccess {
  readonly artifact: NavigableInlineSvg;
  readonly ascii: string | null;
  readonly asciiError: ErrorProjection | null;
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
  | RenderSuccessState
  | RenderPartialState
  | RenderFailedState;

export type RenderCoordinatorState =
  | { readonly status: "empty" }
  | {
      readonly status: "pending";
      readonly snapshot: FrozenRenderSnapshot;
    }
  | {
      readonly status: "updating";
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
    publicationId: RenderPublicationId,
    engine: "merman" | "mermaid",
    at: number
  ): void;
  pause(): Promise<() => void>;
  refresh(): void;
  resume(): void;
  setFeatures(features: RenderFeatures): void;
  setInput(input: RenderCoordinatorInput): void;
  suspend(): void;
}

export interface RenderFeatures {
  readonly compareEnabled: boolean;
  readonly diagnosticsEnabled: boolean;
}

export interface RenderCoordinatorOptions {
  readonly captureLayoutEnvironment?: () => MermanLayoutEnvironment;
  readonly compare: MermaidRealmController;
  readonly compareViewport: RealmViewport;
  readonly debounceMs?: number;
  readonly now?: () => number;
}

interface ScheduledRequest {
  readonly facade: MermanDomainFacade;
  readonly scheduledAt: number;
  readonly snapshot: FrozenRenderSnapshot;
}

const EMPTY_STATE: RenderCoordinatorState = Object.freeze({
  status: "empty",
});
export function createRenderCoordinator({
  captureLayoutEnvironment,
  compare,
  compareViewport,
  debounceMs = 300,
  now = () => performance.now(),
}: RenderCoordinatorOptions): RenderCoordinator {
  const captureEnvironment =
    captureLayoutEnvironment ??
    (() => ({
      containerWidth: compareViewport.width,
      containerHeight: compareViewport.height,
    }));
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
    const { facade, workspace } = currentInput;
    if (!facade || !workspace.code.trim()) {
      requestSequence += 1;
      latest = null;
      clearTimer();
      replaceState(EMPTY_STATE);
      return;
    }

    const operation = freezeRenderOperation({
      compareEnabled,
      diagnosticsEnabled,
      layoutEnvironment: captureEnvironment(),
      versions: {
        merman: facade.packageVersion,
        mermaid: MERMAID_JS_VERSION,
      },
      viewport: compareEnabled ? compareViewport : null,
      workspace,
    });
    if (
      !force &&
      latest !== null &&
      sameRenderOperation(latest.snapshot.operation, operation) &&
      latest.facade === facade
    ) {
      return;
    }

    requestSequence += 1;
    const snapshot: FrozenRenderSnapshot = Object.freeze({
      operation,
      publicationId: requestSequence as RenderPublicationId,
    });
    latest = {
      facade,
      scheduledAt: now(),
      snapshot,
    };
    const previous = previousCompleted();
    replaceState(
      previous
        ? {
            status: "updating",
            previous,
            snapshot,
          }
        : { status: "pending", snapshot }
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
            latest?.snapshot.publicationId === request.snapshot.publicationId
          ) {
            replaceState(completed);
          }
        })
        .finally(() => {
          if (active === execution) active = null;
          if (
            latest &&
            latest.snapshot.publicationId !== request.snapshot.publicationId
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
    const operation = snapshot.operation;
    const detection = detectDiagram(facade, operation);
    const svgPlan = collectSvgPlan(facade, operation);
    const externalRequirements = mermaidExternalRequirementsFor(detection);
    const comparePromise = renderCompare(
      compare,
      operation,
      externalRequirements
    );
    const merman = renderMerman(facade, operation, detection);
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
      mermaid,
      now()
    );
  };

  const setInput = (input: RenderCoordinatorInput) => {
    currentInput = input;
    scheduleCurrent(false);
  };
  const setFeatures = (features: RenderFeatures) => {
    if (
      compareEnabled === features.compareEnabled &&
      diagnosticsEnabled === features.diagnosticsEnabled
    ) {
      return;
    }
    compareEnabled = features.compareEnabled;
    diagnosticsEnabled = features.diagnosticsEnabled;
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
    publicationId: RenderPublicationId,
    engine: "merman" | "mermaid",
    at: number
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
          state.mermaid,
          state.publishedAt
        )
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
        Object.freeze({ ...state.mermaid, presentedAt: at }),
        state.publishedAt
      )
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

function collectSvgPlan(
  facade: MermanDomainFacade,
  operation: FrozenRenderOperation
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
  operation: FrozenRenderOperation
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
  externalRequirements: ReturnType<typeof mermaidExternalRequirementsFor>
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
  return compare
    .render({
      source: operation.source,
      theme: operation.theme,
      configJson: operation.configJson,
      diagramFont: operation.diagramFont,
      externalRequirements,
      viewport: operation.viewport,
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
  operation: FrozenRenderOperation,
  detection: DiagramDetectionFacts
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
        operation
      );
      if (asciiResult.status === "success") {
        ascii = asciiResult.ascii;
      } else {
        asciiError = projectError(asciiResult.error);
      }
    }
  } catch (error) {
    ascii = null;
    asciiError = projectError(error);
  }
  return Object.freeze({
    status: "success",
    engine: "merman",
    artifact: result.artifact,
    ascii,
    asciiError,
    renderTimeMs: result.renderTime,
    presentedAt: null,
  });
}

function toMermaidBatchResult(
  result: MermaidRealmRenderResult,
  operation: FrozenRenderOperation
): MermaidBatchResult {
  if (result.status === "failure") {
    return mermaidFailure(result.stage, result.message, result.detail ?? null);
  }
  if (result.version !== operation.versions.mermaid) {
    return mermaidFailure(
      "protocol",
      `Mermaid realm version mismatch: expected ${operation.versions.mermaid}, received ${result.version}.`,
      null
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
  now: () => number
): RenderDiagnostics {
  return Object.freeze({
    parse: collectDiagnostic(
      () =>
        facade.parseJson(operation),
      now
    ),
    layout: collectDiagnostic(
      () =>
        facade.layoutJson(operation),
      now
    ),
  });
}

function collectDiagnostic(
  operation: () => string,
  now: () => number
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
  mermaid: MermaidBatchResult | null,
  publishedAt: number
): CompletedRenderBatch {
  const base = {
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
  error: unknown
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
  detail: string | null
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
  detection: DiagramDetectionFacts
): DiagramDetectionFacts {
  return Object.freeze({ ...detection });
}

function freezeSvgPlan(plan: SvgPlanResult): SvgPlanResult {
  const presentationAspects = plan.presentation_aspects.map((aspect) =>
    Object.freeze({ ...aspect })
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
  state: RenderCoordinatorState
): state is CompletedRenderBatch {
  return (
    state.status === "success" ||
    state.status === "partial" ||
    state.status === "failed"
  );
}
