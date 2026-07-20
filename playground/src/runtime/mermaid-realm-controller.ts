import type { DiagramFont } from "../lib/diagram-font.ts";
import type { MermaidExternalRequirements } from "./mermaid-requirements.ts";
import {
  projectSafeInlineSvg,
  type SafeInlineSvg,
} from "./render-artifact.ts";
import {
  REALM_BUDGETS,
  RealmTimeoutError,
  type CompareFailureStage,
  type RealmKind,
  type RealmViewport,
} from "./realm/channel-protocol.ts";
import { createOperationQueue } from "./realm/operation-queue.ts";
import { projectError } from "./error-projection.ts";

export interface MermaidRealmRenderInput {
  readonly configJson: string;
  readonly diagramFont: DiagramFont;
  readonly externalRequirements: MermaidExternalRequirements;
  readonly source: string;
  readonly theme: string;
  readonly viewport: RealmViewport;
}

export interface MermaidRealmRenderSuccess {
  readonly artifact: SafeInlineSvg;
  readonly prepareTimeMs: number;
  readonly presentationTimeMs: number;
  readonly renderTimeMs: number;
  readonly status: "success";
  readonly version: string;
}

export interface MermaidRealmExecutionSuccess {
  readonly prepareTimeMs: number;
  readonly presentationTimeMs: number;
  readonly renderTimeMs: number;
  readonly status: "success";
  readonly svg: string;
  readonly version: string;
}

export interface MermaidRealmRenderFailure {
  readonly detail: string | null;
  readonly message: string;
  readonly stage: CompareFailureStage;
  readonly status: "failure";
}

export type MermaidRealmRenderResult =
  | MermaidRealmRenderSuccess
  | MermaidRealmRenderFailure;
export type MermaidRealmExecutionResult =
  | MermaidRealmExecutionSuccess
  | MermaidRealmRenderFailure;

export interface MermaidRealmSession {
  dispose(): void;
  render(
    input: MermaidRealmRenderInput,
    requestId: string
  ): Promise<MermaidRealmExecutionResult>;
  setViewport(viewport: RealmViewport): Promise<void>;
}

export interface MermaidRealmController {
  dispose(): void;
  render(input: MermaidRealmRenderInput): Promise<MermaidRealmRenderResult>;
  reset(): void;
}

export interface MermaidRealmControllerOptions {
  readonly createSession: (
    kind: RealmKind,
    viewport: RealmViewport,
    signal: AbortSignal
  ) => Promise<MermaidRealmSession>;
  readonly kind: RealmKind;
  readonly operationTimeoutMs?: number;
}

export function createMermaidRealmController({
  createSession,
  kind,
  operationTimeoutMs = REALM_BUDGETS.runTimeoutMs,
}: MermaidRealmControllerOptions): MermaidRealmController {
  let disposed = false;
  let generation = 0;
  let requestSequence = 0;
  let session: MermaidRealmSession | null = null;
  let sessionCreationAbort: AbortController | null = null;
  let activeCancel: ((result: MermaidRealmRenderFailure) => void) | null = null;
  const operationQueue = createOperationQueue();

  const disposeSession = () => {
    const current = session;
    session = null;
    current?.dispose();
  };
  const disposeActiveSession = (activeSession: MermaidRealmSession) => {
    if (session === activeSession) {
      disposeSession();
    } else {
      activeSession.dispose();
    }
  };

  const reset = () => {
    generation += 1;
    sessionCreationAbort?.abort();
    sessionCreationAbort = null;
    activeCancel?.(failure("disposed", "Mermaid realm operation was reset."));
    activeCancel = null;
    disposeSession();
  };

  const dispose = () => {
    if (disposed) return;
    disposed = true;
    reset();
  };

  const run = async (
    input: MermaidRealmRenderInput,
    requestId: string
  ): Promise<MermaidRealmRenderResult> => {
    if (disposed) {
      return failure("disposed", "Mermaid realm controller is disposed.");
    }

    const operationGeneration = generation;
    if (!session) {
      try {
        const abortController = new AbortController();
        sessionCreationAbort = abortController;
        const nextSession = await createSession(
          kind,
          input.viewport,
          abortController.signal
        );
        if (sessionCreationAbort === abortController) {
          sessionCreationAbort = null;
        }
        if (disposed || generation !== operationGeneration) {
          nextSession.dispose();
          return failure("disposed", "Mermaid realm operation was superseded.");
        }
        session = nextSession;
      } catch (error) {
        sessionCreationAbort = null;
        if (disposed || generation !== operationGeneration) {
          return failure("disposed", "Mermaid realm operation was superseded.");
        }
        return failure("handshake", error);
      }
    }

    const activeSession = session;
    try {
      await activeSession.setViewport(input.viewport);
    } catch (error) {
      disposeSession();
      return failure("presentation", error);
    }

    let timeout: ReturnType<typeof setTimeout> | null = null;
    let operationTimedOut = false;
    const timeoutFailure = failure(
      "timeout",
      "Mermaid realm operation timed out."
    );
    const cancellation = Promise.withResolvers<MermaidRealmRenderFailure>();
    const cancelOperation = cancellation.resolve;
    activeCancel = cancelOperation;
    const timedOut = new Promise<MermaidRealmRenderFailure>((resolve) => {
      timeout = setTimeout(() => {
        operationTimedOut = true;
        resolve(timeoutFailure);
        disposeActiveSession(activeSession);
      }, operationTimeoutMs);
    });

    try {
      const result = await Promise.race([
        activeSession.render(input, requestId),
        cancellation.promise,
        timedOut,
      ]);
      if (result.status === "failure") {
        return result;
      }
      try {
        const artifact = projectSafeInlineSvg(result.svg);
        const { svg: _svg, ...evidence } = result;
        return { ...evidence, artifact };
      } catch (error) {
        disposeActiveSession(activeSession);
        return failure("svg-validation", error);
      }
    } catch (error) {
      disposeActiveSession(activeSession);
      if (operationTimedOut) return timeoutFailure;
      return failure(
        error instanceof RealmTimeoutError ? "timeout" : "protocol",
        error
      );
    } finally {
      if (timeout !== null) clearTimeout(timeout);
      if (activeCancel === cancelOperation) activeCancel = null;
    }
  };

  const render = (
    input: MermaidRealmRenderInput
  ): Promise<MermaidRealmRenderResult> => {
    requestSequence += 1;
    const requestId = `${kind}-${requestSequence}`;
    return operationQueue.enqueue(() => run(input, requestId));
  };

  return { dispose, render, reset };
}

function failure(
  stage: CompareFailureStage,
  error: unknown
): MermaidRealmRenderFailure {
  const projection = projectError(error);
  return {
    status: "failure",
    stage,
    message: projection.summary,
    detail: projection.detail,
  };
}
