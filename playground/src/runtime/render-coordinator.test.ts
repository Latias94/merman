import assert from "node:assert/strict";
import test from "node:test";
import { projectSafeInlineSvg } from "./render-artifact.ts";

import type { MermanDomainFacade } from "./merman-core.ts";
import {
  createRenderCoordinator,
  type RenderCoordinatorInput,
} from "./render-coordinator.ts";
import type {
  MermaidRealmController,
  MermaidRealmRenderResult,
} from "./mermaid-realm-controller.ts";
import { projectError } from "./error-projection.ts";

const VIEWPORT = { width: 800, height: 600 };

test("projects binding and cyclic object failures without object coercion", () => {
  const binding = projectError({
    version: 2,
    ok: false,
    code: 5,
    code_name: "MERMAN_PARSE_ERROR",
    message: "Expected a diagram statement.",
  });
  assert.equal(binding.summary, "Expected a diagram statement.");
  assert.doesNotMatch(binding.summary, /\[object Object\]/);
  assert.match(binding.detail ?? "", /"code_name": "MERMAN_PARSE_ERROR"/);

  const cyclic: { message: string; self?: unknown } = {
    message: "Structured failure",
  };
  cyclic.self = cyclic;
  const projected = projectError(cyclic);
  assert.equal(projected.summary, "Structured failure");
  assert.match(projected.detail ?? "", /\[circular\]/);
  assert.doesNotMatch(projected.detail ?? "", /\[object Object\]/);

  const opaque = new Proxy(
    {},
    {
      get() {
        throw new Error("unreadable getter");
      },
      ownKeys() {
        throw new Error("unreadable keys");
      },
    }
  );
  assert.doesNotThrow(() => projectError(opaque));
  assert.equal(projectError(opaque).detail, '"[unreadable object]"');

  const hostilePrototype = new Proxy(
    {},
    {
      getPrototypeOf() {
        throw new Error("unreadable prototype");
      },
    }
  );
  assert.deepEqual(projectError(hostilePrototype), {
    summary: "Unexpected error.",
    detail: '"[unreadable error]"',
  });
});

test("latest request publishes Merman and Mermaid as one coherent batch", async () => {
  const first = deferred<MermaidRealmRenderResult>();
  const second = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([first.promise, second.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setCompareEnabled(true);

  coordinator.setInput(input("first"));
  await waitFor(() => compare.calls.length === 1);
  coordinator.setInput(input("second"));
  assert.match(coordinator.store.getState().status, /pending|updating/);

  first.resolve(mermaidSuccess("first"));
  await waitFor(() => compare.calls.length === 2);
  second.resolve(mermaidSuccess("second"));
  await waitFor(() => coordinator.store.getState().status === "success");

  const state = coordinator.store.getState();
  assert.equal(state.status, "success");
  if (state.status !== "success") return;
  assert.match(state.merman.svg, /second/);
  assert.ok(state.mermaid);
  assert.equal(state.mermaid.artifact.kind, "safe-inline-svg");
  if (state.mermaid.artifact.kind === "safe-inline-svg") {
    assert.match(state.mermaid.artifact.svg, /second/);
  }
  assert.equal(state.snapshot.source, "second");
});

test("updating disables old pair and partial replaces the failed pane", async () => {
  const first = deferred<MermaidRealmRenderResult>();
  const second = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([first.promise, second.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setCompareEnabled(true);

  coordinator.setInput(input("stable"));
  await waitFor(() => compare.calls.length === 1);
  first.resolve(mermaidSuccess("stable"));
  await waitFor(() => coordinator.store.getState().status === "success");

  coordinator.setInput(input("partial"));
  await waitFor(() => compare.calls.length === 2);
  const updating = coordinator.store.getState();
  assert.equal(updating.status, "updating");
  if (updating.status === "updating") {
    assert.equal(updating.actionsEnabled, false);
    const previousMermaid = updating.previous.mermaid;
    assert.equal(previousMermaid?.status, "success");
    assert.match(
      previousMermaid?.status === "success" &&
        previousMermaid.artifact.kind === "safe-inline-svg"
        ? previousMermaid.artifact.svg
        : "",
      /stable/
    );
  }

  second.resolve({ status: "failure", stage: "render", message: "broken" });
  await waitFor(() => coordinator.store.getState().status === "partial");
  const partial = coordinator.store.getState();
  assert.equal(partial.status, "partial");
  if (partial.status !== "partial") return;
  assert.equal(partial.actionsEnabled, true);
  assert.equal(partial.merman.status, "success");
  assert.match(
    partial.merman.status === "success" ? partial.merman.svg : "",
    /partial/
  );
  assert.equal(partial.mermaid.status, "failure");
  assert.equal("svg" in partial.mermaid, false);
});

test("pause waits for active work and resumes only the latest snapshot", async () => {
  const active = deferred<MermaidRealmRenderResult>();
  const resumed = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([active.promise, resumed.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setCompareEnabled(true);
  coordinator.setInput(input("active"));
  await waitFor(() => compare.calls.length === 1);

  const leasePromise = coordinator.pause();
  coordinator.setInput(input("ignored"));
  coordinator.setInput(input("latest"));
  active.resolve(mermaidSuccess("active"));
  const release = await leasePromise;
  assert.equal(compare.calls.length, 1);
  release();
  release();

  await waitFor(() => compare.calls.length === 2);
  assert.equal(compare.calls[1].source, "latest");
  resumed.resolve(mermaidSuccess("latest"));
  await waitFor(() => coordinator.store.getState().status === "success");
});

test("blank source and suspend reject every late completion", async () => {
  const active = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([active.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setCompareEnabled(true);
  coordinator.setInput(input("active"));
  await waitFor(() => compare.calls.length === 1);

  coordinator.suspend();
  coordinator.setInput(input(""));
  active.resolve(mermaidSuccess("late"));
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(coordinator.store.getState().status, "empty");
  assert.equal(compare.resetCalls, 1);
});

test("request exceptions become typed failures and later work still runs", async () => {
  const rejected = deferred<MermaidRealmRenderResult>();
  const recovered = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([rejected.promise, recovered.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setCompareEnabled(true);
  coordinator.setInput(input("throws", throwingFacade()));
  await waitFor(() => compare.calls.length === 1);
  rejected.reject(new Error("channel failed"));
  await waitFor(() => coordinator.store.getState().status === "failed");

  const failed = coordinator.store.getState();
  assert.equal(failed.status, "failed");
  if (failed.status === "failed") {
    assert.equal(failed.detection.status, "unavailable");
    assert.equal(failed.merman.stage, "render");
    assert.equal(failed.mermaid?.stage, "protocol");
  }

  coordinator.setInput(input("recovered"));
  await waitFor(() => compare.calls.length === 2);
  recovered.resolve(mermaidSuccess("recovered"));
  await waitFor(() => coordinator.store.getState().status === "success");
});

test("render failures retain binding details in the completed batch", async () => {
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    compareViewport: VIEWPORT,
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setInput(input("broken", bindingFailureFacade()));
  await waitFor(() => coordinator.store.getState().status === "failed");

  const state = coordinator.store.getState();
  assert.equal(state.status, "failed");
  if (state.status !== "failed") return;
  assert.equal(state.merman.message, "Source is invalid.");
  assert.match(state.merman.detail ?? "", /MERMAN_PARSE_ERROR/);
  assert.doesNotMatch(state.merman.message, /\[object Object\]/);
});

function input(
  source: string,
  domainFacade: MermanDomainFacade = facade()
): RenderCoordinatorInput {
  return {
    facade: domainFacade,
    source,
    theme: "default",
    configJson: "{}",
    options: {
      diagramFont: "trebuchet",
      textMeasurementMode: "browser",
    },
  };
}

function throwingFacade(): MermanDomainFacade {
  return {
    ...facade(),
    detectDiagram() {
      throw new Error("detection failed");
    },
    render() {
      throw new Error("render failed");
    },
  };
}

function bindingFailureFacade(): MermanDomainFacade {
  return {
    ...facade(),
    render() {
      throw {
        version: 2,
        ok: false,
        code: 5,
        code_name: "MERMAN_PARSE_ERROR",
        message: "Source is invalid.",
      };
    },
  };
}

function facade(): MermanDomainFacade {
  return {
    packageVersion: "test-merman",
    detectDiagram: () => ({
      status: "available",
      validity: "valid",
      diagramType: "flowchart",
      syntaxId: "flowchart-v2",
      effectiveLayoutId: "dagre",
    }),
    getAsciiSupportedDiagrams: () => ["flowchart"],
    render: (source: string) => ({
      svg: `<svg xmlns="http://www.w3.org/2000/svg"><text>${source}</text></svg>`,
      error: null,
      renderTime: 2,
    }),
    renderAscii: (source: string) => source,
  } as unknown as MermanDomainFacade;
}

function fakeCompare(
  results: Promise<MermaidRealmRenderResult>[]
): MermaidRealmController & {
  calls: Parameters<MermaidRealmController["render"]>[0][];
  resetCalls: number;
} {
  return {
    calls: [],
    resetCalls: 0,
    dispose() {},
    reset() {
      this.resetCalls += 1;
    },
    render(input) {
      this.calls.push(input);
      const result = results.shift();
      if (!result) throw new Error("Unexpected Compare render.");
      return result;
    },
  };
}

function mermaidSuccess(label: string): MermaidRealmRenderResult {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg"><text>${label}</text></svg>`;
  return {
    status: "success",
    artifact: projectSafeInlineSvg(svg),
    prepareTimeMs: 1,
    renderTimeMs: 2,
    presentationTimeMs: 3,
    version: "11.16.0",
  };
}

function deferred<T>() {
  return Promise.withResolvers<T>();
}

async function waitFor(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  throw new Error("Timed out waiting for test condition.");
}
