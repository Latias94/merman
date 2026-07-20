export const DEFAULT_PNG_MAX_SIDE = 8192;
export const DEFAULT_PNG_MAX_PIXELS = 8192 * 8192;

export interface PngRasterLimits {
  maxSide: number;
  maxPixels: number;
}

export interface PngRasterPlan {
  sourceWidth: number;
  sourceHeight: number;
  requestedScale: number;
  effectiveScale: number;
  outputWidth: number;
  outputHeight: number;
  downscaled: boolean;
}

const DEFAULT_LIMITS: PngRasterLimits = {
  maxSide: DEFAULT_PNG_MAX_SIDE,
  maxPixels: DEFAULT_PNG_MAX_PIXELS,
};

/**
 * Browser mirror of `raster_plan_for_geometry` in Merman's Rust raster planner.
 * Keep base-size rounding, final-size rounding, and the eight-pass pixel-limit
 * correction in the same order so Playground exports match native bindings.
 */
export function planPngRaster(
  sourceWidth: number,
  sourceHeight: number,
  requestedScale: number = 2,
  limits: PngRasterLimits = DEFAULT_LIMITS
): PngRasterPlan {
  requirePositiveFinite(sourceWidth, "SVG width");
  requirePositiveFinite(sourceHeight, "SVG height");
  requirePositiveFinite(requestedScale, "PNG scale");
  requirePositiveInteger(limits.maxSide, "PNG maximum side");
  requirePositiveInteger(limits.maxPixels, "PNG maximum pixel count");

  const baseWidth = Math.max(1, Math.ceil(sourceWidth));
  const baseHeight = Math.max(1, Math.ceil(sourceHeight));
  const requestedWidth = rasterDimension(baseWidth * requestedScale);
  const requestedHeight = rasterDimension(baseHeight * requestedScale);

  let limitScale = Math.min(
    1,
    limits.maxSide / (baseWidth * requestedScale),
    limits.maxSide / (baseHeight * requestedScale)
  );
  const limitedPixels =
    baseWidth *
    requestedScale *
    baseHeight *
    requestedScale *
    limitScale *
    limitScale;
  if (limitedPixels > limits.maxPixels) {
    limitScale *= Math.sqrt(limits.maxPixels / limitedPixels);
  }

  let effectiveScale = requestedScale * Math.min(1, Math.max(0, limitScale));
  requirePositiveFinite(effectiveScale, "effective PNG scale");
  let [outputWidth, outputHeight] = limitedDimensions(
    baseWidth,
    baseHeight,
    effectiveScale,
    limits.maxSide
  );

  for (let attempt = 0; attempt < 8; attempt += 1) {
    const outputPixels = outputWidth * outputHeight;
    if (outputPixels <= limits.maxPixels) break;

    const shrink =
      Math.sqrt(limits.maxPixels / outputPixels) * 0.999_999;
    effectiveScale *= shrink;
    [outputWidth, outputHeight] = limitedDimensions(
      baseWidth,
      baseHeight,
      effectiveScale,
      limits.maxSide
    );
  }

  return {
    sourceWidth,
    sourceHeight,
    requestedScale,
    effectiveScale,
    outputWidth,
    outputHeight,
    downscaled:
      outputWidth !== requestedWidth || outputHeight !== requestedHeight,
  };
}

function limitedDimensions(
  baseWidth: number,
  baseHeight: number,
  scale: number,
  maxSide: number
): [number, number] {
  return [
    Math.min(maxSide, rasterDimension(baseWidth * scale)),
    Math.min(maxSide, rasterDimension(baseHeight * scale)),
  ];
}

function rasterDimension(value: number): number {
  requirePositiveFinite(value, "computed PNG dimension");
  return Math.max(1, Math.ceil(value));
}

function requirePositiveFinite(value: number, label: string): void {
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`${label} must be a positive finite number`);
  }
}

function requirePositiveInteger(value: number, label: string): void {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`${label} must be a positive safe integer`);
  }
}
