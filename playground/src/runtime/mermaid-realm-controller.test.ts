import assert from "node:assert/strict";
import test from "node:test";

import {
  createMermaidRealmController,
  type MermaidRealmExecutionResult,
  type MermaidRealmSession,
} from "./mermaid-realm-controller.ts";
import { RealmTimeoutError } from "./realm/channel-protocol.ts";

const INPUT = {
  source: "flowchart TD\nA-->B",
  configJson: "{}",
  theme: "default",
  diagramFont: "trebuchet" as const,
  externalRequirements: { externalDiagrams: [], layoutModules: [] },
  viewport: { width: 800, height: 600 },
};

test("ordinary realm failures do not poison the serialized operation queue", async () => {
  const calls: string[] = [];
  const results: MermaidRealmExecutionResult[] = [
    failure("render", "bad syntax"),
    success("next"),
  ];
  const session = fakeSession(async (_input, requestId) => {
    calls.push(requestId);
    return results.shift()!;
  });
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => session,
  });

  assert.equal((await controller.render(INPUT)).status, "failure");
  assert.equal((await controller.render(INPUT)).status, "success");
  assert.deepEqual(calls, ["compare-1", "compare-2"]);
  assert.equal(session.disposeCalls, 0);
});

test("concurrent controller callers cannot interleave one realm", async () => {
  const first = deferred<MermaidRealmExecutionResult>();
  const calls: string[] = [];
  const session = fakeSession(async (_input, requestId) => {
    calls.push(requestId);
    return requestId === "compare-1" ? first.promise : success("second");
  });
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => session,
  });

  const firstRender = controller.render(INPUT);
  const secondRender = controller.render(INPUT);
  await waitFor(() => calls.length === 1);
  assert.deepEqual(calls, ["compare-1"]);

  first.resolve(success("first"));
  assert.equal((await firstRender).status, "success");
  assert.equal((await secondRender).status, "success");
  assert.deepEqual(calls, ["compare-1", "compare-2"]);
});

test("timeout destroys the old realm before a later operation creates one", async () => {
  const stuck = deferred<MermaidRealmExecutionResult>();
  const first = fakeSession(async () => stuck.promise);
  const second = fakeSession(async () => success("recovered"));
  const sessions = [first, second];
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => sessions.shift()!,
    operationTimeoutMs: 5,
  });

  const timedOut = await controller.render(INPUT);
  assert.deepEqual(timedOut, failure("timeout", "Mermaid realm operation timed out."));
  assert.equal(first.disposeCalls, 1);

  const recovered = await controller.render(INPUT);
  assert.equal(recovered.status, "success");
  assert.equal(second.disposeCalls, 0);
});

test("timeout remains classified when disposing rejects the active render", async () => {
  const stuck = deferred<MermaidRealmExecutionResult>();
  const session = fakeSession(
    async () => stuck.promise,
    () => stuck.reject(new Error("channel closed"))
  );
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => session,
    operationTimeoutMs: 5,
  });

  assert.deepEqual(
    await controller.render(INPUT),
    failure("timeout", "Mermaid realm operation timed out.")
  );
  assert.equal(session.disposeCalls, 1);
});

test("channel stage timeouts retain their timeout failure stage", async () => {
  const session = fakeSession(async () => {
    throw new RealmTimeoutError("Mermaid realm timed out during render.");
  });
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => session,
  });

  assert.deepEqual(
    await controller.render(INPUT),
    failure("timeout", "Mermaid realm timed out during render.")
  );
  assert.equal(session.disposeCalls, 1);
});

test("parent-side SVG rejection poisons a realm that claimed success", async () => {
  const unsafe = fakeSession(async () => ({
    ...success("unsafe"),
    svg: '<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>',
  }));
  const safe = fakeSession(async () => success("safe"));
  const sessions = [unsafe, safe];
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => sessions.shift()!,
  });

  const rejected = await controller.render(INPUT);
  assert.equal(rejected.status, "failure");
  if (rejected.status === "failure") {
    assert.equal(rejected.stage, "svg-validation");
  }
  assert.equal(unsafe.disposeCalls, 1);
  assert.equal((await controller.render(INPUT)).status, "success");
});

test("dispose is idempotent and prevents queued work from acquiring a realm", async () => {
  let creates = 0;
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => {
      creates += 1;
      return fakeSession(async () => success("unused"));
    },
  });

  controller.dispose();
  controller.dispose();
  assert.deepEqual(
    await controller.render(INPUT),
    failure("disposed", "Mermaid realm controller is disposed.")
  );
  assert.equal(creates, 0);
});

function success(label: string): MermaidRealmExecutionResult {
  return {
    status: "success",
    svg: `<svg xmlns="http://www.w3.org/2000/svg"><text>${label}</text></svg>`,
    prepareTimeMs: 1,
    renderTimeMs: 2,
    presentationTimeMs: 3,
    version: "11.16.0",
  };
}

function failure(
  stage: "render" | "timeout" | "svg-validation" | "disposed",
  message: string
): Extract<MermaidRealmExecutionResult, { status: "failure" }> {
  return { status: "failure", stage, message };
}

function fakeSession(
  render: MermaidRealmSession["render"],
  onDispose?: () => void
): MermaidRealmSession & { disposeCalls: number } {
  return {
    disposeCalls: 0,
    render,
    async setViewport() {},
    dispose() {
      this.disposeCalls += 1;
      onDispose?.();
    },
  };
}

function deferred<T>() {
  return Promise.withResolvers<T>();
}

async function waitFor(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  throw new Error("Timed out waiting for test condition.");
}
