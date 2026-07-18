import assert from "node:assert/strict";
import test from "node:test";

import {
  bindRenderArtifact,
  createDiagramDetectionKey,
  createPreviewRenderKey,
  freshRenderArtifactValue,
  isFreshRenderArtifact,
  type PreviewRenderInputs,
} from "./render-artifacts.ts";

const BASE_INPUTS: PreviewRenderInputs = {
  code: "flowchart TD\nA --> B",
  diagramTheme: "default",
  mermaidConfig: "{}",
  hostThemePreset: null,
  textMeasurementMode: "browser",
  diagramFont: "trebuchet",
  refreshNonce: 0,
};

test("preview render key is stable for identical inputs", () => {
  assert.equal(
    createPreviewRenderKey(BASE_INPUTS),
    createPreviewRenderKey({ ...BASE_INPUTS })
  );
});

test("preview render key changes for source and render-affecting inputs", () => {
  const baseKey = createPreviewRenderKey(BASE_INPUTS);
  const variants: PreviewRenderInputs[] = [
    { ...BASE_INPUTS, code: "flowchart TD\nA --> C" },
    { ...BASE_INPUTS, diagramTheme: "dark" },
    { ...BASE_INPUTS, mermaidConfig: '{"themeVariables":{"fontSize":"18px"}}' },
    { ...BASE_INPUTS, hostThemePreset: "github-dark" },
    { ...BASE_INPUTS, textMeasurementMode: "headless" },
    { ...BASE_INPUTS, diagramFont: "arial" },
    { ...BASE_INPUTS, refreshNonce: 1 },
  ];

  for (const variant of variants) {
    assert.notEqual(createPreviewRenderKey(variant), baseKey);
  }
});

test("diagram detection key tracks semantic inputs only", () => {
  const baseKey = createDiagramDetectionKey(BASE_INPUTS);
  for (const variant of [
    { ...BASE_INPUTS, code: "flowchart TD\nA --> C" },
    { ...BASE_INPUTS, diagramTheme: "dark" },
    { ...BASE_INPUTS, mermaidConfig: '{"layout":"elk"}' },
    { ...BASE_INPUTS, hostThemePreset: "github-dark" },
  ]) {
    assert.notEqual(createDiagramDetectionKey(variant), baseKey);
  }

  const nonSemanticVariant: PreviewRenderInputs = {
    ...BASE_INPUTS,
    textMeasurementMode: "headless",
    diagramFont: "arial",
    refreshNonce: 1,
  };
  assert.equal(createDiagramDetectionKey(nonSemanticVariant), baseKey);
});

test("freshness helpers hide stale artifact values", () => {
  const baseKey = createPreviewRenderKey(BASE_INPUTS);
  const nextKey = createPreviewRenderKey({
    ...BASE_INPUTS,
    code: "flowchart TD\nA --> C",
  });
  const artifact = bindRenderArtifact(baseKey, "<svg />");

  assert.equal(isFreshRenderArtifact(artifact, baseKey), true);
  assert.equal(freshRenderArtifactValue(artifact, baseKey), "<svg />");
  assert.equal(isFreshRenderArtifact(artifact, nextKey), false);
  assert.equal(freshRenderArtifactValue(artifact, nextKey), null);
});
