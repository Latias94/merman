import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  assertEncodedMessageBudget,
  createOneTimeRealmInitGate,
  isRealmMessageType,
  validateCompareRenderRequest,
  validateCompareRenderResponse,
  validateRealmHello,
  type CompareFailureStage,
  type CompareOperationStage,
  type RealmBootIdentity,
  type RealmIdentity,
} from "./channel-protocol.ts";
import { createOperationQueue } from "./operation-queue.ts";
import "./compare-realm.css";

void startCompareRealm();

async function startCompareRealm(): Promise<void> {
  const boot = readBootIdentity();
  if (window.parent === window) {
    throw new RealmProtocolError("Compare realm must run inside an iframe.");
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
    window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}`);
    servePort(event.ports[0], init);
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
  window.parent.postMessage(hello, window.location.origin);
}

function servePort(
  port: MessagePort,
  init: RealmIdentity
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

  const post = (message: unknown) => {
    if (closed) return;
    assertEncodedMessageBudget(message);
    port.postMessage(message);
  };
  const postValidated = (message: unknown) => {
    if (!closed) port.postMessage(message);
  };
  const fatal = (error: unknown) => {
    if (closed) return;
    outgoingSequence += 1;
    post({
      type: "realm-fatal",
      protocol: REALM_PROTOCOL_VERSION,
      ...identity,
      sequence: outgoingSequence,
      message: boundedErrorMessage(error),
    });
    closed = true;
    port.close();
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
          outgoingSequence += 1;
          post({
            type: "render-progress",
            protocol: REALM_PROTOCOL_VERSION,
            ...identity,
            sequence: outgoingSequence,
            requestId: request.requestId,
            stage,
          });
        };

        reportStage("fonts");
        await document.fonts.ready;
        reportStage("adapter-import");
        let engine;
        try {
          engine = await import("./engines/mermaid.ts");
        } catch (error) {
          throw new RealmOperationError("adapter-import", error);
        }
        try {
          return await engine.renderWithMermaid(
            request.payload,
            host,
            reportStage
          );
        } catch (error) {
          if (error instanceof engine.MermaidEngineError) {
            throw new RealmOperationError(error.stage, error);
          }
          throw error;
        }
      })
      .then(
        (result) => {
          outgoingSequence += 1;
          const response = validateCompareRenderResponse(
            {
              type: "render-success",
              protocol: REALM_PROTOCOL_VERSION,
              ...identity,
              sequence: outgoingSequence,
              requestId: request.requestId,
              svg: result.svg,
              prepareTimeMs: result.prepareTimeMs,
              renderTimeMs: result.renderTimeMs,
              presentationTimeMs: result.presentationTimeMs,
              version: result.version,
            },
            identity,
            outgoingSequence,
            request.requestId
          );
          postValidated(response);
        },
        (error) => {
          outgoingSequence += 1;
          const stage =
            error instanceof RealmOperationError ? error.stage : "render";
          const response = validateCompareRenderResponse(
            {
              type: "render-failure",
              protocol: REALM_PROTOCOL_VERSION,
              ...identity,
              sequence: outgoingSequence,
              requestId: request.requestId,
              stage,
              message: boundedErrorMessage(error),
            },
            identity,
            outgoingSequence,
            request.requestId
          );
          postValidated(response);
        }
      );
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

class RealmOperationError extends Error {
  readonly stage: CompareFailureStage;

  constructor(stage: CompareFailureStage, cause: unknown) {
    super(errorMessage(cause));
    this.name = "RealmOperationError";
    this.stage = stage;
  }
}

function readBootIdentity(): RealmBootIdentity {
  const params = new URLSearchParams(window.location.hash.slice(1));
  if (
    params.size !== 4 ||
    params.get("protocol") !== String(REALM_PROTOCOL_VERSION)
  ) {
    throw new RealmProtocolError("Compare realm boot fragment is invalid.");
  }
  const kind = params.get("kind");
  const realmId = params.get("realm");
  const bootNonce = params.get("boot");
  if (
    kind !== "compare" ||
    !realmId ||
    !bootNonce
  ) {
    throw new RealmProtocolError("Compare realm boot identity is invalid.");
  }
  const boot: RealmBootIdentity = { kind, realmId, bootNonce };
  validateRealmHello(
    {
      type: "realm-hello",
      protocol: REALM_PROTOCOL_VERSION,
      ...boot,
    },
    boot
  );
  return boot;
}

function boundedErrorMessage(error: unknown): string {
  const message = errorMessage(error);
  return message.slice(0, Math.floor(REALM_BUDGETS.errorBytes / 4));
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
