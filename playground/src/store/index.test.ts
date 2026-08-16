import assert from "node:assert/strict";
import test from "node:test";

import {
  selectWorkspaceSnapshot,
  useAppStore,
  type WorkspaceSnapshot,
} from "./index.ts";
import type { StartupShareHydration } from "../lib/share-view.ts";

test("presentation setters update only their own axis", () => {
  useAppStore.setState({
    diagramTheme: "forest",
    presentationProfileId: null,
    presentationThemePresetId: null,
    svgPipeline: "parity",
  });

  useAppStore.getState().setPresentationThemePresetId("future-theme");
  assert.deepEqual(presentationState(), {
    diagramTheme: "forest",
    presentationProfileId: null,
    presentationThemePresetId: "future-theme",
    svgPipeline: "parity",
  });

  useAppStore.getState().setPresentationProfileId("future-profile");
  assert.deepEqual(presentationState(), {
    diagramTheme: "forest",
    presentationProfileId: "future-profile",
    presentationThemePresetId: "future-theme",
    svgPipeline: "parity",
  });

  useAppStore.getState().setSvgPipeline("readable");
  assert.deepEqual(presentationState(), {
    diagramTheme: "forest",
    presentationProfileId: "future-profile",
    presentationThemePresetId: "future-theme",
    svgPipeline: "readable",
  });

  useAppStore.getState().setDiagramTheme("dark");
  assert.deepEqual(presentationState(), {
    diagramTheme: "dark",
    presentationProfileId: "future-profile",
    presentationThemePresetId: "future-theme",
    svgPipeline: "readable",
  });
});

test("applies one complete workspace snapshot with one coherent notification", () => {
  const next: WorkspaceSnapshot = {
    code: "sequenceDiagram\nA->>B: hello",
    mermaidConfig: '{"look":"neo"}',
    diagramTheme: "forest",
    presentationProfileId: "future-profile",
    presentationThemePresetId: "future-theme",
    renderViewportMode: "host",
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

test("stores viewport intent but keeps measured Host pixels transient", () => {
  useAppStore.setState({
    hostRenderViewport: null,
    renderViewportMode: "canonical",
  });

  useAppStore.getState().setRenderViewportMode("host");
  useAppStore
    .getState()
    .setHostRenderViewport({ width: 959.6, height: 539.5 });
  assert.deepEqual(useAppStore.getState().hostRenderViewport, {
    width: 960,
    height: 540,
  });

  useAppStore
    .getState()
    .setHostRenderViewport({ width: 0, height: 0 });
  assert.deepEqual(useAppStore.getState().hostRenderViewport, {
    width: 960,
    height: 540,
  });
  assert.equal(selectWorkspaceSnapshot(useAppStore.getState()).renderViewportMode, "host");
  assert.equal("hostRenderViewport" in selectWorkspaceSnapshot(useAppStore.getState()), false);
});

test("applies startup workspace, view, lock, and warning in one store transition", () => {
  const hydration: StartupShareHydration = {
    workspace: {
      code: "flowchart TD\nA --> B",
      mermaidConfig: '{"look":"neo"}',
      diagramTheme: "forest",
      diagramFont: "arial",
      presentationProfileId: "future-profile",
      presentationThemePresetId: "future-theme",
      renderViewportMode: "host",
      svgPipeline: "readable",
      textMeasurementMode: "headless",
    },
    view: {
      workspacePane: "preview",
      editorMode: "config",
      previewMode: "compare",
      lockedEnvironment: {
        width: 640,
        height: 480,
        screenAvailableWidth: 1512,
      },
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
      sharedRenderEnvironmentLock: state.sharedRenderEnvironmentLock,
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
      sharedRenderEnvironmentLock: hydration.view.lockedEnvironment,
      shareViewWarning: hydration.warning,
    },
  ]);
});

function presentationState() {
  const state = useAppStore.getState();
  return {
    diagramTheme: state.diagramTheme,
    presentationProfileId: state.presentationProfileId,
    presentationThemePresetId: state.presentationThemePresetId,
    svgPipeline: state.svgPipeline,
  };
}
