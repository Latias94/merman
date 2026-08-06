import assert from "node:assert/strict";
import test from "node:test";

import {
  createMermaidRealmController,
  type MermaidRealmExecutionResult,
  type MermaidRealmRenderInput,
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

test("ordinary Mermaid failures retain their realm-owned structured detail", async () => {
  const session = fakeSession(async () => ({
    status: "failure",
    stage: "render",
    message: "Parse error on line 2",
    detail: '{"hash":{"token":"INVALID"}}',
  }));
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => session,
  });

  assert.deepEqual(await controller.render(INPUT), {
    status: "failure",
    stage: "render",
    message: "Parse error on line 2",
    detail: '{"hash":{"token":"INVALID"}}',
  });
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

test("snapshots and freezes a queued render input before the realm can observe it", async () => {
  const first = deferred<MermaidRealmExecutionResult>();
  const observed: MermaidRealmRenderInput[] = [];
  const session = fakeSession(async (input, requestId) => {
    observed.push(input);
    return requestId === "compare-1" ? first.promise : success("second");
  });
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => session,
  });
  const mutableInput = {
    source: "flowchart TD\nA-->B",
    configJson: "{}",
    theme: "forest",
    diagramFont: "arial" as const,
    externalRequirements: {
      externalDiagrams: ["zenuml"],
      layoutModules: ["elk"],
    },
    viewport: { width: 640, height: 480 },
  };

  const active = controller.render(INPUT);
  const queued = controller.render(
    mutableInput as unknown as MermaidRealmRenderInput
  );
  mutableInput.source = "mutated";
  mutableInput.configJson = '{"theme":"dark"}';
  mutableInput.externalRequirements.externalDiagrams.push("zenuml");
  mutableInput.externalRequirements.layoutModules.push("tidy-tree");
  mutableInput.viewport.width = 1;
  await waitFor(() => observed.length === 1);
  first.resolve(success("first"));
  await active;
  await queued;

  assert.equal(observed.length, 2);
  const snapshot = observed[1];
  assert.equal(snapshot.source, "flowchart TD\nA-->B");
  assert.equal(snapshot.configJson, "{}");
  assert.deepEqual(snapshot.externalRequirements.externalDiagrams, ["zenuml"]);
  assert.deepEqual(snapshot.externalRequirements.layoutModules, ["elk"]);
  assert.deepEqual(snapshot.viewport, { width: 640, height: 480 });
  assert.equal(Object.isFrozen(snapshot), true);
  assert.equal(Object.isFrozen(snapshot.externalRequirements), true);
  assert.equal(Object.isFrozen(snapshot.externalRequirements.externalDiagrams), true);
  assert.equal(Object.isFrozen(snapshot.externalRequirements.layoutModules), true);
  assert.equal(Object.isFrozen(snapshot.viewport), true);
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

test("timeout interrupts a stalled viewport update before rendering", async () => {
  const stuckViewport = deferred<void>();
  let renderCalls = 0;
  const first = fakeSession(
    async () => {
      renderCalls += 1;
      return success("unreachable");
    },
    undefined,
    () => stuckViewport.promise,
  );
  const second = fakeSession(async () => success("recovered"));
  const sessions = [first, second];
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => sessions.shift()!,
    operationTimeoutMs: 5,
  });

  assert.deepEqual(
    await controller.render(INPUT),
    failure("timeout", "Mermaid realm operation timed out."),
  );
  assert.equal(first.disposeCalls, 1);
  assert.equal(renderCalls, 0);
  assert.equal((await controller.render(INPUT)).status, "success");
});

test("reset interrupts a stalled viewport update before rendering", async () => {
  const stuckViewport = deferred<void>();
  let viewportStarted = false;
  let renderCalls = 0;
  const session = fakeSession(
    async () => {
      renderCalls += 1;
      return success("unreachable");
    },
    undefined,
    () => {
      viewportStarted = true;
      return stuckViewport.promise;
    },
  );
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => session,
  });

  const pending = controller.render(INPUT);
  await waitFor(() => viewportStarted);
  controller.reset();

  assert.deepEqual(
    await pending,
    failure("disposed", "Mermaid realm operation was reset."),
  );
  assert.equal(session.disposeCalls, 1);
  assert.equal(renderCalls, 0);
});

test("reset invalidates operations that were already queued", async () => {
  const stuck = deferred<MermaidRealmExecutionResult>();
  const firstCalls: string[] = [];
  const first = fakeSession(async (_input, requestId) => {
    firstCalls.push(requestId);
    return stuck.promise;
  });
  const secondCalls: string[] = [];
  const second = fakeSession(async (_input, requestId) => {
    secondCalls.push(requestId);
    return success("recovered");
  });
  const sessions = [first, second];
  let createCalls = 0;
  const controller = createMermaidRealmController({
    kind: "compare",
    createSession: async () => {
      createCalls += 1;
      return sessions.shift()!;
    },
  });

  const active = controller.render(INPUT);
  const queued = controller.render(INPUT);
  await waitFor(() => firstCalls.length === 1);
  controller.reset();

  assert.deepEqual(
    await active,
    failure("disposed", "Mermaid realm operation was reset."),
  );
  assert.deepEqual(
    await queued,
    failure("disposed", "Mermaid realm operation was superseded."),
  );
  assert.deepEqual(firstCalls, ["compare-1"]);
  assert.equal(first.disposeCalls, 1);
  assert.equal(createCalls, 1);

  assert.equal((await controller.render(INPUT)).status, "success");
  assert.deepEqual(secondCalls, ["compare-3"]);
  assert.equal(createCalls, 2);
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

  const result = await controller.render(INPUT);
  assert.equal(result.status, "failure");
  if (result.status === "failure") {
    assert.equal(result.stage, "timeout");
    assert.equal(result.message, "Mermaid realm timed out during render.");
    assert.match(result.detail ?? "", /RealmTimeoutError/);
  }
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
  return { status: "failure", stage, message, detail: null };
}

function fakeSession(
  render: MermaidRealmSession["render"],
  onDispose?: () => void,
  setViewport: MermaidRealmSession["setViewport"] = async () => undefined,
): MermaidRealmSession & { disposeCalls: number } {
  return {
    disposeCalls: 0,
    render,
    setViewport,
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
