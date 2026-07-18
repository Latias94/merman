import type {
  MermaidRealmRenderInput,
  MermaidRealmRenderResult,
  MermaidRealmSession,
} from "../mermaid-realm-controller.ts";
import {
  COMPARE_OPERATION_STAGES,
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  RealmTimeoutError,
  assertEncodedMessageBudget,
  createRealmToken,
  isRealmMessageType,
  validateCompareRenderProgress,
  validateCompareRenderRequest,
  validateCompareRenderResponse,
  validateRealmFatal,
  validateRealmHello,
  validateRealmReady,
  validateRealmViewport,
  type CompareOperationStage,
  type RealmBootIdentity,
  type RealmIdentity,
  type RealmKind,
  type RealmViewport,
} from "./channel-protocol.ts";

interface PendingRender {
  readonly reject: (error: unknown) => void;
  readonly requestId: string;
  readonly resolve: (result: MermaidRealmRenderResult) => void;
  stageIndex: number;
  timer: ReturnType<typeof setTimeout> | null;
}

export async function createBrowserCompareRealmSession(
  initialViewport: RealmViewport,
  signal: AbortSignal
): Promise<MermaidRealmSession> {
  const kind: RealmKind = "compare";
  const viewport = validateRealmViewport(initialViewport);
  const boot: RealmBootIdentity = {
    bootNonce: createRealmToken(),
    kind,
    realmId: createRealmToken(),
  };
  const identity: RealmIdentity = {
    kind,
    realmId: boot.realmId,
    realmToken: createRealmToken(),
  };
  const realmUrl = new URL(
    `${import.meta.env.BASE_URL}compare-realm.html`,
    window.location.origin
  );
  realmUrl.hash = new URLSearchParams({
    protocol: String(REALM_PROTOCOL_VERSION),
    kind,
    realm: boot.realmId,
    boot: boot.bootNonce,
  }).toString();

  const iframe = document.createElement("iframe");
  iframe.dataset.mermanRealm = kind;
  iframe.setAttribute("aria-hidden", "true");
  iframe.setAttribute("inert", "");
  iframe.tabIndex = -1;
  iframe.title = "Mermaid Compare Realm";
  iframe.style.position = "fixed";
  iframe.style.left = "-10000px";
  iframe.style.top = "0";
  iframe.style.border = "0";
  iframe.style.display = "block";
  iframe.style.visibility = "visible";
  iframe.style.opacity = "0";
  iframe.style.pointerEvents = "none";
  iframe.style.contentVisibility = "visible";
  applyViewport(iframe, viewport);

  let disposed = false;
  let ready = false;
  let confirmedViewport: RealmViewport | null = null;
  let iframeLoadCount = 0;
  const channel = new MessageChannel();
  let port: MessagePort | null = channel.port1;
  let transferredPort: MessagePort | null = channel.port2;
  let incomingSequence = 0;
  let outgoingSequence = 0;
  let pending: PendingRender | null = null;
  let handshakeTimer: ReturnType<typeof setTimeout> | null = null;
  let rejectHandshake: ((error: unknown) => void) | null = null;

  const cleanupWindowListener = () => {
    window.removeEventListener("message", onHello);
    iframe.removeEventListener("error", onIframeError);
    signal.removeEventListener("abort", onAbort);
    if (handshakeTimer !== null) {
      clearTimeout(handshakeTimer);
      handshakeTimer = null;
    }
  };
  const rejectPending = (error: unknown) => {
    if (!pending) return;
    if (pending.timer !== null) clearTimeout(pending.timer);
    const reject = pending.reject;
    pending = null;
    reject(error);
  };
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    cleanupWindowListener();
    iframe.removeEventListener("load", onIframeLoad);
    rejectHandshake?.(new RealmProtocolError("Mermaid realm was disposed."));
    rejectHandshake = null;
    rejectPending(new RealmProtocolError("Mermaid realm was disposed."));
    if (port) {
      port.onmessage = null;
      port.onmessageerror = null;
      port.close();
      port = null;
    }
    transferredPort?.close();
    transferredPort = null;
    iframe.remove();
  };
  const fatal = (error: unknown) => {
    const failure =
      error instanceof Error
        ? error
        : new RealmProtocolError(String(error));
    rejectHandshake?.(failure);
    rejectHandshake = null;
    rejectPending(failure);
  };
  const poison = (error: unknown) => {
    fatal(error);
    dispose();
  };
  const onAbort = () => {
    poison(new DOMException("Mermaid realm handshake was aborted.", "AbortError"));
  };
  const onIframeError = () => {
    poison(new RealmProtocolError("Mermaid realm iframe failed to load."));
  };
  const onIframeLoad = () => {
    iframeLoadCount += 1;
    if (ready && iframeLoadCount > 1) {
      poison(new RealmProtocolError("Mermaid realm navigated after handshake."));
    }
  };

  const readyPromise = new Promise<void>((resolve, reject) => {
    rejectHandshake = reject;
    const parentPort = port;
    if (!parentPort) {
      reject(new RealmProtocolError("Mermaid realm channel was not created."));
      return;
    }
    parentPort.onmessageerror = () => {
      poison(new RealmProtocolError("Mermaid realm response could not be cloned."));
    };
    parentPort.onmessage = (event) => {
      try {
        if (!ready) {
          const message = validateRealmReady(event.data, identity);
          assertMatchingViewport(message.viewport, viewport);
          confirmedViewport = viewport;
          ready = true;
          incomingSequence = 0;
          rejectHandshake = null;
          resolve();
          return;
        }
        handlePortMessage(event.data);
      } catch (error) {
        poison(error);
      }
    };
    parentPort.start();

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
      pending = null;
      if (current.timer !== null) clearTimeout(current.timer);
      if (response.type === "render-failure") {
        if (response.stage === "protocol") {
          throw new RealmProtocolError(response.message);
        }
        current.resolve({
          status: "failure",
          stage: response.stage,
          message: response.message,
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
          new RealmTimeoutError(
            `Mermaid realm timed out during ${stage}.`
          )
        );
      }, REALM_BUDGETS.stageTimeoutMs);
    }
  });

  function onHello(event: MessageEvent) {
    if (
      event.origin !== realmUrl.origin ||
      event.source !== iframe.contentWindow ||
      !isRealmMessageType(event.data, "realm-hello")
    ) {
      return;
    }
    try {
      if (iframe.contentWindow?.location.pathname !== realmUrl.pathname) {
        throw new RealmProtocolError("Mermaid realm loaded an unexpected path.");
      }
      validateRealmHello(event.data, boot);
      window.removeEventListener("message", onHello);
      const init = {
        type: "realm-init",
        protocol: REALM_PROTOCOL_VERSION,
        ...boot,
        realmToken: identity.realmToken,
      };
      assertEncodedMessageBudget(init);
      const peer = transferredPort;
      if (!peer) {
        throw new RealmProtocolError("Mermaid realm INIT was replayed.");
      }
      transferredPort = null;
      iframe.contentWindow?.postMessage(init, realmUrl.origin, [peer]);
    } catch (error) {
      poison(error);
    }
  }

  window.addEventListener("message", onHello);
  iframe.addEventListener("error", onIframeError);
  iframe.addEventListener("load", onIframeLoad);
  signal.addEventListener("abort", onAbort, { once: true });
  handshakeTimer = setTimeout(() => {
    poison(new RealmProtocolError("Mermaid realm handshake timed out."));
  }, REALM_BUDGETS.stageTimeoutMs);
  iframe.src = realmUrl.href;
  document.body.appendChild(iframe);
  if (signal.aborted) onAbort();

  await readyPromise;
  cleanupWindowListener();

  return {
    dispose,
    async setViewport(nextViewport) {
      const normalized = validateRealmViewport(nextViewport);
      if (!sameViewport(confirmedViewport, normalized)) {
        applyViewport(iframe, normalized);
        await nextAnimationFrame();
        await nextAnimationFrame();
      }
      if (disposed || !iframe.isConnected || !iframe.contentWindow) {
        throw new RealmProtocolError("Mermaid realm viewport host is unavailable.");
      }
      assertMatchingViewport(
        {
          width: iframe.contentWindow.innerWidth,
          height: iframe.contentWindow.innerHeight,
        },
        normalized
      );
      confirmedViewport = normalized;
    },
    render(input: MermaidRealmRenderInput, requestId: string) {
      if (disposed || !ready || !port) {
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
      return new Promise<MermaidRealmRenderResult>((resolve, reject) => {
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
        port?.postMessage(request);
      });
    },
  };
}

function applyViewport(iframe: HTMLIFrameElement, viewport: RealmViewport) {
  iframe.style.width = `${Math.round(viewport.width)}px`;
  iframe.style.height = `${Math.round(viewport.height)}px`;
}

function assertMatchingViewport(
  actual: RealmViewport,
  expected: RealmViewport
): void {
  if (
    Math.round(actual.width) !== Math.round(expected.width) ||
    Math.round(actual.height) !== Math.round(expected.height)
  ) {
    throw new RealmProtocolError("Mermaid realm viewport does not match its host.");
  }
}

function sameViewport(
  left: RealmViewport | null,
  right: RealmViewport
): boolean {
  return (
    left !== null &&
    left.width === right.width &&
    left.height === right.height
  );
}

function nextAnimationFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}
