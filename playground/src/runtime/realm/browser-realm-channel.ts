import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  assertRealmInitBudget,
  createRealmToken,
  isRealmMessageType,
  validateRealmHello,
  validateRealmReady,
  validateRealmViewport,
  type RealmBootIdentity,
  type RealmEngineArtifact,
  type RealmIdentity,
  type RealmKind,
  type RealmViewport,
} from "./channel-protocol.ts";
import { projectError } from "../error-projection.ts";

export interface AuthenticatedBrowserRealmChannel {
  readonly identity: RealmIdentity;
  readonly port: MessagePort;
  dispose(): void;
  poison(error: unknown): void;
  setViewport(viewport: RealmViewport): Promise<void>;
}

interface BrowserRealmChannelBaseOptions {
  readonly engineArtifact: RealmEngineArtifact;
  readonly handshakeTimeoutMs?: number;
  readonly initialViewport: RealmViewport;
  readonly kind: RealmKind;
  readonly label: string;
  readonly onFailure: (error: Error) => void;
  readonly signal: AbortSignal;
  readonly title: string;
}

export type BrowserRealmChannelOptions = BrowserRealmChannelBaseOptions &
  (
    | {
        readonly createRealmDocument: (identity: RealmBootIdentity) => string;
        readonly realmUrl?: never;
      }
    | {
        readonly createRealmDocument?: never;
        readonly realmUrl: URL;
      }
  );

/**
 * Creates an authenticated execution realm. Generated documents use an
 * opaque origin and bind authentication to the exact child window, one boot
 * nonce, and the only transferred MessagePort. Trusted local documents keep
 * their same-origin URL while using the same closed transport.
 */
export async function createAuthenticatedBrowserRealmChannel(
  options: BrowserRealmChannelOptions
): Promise<AuthenticatedBrowserRealmChannel> {
  const {
    handshakeTimeoutMs = REALM_BUDGETS.stageTimeoutMs,
    engineArtifact,
    initialViewport,
    kind,
    label,
    onFailure,
    signal,
    title,
  } = options;
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
  const createRealmDocument = options.createRealmDocument;
  const opaque = typeof createRealmDocument === "function";
  let srcdoc: string | null = null;
  let frameUrl: URL | null = null;
  if (opaque) {
    srcdoc = createRealmDocument(boot);
    if (typeof srcdoc !== "string" || srcdoc.length === 0) {
      throw new RealmProtocolError(`${label} document is unavailable.`);
    }
  } else {
    const realmUrl = options.realmUrl;
    if (!realmUrl || realmUrl.origin !== window.location.origin) {
      throw new RealmProtocolError(`${label} must use a same-origin URL.`);
    }
    frameUrl = new URL(realmUrl.href);
    frameUrl.hash = new URLSearchParams({
      protocol: String(REALM_PROTOCOL_VERSION),
      kind,
      realm: boot.realmId,
      boot: boot.bootNonce,
    }).toString();
  }

  const iframe = createRealmFrame(kind, title, viewport, opaque);
  const messageChannel = new MessageChannel();
  const port = messageChannel.port1;
  let transferredPort: MessagePort | null = messageChannel.port2;
  let state: "handshaking" | "ready" | "disposed" = "handshaking";
  let iframeLoadCount = 0;
  let handshakeTimer: ReturnType<typeof setTimeout> | null = null;
  let rejectHandshake: ((error: unknown) => void) | null = null;
  let resolveHandshake: (() => void) | null = null;
  let terminalFailure: Error | null = null;
  const viewportAbort = new AbortController();
  const isReady = () => state === "ready";

  const cleanupHandshake = () => {
    window.removeEventListener("message", onHello);
    signal.removeEventListener("abort", onAbort);
    if (handshakeTimer !== null) {
      clearTimeout(handshakeTimer);
      handshakeTimer = null;
    }
  };
  const closeTransport = () => {
    if (!viewportAbort.signal.aborted) {
      viewportAbort.abort(
        terminalFailure ??
          new RealmProtocolError(`${label} viewport host is unavailable.`)
      );
    }
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
  const fail = (error: unknown) => {
    if (state === "disposed") return;
    const failure = asRealmError(error);
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
  const onAbort = () =>
    fail(new DOMException(`${label} handshake was aborted.`, "AbortError"));
  const onIframeError = () =>
    fail(new RealmProtocolError(`${label} iframe failed to load.`));
  const onIframeLoad = () => {
    iframeLoadCount += 1;
    if (iframeLoadCount > 1) {
      fail(new RealmProtocolError(`${label} navigated after handshake.`));
    }
  };
  const onPortMessageError = () =>
    fail(new RealmProtocolError(`${label} response could not be cloned.`));
  const onReady = (event: MessageEvent) => {
    if (state !== "handshaking") return;
    try {
      const message = validateRealmReady(event.data, identity);
      assertMatchingViewport(message.viewport, viewport, label);
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
      event.origin !== (opaque ? "null" : frameUrl?.origin) ||
      event.source !== iframe.contentWindow ||
      !isRealmMessageType(event.data, "realm-hello")
    ) {
      return;
    }
    try {
      if (
        frameUrl &&
        iframe.contentWindow?.location.pathname !== frameUrl.pathname
      ) {
        throw new RealmProtocolError(`${label} loaded an unexpected path.`);
      }
      validateRealmHello(event.data, boot);
      window.removeEventListener("message", onHello);
      const init = {
        type: "realm-init",
        protocol: REALM_PROTOCOL_VERSION,
        ...boot,
        realmToken: identity.realmToken,
        engineArtifact,
      };
      assertRealmInitBudget(init);
      const peer = transferredPort;
      if (!peer) throw new RealmProtocolError(`${label} INIT was replayed.`);
      transferredPort = null;
      iframe.contentWindow?.postMessage(init, frameUrl?.origin ?? "*", [peer]);
    } catch (error) {
      fail(error);
    }
  };

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
  handshakeTimer = setTimeout(
    () => fail(new RealmProtocolError(`${label} handshake timed out.`)),
    handshakeTimeoutMs
  );
  try {
    if (!document.body) {
      throw new RealmProtocolError(`${label} has no document body host.`);
    }
    if (srcdoc !== null) iframe.srcdoc = srcdoc;
    else if (frameUrl) iframe.src = frameUrl.href;
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
    poison: fail,
    async setViewport(nextViewport) {
      const normalized = validateRealmViewport(nextViewport);
      if (state !== "ready" || !iframe.isConnected || !iframe.contentWindow) {
        throw new RealmProtocolError(`${label} viewport host is unavailable.`);
      }
      applyViewport(iframe, normalized);
      await nextAnimationFrame(viewportAbort.signal, label);
      await nextAnimationFrame(viewportAbort.signal, label);
      if (state !== "ready" || !iframe.isConnected || !iframe.contentWindow) {
        throw new RealmProtocolError(`${label} viewport host is unavailable.`);
      }
    },
  };
}

function createRealmFrame(
  kind: RealmKind,
  title: string,
  viewport: RealmViewport,
  opaque: boolean
): HTMLIFrameElement {
  const iframe = document.createElement("iframe");
  iframe.dataset.mermanRealm = kind;
  if (opaque) iframe.setAttribute("sandbox", "allow-scripts");
  iframe.setAttribute("referrerpolicy", "no-referrer");
  iframe.setAttribute("aria-hidden", "true");
  iframe.setAttribute("inert", "");
  iframe.tabIndex = -1;
  iframe.title = title;
  iframe.style.position = "fixed";
  iframe.style.left = "0";
  iframe.style.top = "0";
  iframe.style.border = "0";
  iframe.style.display = "block";
  iframe.style.visibility = "visible";
  iframe.style.opacity = "1";
  iframe.style.pointerEvents = "none";
  iframe.style.contentVisibility = "visible";
  iframe.style.transformOrigin = "top left";
  iframe.style.zIndex = "-1";
  applyViewport(iframe, viewport);
  return iframe;
}

function applyViewport(iframe: HTMLIFrameElement, viewport: RealmViewport) {
  const width = Math.round(viewport.width);
  const height = Math.round(viewport.height);
  iframe.style.width = `${width}px`;
  iframe.style.height = `${height}px`;
  iframe.style.transform = `scale(${1 / Math.max(width, height)})`;
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

function asRealmError(error: unknown): Error {
  if (error instanceof Error) return error;
  const projection = projectError(error);
  const failure = new RealmProtocolError(projection.summary);
  if (projection.detail !== null) {
    Object.defineProperty(failure, "detail", {
      configurable: true,
      enumerable: true,
      value: projection.detail,
      writable: false,
    });
  }
  return failure;
}

function nextAnimationFrame(signal: AbortSignal, label: string): Promise<void> {
  if (signal.aborted) {
    return Promise.reject(viewportAbortError(signal, label));
  }

  if (document.visibilityState !== "visible") {
    return nextTask(signal, label);
  }

  return new Promise((resolve, reject) => {
    let frameId: number | null = null;
    let settled = false;
    let onAbort = () => {};
    let onVisibilityChange = () => {};

    const cleanup = () => {
      signal.removeEventListener("abort", onAbort);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
    const settle = (callback: () => void) => {
      if (settled) return;
      settled = true;
      cleanup();
      callback();
    };
    onAbort = () => {
      if (frameId !== null) cancelAnimationFrame(frameId);
      settle(() => reject(viewportAbortError(signal, label)));
    };
    onVisibilityChange = () => {
      if (document.visibilityState !== "hidden") return;
      if (frameId !== null) cancelAnimationFrame(frameId);
      settle(() => {
        void nextTask(signal, label).then(resolve, reject);
      });
    };

    signal.addEventListener("abort", onAbort, { once: true });
    document.addEventListener("visibilitychange", onVisibilityChange);
    frameId = requestAnimationFrame(() => settle(resolve));
    if (signal.aborted) onAbort();
  });
}

function nextTask(signal: AbortSignal, label: string): Promise<void> {
  return new Promise((resolve, reject) => {
    let settled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const onAbort = () => settle(() => reject(viewportAbortError(signal, label)));
    const settle = (callback: () => void) => {
      if (settled) return;
      settled = true;
      if (timer !== null) clearTimeout(timer);
      signal.removeEventListener("abort", onAbort);
      callback();
    };
    signal.addEventListener("abort", onAbort, { once: true });
    timer = setTimeout(() => settle(resolve), 0);
    if (signal.aborted) onAbort();
  });
}

function viewportAbortError(signal: AbortSignal, label: string): Error {
  return signal.reason instanceof Error
    ? signal.reason
    : new RealmProtocolError(`${label} viewport host is unavailable.`);
}
