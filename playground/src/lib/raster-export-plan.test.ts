import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_RASTER_MAX_PIXELS,
  DEFAULT_RASTER_MAX_SIDE,
  planRasterExport,
  type RasterExportRequest,
} from "./raster-export-plan.ts";

const SOURCE = Object.freeze({
  width: 640,
  height: 360,
  originalBackground: Object.freeze({ color: "#ffffff", opaque: true }),
});

test("plans scale, width, height, and fit sizing with a locked aspect ratio", () => {
  const cases = [
    [{ mode: "scale", scale: 2 }, [1280, 720]],
    [{ mode: "width", width: 960 }, [960, 540]],
    [{ mode: "height", height: 540 }, [960, 540]],
    [{ mode: "fit", width: 1000, height: 300 }, [534, 300]],
  ] as const;

  for (const [sizing, dimensions] of cases) {
    const plan = planRasterExport(SOURCE, {
      format: "png",
      background: { mode: "original" },
      sizing,
    });
    assert.deepEqual([plan.outputWidth, plan.outputHeight], dimensions);
    assert.equal(plan.downscaled, false);
    assert.equal(Object.isFrozen(plan), true);
    assert.equal(Object.isFrozen(plan.sizing), true);
  }
});

test("uses native base rounding before applying raster sizing", () => {
  const plan = planRasterExport(
    { width: 342.36, height: 100.1, originalBackground: null },
    {
      format: "png",
      background: { mode: "transparent" },
      sizing: { mode: "scale", scale: 2 },
    },
  );

  assert.deepEqual([plan.outputWidth, plan.outputHeight], [686, 202]);
  assert.deepEqual([plan.requestedWidth, plan.requestedHeight], [686, 202]);
});

test("limits fractional scaled dimensions before rounding like native export", () => {
  const plan = planRasterExport(
    { width: 3219.26, height: 4096.1, originalBackground: null },
    {
      format: "png",
      background: { mode: "transparent" },
      sizing: { mode: "scale", scale: 1.1 },
    },
  );

  assert.deepEqual([plan.requestedWidth, plan.requestedHeight], [3543, 4507]);
  assert.deepEqual([plan.outputWidth, plan.outputHeight], [3220, 4096]);
  assert.equal(plan.downscaled, true);
});

test("bounds square and skinny outputs before allocation", () => {
  const square = planRasterExport(
    { width: 100_000_000, height: 100_000_000, originalBackground: null },
    pngScale(4),
  );
  assert.deepEqual(
    [square.outputWidth, square.outputHeight],
    [DEFAULT_RASTER_MAX_SIDE, DEFAULT_RASTER_MAX_SIDE],
  );
  assert.equal(
    square.outputWidth * square.outputHeight,
    DEFAULT_RASTER_MAX_PIXELS,
  );
  assert.equal(square.downscaled, true);

  const skinny = planRasterExport(
    { width: 100_000_000, height: 1_000, originalBackground: null },
    pngScale(4),
  );
  assert.deepEqual([skinny.outputWidth, skinny.outputHeight], [4096, 1]);
});

test("resolves PNG and JPEG background policies without implicit alpha conversion", () => {
  const transparent = planRasterExport(SOURCE, {
    format: "png",
    background: { mode: "transparent" },
    sizing: { mode: "scale", scale: 1 },
  });
  assert.deepEqual(transparent.background, {
    mode: "transparent",
    color: null,
    opaque: false,
  });

  const custom = planRasterExport(SOURCE, {
    format: "jpeg",
    background: { mode: "custom", color: "#12ABef" },
    quality: 90,
    sizing: { mode: "width", width: 800 },
  });
  assert.equal(custom.mimeType, "image/jpeg");
  assert.equal(custom.quality, 90);
  assert.deepEqual(custom.background, {
    mode: "custom",
    color: "#12abef",
    opaque: true,
  });

  const original = planRasterExport(SOURCE, {
    format: "jpeg",
    background: { mode: "original" },
    sizing: { mode: "scale", scale: 1 },
  });
  assert.equal(original.quality, 90);
  assert.equal(original.background.color, "#ffffff");
});

test("rejects ambiguous sizing, transparent JPEG, and invalid quality or color", () => {
  const invalid = [
    pngScale(0),
    {
      format: "png",
      background: { mode: "custom", color: "not-a-color" },
      sizing: { mode: "width", width: 100 },
    },
    {
      format: "jpeg",
      background: { mode: "transparent" },
      sizing: { mode: "scale", scale: 1 },
    },
    {
      format: "jpeg",
      background: { mode: "custom", color: "#ffffff" },
      quality: 90.5,
      sizing: { mode: "scale", scale: 1 },
    },
  ] as unknown as RasterExportRequest[];

  for (const request of invalid) {
    assert.throws(() => planRasterExport(SOURCE, request));
  }
  assert.throws(() =>
    planRasterExport(
      { ...SOURCE, originalBackground: null },
      {
        format: "jpeg",
        background: { mode: "original" },
        sizing: { mode: "scale", scale: 1 },
      },
    ),
  );
});

function pngScale(scale: number): RasterExportRequest {
  return {
    format: "png",
    background: { mode: "original" },
    sizing: { mode: "scale", scale },
  };
}
