import assert from "node:assert/strict";
import test from "node:test";

import { REALM_PROTOCOL_VERSION } from "./channel-protocol.ts";
import { createAuthenticatedBrowserRealmChannel } from "./browser-realm-channel.ts";

test("browser realm channel authenticates one exact peer and transfers one port", async () => {
  const harness = installBrowserHarness();
  try {
    const failures: Error[] = [];
    const channel = await createAuthenticatedBrowserRealmChannel({
      kind: "compare",
      realmUrl: new URL("https://play.test/compare-realm.html"),
      initialViewport: { width: 800, height: 600 },
      signal: new AbortController().signal,
      label: "Test realm",
      title: "Test Realm",
      onFailure: (error) => failures.push(error),
    });

    assert.equal(harness.initMessages.length, 1);
    assert.equal(harness.transfers.length, 1);
    assert.equal(harness.transfers[0]?.length, 1);
    assert.deepEqual(harness.initMessages[0], {
      type: "realm-init",
      protocol: REALM_PROTOCOL_VERSION,
      kind: "compare",
      realmId: channel.identity.realmId,
      bootNonce: harness.bootNonce,
      realmToken: channel.identity.realmToken,
    });
    assert.match(channel.identity.realmId, /^[A-Za-z0-9_-]{43}$/);
    assert.equal(harness.frame.dataset.mermanRealm, "compare");
    assert.equal(harness.frame.title, "Test Realm");
    assert.equal(harness.frame.style.width, "800px");
    assert.equal(harness.frame.style.height, "600px");
    assert.equal(failures.length, 0);

    await channel.setViewport({ width: 640, height: 480 });
    assert.equal(harness.frame.style.width, "640px");
    assert.equal(harness.frame.style.height, "480px");

    channel.dispose();
    channel.dispose();
    assert.equal(harness.frame.removeCount, 1);
    assert.equal(failures.length, 0);
  } finally {
    harness.restore();
  }
});

test("browser realm channel rejects an authenticated hello from the wrong path", async () => {
  const harness = installBrowserHarness({ realmPath: "/unexpected.html" });
  try {
    const failures: Error[] = [];
    await assert.rejects(
      createAuthenticatedBrowserRealmChannel({
        kind: "compare",
        realmUrl: new URL("https://play.test/compare-realm.html"),
        initialViewport: { width: 800, height: 600 },
        signal: new AbortController().signal,
        label: "Test realm",
        title: "Test Realm",
        onFailure: (error) => failures.push(error),
      }),
      /unexpected path/
    );
    assert.equal(harness.initMessages.length, 0);
    assert.equal(failures.length, 1);
    assert.equal(harness.frame.removeCount, 1);
  } finally {
    harness.restore();
  }
});

test("browser realm channel refuses a cross-origin realm before attachment", async () => {
  const harness = installBrowserHarness({ autoHandshake: false });
  try {
    await assert.rejects(
      createAuthenticatedBrowserRealmChannel({
        kind: "compare",
        realmUrl: new URL("https://foreign.test/compare-realm.html"),
        initialViewport: { width: 800, height: 600 },
        signal: new AbortController().signal,
        label: "Test realm",
        title: "Test Realm",
        onFailure: () => {},
      }),
      /same-origin URL/
    );
    assert.equal(harness.frame.isConnected, false);
  } finally {
    harness.restore();
  }
});

test("browser realm channel poisons a realm that navigates after authentication", async () => {
  const harness = installBrowserHarness();
  try {
    const failed = Promise.withResolvers<Error>();
    const channel = await createAuthenticatedBrowserRealmChannel({
      kind: "compare",
      realmUrl: new URL("https://play.test/compare-realm.html"),
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
    assert.equal(harness.frame.removeCount, 1);
  } finally {
    harness.restore();
  }
});

test("browser realm channel rejects timeout and pre-aborted handshakes", async () => {
  const timeoutHarness = installBrowserHarness({ autoHandshake: false });
  try {
    const failures: Error[] = [];
    await assert.rejects(
      createAuthenticatedBrowserRealmChannel({
        kind: "benchmark",
        realmUrl: new URL("https://play.test/benchmark.html"),
        initialViewport: { width: 800, height: 600 },
        signal: new AbortController().signal,
        handshakeTimeoutMs: 5,
        label: "Benchmark realm",
        title: "Benchmark Realm",
        onFailure: (error) => failures.push(error),
      }),
      /handshake timed out/
    );
    assert.equal(failures.length, 1);
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
        realmUrl: new URL("https://play.test/benchmark.html"),
        initialViewport: { width: 800, height: 600 },
        signal: controller.signal,
        label: "Benchmark realm",
        title: "Benchmark Realm",
        onFailure: () => {},
      }),
      (error: unknown) =>
        error instanceof DOMException && error.name === "AbortError"
    );
    assert.equal(abortHarness.frame.removeCount, 1);
  } finally {
    abortHarness.restore();
  }
});

interface HarnessOptions {
  readonly autoHandshake?: boolean;
  readonly realmPath?: string;
}

function installBrowserHarness({
  autoHandshake = true,
  realmPath,
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
  const transfers: MessagePort[][] = [];
  let realmPort: MessagePort | null = null;
  let bootNonce = "";

  const contentWindow = {
    innerWidth: 800,
    innerHeight: 600,
    location: { pathname: realmPath ?? "/compare-realm.html" },
    postMessage(data: unknown, targetOrigin: string, ports: MessagePort[]) {
      assert.equal(targetOrigin, "https://play.test");
      initMessages.push(data);
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
          viewport: {
            width: contentWindow.innerWidth,
            height: contentWindow.innerHeight,
          },
        });
      });
    },
  };
  const frame = new FakeIFrame(contentWindow);
  const document = {
    body: {
      appendChild(element: FakeIFrame) {
        assert.equal(element, frame);
        frame.isConnected = true;
        frame.dispatchEvent(new Event("load"));
        if (!autoHandshake) return;
        queueMicrotask(() => {
          const url = new URL(frame.src);
          const params = new URLSearchParams(url.hash.slice(1));
          bootNonce = params.get("boot") ?? "";
          const hello = {
            type: "realm-hello",
            protocol: REALM_PROTOCOL_VERSION,
            kind: params.get("kind"),
            realmId: params.get("realm"),
            bootNonce,
          };
          dispatchWindowMessage(
            parentWindow,
            hello,
            "https://foreign.test",
            contentWindow
          );
          dispatchWindowMessage(
            parentWindow,
            hello,
            "https://play.test",
            {}
          );
          dispatchWindowMessage(
            parentWindow,
            hello,
            "https://play.test",
            contentWindow
          );
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
    frame,
    initMessages,
    transfers,
    get bootNonce() {
      return bootNonce;
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
    innerHeight: number;
    innerWidth: number;
    location: { pathname: string };
    postMessage(data: unknown, targetOrigin: string, ports: MessagePort[]): void;
  };
  readonly dataset: Record<string, string> = {};
  readonly style: Record<string, string> = {};
  isConnected = false;
  removeCount = 0;
  src = "";
  tabIndex = 0;
  title = "";

  constructor(contentWindow: FakeIFrame["contentWindow"]) {
    super();
    this.contentWindow = contentWindow;
    defineDimension(this.style, "width", (value) => {
      this.contentWindow.innerWidth = value;
    });
    defineDimension(this.style, "height", (value) => {
      this.contentWindow.innerHeight = value;
    });
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

function defineDimension(
  style: Record<string, string>,
  name: "height" | "width",
  update: (value: number) => void
) {
  let stored = "";
  Object.defineProperty(style, name, {
    configurable: true,
    enumerable: true,
    get: () => stored,
    set(value: string) {
      stored = value;
      update(Number.parseFloat(value));
    },
  });
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

function restoreGlobal(
  name: string,
  descriptor: PropertyDescriptor | undefined
) {
  if (descriptor) {
    Object.defineProperty(globalThis, name, descriptor);
    return;
  }
  Reflect.deleteProperty(globalThis, name);
}
