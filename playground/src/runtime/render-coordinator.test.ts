import assert from "node:assert/strict";
import test from "node:test";
import { projectNavigableInlineSvg } from "./render-artifact.ts";

import type { MermanDomainFacade } from "./merman-core.ts";
import type {
  ConfiguredMermanOperationInput,
  FrozenRenderOperation,
} from "./merman-operation-input.ts";
import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  type WorkspaceSnapshot,
} from "../lib/workspace-snapshot.ts";
import { MERMAID_JS_VERSION } from "./mermaid-requirements.ts";
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
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });
  coordinator.setFeatures({ compareEnabled: true, diagnosticsEnabled: false });

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
  assert.equal(state.mermaid.artifact.kind, "navigable-inline-svg");
  if (state.mermaid.artifact.kind === "navigable-inline-svg") {
    assert.match(state.mermaid.artifact.svg, /second/);
  }
  assert.equal(state.snapshot.operation.source, "second");
});

test("request identity includes all presentation axes and stores the same-snapshot SVG plan", async () => {
  const planCalls: {
    operation: FrozenRenderOperation;
    result: ReturnType<MermanDomainFacade["svgPlan"]>;
  }[] = [];
  const domainFacade: MermanDomainFacade = {
    ...facade(),
    svgPlan(input) {
      const operation = input as FrozenRenderOperation;
      const result = svgPlan({
        diagramType: operation.source,
        profileId: operation.presentationProfileId,
      });
      planCalls.push({ operation, result });
      return result;
    },
  };
  const workspaceCases: Array<Partial<WorkspaceSnapshot>> = [
    {
      diagramFont: "trebuchet",
      presentationProfileId: null,
      presentationThemePresetId: null,
      svgPipeline: "parity",
      textMeasurementMode: "browser",
    },
    {
      diagramFont: "trebuchet",
      presentationProfileId: null,
      presentationThemePresetId: "future-theme",
      svgPipeline: "parity",
      textMeasurementMode: "browser",
    },
    {
      diagramFont: "trebuchet",
      presentationProfileId: "future-profile",
      presentationThemePresetId: "future-theme",
      svgPipeline: "parity",
      textMeasurementMode: "browser",
    },
    {
      diagramFont: "trebuchet",
      presentationProfileId: "future-profile",
      presentationThemePresetId: "future-theme",
      svgPipeline: "readable",
      textMeasurementMode: "browser",
    },
  ];
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });

  for (const workspace of workspaceCases) {
    const previousPlanCallCount = planCalls.length;
    coordinator.setInput(input("identity", domainFacade, workspace));
    await waitFor(() => coordinator.store.getState().status === "success");
    const state = coordinator.store.getState();
    assert.equal(state.status, "success");
    if (state.status !== "success") continue;
    assert.equal(Object.isFrozen(state.snapshot.operation), true);
    assert.equal(Object.isFrozen(state.snapshot.operation.bindingOptions), true);

    if (!workspace.presentationProfileId) {
      assert.equal(planCalls.length, previousPlanCallCount);
      assert.equal(state.svgPlan, null);
      continue;
    }

    assert.equal(planCalls.length, previousPlanCallCount + 1);
    const planCall = planCalls.at(-1);
    assert.ok(planCall);
    assert.equal(planCall.operation, state.snapshot.operation);
    assert.deepEqual(state.svgPlan, planCall.result);
    assert.equal(Object.isFrozen(state.svgPlan), true);
    assert.equal(Object.isFrozen(state.svgPlan?.presentation_aspects), true);
    planCall.result.required_capability_ids.push("late-capability");
    assert.deepEqual(state.svgPlan?.required_capability_ids, []);
  }

  assert.equal(planCalls.length, 2);
});

test("deduplicates only when both the operation and facade authority are unchanged", async () => {
  let firstRenderCalls = 0;
  const firstFacade: MermanDomainFacade = {
    ...facade("same-version"),
    render(input) {
      firstRenderCalls += 1;
      return facade("same-version").render(input);
    },
  };
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });

  coordinator.setInput(input("stable", firstFacade));
  await waitFor(() => firstRenderCalls === 1);
  coordinator.setInput(input("stable", firstFacade));
  await Promise.resolve();
  assert.equal(firstRenderCalls, 1);

  let replacementRenderCalls = 0;
  const replacementFacade: MermanDomainFacade = {
    ...facade("same-version"),
    render(input) {
      replacementRenderCalls += 1;
      return facade("same-version").render(input);
    },
  };
  coordinator.setInput(input("stable", replacementFacade));
  await waitFor(() => replacementRenderCalls === 1);
  assert.equal(firstRenderCalls, 1);
});

test("passes one frozen operation to every Merman projection", async () => {
  const operations: FrozenRenderOperation[] = [];
  const capture = (input: ConfiguredMermanOperationInput) => {
    operations.push(input as FrozenRenderOperation);
    return input as FrozenRenderOperation;
  };
  const domainFacade: MermanDomainFacade = {
    ...facade(),
    detectDiagram(input) {
      capture(input);
      return facade().detectDiagram(input);
    },
    svgPlan(input) {
      const operation = capture(input);
      return svgPlan({ profileId: operation.presentationProfileId });
    },
    render(input) {
      const operation = capture(input);
      return {
        artifact: projectNavigableInlineSvg(
          `<svg xmlns="http://www.w3.org/2000/svg"><text>${operation.source}</text></svg>`
        ),
        error: null,
        renderTime: 1,
        status: "success",
      };
    },
    renderAscii(input) {
      capture(input);
      return { ascii: "ascii", error: null, status: "success" };
    },
    parseJson(input) {
      capture(input);
      return "{}";
    },
    layoutJson(input) {
      capture(input);
      return "{}";
    },
  };
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });
  coordinator.setFeatures({ compareEnabled: false, diagnosticsEnabled: true });
  coordinator.setInput(
    input("one-operation", domainFacade, {
      presentationProfileId: "future-profile",
    })
  );
  await waitFor(() => coordinator.store.getState().status === "success");

  const state = coordinator.store.getState();
  assert.equal(state.status, "success");
  if (state.status !== "success") return;
  assert.equal(operations.length, 6);
  assert.ok(operations.every((operation) => operation === state.snapshot.operation));
  assert.equal(Object.isFrozen(state), true);
  assert.equal(Object.isFrozen(state.detection), true);
  assert.equal(Object.isFrozen(state.diagnostics), true);
  assert.equal(Object.isFrozen(state.merman), true);
  assert.equal(Reflect.set(state, "publishedAt", 99), false);
});

test("freezes browser layout geometry into each render snapshot", async () => {
  let screenAvailableWidth = 1280;
  const renderOperations: FrozenRenderOperation[] = [];
  const domainFacade: MermanDomainFacade = {
    ...facade(),
    render(input) {
      const operation = input as FrozenRenderOperation;
      renderOperations.push(operation);
      return {
        artifact: projectNavigableInlineSvg(
          `<svg xmlns="http://www.w3.org/2000/svg"><text>${operation.source}</text></svg>`
        ),
        error: null,
        renderTime: 2,
        status: "success",
      };
    },
  };
  const coordinator = createRenderCoordinator({
    captureLayoutEnvironment: () => ({
      containerWidth: VIEWPORT.width,
      containerHeight: VIEWPORT.height,
      screenAvailableWidth,
    }),
    compare: fakeCompare([]),
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });

  coordinator.setInput(input("layout-environment", domainFacade));
  await waitFor(() => renderOperations.length === 1);
  assert.deepEqual(renderOperations[0].layoutEnvironment, {
    containerWidth: 800,
    containerHeight: 600,
    screenAvailableWidth: 1280,
  });
  assert.equal(Object.isFrozen(renderOperations[0].layoutEnvironment), true);

  screenAvailableWidth = 1440;
  coordinator.setInput(input("layout-environment", domainFacade));
  await waitFor(() => renderOperations.length === 2);
  assert.equal(renderOperations[1].layoutEnvironment.screenAvailableWidth, 1440);
});

test("keeps a successful render when SVG plan collection fails", async () => {
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });
  coordinator.setInput(
    input("plan-failure", {
      ...facade(),
      svgPlan() {
        throw new Error("SVG plan unavailable.");
      },
    }, {
      presentationProfileId: "future-profile",
    })
  );
  await waitFor(() => coordinator.store.getState().status === "success");

  const state = coordinator.store.getState();
  assert.equal(state.status, "success");
  if (state.status !== "success") return;
  assert.equal(state.svgPlan, null);
});

test("publishes the producer-owned Merman artifact without reprojecting it", async () => {
  const artifact = projectNavigableInlineSvg(
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
  const artifact = projectNavigableInlineSvg(
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
  coordinator.setFeatures({ compareEnabled: true, diagnosticsEnabled: false });

  coordinator.setInput(input("stable"));
  await waitFor(() => compare.calls.length === 1);
  first.resolve(mermaidSuccess("stable"));
  await waitFor(() => coordinator.store.getState().status === "success");

  coordinator.setInput(input("partial"));
  await waitFor(() => compare.calls.length === 2);
  const updating = coordinator.store.getState();
  assert.equal(updating.status, "updating");
  if (updating.status === "updating") {
    const previousMermaid = updating.previous.mermaid;
    assert.equal(previousMermaid?.status, "success");
    assert.match(
      previousMermaid?.status === "success" &&
        previousMermaid.artifact.kind === "navigable-inline-svg"
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

test("treats a Mermaid realm version mismatch as a protocol failure", async () => {
  const realmResult = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([realmResult.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });
  coordinator.setFeatures({ compareEnabled: true, diagnosticsEnabled: false });
  coordinator.setInput(input("version-mismatch"));
  await waitFor(() => compare.calls.length === 1);
  realmResult.resolve({
    ...mermaidSuccess("version-mismatch"),
    version: "0.0.0",
  });
  await waitFor(() => coordinator.store.getState().status === "partial");

  const state = coordinator.store.getState();
  assert.equal(state.status, "partial");
  if (state.status !== "partial" || state.mermaid.status !== "failure") return;
  assert.equal(state.mermaid.stage, "protocol");
  assert.match(state.mermaid.message, new RegExp(MERMAID_JS_VERSION));
  assert.match(state.mermaid.message, /0\.0\.0/);
});

test("marks each presented engine by rebuilding an immutable completed publication", async () => {
  const realmResult = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([realmResult.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    compareViewport: VIEWPORT,
    debounceMs: 0,
  });
  coordinator.setFeatures({ compareEnabled: true, diagnosticsEnabled: false });
  coordinator.setInput(input("presentation"));
  await waitFor(() => compare.calls.length === 1);
  realmResult.resolve(mermaidSuccess("presentation"));
  await waitFor(() => coordinator.store.getState().status === "success");

  const initial = coordinator.store.getState();
  assert.equal(initial.status, "success");
  if (initial.status !== "success" || !initial.mermaid) return;
  const publicationId = initial.snapshot.publicationId;

  coordinator.markPresented(publicationId, "merman", 10);
  const afterMerman = coordinator.store.getState();
  assert.notEqual(afterMerman, initial);
  assert.equal(initial.merman.presentedAt, null);
  assert.equal(afterMerman.status, "success");
  if (afterMerman.status !== "success" || !afterMerman.mermaid) return;
  assert.equal(afterMerman.merman.presentedAt, 10);
  assert.equal(Object.isFrozen(afterMerman), true);
  assert.equal(Object.isFrozen(afterMerman.merman), true);

  coordinator.markPresented(publicationId, "mermaid", 20);
  const afterMermaid = coordinator.store.getState();
  assert.notEqual(afterMermaid, afterMerman);
  assert.equal(afterMerman.mermaid.presentedAt, null);
  assert.equal(afterMermaid.status, "success");
  if (afterMermaid.status !== "success" || !afterMermaid.mermaid) return;
  assert.equal(afterMermaid.mermaid.presentedAt, 20);
  assert.equal(Object.isFrozen(afterMermaid.mermaid), true);
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
  coordinator.setFeatures({ compareEnabled: true, diagnosticsEnabled: false });

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
  assert.equal(completed.snapshot.operation.source, "invalid");
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
  coordinator.setFeatures({ compareEnabled: true, diagnosticsEnabled: false });
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
  coordinator.setFeatures({ compareEnabled: true, diagnosticsEnabled: false });
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
  coordinator.setFeatures({ compareEnabled: true, diagnosticsEnabled: false });
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
  domainFacade: MermanDomainFacade = facade(),
  workspace: Partial<WorkspaceSnapshot> = {}
): RenderCoordinatorInput {
  return {
    facade: domainFacade,
    workspace: {
      ...DEFAULT_WORKSPACE_SNAPSHOT,
      code: source,
      ...workspace,
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

function facade(packageVersion = "test-merman"): MermanDomainFacade {
  return {
    packageVersion,
    presentationCatalog: () => ({
      schema_version: 1,
      theme_presets: [],
      profiles: [],
    }),
    detectDiagram: () => ({
      status: "available",
      validity: "valid",
      diagramType: "flowchart",
      syntaxId: "flowchart-v2",
      effectiveLayoutId: "dagre",
    }),
    getAsciiSupportedDiagrams: () => ["flowchart"],
    render: (input: ConfiguredMermanOperationInput) => ({
      artifact: projectNavigableInlineSvg(
        `<svg xmlns="http://www.w3.org/2000/svg"><text>${input.source}</text></svg>`
      ),
      error: null,
      renderTime: 2,
      status: "success",
    }),
    renderAscii: (input: ConfiguredMermanOperationInput) => ({
      ascii: input.source,
      error: null,
      status: "success",
    }),
    svgPlan: (input: ConfiguredMermanOperationInput) =>
      svgPlan({
        diagramType: input.source,
        profileId:
          (input as FrozenRenderOperation).presentationProfileId ?? null,
      }),
  } as unknown as MermanDomainFacade;
}

function svgPlan({
  diagramType = "flowchart-v2",
  profileId = null,
}: {
  diagramType?: string;
  profileId?: string | null;
} = {}): ReturnType<MermanDomainFacade["svgPlan"]> {
  return {
    schema_version: 1,
    planned_operation_id: "svg",
    diagram_type: diagramType,
    presentation_profile_id: profileId,
    presentation_aspects: [],
    required_capability_ids: [],
    missing_capability_ids: [],
    ready: true,
  };
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

function mermaidSuccess(
  label: string
): Extract<MermaidRealmRenderResult, { status: "success" }> {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg"><text>${label}</text></svg>`;
  return {
    status: "success",
    artifact: projectNavigableInlineSvg(svg),
    prepareTimeMs: 1,
    renderTimeMs: 2,
    presentationTimeMs: 3,
    version: MERMAID_JS_VERSION,
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
