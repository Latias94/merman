import assert from "node:assert/strict";
import test from "node:test";

import {
  selectWorkspaceSnapshot,
  useAppStore,
  type WorkspaceSnapshot,
} from "./index.ts";
import type { StartupShareHydration } from "../lib/share-view.ts";

test("diagram rendering setters update only their own axis", () => {
  useAppStore.setState({
    diagramTheme: "forest",
    themePresetId: null,
    svgPipeline: "parity",
  });

  useAppStore.getState().setThemePresetId("future-theme");
  assert.deepEqual(renderingState(), {
    diagramTheme: "forest",
    themePresetId: "future-theme",
    svgPipeline: "parity",
  });

  useAppStore.getState().setSvgPipeline("readable");
  assert.deepEqual(renderingState(), {
    diagramTheme: "forest",
    themePresetId: "future-theme",
    svgPipeline: "readable",
  });

  useAppStore.getState().setDiagramTheme("dark");
  assert.deepEqual(renderingState(), {
    diagramTheme: "dark",
    themePresetId: "future-theme",
    svgPipeline: "readable",
  });
});

test("applies one complete workspace snapshot with one coherent notification", () => {
  const next: WorkspaceSnapshot = {
    code: "sequenceDiagram\nA->>B: hello",
    mermaidConfig: '{"look":"neo"}',
    diagramTheme: "forest",
    themePresetId: "future-theme",
    themeRecipeJson: null,
    svgPipeline: "readable",
    textMeasurementMode: "headless",
    diagramFont: "arial",
  };
  const notifications: WorkspaceSnapshot[] = [];
  const unsubscribe = useAppStore.subscribe((state) => {
    notifications.push(selectWorkspaceSnapshot(state));
  });

  useAppStore.getState().applyWorkspaceSnapshot(next);
  unsubscribe();

  assert.deepEqual(notifications, [next]);
  assert.deepEqual(selectWorkspaceSnapshot(useAppStore.getState()), next);
});

test("stores Preview-owned SVG presentation preferences outside workspace snapshots", () => {
  useAppStore.setState({
    showSvgBounds: false,
    svgPresentationMode: "infinite",
  });

  useAppStore.getState().setShowSvgBounds(true);
  useAppStore.getState().setSvgPresentationMode("viewbox");

  assert.equal(useAppStore.getState().showSvgBounds, true);
  assert.equal(useAppStore.getState().svgPresentationMode, "viewbox");
  assert.equal("showSvgBounds" in selectWorkspaceSnapshot(useAppStore.getState()), false);
  assert.equal(
    "svgPresentationMode" in selectWorkspaceSnapshot(useAppStore.getState()),
    false,
  );
});

test("applies startup workspace, view preferences, and warning in one store transition", () => {
  const hydration: StartupShareHydration = {
    workspace: {
      code: "flowchart TD\nA --> B",
      mermaidConfig: '{"look":"neo"}',
      diagramTheme: "forest",
      diagramFont: "arial",
      themePresetId: "future-theme",
      themeRecipeJson: null,
      svgPipeline: "readable",
      textMeasurementMode: "headless",
    },
    view: {
      workspacePane: "preview",
      editorMode: "config",
      previewMode: "compare",
      showSvgBounds: true,
      svgPresentationMode: "viewbox",
    },
    warning: {
      code: "share-view-not-restored",
      message: "warning",
    },
  };
  const notifications: Array<Record<string, unknown>> = [];
  const unsubscribe = useAppStore.subscribe((state) => {
    notifications.push({
      workspace: selectWorkspaceSnapshot(state),
      workspacePane: state.workspacePane,
      editorMode: state.editorMode,
      previewMode: state.previewMode,
      showSvgBounds: state.showSvgBounds,
      svgPresentationMode: state.svgPresentationMode,
      shareViewWarning: state.shareViewWarning,
    });
  });

  useAppStore.getState().applyStartupShareHydration(hydration);
  unsubscribe();

  assert.deepEqual(notifications, [
    {
      workspace: hydration.workspace,
      workspacePane: hydration.view.workspacePane,
      editorMode: hydration.view.editorMode,
      previewMode: hydration.view.previewMode,
      showSvgBounds: hydration.view.showSvgBounds,
      svgPresentationMode: hydration.view.svgPresentationMode,
      shareViewWarning: hydration.warning,
    },
  ]);
});

function renderingState() {
  const state = useAppStore.getState();
  return {
    diagramTheme: state.diagramTheme,
    themePresetId: state.themePresetId,
    svgPipeline: state.svgPipeline,
  };
}

test("preset and custom recipe setters replace one another atomically", () => {
  useAppStore.getState().setThemePresetId("brutalist");
  const recipe = '{"schema_version":1,"kind":"definition","definition":{}}';
  const notifications: Array<[string | null, string | null]> = [];
  const unsubscribe = useAppStore.subscribe((state) => {
    notifications.push([state.themePresetId, state.themeRecipeJson]);
  });

  useAppStore.getState().setThemeRecipeJson(recipe);
  assert.equal(
    selectWorkspaceSnapshot(useAppStore.getState()).themeRecipeJson,
    recipe,
  );
  useAppStore.getState().setThemePresetId("spotless");
  useAppStore.getState().setThemeRecipeJson(null);
  useAppStore.getState().setThemeRecipeJson(recipe);
  useAppStore.getState().setThemePresetId(null);
  unsubscribe();

  assert.deepEqual(notifications, [
    [null, recipe],
    ["spotless", null],
    ["spotless", null],
    [null, recipe],
    [null, null],
  ]);
});

test("applying and selecting a custom workspace retains its recipe", () => {
  const snapshot: WorkspaceSnapshot = {
    ...selectWorkspaceSnapshot(useAppStore.getState()),
    themePresetId: null,
    themeRecipeJson: '{"schema_version":1,"kind":"complete_spec","complete_spec":{}}',
  };
  useAppStore.getState().applyWorkspaceSnapshot(snapshot);
  assert.deepEqual(selectWorkspaceSnapshot(useAppStore.getState()), snapshot);
});
