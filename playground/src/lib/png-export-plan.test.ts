import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_PNG_MAX_PIXELS,
  DEFAULT_PNG_MAX_SIDE,
  planPngRaster,
} from "./png-export-plan.ts";

test("keeps the requested scale when the raster fits the default budget", () => {
  const plan = planPngRaster(640, 360, 2);
  assert.deepEqual(plan, {
    sourceWidth: 640,
    sourceHeight: 360,
    requestedScale: 2,
    effectiveScale: 2,
    outputWidth: 1280,
    outputHeight: 720,
    downscaled: false,
  });
  assert.equal(Object.isFrozen(plan), true);
});

test("clones limits before calculation", () => {
  const limits = { maxSide: 1_024, maxPixels: 1_000_000 };
  const plan = planPngRaster(800, 600, 2, limits);
  limits.maxSide = 1;
  limits.maxPixels = 1;

  assert.deepEqual([plan.outputWidth, plan.outputHeight], [1_024, 768]);
  assert.equal(Object.isFrozen(plan), true);
});

test("a callback cannot mutate the published raster plan", () => {
  const plan = planPngRaster(640, 360, 2);
  const observer = (received: typeof plan) => {
    assert.equal(Reflect.set(received, "outputWidth", 1), false);
  };

  observer(plan);
  assert.equal(plan.outputWidth, 1_280);
});

test("proportionally downsamples a huge square before canvas allocation", () => {
  const plan = planPngRaster(100_000_000, 100_000_000, 2);

  assert.equal(plan.outputWidth, DEFAULT_PNG_MAX_SIDE);
  assert.equal(plan.outputHeight, DEFAULT_PNG_MAX_SIDE);
  assert.equal(plan.outputWidth * plan.outputHeight, DEFAULT_PNG_MAX_PIXELS);
  assert.equal(plan.downscaled, true);
  assert.ok(plan.effectiveScale < 0.001);
});

test("bounds long skinny diagrams without distorting their aspect ratio", () => {
  const plan = planPngRaster(100_000_000, 1_000, 2);

  assert.equal(plan.outputWidth, DEFAULT_PNG_MAX_SIDE);
  assert.equal(plan.outputHeight, 1);
  assert.ok(plan.outputWidth * plan.outputHeight <= DEFAULT_PNG_MAX_PIXELS);
});

test("rounds the base dimensions before applying the requested scale", () => {
  const plan = planPngRaster(342.36, 100.1, 2);

  assert.deepEqual([plan.outputWidth, plan.outputHeight], [686, 202]);
  assert.equal(plan.downscaled, false);
});

test("honors a tighter total-pixel budget independently of side length", () => {
  const plan = planPngRaster(4_000, 4_000, 2, {
    maxSide: 10_000,
    maxPixels: 4_000_000,
  });

  assert.deepEqual(
    [plan.outputWidth, plan.outputHeight],
    [2_000, 2_000]
  );
  assert.equal(plan.downscaled, true);
});

test("rejects invalid dimensions, scales, and budgets", () => {
  assert.throws(() => planPngRaster(0, 10), /SVG width/);
  assert.throws(() => planPngRaster(10, Number.POSITIVE_INFINITY), /SVG height/);
  assert.throws(() => planPngRaster(10, 10, Number.NaN), /PNG scale/);
  assert.throws(
    () => planPngRaster(10, 10, 1, { maxSide: 0, maxPixels: 10 }),
    /maximum side/
  );
});

test("shrinks again when final ceil rounding crosses the pixel budget", () => {
  const plan = planPngRaster(9, 9, 1, {
    maxSide: 10_000,
    maxPixels: 50,
  });

  assert.deepEqual([plan.outputWidth, plan.outputHeight], [7, 7]);
  assert.ok(plan.outputWidth * plan.outputHeight <= 50);
  assert.equal(plan.downscaled, true);
});
