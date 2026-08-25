import assert from "node:assert/strict";
import test from "node:test";

import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
  type CompareRenderRequest,
  type RealmIdentity,
} from "./channel-protocol.ts";
import { serveCompareRealmPort } from "./compare-bootstrap.ts";

const IDENTITY: RealmIdentity = {
  kind: "compare",
  realmId: "realm-1",
  realmToken: "t".repeat(43),
};

test("encoded response overflow fails fast without poisoning the realm queue", async () => {
  const escapedSvg = '"'.repeat(Math.floor(REALM_BUDGETS.messageBytes / 2) + 1024);
  assert.ok(Buffer.byteLength(escapedSvg) < REALM_BUDGETS.svgBytes);
  assert.ok(
    Buffer.byteLength(JSON.stringify(successResponseShape(escapedSvg))) >
      REALM_BUDGETS.messageBytes
  );

  const harness = createRealmServerHarness([
    async () => engineSuccess(escapedSvg),
    async () => engineSuccess("<svg />"),
  ]);
  try {
    harness.dispatch(renderRequest(1, "request-1"));
    const failure = await harness.waitForTerminal("request-1");
    assert.deepEqual(failure, {
      type: "render-failure",
      protocol: REALM_PROTOCOL_VERSION,
      ...IDENTITY,
      sequence: failure.sequence,
      requestId: "request-1",
      stage: "svg-budget",
      message: "Realm message exceeds the 25 MiB budget.",
      detail: JSON.stringify(
        { name: "RealmBudgetError", resource: "message" },
        null,
        2
      ),
    });

    harness.dispatch(renderRequest(2, "request-2"));
    const recovered = await harness.waitForTerminal("request-2");
    assert.equal(recovered.type, "render-success");
    assertConsecutiveSequences(harness.messages);
  } finally {
    harness.dispose();
  }
});

test("Mermaid engine errors retain their stage and detail without blocking later work", async () => {
  const engineError = Object.assign(new Error("Parse error on line 2"), {
    stage: "render" as const,
    error: {
      summary: "Parse error on line 2",
      detail: '{"hash":{"token":"INVALID"}}',
    },
  });
  const harness = createRealmServerHarness([
    async () => {
      throw engineError;
    },
    async () => engineSuccess("<svg />"),
  ]);
  try {
    harness.dispatch(renderRequest(1, "request-1"));
    const failure = await harness.waitForTerminal("request-1");
    assert.equal(failure.type, "render-failure");
    if (failure.type === "render-failure") {
      assert.equal(failure.stage, "render");
      assert.equal(failure.message, "Parse error on line 2");
      assert.equal(failure.detail, '{"hash":{"token":"INVALID"}}');
    }

    harness.dispatch(renderRequest(2, "request-2"));
    assert.equal((await harness.waitForTerminal("request-2")).type, "render-success");
    assertConsecutiveSequences(harness.messages);
  } finally {
    harness.dispose();
  }
});

test("sets the controlled screen width before loading the Mermaid engine", async () => {
  let screenWidthAtLoad: number | null = null;
  const harness = createRealmServerHarness(
    [async () => engineSuccess("<svg />")],
    () => {
      screenWidthAtLoad = window.screen.availWidth;
    },
  );
  try {
    harness.dispatch(renderRequest(1, "request-1", 1512));
    assert.equal((await harness.waitForTerminal("request-1")).type, "render-success");
    assert.equal(screenWidthAtLoad, 1512);
  } finally {
    harness.dispose();
  }
});

function createRealmServerHarness(
  renders: Array<() => Promise<ReturnType<typeof engineSuccess>>>,
  onLoadEngine: () => void = () => undefined,
) {
  const previousDocument = Object.getOwnPropertyDescriptor(globalThis, "document");
  const previousElement = Object.getOwnPropertyDescriptor(globalThis, "HTMLElement");
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  class FakeElement {}
  const host = new FakeElement();
  defineGlobal("HTMLElement", FakeElement);
  defineGlobal("document", {
    fonts: { ready: Promise.resolve() },
    getElementById: (id: string) => (id === "presentation-host" ? host : null),
  });
  defineGlobal("window", {
    innerWidth: 800,
    innerHeight: 600,
    screen: { availWidth: 800 },
  });

  const channel = new MessageChannel();
  const messages: Array<Record<string, unknown>> = [];
  channel.port2.onmessage = (event) => {
    messages.push(event.data as Record<string, unknown>);
  };
  channel.port2.start();

  let renderIndex = 0;
  serveCompareRealmPort(channel.port1, IDENTITY, async () => {
    onLoadEngine();
    return {
      renderWithMermaid: async () => {
        const render = renders[renderIndex];
        renderIndex += 1;
        if (!render) throw new Error("Unexpected render request.");
        return render();
      },
    };
  });

  return {
    messages,
    dispatch(request: CompareRenderRequest) {
      channel.port2.postMessage(request);
    },
    async waitForTerminal(requestId: string) {
      const message = await waitFor(() =>
        messages.find(
          (candidate) =>
            candidate.requestId === requestId &&
            (candidate.type === "render-success" ||
              candidate.type === "render-failure")
        )
      );
      return message as
        | {
            type: "render-success";
            sequence: number;
          }
        | {
            type: "render-failure";
            detail: string | null;
            message: string;
            sequence: number;
            stage: string;
          };
    },
    dispose() {
      channel.port1.close();
      channel.port2.close();
      restoreGlobal("document", previousDocument);
      restoreGlobal("HTMLElement", previousElement);
      restoreGlobal("window", previousWindow);
    },
  };
}

function renderRequest(
  sequence: number,
  requestId: string,
  screenAvailableWidth = 1512,
): CompareRenderRequest {
  return {
    type: "render",
    protocol: REALM_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence,
    requestId,
    payload: {
      source: "flowchart TD\nA-->B",
      configJson: "{}",
      theme: "default",
      diagramFont: "trebuchet",
      externalRequirements: { externalDiagrams: [], layoutModules: [] },
      screenAvailableWidth,
      viewport: { width: 800, height: 600 },
    },
  };
}

function engineSuccess(svg: string) {
  return {
    svg,
    prepareTimeMs: 1,
    renderTimeMs: 2,
    presentationTimeMs: 3,
    version: "11.16.0",
  };
}

function successResponseShape(svg: string) {
  return {
    type: "render-success",
    protocol: REALM_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence: 1,
    requestId: "request-1",
    ...engineSuccess(svg),
  };
}

function assertConsecutiveSequences(messages: Array<Record<string, unknown>>) {
  assert.deepEqual(
    messages.map((message) => message.sequence),
    messages.map((_, index) => index)
  );
}

async function waitFor<T>(read: () => T | undefined): Promise<T> {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const value = read();
    if (value !== undefined) return value;
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
  throw new Error("Timed out waiting for Compare realm terminal response.");
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
