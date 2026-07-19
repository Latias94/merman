import assert from "node:assert/strict";
import test from "node:test";

import {
  REALM_PROTOCOL_VERSION,
  type RealmBootIdentity,
} from "./channel-protocol.ts";
import { createAuthenticatedBrowserRealmChannel } from "./browser-realm-channel.ts";

const ENGINE_ARTIFACT = {
  schemaVersion: 1 as const,
  id: "compare-mermaid" as const,
  bytes: 17,
  sha256: "a".repeat(64),
  resourceUrl: null,
  source: "export default 1;",
};

test("opaque realm authenticates exact source and transfers one port", async () => {
  const harness = installBrowserHarness();
  try {
    const failures: Error[] = [];
    const channel = await createAuthenticatedBrowserRealmChannel({
      kind: "compare",
      engineArtifact: ENGINE_ARTIFACT,
      createRealmDocument: harness.createRealmDocument,
      initialViewport: { width: 800, height: 600 },
      signal: new AbortController().signal,
      label: "Test realm",
      title: "Test Realm",
      onFailure: (error) => failures.push(error),
    });

    assert.equal(harness.initMessages.length, 1);
    assert.equal(harness.transfers.length, 1);
    assert.equal(harness.targetOrigins[0], "*");
    assert.deepEqual(harness.initMessages[0], {
      type: "realm-init",
      protocol: REALM_PROTOCOL_VERSION,
      kind: "compare",
      realmId: channel.identity.realmId,
      bootNonce: harness.boot?.bootNonce,
      realmToken: channel.identity.realmToken,
      engineArtifact: ENGINE_ARTIFACT,
    });
    assert.equal(harness.frame.attributes.get("sandbox"), "allow-scripts");
    assert.equal(harness.frame.attributes.has("allow-same-origin"), false);
    assert.equal(harness.frame.src, "");
    assert.equal(harness.frame.srcdoc, "<!doctype html><title>opaque compare</title>");
    assert.equal(harness.frame.dataset.mermanRealm, "compare");
    assert.equal(harness.frame.style.left, "0");
    assert.equal(harness.frame.style.opacity, "1");
    assert.equal(harness.frame.style.transform, "scale(0.00125)");
    assert.equal(harness.frame.style.zIndex, "-1");
    assert.equal(failures.length, 0);

    await channel.setViewport({ width: 640, height: 480 });
    assert.equal(harness.frame.style.width, "640px");
    assert.equal(harness.frame.style.height, "480px");
    assert.equal(harness.frame.style.transform, "scale(0.0015625)");

    channel.dispose();
    channel.dispose();
    assert.equal(harness.frame.removeCount, 1);
  } finally {
    harness.restore();
  }
});

test("opaque realm ignores forged origin, source, and boot messages", async () => {
  const harness = installBrowserHarness({ includeForgedMessages: true });
  try {
    const channel = await createAuthenticatedBrowserRealmChannel({
      kind: "compare",
      engineArtifact: ENGINE_ARTIFACT,
      createRealmDocument: harness.createRealmDocument,
      initialViewport: { width: 800, height: 600 },
      signal: new AbortController().signal,
      label: "Test realm",
      title: "Test Realm",
      onFailure: () => {},
    });
    assert.equal(harness.initMessages.length, 1);
    channel.dispose();
  } finally {
    harness.restore();
  }
});

test("opaque realm is poisoned if its frame navigates after authentication", async () => {
  const harness = installBrowserHarness();
  try {
    const failed = Promise.withResolvers<Error>();
    const channel = await createAuthenticatedBrowserRealmChannel({
      kind: "compare",
      engineArtifact: ENGINE_ARTIFACT,
      createRealmDocument: harness.createRealmDocument,
      initialViewport: { width: 800, height: 600 },
      signal: new AbortController().signal,
      label: "Test realm",
      title: "Test Realm",
      onFailure: failed.resolve,
    });
    harness.frame.dispatchEvent(new Event("load"));
    assert.match((await failed.promise).message, /navigated after handshake/);
    await assert.rejects(
      channel.setViewport({ width: 640, height: 480 }),
      /viewport host is unavailable/
    );
  } finally {
    harness.restore();
  }
});

test("opaque realm rejects timeout and pre-aborted handshakes", async () => {
  const timeoutHarness = installBrowserHarness({ autoHandshake: false });
  try {
    await assert.rejects(
      createAuthenticatedBrowserRealmChannel({
        kind: "benchmark",
        engineArtifact: { ...ENGINE_ARTIFACT, id: "benchmark-mermaid" },
        createRealmDocument: timeoutHarness.createRealmDocument,
        initialViewport: { width: 800, height: 600 },
        signal: new AbortController().signal,
        handshakeTimeoutMs: 5,
        label: "Benchmark realm",
        title: "Benchmark Realm",
        onFailure: () => {},
      }),
      /handshake timed out/
    );
    assert.equal(timeoutHarness.frame.removeCount, 1);
  } finally {
    timeoutHarness.restore();
  }

  const abortHarness = installBrowserHarness({ autoHandshake: false });
  try {
    const controller = new AbortController();
    controller.abort();
    await assert.rejects(
      createAuthenticatedBrowserRealmChannel({
        kind: "benchmark",
        engineArtifact: { ...ENGINE_ARTIFACT, id: "benchmark-mermaid" },
        createRealmDocument: abortHarness.createRealmDocument,
        initialViewport: { width: 800, height: 600 },
        signal: controller.signal,
        label: "Benchmark realm",
        title: "Benchmark Realm",
        onFailure: () => {},
      }),
      (error: unknown) =>
        error instanceof DOMException && error.name === "AbortError"
    );
  } finally {
    abortHarness.restore();
  }
});

interface HarnessOptions {
  readonly autoHandshake?: boolean;
  readonly includeForgedMessages?: boolean;
}

function installBrowserHarness({
  autoHandshake = true,
  includeForgedMessages = false,
}: HarnessOptions = {}) {
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  const previousDocument = Object.getOwnPropertyDescriptor(globalThis, "document");
  const previousAnimationFrame = Object.getOwnPropertyDescriptor(
    globalThis,
    "requestAnimationFrame"
  );
  const parentWindow = Object.assign(new EventTarget(), {
    location: { origin: "https://play.test" },
  });
  const initMessages: unknown[] = [];
  const targetOrigins: string[] = [];
  const transfers: MessagePort[][] = [];
  let realmPort: MessagePort | null = null;
  let boot: RealmBootIdentity | null = null;

  const contentWindow = {
    postMessage(data: unknown, targetOrigin: string, ports: MessagePort[]) {
      initMessages.push(data);
      targetOrigins.push(targetOrigin);
      transfers.push(ports);
      realmPort = ports[0] ?? null;
      assert.ok(realmPort);
      const init = data as {
        readonly kind: "compare" | "benchmark";
        readonly realmId: string;
        readonly realmToken: string;
      };
      realmPort.start();
      queueMicrotask(() => {
        realmPort?.postMessage({
          type: "realm-ready",
          protocol: REALM_PROTOCOL_VERSION,
          kind: init.kind,
          realmId: init.realmId,
          realmToken: init.realmToken,
          sequence: 0,
          viewport: { width: 800, height: 600 },
        });
      });
    },
  };
  const frame = new FakeIFrame(contentWindow);
  const createRealmDocument = (identity: RealmBootIdentity) => {
    boot = identity;
    return `<!doctype html><title>opaque ${identity.kind}</title>`;
  };
  const document = {
    body: {
      appendChild(element: FakeIFrame) {
        assert.equal(element, frame);
        frame.isConnected = true;
        frame.dispatchEvent(new Event("load"));
        if (!autoHandshake) return;
        queueMicrotask(() => {
          assert.ok(boot);
          const hello = {
            type: "realm-hello",
            protocol: REALM_PROTOCOL_VERSION,
            ...boot,
          };
          if (includeForgedMessages) {
            dispatchWindowMessage(parentWindow, hello, "https://play.test", contentWindow);
            dispatchWindowMessage(parentWindow, hello, "null", {});
            dispatchWindowMessage(
              parentWindow,
              { ...hello, bootNonce: "x".repeat(43) },
              "null",
              {}
            );
          }
          dispatchWindowMessage(parentWindow, hello, "null", contentWindow);
        });
      },
    },
    createElement(name: string) {
      assert.equal(name, "iframe");
      return frame;
    },
  };

  defineGlobal("window", parentWindow);
  defineGlobal("document", document);
  defineGlobal("requestAnimationFrame", (callback: (time: number) => void) => {
    callback(performance.now());
    return 1;
  });

  return {
    createRealmDocument,
    frame,
    initMessages,
    targetOrigins,
    transfers,
    get boot() {
      return boot;
    },
    restore() {
      realmPort?.close();
      restoreGlobal("window", previousWindow);
      restoreGlobal("document", previousDocument);
      restoreGlobal("requestAnimationFrame", previousAnimationFrame);
    },
  };
}

class FakeIFrame extends EventTarget {
  readonly attributes = new Map<string, string>();
  readonly contentWindow: {
    postMessage(data: unknown, targetOrigin: string, ports: MessagePort[]): void;
  };
  readonly dataset: Record<string, string> = {};
  readonly style: Record<string, string> = {};
  isConnected = false;
  removeCount = 0;
  src = "";
  srcdoc = "";
  tabIndex = 0;
  title = "";

  constructor(contentWindow: FakeIFrame["contentWindow"]) {
    super();
    this.contentWindow = contentWindow;
  }

  remove() {
    if (!this.isConnected) return;
    this.isConnected = false;
    this.removeCount += 1;
  }

  setAttribute(name: string, value: string) {
    this.attributes.set(name, value);
  }
}

function dispatchWindowMessage(
  target: EventTarget,
  data: unknown,
  origin: string,
  source: unknown
) {
  const event = new Event("message");
  Object.defineProperties(event, {
    data: { value: data },
    origin: { value: origin },
    source: { value: source },
  });
  target.dispatchEvent(event);
}

function defineGlobal(name: string, value: unknown) {
  Object.defineProperty(globalThis, name, {
    configurable: true,
    writable: true,
    value,
  });
}

function restoreGlobal(name: string, descriptor?: PropertyDescriptor) {
  if (descriptor) Object.defineProperty(globalThis, name, descriptor);
  else Reflect.deleteProperty(globalThis, name);
}
