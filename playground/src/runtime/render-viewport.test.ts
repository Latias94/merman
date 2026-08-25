import assert from "node:assert/strict";
import test from "node:test";

import {
  CANONICAL_RENDER_VIEWPORT,
  CANONICAL_SCREEN_AVAILABLE_WIDTH,
  captureRenderViewport,
} from "./render-viewport.ts";

test("captures one frozen canonical Playground render environment", () => {
  const captured = captureRenderViewport();

  assert.deepEqual(captured, {
    viewport: CANONICAL_RENDER_VIEWPORT,
    layoutEnvironment: {
      containerWidth: 800,
      containerHeight: 600,
      screenAvailableWidth: 800,
    },
  });
  assert.equal(CANONICAL_SCREEN_AVAILABLE_WIDTH, 800);
  assert.equal(Object.isFrozen(captured), true);
  assert.equal(Object.isFrozen(captured.viewport), true);
  assert.equal(Object.isFrozen(captured.layoutEnvironment), true);
  assert.equal("mode" in captured, false);
  assert.equal("status" in captured, false);
});

test("reuses the same canonical values for every capture", () => {
  assert.deepEqual(captureRenderViewport(), captureRenderViewport());
});
