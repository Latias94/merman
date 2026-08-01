import assert from "node:assert/strict";
import test from "node:test";
import { runInNewContext } from "node:vm";
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
    version: 1,
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

  const parserError = new Error("Parse error on line 2");
  Object.assign(parserError, {
    hash: {
      expected: ["NODE_TEXT"],
      loc: { first_column: 4, first_line: 2 },
      token: "INVALID",
    },
  });
  const parserProjection = projectError(parserError);
  assert.equal(parserProjection.summary, "Parse error on line 2");
  assert.match(parserProjection.detail ?? "", /"token": "INVALID"/);
  assert.match(parserProjection.detail ?? "", /"first_line": 2/);

  const crossRealmError = runInNewContext(
    'Object.assign(new Error("Cross-realm Merman failure."), { code: "MERMAN_CROSS_REALM" })'
  );
  assert.equal(crossRealmError instanceof Error, false);
  const crossRealmProjection = projectError(crossRealmError);
  assert.equal(crossRealmProjection.summary, "Cross-realm Merman failure.");
  assert.match(crossRealmProjection.detail ?? "", /MERMAN_CROSS_REALM/);
  assert.doesNotMatch(crossRealmProjection.summary, /\[object Object\]/);
  assert.doesNotMatch(crossRealmProjection.detail ?? "", /\[object Object\]/);

  const oversizedBinding = projectError({
    version: 1,
    ok: false,
    code: 5,
    code_name: "MERMAN_PARSE_ERROR",
    message: "x".repeat(9_001),
  });
  assert.ok(oversizedBinding.summary.length < 9_001);
  assert.match(oversizedBinding.summary, /\[truncated\]$/);
});

test("latest request publishes Merman and Mermaid as one coherent batch", async () => {
  const first = deferred<MermaidRealmRenderResult>();
  const second = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([first.promise, second.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
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
  assert.match(state.merman.artifact.svg, /second/);
  assert.ok(state.mermaid);
  assert.equal(state.mermaid.artifact.kind, "safe-inline-svg");
  if (state.mermaid.artifact.kind === "safe-inline-svg") {
    assert.match(state.mermaid.artifact.svg, /second/);
  }
  assert.equal(state.snapshot.source, "second");
});

test("publishes the producer-owned Merman artifact without reprojecting it", async () => {
  const artifact = projectSafeInlineSvg(
    '<svg xmlns="http://www.w3.org/2000/svg"><text>owned</text></svg>'
  );
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });
  coordinator.setInput(
    input("owned", {
      ...facade(),
      render: () => ({
        artifact,
        error: null,
        renderTime: 2,
        status: "success",
      }),
    })
  );
  await waitFor(() => coordinator.store.getState().status === "success");

  const state = coordinator.store.getState();
  assert.equal(state.status, "success");
  if (state.status !== "success") return;
  assert.equal(state.merman.status, "success");
  if (state.merman.status !== "success") return;
  assert.equal(state.merman.artifact, artifact);
});

test("preserves producer SVG validation failures", async () => {
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });
  coordinator.setInput(
    input("unsafe", {
      ...facade(),
      render: () => ({
        artifact: null,
        error: { summary: "Unsafe SVG.", detail: null },
        renderTime: 0,
        stage: "svg-validation",
        status: "failure",
      }),
    })
  );
  await waitFor(() => coordinator.store.getState().status === "failed");

  const state = coordinator.store.getState();
  assert.equal(state.status, "failed");
  if (state.status !== "failed") return;
  assert.equal(state.merman.stage, "svg-validation");
  assert.equal(state.merman.message, "Unsafe SVG.");
});

test("rejects a facade artifact that was not created by the projector", async () => {
  const artifact = projectSafeInlineSvg(
    '<svg xmlns="http://www.w3.org/2000/svg"><text>safe</text></svg>'
  );
  const forgedArtifact = {
    ...artifact,
    svg: '<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)" />',
  };
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });
  coordinator.setInput(
    input("forged", {
      ...facade(),
      render: () => ({
        artifact: forgedArtifact,
        error: null,
        renderTime: 2,
        status: "success",
      }),
    })
  );
  await waitFor(() => coordinator.store.getState().status === "failed");

  const state = coordinator.store.getState();
  assert.equal(state.status, "failed");
  if (state.status !== "failed") return;
  assert.equal(state.merman.stage, "svg-validation");
});

test("updating disables old pair and partial replaces the failed pane", async () => {
  const first = deferred<MermaidRealmRenderResult>();
  const second = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([first.promise, second.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
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

  second.resolve({
    status: "failure",
    stage: "render",
    message: "Mermaid parse failed",
    detail: '{"token":"INVALID"}',
  });
  await waitFor(() => coordinator.store.getState().status === "partial");
  const partial = coordinator.store.getState();
  assert.equal(partial.status, "partial");
  if (partial.status !== "partial") return;
  assert.equal(partial.actionsEnabled, true);
  assert.equal(partial.merman.status, "success");
  assert.match(
    partial.merman.status === "success" ? partial.merman.artifact.svg : "",
    /partial/
  );
  assert.equal(partial.mermaid.status, "failure");
  assert.equal(partial.mermaid.message, "Mermaid parse failed");
  assert.equal(partial.mermaid.detail, '{"token":"INVALID"}');
  assert.notEqual(partial.mermaid.message, partial.merman.status);
  assert.equal("svg" in partial.mermaid, false);
});

test("a completed Mermaid failure replaces stale success without borrowing Merman error", async () => {
  const first = deferred<MermaidRealmRenderResult>();
  const second = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([first.promise, second.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });
  coordinator.setCompareEnabled(true);

  coordinator.setInput(input("stable"));
  await waitFor(() => compare.calls.length === 1);
  first.resolve(mermaidSuccess("stable"));
  await waitFor(() => coordinator.store.getState().status === "success");

  coordinator.setInput(input("invalid", bindingFailureFacade()));
  await waitFor(() => compare.calls.length === 2);
  assert.equal(coordinator.store.getState().status, "updating");
  second.resolve({
    status: "failure",
    stage: "render",
    message: "Parse error on line 1",
    detail: '{"engine":"mermaid","token":"INVALID"}',
  });
  await waitFor(() => coordinator.store.getState().status === "failed");

  const completed = coordinator.store.getState();
  assert.equal(completed.status, "failed");
  if (completed.status !== "failed" || !completed.mermaid) return;
  assert.equal(completed.snapshot.source, "invalid");
  assert.equal(completed.merman.message, "Source is invalid.");
  assert.match(completed.merman.detail ?? "", /MERMAN_PARSE_ERROR/);
  assert.equal(completed.mermaid.message, "Parse error on line 1");
  assert.match(completed.mermaid.detail ?? "", /"engine":"mermaid"/);
  assert.doesNotMatch(completed.mermaid.detail ?? "", /MERMAN_PARSE_ERROR/);
});

test("pause waits for active work and resumes only the latest snapshot", async () => {
  const active = deferred<MermaidRealmRenderResult>();
  const resumed = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([active.promise, resumed.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
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

test("normalizes raw Merman Error and object payloads before publication", async () => {
  const nativeFailure = Object.assign(
    new Error("Native Merman render failure."),
    { code: "MERMAN_RENDER_ERROR" }
  );
  const structuredFailure = {
    message: "Structured Merman render failure.",
    reason: { code: "MERMAN_PARSE_ERROR" },
  };

  for (const [failure, message, detail] of [
    [nativeFailure, "Native Merman render failure.", "MERMAN_RENDER_ERROR"],
    [
      structuredFailure,
      "Structured Merman render failure.",
      "MERMAN_PARSE_ERROR",
    ],
  ] as const) {
    const coordinator = createRenderCoordinator({
      compare: fakeCompare([]),
      compareViewport: VIEWPORT,
      debounceMs: 0,
    });
    coordinator.setInput(input("broken", rawFailureFacade(failure)));
    await waitFor(() => coordinator.store.getState().status === "failed");

    const state = coordinator.store.getState();
    assert.equal(state.status, "failed");
    if (state.status !== "failed") continue;
    assert.equal(state.merman.message, message);
    assert.match(state.merman.detail ?? "", new RegExp(detail));
    assert.doesNotMatch(state.merman.message, /\[object Object\]/);
    assert.doesNotMatch(state.merman.detail ?? "", /\[object Object\]/);
    coordinator.dispose();
  }
});

test("normalizes an unprojected ASCII failure without failing the SVG result", async () => {
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });
  coordinator.setInput(
    input("ascii", {
      ...facade(),
      renderAscii() {
        return {
          ascii: null,
          error: {
            message: "Structured Merman ASCII failure.",
            reason: { code: "MERMAN_ASCII_ERROR" },
          },
          status: "failure",
        } as unknown as ReturnType<MermanDomainFacade["renderAscii"]>;
      },
    })
  );
  await waitFor(() => coordinator.store.getState().status === "success");

  const state = coordinator.store.getState();
  assert.equal(state.status, "success");
  if (state.status !== "success") return;
  assert.equal(state.merman.ascii, null);
  assert.equal(
    state.merman.asciiError?.summary,
    "Structured Merman ASCII failure."
  );
  assert.match(state.merman.asciiError?.detail ?? "", /MERMAN_ASCII_ERROR/);
  assert.doesNotMatch(state.merman.asciiError?.detail ?? "", /\[object Object\]/);
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
        version: 1,
        ok: false,
        code: 5,
        code_name: "MERMAN_PARSE_ERROR",
        message: "Source is invalid.",
      };
    },
  };
}

function rawFailureFacade(error: unknown): MermanDomainFacade {
  return {
    ...facade(),
    render() {
      return {
        artifact: null,
        error,
        renderTime: 0,
        stage: "render",
        status: "failure",
      } as unknown as ReturnType<MermanDomainFacade["render"]>;
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
      artifact: projectSafeInlineSvg(
        `<svg xmlns="http://www.w3.org/2000/svg"><text>${source}</text></svg>`
      ),
      error: null,
      renderTime: 2,
      status: "success",
    }),
    renderAscii: (source: string) => ({
      ascii: source,
      error: null,
      status: "success",
    }),
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
