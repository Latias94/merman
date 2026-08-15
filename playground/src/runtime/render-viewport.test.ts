import assert from "node:assert/strict";
import test from "node:test";

import {
  CANONICAL_RENDER_VIEWPORT,
  resolveRenderViewport,
} from "./render-viewport.ts";

test("resolves Canonical independently from host measurements", () => {
  assert.deepEqual(resolveRenderViewport("canonical", { width: 960, height: 540 }), {
    mode: "canonical",
    status: "canonical",
    viewport: CANONICAL_RENDER_VIEWPORT,
  });
});

test("uses an explicit measuring fallback until Host has a valid size", () => {
  for (const candidate of [
    null,
    { width: 0, height: 540 },
    { width: Number.NaN, height: 540 },
    { width: 960, height: Number.POSITIVE_INFINITY },
    { width: 5000, height: 5000 },
  ]) {
    assert.deepEqual(resolveRenderViewport("host", candidate), {
      mode: "host",
      status: "host-measuring",
      viewport: CANONICAL_RENDER_VIEWPORT,
    });
  }
});

test("normalizes a positive Host measurement into operation dimensions", () => {
  assert.deepEqual(
    resolveRenderViewport("host", { width: 959.6, height: 539.5 }),
    {
      mode: "host",
      status: "host",
      viewport: { width: 960, height: 540 },
    },
  );
});
