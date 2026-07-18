import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  assertEncodedMessageBudget,
  createRealmToken,
  isRealmMessageType,
  validateRealmHello,
  validateRealmReady,
  validateRealmViewport,
  type RealmBootIdentity,
  type RealmIdentity,
  type RealmKind,
  type RealmViewport,
} from "./channel-protocol.ts";

export interface AuthenticatedBrowserRealmChannel {
  readonly identity: RealmIdentity;
  readonly port: MessagePort;
  dispose(): void;
  poison(error: unknown): void;
  setViewport(viewport: RealmViewport): Promise<void>;
}

export interface BrowserRealmChannelOptions {
  readonly handshakeTimeoutMs?: number;
  readonly initialViewport: RealmViewport;
  readonly kind: RealmKind;
  readonly label: string;
  readonly onFailure: (error: Error) => void;
  readonly realmUrl: URL;
  readonly signal: AbortSignal;
  readonly title: string;
}

/**
 * Creates an attached same-origin realm and returns only after its transferred
 * MessagePort has authenticated the peer. Operation messages remain owned by
 * the consumer so this transport cannot accidentally couple realm protocols.
 */
export async function createAuthenticatedBrowserRealmChannel({
  handshakeTimeoutMs = REALM_BUDGETS.stageTimeoutMs,
  initialViewport,
  kind,
  label,
  onFailure,
  realmUrl,
  signal,
  title,
}: BrowserRealmChannelOptions): Promise<AuthenticatedBrowserRealmChannel> {
  if (realmUrl.origin !== window.location.origin) {
    throw new RealmProtocolError(`${label} must use a same-origin URL.`);
  }
  if (!Number.isFinite(handshakeTimeoutMs) || handshakeTimeoutMs <= 0) {
    throw new RealmProtocolError(`${label} handshake timeout is invalid.`);
  }

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
  const frameUrl = new URL(realmUrl.href);
  frameUrl.hash = new URLSearchParams({
    protocol: String(REALM_PROTOCOL_VERSION),
    kind,
    realm: boot.realmId,
    boot: boot.bootNonce,
  }).toString();

  const iframe = createRealmFrame(kind, title, viewport);
  const messageChannel = new MessageChannel();
  const port = messageChannel.port1;
  let transferredPort: MessagePort | null = messageChannel.port2;
  let state: "handshaking" | "ready" | "disposed" = "handshaking";
  let confirmedViewport: RealmViewport | null = null;
  let iframeLoadCount = 0;
  let handshakeTimer: ReturnType<typeof setTimeout> | null = null;
  let rejectHandshake: ((error: unknown) => void) | null = null;
  let terminalFailure: Error | null = null;

  const cleanupHandshake = () => {
    window.removeEventListener("message", onHello);
    signal.removeEventListener("abort", onAbort);
    if (handshakeTimer !== null) {
      clearTimeout(handshakeTimer);
      handshakeTimer = null;
    }
  };
  const closeTransport = () => {
    cleanupHandshake();
    iframe.removeEventListener("error", onIframeError);
    iframe.removeEventListener("load", onIframeLoad);
    port.removeEventListener("message", onReady);
    port.removeEventListener("messageerror", onPortMessageError);
    port.close();
    transferredPort?.close();
    transferredPort = null;
    iframe.remove();
  };
  const asError = (error: unknown): Error =>
    error instanceof Error ? error : new RealmProtocolError(String(error));
  const fail = (error: unknown) => {
    if (state === "disposed") return;
    const failure = asError(error);
    const wasHandshaking = state === "handshaking";
    terminalFailure = failure;
    state = "disposed";
    closeTransport();
    if (wasHandshaking) {
      rejectHandshake?.(failure);
      rejectHandshake = null;
    }
    onFailure(failure);
  };
  const dispose = () => {
    if (state === "disposed") return;
    state = "disposed";
    closeTransport();
  };
  const isReady = () => state === "ready";
  const poison = (error: unknown) => fail(error);
  const onAbort = () => {
    fail(new DOMException(`${label} handshake was aborted.`, "AbortError"));
  };
  const onIframeError = () => {
    fail(new RealmProtocolError(`${label} iframe failed to load.`));
  };
  const onIframeLoad = () => {
    iframeLoadCount += 1;
    if (iframeLoadCount > 1) {
      fail(new RealmProtocolError(`${label} navigated after handshake.`));
    }
  };
  const onPortMessageError = () => {
    fail(new RealmProtocolError(`${label} response could not be cloned.`));
  };
  const onReady = (event: MessageEvent) => {
    if (state !== "handshaking") return;
    try {
      const message = validateRealmReady(event.data, identity);
      assertMatchingViewport(message.viewport, viewport, label);
      confirmedViewport = viewport;
      state = "ready";
      rejectHandshake = null;
      port.removeEventListener("message", onReady);
      cleanupHandshake();
      resolveHandshake?.();
      resolveHandshake = null;
    } catch (error) {
      fail(error);
    }
  };
  const onHello = (event: MessageEvent) => {
    if (
      state !== "handshaking" ||
      event.origin !== frameUrl.origin ||
      event.source !== iframe.contentWindow ||
      !isRealmMessageType(event.data, "realm-hello")
    ) {
      return;
    }
    try {
      if (iframe.contentWindow?.location.pathname !== frameUrl.pathname) {
        throw new RealmProtocolError(`${label} loaded an unexpected path.`);
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
        throw new RealmProtocolError(`${label} INIT was replayed.`);
      }
      transferredPort = null;
      iframe.contentWindow?.postMessage(init, frameUrl.origin, [peer]);
    } catch (error) {
      fail(error);
    }
  };

  let resolveHandshake: (() => void) | null = null;
  const readyPromise = new Promise<void>((resolve, reject) => {
    resolveHandshake = resolve;
    rejectHandshake = reject;
  });

  window.addEventListener("message", onHello);
  iframe.addEventListener("error", onIframeError);
  iframe.addEventListener("load", onIframeLoad);
  port.addEventListener("message", onReady);
  port.addEventListener("messageerror", onPortMessageError);
  port.start();
  signal.addEventListener("abort", onAbort, { once: true });
  handshakeTimer = setTimeout(() => {
    fail(new RealmProtocolError(`${label} handshake timed out.`));
  }, handshakeTimeoutMs);
  try {
    if (!document.body) {
      throw new RealmProtocolError(`${label} has no document body host.`);
    }
    iframe.src = frameUrl.href;
    document.body.appendChild(iframe);
  } catch (error) {
    fail(error);
  }
  if (signal.aborted) onAbort();

  await readyPromise;
  if (!isReady()) {
    throw (
      terminalFailure ??
      new RealmProtocolError(`${label} closed before authentication completed.`)
    );
  }

  return {
    identity,
    port,
    dispose,
    poison,
    async setViewport(nextViewport) {
      const normalized = validateRealmViewport(nextViewport);
      if (state !== "ready" || !iframe.isConnected || !iframe.contentWindow) {
        throw new RealmProtocolError(`${label} viewport host is unavailable.`);
      }
      if (!sameViewport(confirmedViewport, normalized)) {
        applyViewport(iframe, normalized);
        await nextAnimationFrame();
        await nextAnimationFrame();
      }
      if (state !== "ready" || !iframe.isConnected || !iframe.contentWindow) {
        throw new RealmProtocolError(`${label} viewport host is unavailable.`);
      }
      assertMatchingViewport(
        {
          width: iframe.contentWindow.innerWidth,
          height: iframe.contentWindow.innerHeight,
        },
        normalized,
        label
      );
      confirmedViewport = normalized;
    },
  };
}

function createRealmFrame(
  kind: RealmKind,
  title: string,
  viewport: RealmViewport
): HTMLIFrameElement {
  const iframe = document.createElement("iframe");
  iframe.dataset.mermanRealm = kind;
  iframe.setAttribute("aria-hidden", "true");
  iframe.setAttribute("inert", "");
  iframe.tabIndex = -1;
  iframe.title = title;
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
  return iframe;
}

function applyViewport(iframe: HTMLIFrameElement, viewport: RealmViewport) {
  iframe.style.width = `${Math.round(viewport.width)}px`;
  iframe.style.height = `${Math.round(viewport.height)}px`;
}

function assertMatchingViewport(
  actual: RealmViewport,
  expected: RealmViewport,
  label: string
): void {
  if (
    Math.round(actual.width) !== Math.round(expected.width) ||
    Math.round(actual.height) !== Math.round(expected.height)
  ) {
    throw new RealmProtocolError(`${label} viewport does not match its host.`);
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
