import type {
  MermaidRealmRenderInput,
  MermaidRealmExecutionResult,
  MermaidRealmSession,
} from "../mermaid-realm-controller.ts";
import { createAuthenticatedBrowserRealmChannel } from "./browser-realm-channel.ts";
import {
  COMPARE_OPERATION_STAGES,
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  RealmTimeoutError,
  isRealmMessageType,
  validateCompareRenderProgress,
  validateCompareRenderRequest,
  validateCompareRenderResponse,
  validateRealmFatal,
  type CompareOperationStage,
  type RealmKind,
  type RealmViewport,
} from "./channel-protocol.ts";

interface PendingRender {
  readonly reject: (error: unknown) => void;
  readonly requestId: string;
  readonly resolve: (result: MermaidRealmExecutionResult) => void;
  stageIndex: number;
  timer: ReturnType<typeof setTimeout> | null;
}

export async function createBrowserCompareRealmSession(
  initialViewport: RealmViewport,
  signal: AbortSignal
): Promise<MermaidRealmSession> {
  const kind: RealmKind = "compare";
  const {
    compareMermaidEngineArtifact,
    createOpaqueCompareRealmDocument,
  } = await import(
    "./opaque-realm-artifacts.ts"
  );
  let disposed = false;
  let transportAvailable = false;
  let incomingSequence = 0;
  let outgoingSequence = 0;
  let pending: PendingRender | null = null;

  const rejectPending = (error: unknown) => {
    if (!pending) return;
    if (pending.timer !== null) clearTimeout(pending.timer);
    const reject = pending.reject;
    pending = null;
    reject(error);
  };
  const onTransportFailure = (error: Error) => {
    transportAvailable = false;
    rejectPending(error);
  };

  const channel = await createAuthenticatedBrowserRealmChannel({
    kind,
    createRealmDocument: createOpaqueCompareRealmDocument,
    engineArtifact: compareMermaidEngineArtifact,
    initialViewport,
    signal,
    label: "Mermaid realm",
    title: "Mermaid Compare Realm",
    onFailure: onTransportFailure,
  });
  transportAvailable = true;
  const { identity, port } = channel;

  const poison = (error: unknown) => channel.poison(error);
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    transportAvailable = false;
    rejectPending(new RealmProtocolError("Mermaid realm was disposed."));
    port.onmessage = null;
    channel.dispose();
  };

  port.onmessage = (event) => {
    try {
      handlePortMessage(event.data);
    } catch (error) {
      poison(error);
    }
  };

  function handlePortMessage(data: unknown) {
    const expectedSequence = incomingSequence + 1;
    if (isRealmMessageType(data, "realm-fatal")) {
      const message = validateRealmFatal(data, identity, expectedSequence);
      incomingSequence = expectedSequence;
      throw new RealmProtocolError(message.message);
    }
    if (!pending) {
      throw new RealmProtocolError("Mermaid realm sent an unsolicited response.");
    }
    if (isRealmMessageType(data, "render-progress")) {
      const progress = validateCompareRenderProgress(
        data,
        identity,
        expectedSequence,
        pending.requestId
      );
      const nextStageIndex = COMPARE_OPERATION_STAGES.indexOf(progress.stage);
      if (nextStageIndex <= pending.stageIndex) {
        throw new RealmProtocolError("Mermaid realm progress is out of order.");
      }
      pending.stageIndex = nextStageIndex;
      incomingSequence = expectedSequence;
      armStageTimer(pending, progress.stage);
      return;
    }

    const response = validateCompareRenderResponse(
      data,
      identity,
      expectedSequence,
      pending.requestId
    );
    incomingSequence = expectedSequence;
    const current = pending;
    if (response.type === "render-failure" && response.stage === "protocol") {
      throw new RealmProtocolError(response.message);
    }
    pending = null;
    if (current.timer !== null) clearTimeout(current.timer);
    if (response.type === "render-failure") {
      current.resolve({
        status: "failure",
        stage: response.stage,
        message: response.message,
        detail: response.detail,
      });
      return;
    }
    current.resolve({
      status: "success",
      svg: response.svg,
      prepareTimeMs: response.prepareTimeMs,
      renderTimeMs: response.renderTimeMs,
      presentationTimeMs: response.presentationTimeMs,
      version: response.version,
    });
  }

  function armStageTimer(
    current: PendingRender,
    stage: CompareOperationStage | "dispatch"
  ) {
    if (current.timer !== null) clearTimeout(current.timer);
    current.timer = setTimeout(() => {
      poison(
        new RealmTimeoutError(`Mermaid realm timed out during ${stage}.`)
      );
    }, REALM_BUDGETS.stageTimeoutMs);
  }

  return {
    dispose,
    setViewport: (viewport) => channel.setViewport(viewport),
    render(input: MermaidRealmRenderInput, requestId: string) {
      if (disposed || !transportAvailable) {
        return Promise.reject(
          new RealmProtocolError("Mermaid realm is not ready.")
        );
      }
      if (pending) {
        return Promise.reject(
          new RealmProtocolError("Mermaid realm already has active work.")
        );
      }
      outgoingSequence += 1;
      const request = validateCompareRenderRequest(
        {
          type: "render",
          protocol: REALM_PROTOCOL_VERSION,
          ...identity,
          sequence: outgoingSequence,
          requestId,
          payload: input,
        },
        identity,
        outgoingSequence
      );
      return new Promise<MermaidRealmExecutionResult>((resolve, reject) => {
        pending = {
          requestId,
          resolve,
          reject,
          stageIndex: -1,
          timer: setTimeout(() => {
            poison(
              new RealmTimeoutError(
                "Mermaid realm timed out before reporting progress."
              )
            );
          }, REALM_BUDGETS.stageTimeoutMs),
        };
        port.postMessage(request);
      });
    },
  };
}
