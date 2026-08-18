import assert from "node:assert/strict";
import test from "node:test";
import { projectNavigableInlineSvg } from "./render-artifact.ts";

import type { MermanDomainFacade } from "./merman-core.ts";
import {
  freezeRenderOperation,
  type FreezeRenderOperationInput,
  type ConfiguredMermanOperationInput,
  type FrozenRenderOperation,
} from "./merman-operation-input.ts";
import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  type WorkspaceSnapshot,
} from "../lib/workspace-snapshot.ts";
import { MERMAID_JS_VERSION } from "./mermaid-requirements.ts";
import {
  captureRenderViewport,
  type CapturedRenderViewport,
} from "./render-viewport.ts";
import {
  createRenderCoordinator,
  type RenderCoordinatorInput,
} from "./render-coordinator.ts";
import { selectCurrentDiagramType } from "./use-render-coordinator.ts";
import type {
  MermaidRealmController,
  MermaidRealmRenderResult,
} from "./mermaid-realm-controller.ts";

test("freezes one canonical environment into Merman and Mermaid inputs", async () => {
  const compare = fakeCompare([Promise.resolve(mermaidSuccess("canonical"))]);
  const renderedOperations: FrozenRenderOperation[] = [];
  const domainFacade: MermanDomainFacade = {
    ...facade(),
    render(operation) {
      renderedOperations.push(operation as FrozenRenderOperation);
      return facade().render(operation);
    },
  };
  const coordinator = createRenderCoordinator({ compare, debounceMs: 0 });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });
  coordinator.setInput(
    input(
      "canonical",
      domainFacade,
      {},
      captureRenderViewport(),
    ),
  );

  await waitFor(() => coordinator.store.getState().status === "success");

  const renderedOperation = renderedOperations[0];
  assert.ok(renderedOperation);
  assert.equal("renderViewportMode" in renderedOperation, false);
  assert.equal("renderViewportStatus" in renderedOperation, false);
  assert.deepEqual(renderedOperation.layoutEnvironment, {
    containerWidth: 800,
    containerHeight: 600,
    screenAvailableWidth: 800,
  });
  assert.deepEqual(renderedOperation.viewport, { width: 800, height: 600 });
  assert.deepEqual(compare.calls[0]?.viewport, { width: 800, height: 600 });
  assert.equal(compare.calls[0]?.screenAvailableWidth, 800);
});

test("latest request publishes Merman and Mermaid as one coherent batch", async () => {
  const first = deferred<MermaidRealmRenderResult>();
  const second = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([first.promise, second.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });

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

test("freezes only the latest input after the debounce window", async () => {
  const frozenSources: string[] = [];
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    debounceMs: 10,
    freezeOperation(input: FreezeRenderOperationInput) {
      frozenSources.push(input.workspace.code);
      return freezeRenderOperation(input);
    },
  });

  coordinator.setInput(input("first"));
  coordinator.setInput(input("second"));
  coordinator.setInput(input("third"));

  assert.deepEqual(frozenSources, []);
  const pending = coordinator.store.getState();
  assert.equal(pending.status, "pending");
  await waitFor(() => coordinator.store.getState().status === "success");
  assert.deepEqual(frozenSources, ["third"]);
  const completed = coordinator.store.getState();
  assert.equal(completed.status, "success");
  if (completed.status !== "success") return;
  assert.equal(completed.snapshot.operation.source, "third");
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
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: true,
    compareEnabled: false,
    diagnosticsEnabled: true,
  });
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

test("external pane geometry cannot change operation identity or enqueue another render", async () => {
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
    compare: fakeCompare([]),
    debounceMs: 0,
  });

  coordinator.setInput(
    input(
      "layout-environment",
      domainFacade,
      {},
      captureRenderViewport(),
    ),
  );
  await waitFor(() => renderOperations.length === 1);
  assert.deepEqual(renderOperations[0].layoutEnvironment, {
    containerWidth: 800,
    containerHeight: 600,
    screenAvailableWidth: 800,
  });
  assert.equal(Object.isFrozen(renderOperations[0].layoutEnvironment), true);

  coordinator.setInput(
    input(
      "layout-environment",
      domainFacade,
      {},
      captureRenderViewport(),
    ),
  );
  await Promise.resolve();
  assert.equal(renderOperations.length, 1);
});

test("keeps a successful render when SVG plan collection fails", async () => {
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
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

test("renders ASCII only after the feature is activated", async () => {
  let asciiRenderCalls = 0;
  let svgRenderCalls = 0;
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    debounceMs: 0,
  });
  const domainFacade: MermanDomainFacade = {
    ...facade(),
    render(operation) {
      svgRenderCalls += 1;
      return facade().render(operation);
    },
    renderAscii(operation) {
      asciiRenderCalls += 1;
      return facade().renderAscii(operation);
    },
  };

  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: false,
    diagnosticsEnabled: false,
  });
  coordinator.setInput(input("svg-only", domainFacade));
  await waitFor(() => coordinator.store.getState().status === "success");
  assert.equal(asciiRenderCalls, 0);
  assert.equal(svgRenderCalls, 1);

  coordinator.setFeatures({
    asciiEnabled: true,
    compareEnabled: false,
    diagnosticsEnabled: false,
  });
  await waitFor(() => asciiRenderCalls === 1);
  const state = coordinator.store.getState();
  assert.equal(state.status, "success");
  if (state.status !== "success") return;
  assert.deepEqual(state.ascii, {
    artifact: "svg-only",
    status: "success",
  });
  assert.equal(svgRenderCalls, 2);

  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: false,
    diagnosticsEnabled: false,
  });
  assert.equal(coordinator.store.getState(), state);
  assert.equal(svgRenderCalls, 2);
});

test("publishes ASCII independently when SVG validation fails", async () => {
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: true,
    compareEnabled: false,
    diagnosticsEnabled: false,
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
      renderAscii: () => ({
        ascii: "ascii-unsafe",
        error: null,
        status: "success",
      }),
    })
  );
  await waitFor(() => coordinator.store.getState().status === "failed");

  const state = coordinator.store.getState();
  assert.equal(state.status, "failed");
  if (state.status !== "failed") return;
  assert.deepEqual(state.ascii, {
    artifact: "ascii-unsafe",
    status: "success",
  });
});

test("publishes an explicit unsupported ASCII result without invoking the renderer", async () => {
  let asciiRenderCalls = 0;
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: true,
    compareEnabled: false,
    diagnosticsEnabled: false,
  });
  coordinator.setInput(
    input("pie", {
      ...facade(),
      detectDiagram: () => ({
        status: "available",
        validity: "valid",
        diagramType: "pie",
        syntaxId: "pie",
        effectiveLayoutId: "builtin",
      }),
      renderAscii: (operation) => {
        asciiRenderCalls += 1;
        return facade().renderAscii(operation);
      },
    })
  );
  await waitFor(() => coordinator.store.getState().status === "success");

  const state = coordinator.store.getState();
  assert.equal(state.status, "success");
  if (state.status !== "success") return;
  assert.equal(asciiRenderCalls, 0);
  assert.deepEqual(state.ascii, {
    diagramType: "pie",
    status: "unsupported",
  });
});

test("contains ASCII capability failures without failing the SVG publication", async () => {
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: true,
    compareEnabled: false,
    diagnosticsEnabled: false,
  });
  coordinator.setInput(
    input("flowchart", {
      ...facade(),
      getAsciiSupportedDiagrams: () => {
        throw new Error("ASCII capability lookup failed.");
      },
    })
  );
  await waitFor(() => coordinator.store.getState().status === "success");

  const state = coordinator.store.getState();
  assert.equal(state.status, "success");
  if (state.status !== "success") return;
  assert.equal(state.merman.status, "success");
  assert.ok(state.ascii);
  assert.equal(state.ascii.status, "failure");
  if (state.ascii.status !== "failure") return;
  assert.equal(state.ascii.error.summary, "ASCII capability lookup failed.");
});

test("keeps the visible diagram type while a replacement render is updating", async () => {
  const compare = fakeCompare([Promise.resolve(mermaidSuccess("visible"))]);
  const coordinator = createRenderCoordinator({
    compare,
    debounceMs: 0,
  });
  coordinator.setInput(input("visible"));
  await waitFor(() => coordinator.store.getState().status === "success");

  const previous = coordinator.store.getState();
  assert.equal(previous.status, "success");
  if (previous.status !== "success") return;
  const updating = Object.freeze({
    status: "updating" as const,
    previous,
    snapshot: Object.freeze({
      ...previous.snapshot,
      publicationId: (previous.snapshot.publicationId + 1) as typeof previous.snapshot.publicationId,
    }),
  });

  assert.equal(selectCurrentDiagramType(updating), "flowchart");
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
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });

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
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });
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
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });
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
  assert.equal(afterMerman.ascii, initial.ascii);
  assert.equal(Object.isFrozen(afterMerman), true);
  assert.equal(Object.isFrozen(afterMerman.merman), true);

  coordinator.markPresented(publicationId, "mermaid", 20);
  const afterMermaid = coordinator.store.getState();
  assert.notEqual(afterMermaid, afterMerman);
  assert.equal(afterMerman.mermaid.presentedAt, null);
  assert.equal(afterMermaid.status, "success");
  if (afterMermaid.status !== "success" || !afterMermaid.mermaid) return;
  assert.equal(afterMermaid.mermaid.presentedAt, 20);
  assert.equal(afterMermaid.ascii, initial.ascii);
  assert.equal(Object.isFrozen(afterMermaid.mermaid), true);
});

test("a completed Mermaid failure replaces stale success without borrowing Merman error", async () => {
  const first = deferred<MermaidRealmRenderResult>();
  const second = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([first.promise, second.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });

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
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });
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
  });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });
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
  });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });
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

test("synchronous compare exceptions become protocol failures and later work still runs", async () => {
  let renderCalls = 0;
  const compare: MermaidRealmController = {
    dispose() {},
    reset() {},
    render(operation) {
      renderCalls += 1;
      if (renderCalls === 1) {
        throw new Error("synchronous compare failure");
      }
      return Promise.resolve(mermaidSuccess(operation.source));
    },
  };
  const coordinator = createRenderCoordinator({
    compare,
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });

  coordinator.setInput(input("throws"));
  await waitFor(() => coordinator.store.getState().status === "partial");
  const failed = coordinator.store.getState();
  assert.equal(failed.status, "partial");
  if (failed.status === "partial") {
    assert.equal(failed.mermaid.status, "failure");
    assert.equal(failed.mermaid.stage, "protocol");
    assert.equal(failed.mermaid.message, "synchronous compare failure");
  }

  coordinator.setInput(input("recovered"));
  await waitFor(() => coordinator.store.getState().status === "success");
  assert.equal(renderCalls, 2);
});

test("superseding Compare work is cancelled before publishing the latest SVG batch", async () => {
  const pending = deferred<MermaidRealmRenderResult>();
  const compare = fakeCompare([pending.promise]);
  const coordinator = createRenderCoordinator({
    compare,
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: true,
    diagnosticsEnabled: false,
  });
  coordinator.setInput(input("compare"));
  await waitFor(() => compare.calls.length === 1);

  coordinator.setFeatures({
    asciiEnabled: false,
    compareEnabled: false,
    diagnosticsEnabled: false,
  });
  await waitFor(() => coordinator.store.getState().status === "success");

  const state = coordinator.store.getState();
  assert.equal(state.status, "success");
  assert.equal(compare.resetCalls, 1);
  assert.equal(state.snapshot.operation.compareEnabled, false);
});

test("render failures retain binding details in the completed batch", async () => {
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
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
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: true,
    compareEnabled: false,
    diagnosticsEnabled: false,
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
  assert.ok(state.ascii);
  assert.equal(state.ascii.status, "failure");
  if (state.ascii.status !== "failure") return;
  assert.equal(
    state.ascii.error.summary,
    "Structured Merman ASCII failure."
  );
  assert.match(state.ascii.error.detail ?? "", /MERMAN_ASCII_ERROR/);
  assert.doesNotMatch(state.ascii.error.detail ?? "", /\[object Object\]/);
});

test("publishes invalid configuration as an ASCII failure before detection", async () => {
  let asciiCalls = 0;
  const coordinator = createRenderCoordinator({
    compare: fakeCompare([]),
    debounceMs: 0,
  });
  coordinator.setFeatures({
    asciiEnabled: true,
    compareEnabled: false,
    diagnosticsEnabled: false,
  });
  coordinator.setInput(
    input(
      "flowchart TD\nA --> B",
      {
        ...facade(),
        detectDiagram: () => ({
          status: "unavailable",
          validity: "unknown",
          diagramType: null,
          syntaxId: null,
          effectiveLayoutId: null,
        }),
        renderAscii(operation) {
          asciiCalls += 1;
          return facade().renderAscii(operation);
        },
      },
      { mermaidConfig: "{" }
    )
  );
  await waitFor(() => !/pending|updating/.test(coordinator.store.getState().status));

  const state = coordinator.store.getState();
  if (state.status === "empty" || state.status === "pending" || state.status === "updating") {
    assert.fail(`Expected a completed render, received ${state.status}.`);
  }
  assert.ok(state.ascii);
  assert.equal(state.ascii.status, "failure");
  if (state.ascii.status !== "failure") return;
  assert.match(state.ascii.error.summary, /JSON|configuration/i);
  assert.equal(asciiCalls, 0);
});

function input(
  source: string,
  domainFacade: MermanDomainFacade = facade(),
  workspace: Partial<WorkspaceSnapshot> = {},
  renderViewport: CapturedRenderViewport = captureRenderViewport(),
): RenderCoordinatorInput {
  return {
    facade: domainFacade,
    renderViewport,
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
  let activeCancel:
    | ((result: MermaidRealmRenderResult) => void)
    | null = null;
  const controller: MermaidRealmController & {
    calls: Parameters<MermaidRealmController["render"]>[0][];
    resetCalls: number;
  } = {
    calls: [],
    resetCalls: 0,
    dispose() {},
    reset() {
      this.resetCalls += 1;
      activeCancel?.({
        status: "failure",
        stage: "disposed",
        message: "Mermaid realm operation was reset.",
        detail: null,
      });
      activeCancel = null;
    },
    render(input) {
      this.calls.push(input);
      const result = results.shift();
      if (!result) throw new Error("Unexpected Compare render.");
      const cancellation = Promise.withResolvers<MermaidRealmRenderResult>();
      activeCancel = cancellation.resolve;
      return Promise.race([result, cancellation.promise]).finally(() => {
        if (activeCancel === cancellation.resolve) activeCancel = null;
      });
    },
  };
  return controller;
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
