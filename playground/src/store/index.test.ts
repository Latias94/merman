import assert from "node:assert/strict";
import test from "node:test";

import { useAppStore } from "./index.ts";

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

function presentationState() {
  const state = useAppStore.getState();
  return {
    diagramTheme: state.diagramTheme,
    presentationProfileId: state.presentationProfileId,
    presentationThemePresetId: state.presentationThemePresetId,
    svgPipeline: state.svgPipeline,
  };
}
