import assert from "node:assert/strict";
import test from "node:test";

import {
  selectWorkspaceSnapshot,
  useAppStore,
  type WorkspaceSnapshot,
} from "./index.ts";

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

function renderingState() {
  const state = useAppStore.getState();
  return {
    diagramTheme: state.diagramTheme,
    themePresetId: state.themePresetId,
    svgPipeline: state.svgPipeline,
  };
}
