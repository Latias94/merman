import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmBudgetError,
  RealmProtocolError,
  assertEncodedMessageBudget,
  createOneTimeRealmInitGate,
  isRealmMessageType,
  validateCompareRenderRequest,
  validateCompareRenderResponse,
  validateRealmHello,
  type CompareFailureStage,
  type CompareOperationStage,
  type CompareRenderResponse,
  type RealmBootIdentity,
  type RealmEngineArtifactIdentity,
  type RealmIdentity,
} from "./channel-protocol.ts";
import { REALM_ENGINE_MODULE_EXPORTS } from "./generated/opaque-realm-plan.generated.ts";
import {
  verifyAndCreateRealmEngineModuleLoader,
} from "./engine-artifact-loader.ts";
import { createOperationQueue } from "./operation-queue.ts";
import {
  projectError,
  type ErrorProjection,
} from "../error-projection.ts";
import { applyScreenAvailableWidth } from "./screen-environment.ts";

export async function startCompareRealm(
  boot: RealmBootIdentity,
  expectedArtifact: RealmEngineArtifactIdentity
): Promise<void> {
  if (window.parent === window) {
    throw new RealmProtocolError("Compare realm must run inside an iframe.");
  }

  const initGate = createOneTimeRealmInitGate(boot, expectedArtifact);
  const onInit = (event: MessageEvent) => {
    if (
      event.source !== window.parent ||
      !isRealmMessageType(event.data, "realm-init")
    ) {
      return;
    }
    void acceptInit(event);
  };
  const acceptInit = async (event: MessageEvent) => {
    try {
      const init = initGate.consume(event.data, event.ports.length);
      window.removeEventListener("message", onInit);
      const loadEngine = await verifyAndCreateRealmEngineModuleLoader(
        init.engineArtifact,
        validateCompareEngineModule
      );
      serveCompareRealmPort(event.ports[0], init, loadEngine);
    } catch {
      for (const port of event.ports) port.close();
      window.removeEventListener("message", onInit);
    }
  };

  window.addEventListener("message", onInit);
  const hello = validateRealmHello(
    {
      type: "realm-hello",
      protocol: REALM_PROTOCOL_VERSION,
      ...boot,
    },
    boot
  );
  window.parent.postMessage(hello, "*");
}

export function serveCompareRealmPort(
  port: MessagePort,
  init: RealmIdentity,
  loadEngine: () => Promise<CompareEngineModule>
): void {
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

  const queue = createOperationQueue();
  let incomingSequence = 0;
  let outgoingSequence = 0;
  let closed = false;

  const post = (message: unknown): void => {
    if (closed) {
      throw new RealmProtocolError("Compare realm transport is closed.");
    }
    assertEncodedMessageBudget(message);
    port.postMessage(message);
  };
  const postSequenced = (sequence: number, message: unknown): void => {
    if (sequence !== outgoingSequence + 1) {
      throw new RealmProtocolError("Compare realm response sequence is invalid.");
    }
    if (closed) {
      throw new RealmProtocolError("Compare realm transport is closed.");
    }
    port.postMessage(message);
    outgoingSequence = sequence;
  };
  const fatal = (error: unknown) => {
    if (closed) return;
    const sequence = outgoingSequence + 1;
    try {
      const message = {
        type: "realm-fatal",
        protocol: REALM_PROTOCOL_VERSION,
        ...identity,
        sequence,
        message: boundedErrorMessage(error),
      };
      assertEncodedMessageBudget(message);
      postSequenced(sequence, message);
    } catch {
      // Closing the transport is the only remaining fail-closed action.
    } finally {
      closed = true;
      port.close();
    }
  };
  const postFailure = (requestId: string, error: unknown): void => {
    const sequence = outgoingSequence + 1;
    const stage = error instanceof RealmOperationError ? error.stage : "render";
    const projection =
      error instanceof RealmOperationError ? error.error : projectError(error);
    const response = validateCompareRenderResponse(
      {
        type: "render-failure",
        protocol: REALM_PROTOCOL_VERSION,
        ...identity,
        sequence,
        requestId,
        stage,
        message: projection.summary,
        detail: projection.detail,
      },
      identity,
      sequence,
      requestId
    );
    postSequenced(sequence, response);
  };

  port.onmessageerror = () => fatal("Realm port could not clone a message.");
  port.onmessage = (event) => {
    if (closed) return;
    const expectedSequence = incomingSequence + 1;
    let request;
    try {
      request = validateCompareRenderRequest(
        event.data,
        identity,
        expectedSequence
      );
    } catch (error) {
      fatal(error);
      return;
    }
    incomingSequence = expectedSequence;

    void queue
      .enqueue(async () => {
        const reportStage = (stage: CompareOperationStage) => {
          const sequence = outgoingSequence + 1;
          const message = {
            type: "render-progress",
            protocol: REALM_PROTOCOL_VERSION,
            ...identity,
            sequence,
            requestId: request.requestId,
            stage,
          };
          assertEncodedMessageBudget(message);
          postSequenced(sequence, message);
        };

        let result: Awaited<ReturnType<CompareEngineModule["renderWithMermaid"]>>;
        try {
          reportStage("fonts");
          await document.fonts.ready;
          reportStage("adapter-import");
          let engine: CompareEngineModule;
          try {
            applyScreenAvailableWidth(request.payload.screenAvailableWidth);
            engine = await loadEngine();
          } catch (error) {
            throw new RealmOperationError("adapter-import", projectError(error));
          }
          try {
            result = await engine.renderWithMermaid(
              request.payload,
              host,
              reportStage
            );
          } catch (error) {
            if (isCompareEngineError(error)) {
              throw new RealmOperationError(error.stage, error.error);
            }
            throw error;
          }
        } catch (error) {
          postFailure(request.requestId, error);
          return;
        }

        const sequence = outgoingSequence + 1;
        let response: CompareRenderResponse;
        try {
          response = validateCompareRenderResponse(
            {
              type: "render-success",
              protocol: REALM_PROTOCOL_VERSION,
              ...identity,
              sequence,
              requestId: request.requestId,
              svg: result.svg,
              prepareTimeMs: result.prepareTimeMs,
              renderTimeMs: result.renderTimeMs,
              presentationTimeMs: result.presentationTimeMs,
              version: result.version,
            },
            identity,
            sequence,
            request.requestId
          );
        } catch (error) {
          if (error instanceof RealmBudgetError) {
            postFailure(
              request.requestId,
              new RealmOperationError("svg-budget", projectError(error))
            );
            return;
          }
          throw error;
        }
        postSequenced(sequence, response);
      })
      .catch(fatal);
  };

  port.start();
  post({
    type: "realm-ready",
    protocol: REALM_PROTOCOL_VERSION,
    ...identity,
    sequence: 0,
    viewport: {
      width: window.innerWidth,
      height: window.innerHeight,
    },
  });
}

interface CompareEngineModule {
  renderWithMermaid: typeof import("./engines/mermaid.ts")["renderWithMermaid"];
}

function validateCompareEngineModule(
  module: Record<string, unknown>
): CompareEngineModule {
  const expectedExports = REALM_ENGINE_MODULE_EXPORTS.mermaid;
  if (
    Object.keys(module).length !== expectedExports.length ||
    !expectedExports.every((name) => Object.hasOwn(module, name)) ||
    typeof module.renderWithMermaid !== "function" ||
    typeof module.benchmarkEngineAdapter !== "object" ||
    module.benchmarkEngineAdapter === null
  ) {
    throw new RealmProtocolError("Compare engine artifact exports are invalid.");
  }
  return module as unknown as CompareEngineModule;
}

function isCompareEngineError(
  error: unknown
): error is Error & {
  readonly error: ErrorProjection;
  readonly stage: CompareOperationStage;
} {
  return (
    error instanceof Error &&
    typeof (error as { stage?: unknown }).stage === "string" &&
    isErrorProjection((error as { error?: unknown }).error)
  );
}

class RealmOperationError extends Error {
  readonly error: ErrorProjection;
  readonly stage: CompareFailureStage;

  constructor(stage: CompareFailureStage, error: ErrorProjection) {
    super(error.summary);
    this.name = "RealmOperationError";
    this.error = error;
    this.stage = stage;
  }
}

function boundedErrorMessage(error: unknown): string {
  const message = projectError(error).summary;
  return message.slice(0, Math.floor(REALM_BUDGETS.errorBytes / 4));
}

function isErrorProjection(value: unknown): value is ErrorProjection {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { summary?: unknown }).summary === "string" &&
    ((value as { detail?: unknown }).detail === null ||
      typeof (value as { detail?: unknown }).detail === "string")
  );
}
