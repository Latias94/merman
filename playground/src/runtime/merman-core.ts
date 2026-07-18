import { createStore, type StoreApi } from "zustand/vanilla";

import type {
  AsciiCapability,
  AsciiDiagramType,
  BindingCapabilities,
  DiagramType,
  EditorCodeAction,
  EditorCompletionList,
  EditorDiagnosticsResult,
  EditorDocumentSymbol,
  EditorHover,
  EditorLocation,
  EditorPosition,
  EditorPrepareRename,
  EditorSemanticToken,
  EditorSemanticTokenLegend,
  EditorWorkspaceEdit,
  HostThemePresetName,
  RegistryProfile,
  ThemeName,
  ValidationResult,
} from "@mermanjs/web";
import type { DiagramFont } from "../lib/diagram-font.ts";

export type MermanLoadStage =
  | "acquire"
  | "module-import"
  | "wasm-fetch"
  | "response-validation"
  | "initialize"
  | "session";

export type MermanRecovery = "reload" | "retry";
export type MermanRequestCache = "default" | "reload";
export type MermanSvgPipeline = "parity" | "readable" | "resvg-safe";
export type MermanTextMeasurementMode = "browser" | "headless";

export interface MermanRenderOptions {
  diagramFont?: DiagramFont;
  hostThemePreset?: HostThemePresetName;
  pipeline?: MermanSvgPipeline;
  textMeasurementMode?: MermanTextMeasurementMode;
}

export interface MermanRenderResult {
  error: string | null;
  renderTime: number;
  svg: string | null;
}

export interface MermanEditorService {
  editorCodeActions(code: string): EditorCodeAction[];
  editorCompletions(code: string, position: EditorPosition): EditorCompletionList;
  editorDefinition(code: string, position: EditorPosition): EditorLocation | null;
  editorDiagnostics(code: string): EditorDiagnosticsResult;
  editorDocumentSymbols(code: string): EditorDocumentSymbol[];
  editorHover(code: string, position: EditorPosition): EditorHover | null;
  editorPrepareRename(
    code: string,
    position: EditorPosition
  ): EditorPrepareRename | null;
  editorReferences(
    code: string,
    position: EditorPosition,
    includeDeclaration: boolean
  ): EditorLocation[];
  editorRename(
    code: string,
    position: EditorPosition,
    newName: string
  ): EditorWorkspaceEdit | null;
  editorSemanticTokenLegend(): EditorSemanticTokenLegend;
  editorSemanticTokens(code: string): EditorSemanticToken[];
}

export interface MermanDomainFacade extends MermanEditorService {
  readonly packageVersion: string;
  bindingCapabilities(): BindingCapabilities;
  getAsciiCapabilities(): AsciiCapability[];
  getAsciiSupportedDiagrams(): AsciiDiagramType[];
  getSupportedDiagrams(): DiagramType[];
  getThemes(): ThemeName[];
  layoutJson(
    code: string,
    theme?: string,
    configJson?: string,
    options?: MermanRenderOptions
  ): string;
  parseJson(
    code: string,
    theme?: string,
    configJson?: string,
    options?: MermanRenderOptions
  ): string;
  registryProfile(): RegistryProfile;
  render(
    code: string,
    theme: string,
    configJson?: string,
    options?: MermanRenderOptions
  ): MermanRenderResult;
  renderAscii(code: string, theme?: string, configJson?: string): string | null;
  validate(code: string): ValidationResult;
}

export interface MermanSession {
  readonly facade: MermanDomainFacade;
  dispose(): void;
}

export interface MermanRuntimeFailure {
  readonly cause: unknown;
  readonly message: string;
  readonly recovery: MermanRecovery;
  readonly stage: MermanLoadStage;
}

interface MermanRuntimeStateBase {
  readonly suspended: boolean;
}

export type MermanRuntimeState =
  | (MermanRuntimeStateBase & { readonly status: "idle" })
  | (MermanRuntimeStateBase & {
      readonly stage: MermanLoadStage;
      readonly status: "loading";
    })
  | (MermanRuntimeStateBase & {
      readonly facade: MermanDomainFacade;
      readonly status: "ready";
    })
  | (MermanRuntimeStateBase & {
      readonly error: MermanRuntimeFailure;
      readonly status: "error";
    });

export interface MermanRuntimeDependencies {
  createSession(): MermanSession;
  fetchWasm(input: {
    cache: MermanRequestCache;
    signal: AbortSignal;
  }): Promise<Response>;
  initialize(input: { module: unknown; wasm: Response }): Promise<void>;
  isInitialized(): boolean;
  isRetryableInitializationError(error: unknown): boolean;
  loadModule(): Promise<unknown>;
}

export type MermanRuntimeStore = Pick<
  StoreApi<MermanRuntimeState>,
  "getInitialState" | "getState" | "subscribe"
>;

export interface MermanRuntime {
  readonly store: MermanRuntimeStore;
  dispose(): void;
  ensureReady(): Promise<MermanDomainFacade>;
  resume(): Promise<MermanDomainFacade>;
  retry(): Promise<MermanDomainFacade>;
  suspend(): void;
}

class StagedRuntimeError extends Error {
  readonly cause: unknown;
  readonly recovery: MermanRecovery;
  readonly stage: MermanLoadStage;

  constructor(
    stage: MermanLoadStage,
    recovery: MermanRecovery,
    cause: unknown
  ) {
    super(errorMessage(cause));
    this.name = "StagedRuntimeError";
    this.cause = cause;
    this.recovery = recovery;
    this.stage = stage;
  }
}

class SupersededRuntimeError extends Error {
  constructor() {
    super("Merman runtime attempt was superseded.");
    this.name = "SupersededRuntimeError";
  }
}

export function createMermanRuntime(
  dependencies: MermanRuntimeDependencies
): MermanRuntime {
  const store = createStore<MermanRuntimeState>(() => ({
    status: "idle",
    suspended: false,
  }));
  const replaceState = (state: MermanRuntimeState) => {
    store.setState(state, true);
  };
  let attemptId = 0;
  let abortController: AbortController | null = null;
  let inFlight: Promise<MermanDomainFacade> | null = null;
  let session: MermanSession | null = null;

  const isCurrent = (candidate: number) => candidate === attemptId;

  const start = (): Promise<MermanDomainFacade> => {
    if (store.getState().suspended) {
      return Promise.reject(new Error("Merman runtime is suspended."));
    }

    attemptId += 1;
    const currentAttempt = attemptId;
    const controller = new AbortController();
    abortController = controller;
    replaceState({
      stage: "acquire",
      status: "loading",
      suspended: false,
    });

    const pending = runAttempt(
      dependencies,
      controller.signal,
      (stage) => {
        if (!isCurrent(currentAttempt)) return;
        replaceState({
          stage,
          status: "loading",
          suspended: false,
        });
      }
    )
      .then((nextSession) => {
        if (!isCurrent(currentAttempt)) {
          nextSession.dispose();
          throw new SupersededRuntimeError();
        }
        session = nextSession;
        replaceState({
          facade: nextSession.facade,
          status: "ready",
          suspended: false,
        });
        return nextSession.facade;
      })
      .catch((error: unknown) => {
        controller.abort();
        if (!isCurrent(currentAttempt) || error instanceof SupersededRuntimeError) {
          throw error instanceof SupersededRuntimeError
            ? error
            : new SupersededRuntimeError();
        }
        const staged = toStagedError(error);
        replaceState({
          error: {
            cause: staged.cause,
            message: staged.message,
            recovery: staged.recovery,
            stage: staged.stage,
          },
          status: "error",
          suspended: false,
        });
        throw staged.cause instanceof Error ? staged.cause : staged;
      })
      .finally(() => {
        if (inFlight === pending) {
          inFlight = null;
          abortController = null;
        }
      });

    inFlight = pending;
    return pending;
  };

  const ensureReady = (): Promise<MermanDomainFacade> => {
    const state = store.getState();
    if (state.status === "ready") {
      return Promise.resolve(state.facade);
    }
    if (inFlight) {
      return inFlight;
    }
    if (state.status === "error") {
      return Promise.reject(runtimeFailureError(state.error));
    }
    return start();
  };

  const dispose = (): void => {
    const state = store.getState();
    if (
      state.status === "idle" &&
      abortController === null &&
      inFlight === null &&
      session === null
    ) {
      return;
    }
    attemptId += 1;
    abortController?.abort();
    abortController = null;
    inFlight = null;
    session?.dispose();
    session = null;
    replaceState({
      status: "idle",
      suspended: false,
    });
  };

  const suspend = (): void => {
    const state = store.getState();
    if (state.suspended) {
      return;
    }
    if (state.status === "loading") {
      attemptId += 1;
      abortController?.abort();
      abortController = null;
      inFlight = null;
      replaceState({ status: "idle", suspended: true });
      return;
    }
    replaceState({ ...state, suspended: true });
  };

  const resume = (): Promise<MermanDomainFacade> => {
    const state = store.getState();
    if (state.suspended) {
      replaceState({ ...state, suspended: false });
    }
    if (state.status === "ready") {
      return Promise.resolve(state.facade);
    }
    if (state.status === "error" && state.error.recovery === "reload") {
      return Promise.reject(runtimeFailureError(state.error));
    }
    if (state.status === "error") {
      return retry();
    }
    return ensureReady();
  };

  const retry = (): Promise<MermanDomainFacade> => {
    const state = store.getState();
    if (state.suspended) {
      return Promise.reject(new Error("Merman runtime is suspended."));
    }
    if (state.status === "error" && state.error.recovery === "reload") {
      return Promise.reject(runtimeFailureError(state.error));
    }
    if (state.status === "ready") {
      return Promise.resolve(state.facade);
    }
    if (inFlight) {
      return inFlight;
    }
    return start();
  };

  return { dispose, ensureReady, resume, retry, store, suspend };
}

async function runAttempt(
  dependencies: MermanRuntimeDependencies,
  signal: AbortSignal,
  setStage: (stage: MermanLoadStage) => void
): Promise<MermanSession> {
  let initialized: boolean;
  try {
    initialized = dependencies.isInitialized();
  } catch (error) {
    throw new StagedRuntimeError("module-import", "reload", error);
  }

  if (!initialized) {
    const modulePromise = dependencies.loadModule().catch((error) => {
      throw new StagedRuntimeError("module-import", "reload", error);
    });
    const wasmPromise = dependencies
      .fetchWasm({ cache: "default", signal })
      .catch((error) => {
        throw new StagedRuntimeError("wasm-fetch", "retry", error);
      });

    const [module, firstResponse] = await Promise.all([modulePromise, wasmPromise]);
    setStage("response-validation");
    validateWasmResponse(firstResponse);
    setStage("initialize");
    try {
      await dependencies.initialize({ module, wasm: firstResponse });
    } catch (error) {
      if (!dependencies.isRetryableInitializationError(error)) {
        throw new StagedRuntimeError("initialize", "retry", error);
      }
      let reloadResponse: Response;
      try {
        setStage("wasm-fetch");
        reloadResponse = await dependencies.fetchWasm({ cache: "reload", signal });
      } catch (reloadError) {
        throw new StagedRuntimeError("wasm-fetch", "retry", reloadError);
      }
      setStage("response-validation");
      validateWasmResponse(reloadResponse);
      setStage("initialize");
      try {
        await dependencies.initialize({ module, wasm: reloadResponse });
      } catch (reloadError) {
        throw new StagedRuntimeError("initialize", "retry", reloadError);
      }
    }
  }

  setStage("session");
  try {
    return dependencies.createSession();
  } catch (error) {
    throw new StagedRuntimeError("session", "retry", error);
  }
}

function validateWasmResponse(response: Response): void {
  if (!response.ok) {
    throw new StagedRuntimeError(
      "response-validation",
      "retry",
      new Error(`WASM request failed with HTTP ${response.status}.`)
    );
  }
  const contentType = response.headers.get("content-type") ?? "";
  if (!/^application\/wasm(?:\s*;|$)/i.test(contentType)) {
    throw new StagedRuntimeError(
      "response-validation",
      "retry",
      new Error(`WASM response must use application/wasm, received ${contentType || "none"}.`)
    );
  }
}

function toStagedError(error: unknown): StagedRuntimeError {
  return error instanceof StagedRuntimeError
    ? error
    : new StagedRuntimeError("session", "retry", error);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function runtimeFailureError(failure: MermanRuntimeFailure): Error {
  return failure.cause instanceof Error
    ? failure.cause
    : new Error(failure.message);
}

export interface MermanLifecycleEventTarget {
  addEventListener(type: string, listener: (event: Event) => void): void;
  removeEventListener(type: string, listener: (event: Event) => void): void;
}

export interface MermanDocumentLifecycleTarget {
  readonly document: MermanLifecycleEventTarget & {
    readonly visibilityState: string;
  };
  readonly window: MermanLifecycleEventTarget;
}

export interface MermanDocumentLifecycleCallbacks {
  onDestroy?(): void;
  onResume?(): void;
  onSuspend?(): void;
  onVisibilityChange?(visible: boolean): void;
}

export function installMermanDocumentLifecycle(
  runtime: MermanRuntime,
  target: MermanDocumentLifecycleTarget,
  callbacks: MermanDocumentLifecycleCallbacks = {}
): () => void {
  let destroyed = false;
  let suspended = false;
  let transitionId = 0;

  const suspend = () => {
    if (destroyed || suspended) return;
    transitionId += 1;
    suspended = true;
    runtime.suspend();
    callbacks.onSuspend?.();
  };
  const resume = () => {
    if (destroyed || !suspended) return;
    transitionId += 1;
    const currentTransition = transitionId;
    suspended = false;
    void runtime
      .resume()
      .then(() => {
        if (
          !destroyed &&
          !suspended &&
          transitionId === currentTransition
        ) {
          callbacks.onResume?.();
        }
      })
      .catch(() => undefined);
  };
  const onPageHide = (event: Event) => {
    if ((event as PageTransitionEvent).persisted) {
      suspend();
      return;
    }
    if (destroyed) return;
    transitionId += 1;
    destroyed = true;
    suspended = false;
    callbacks.onDestroy?.();
    runtime.dispose();
  };
  const onPageShow = (event: Event) => {
    if (!(event as PageTransitionEvent).persisted) return;
    resume();
  };
  const onVisibilityChange = () => {
    if (destroyed) return;
    callbacks.onVisibilityChange?.(target.document.visibilityState === "visible");
  };

  target.window.addEventListener("pagehide", onPageHide);
  target.window.addEventListener("pageshow", onPageShow);
  target.document.addEventListener("freeze", suspend);
  target.document.addEventListener("resume", resume);
  target.document.addEventListener("visibilitychange", onVisibilityChange);

  return () => {
    target.window.removeEventListener("pagehide", onPageHide);
    target.window.removeEventListener("pageshow", onPageShow);
    target.document.removeEventListener("freeze", suspend);
    target.document.removeEventListener("resume", resume);
    target.document.removeEventListener("visibilitychange", onVisibilityChange);
  };
}
