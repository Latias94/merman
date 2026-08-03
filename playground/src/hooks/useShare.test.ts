import assert from "node:assert/strict";
import test from "node:test";

import {
  decodeShareHash,
  encodeShareHash,
  type ShareData,
} from "./useShare.ts";

test("round-trips only the independent presentation fields", () => {
  const data: ShareData = {
    code: "flowchart TD\nA --> B",
    theme: "forest",
    config: '{"look":"neo"}',
    diagramFont: "arial",
    presentationProfileId: "future-profile",
    presentationThemePresetId: "future-theme",
    svgPipeline: "readable",
    textMeasurementMode: "headless",
  };

  const hash = encodeShareHash(data);
  assert.deepEqual(decodeShareHash(hash), data);

  const raw = JSON.parse(decodeURIComponent(atob(hash))) as Record<string, unknown>;
  assert.equal("hostThemePreset" in raw, false);
});

test("migrates legacy host theme values without closing future IDs", () => {
  const cases = [
    ["editor-light", "editor-light", null, "resvg-safe"],
    ["merman-modern", null, "merman-modern", "parity"],
    ["none", null, null, "parity"],
    ["mermaid", null, null, "parity"],
    ["future-theme", "future-theme", null, "parity"],
  ] as const;

  for (const [legacy, themePreset, profile, pipeline] of cases) {
    assert.deepEqual(decodeShareHash(legacyHash(legacy)), {
      code: "flowchart TD\nA",
      theme: "default",
      presentationProfileId: profile,
      presentationThemePresetId: themePreset,
      svgPipeline: pipeline,
    });
  }
});

test("prefers new presentation fields over a conflicting legacy value", () => {
  assert.deepEqual(
    decodeShareHash(
      legacyHash("editor-light", {
        presentationProfileId: "future-profile",
        presentationThemePresetId: null,
        svgPipeline: "readable",
      }),
    ),
    {
      code: "flowchart TD\nA",
      theme: "default",
      presentationProfileId: "future-profile",
      presentationThemePresetId: null,
      svgPipeline: "readable",
    },
  );
});

function legacyHash(
  hostThemePreset: string,
  extra: Record<string, unknown> = {},
): string {
  return btoa(
    encodeURIComponent(
      JSON.stringify({
        code: "flowchart TD\nA",
        theme: "default",
        hostThemePreset,
        ...extra,
      }),
    ),
  );
}
