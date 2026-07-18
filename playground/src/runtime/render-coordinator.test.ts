import assert from "node:assert/strict";
import test from "node:test";

import type { MermanDomainFacade } from "./merman-core.ts";
import {
  createRenderCoordinator,
  type RenderCoordinatorInput,
} from "./render-coordinator.ts";
import type {
  MermaidRealmController,
  MermaidRealmRenderResult,
} from "./mermaid-realm-controller.ts";

const VIEWPORT = { width: 800, height: 600 };

test("latest request publishes Merman and Mermaid as one coherent batch", async () => {
  const first = deferred<MermaidRealmRenderResult>();
  const second = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([first.promise, second.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setCompareViewport(VIEWPORT);
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
  assert.match(state.mermaid.svg, /second/);
  assert.equal(state.snapshot.source, "second");
});

test("updating disables old pair and partial replaces the failed pane", async () => {
  const first = deferred<MermaidRealmRenderResult>();
  const second = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([first.promise, second.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setCompareViewport(VIEWPORT);
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
      previousMermaid?.status === "success" ? previousMermaid.svg : "",
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
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setCompareViewport(VIEWPORT);
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
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setCompareViewport(VIEWPORT);
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
    debounceMs: 0,
    validateSvg: () => {},
  });
  coordinator.setCompareViewport(VIEWPORT);
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

function facade(): MermanDomainFacade {
  return {
    packageVersion: "test-merman",
    detectDiagram: () => ({
      status: "available",
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
  return {
    status: "success",
    svg: `<svg xmlns="http://www.w3.org/2000/svg"><text>${label}</text></svg>`,
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
