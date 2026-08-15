export const DEFAULT_RASTER_MAX_SIDE = 4096;
export const DEFAULT_RASTER_MAX_PIXELS = 16_777_216;

export interface RasterExportSource {
  readonly width: number;
  readonly height: number;
  readonly originalBackground: RasterSourceBackground | null;
}

export interface RasterSourceBackground {
  readonly color: string;
  readonly opaque: boolean;
}

export type RasterSizing =
  | { readonly mode: "scale"; readonly scale: number }
  | { readonly mode: "width"; readonly width: number }
  | { readonly mode: "height"; readonly height: number }
  | {
      readonly mode: "fit";
      readonly width: number;
      readonly height: number;
    };

export type PngBackground =
  | { readonly mode: "original" }
  | { readonly mode: "transparent" }
  | { readonly mode: "custom"; readonly color: string };

export type JpegBackground = Exclude<PngBackground, { readonly mode: "transparent" }>;

export type RasterExportRequest =
  | {
      readonly format: "png";
      readonly background: PngBackground;
      readonly sizing: RasterSizing;
    }
  | {
      readonly format: "jpeg";
      readonly background: JpegBackground;
      readonly quality?: number;
      readonly sizing: RasterSizing;
    };

export interface RasterExportLimits {
  readonly maxSide: number;
  readonly maxPixels: number;
}

export interface PlannedRasterBackground {
  readonly mode: PngBackground["mode"];
  readonly color: string | null;
  readonly opaque: boolean;
}

export interface RasterExportPlan {
  readonly format: RasterExportRequest["format"];
  readonly mimeType: "image/png" | "image/jpeg";
  readonly extension: "png" | "jpg";
  readonly sourceWidth: number;
  readonly sourceHeight: number;
  readonly sizing: Readonly<RasterSizing>;
  readonly requestedScale: number;
  readonly effectiveScale: number;
  readonly requestedWidth: number;
  readonly requestedHeight: number;
  readonly outputWidth: number;
  readonly outputHeight: number;
  readonly downscaled: boolean;
  readonly background: Readonly<PlannedRasterBackground>;
  readonly quality: number | null;
}

const DEFAULT_LIMITS: Readonly<RasterExportLimits> = Object.freeze({
  maxSide: DEFAULT_RASTER_MAX_SIDE,
  maxPixels: DEFAULT_RASTER_MAX_PIXELS,
});

export function planRasterExport(
  source: Readonly<RasterExportSource>,
  request: Readonly<RasterExportRequest>,
  limits: Readonly<RasterExportLimits> = DEFAULT_LIMITS,
): RasterExportPlan {
  requirePositiveFinite(source.width, "SVG width");
  requirePositiveFinite(source.height, "SVG height");
  requirePositiveInteger(limits.maxSide, "raster maximum side");
  requirePositiveInteger(limits.maxPixels, "raster maximum pixel count");

  const baseWidth = Math.max(1, Math.ceil(source.width));
  const baseHeight = Math.max(1, Math.ceil(source.height));
  const sizing = freezeSizing(request.sizing);
  const requestedScale = requestedScaleForSizing(
    baseWidth,
    baseHeight,
    sizing,
  );
  requirePositiveFinite(requestedScale, "raster scale");

  const scaledWidth = baseWidth * requestedScale;
  const scaledHeight = baseHeight * requestedScale;
  const requestedWidth = rasterDimension(scaledWidth);
  const requestedHeight = rasterDimension(scaledHeight);
  let limitScale = Math.min(
    1,
    limits.maxSide / scaledWidth,
    limits.maxSide / scaledHeight,
  );
  const limitedPixels = scaledWidth * scaledHeight * limitScale * limitScale;
  if (limitedPixels > limits.maxPixels) {
    limitScale *= Math.sqrt(limits.maxPixels / limitedPixels);
  }

  let effectiveScale = requestedScale * Math.min(1, Math.max(0, limitScale));
  requirePositiveFinite(effectiveScale, "effective raster scale");
  let [outputWidth, outputHeight] = limitedDimensions(
    baseWidth,
    baseHeight,
    effectiveScale,
    limits.maxSide,
  );

  for (let attempt = 0; attempt < 8; attempt += 1) {
    const outputPixels = outputWidth * outputHeight;
    if (outputPixels <= limits.maxPixels) break;
    effectiveScale *=
      Math.sqrt(limits.maxPixels / outputPixels) * 0.999_999;
    [outputWidth, outputHeight] = limitedDimensions(
      baseWidth,
      baseHeight,
      effectiveScale,
      limits.maxSide,
    );
  }

  return Object.freeze({
    format: request.format,
    mimeType: request.format === "png" ? "image/png" : "image/jpeg",
    extension: request.format === "png" ? "png" : "jpg",
    sourceWidth: source.width,
    sourceHeight: source.height,
    sizing,
    requestedScale,
    effectiveScale,
    requestedWidth,
    requestedHeight,
    outputWidth,
    outputHeight,
    downscaled:
      outputWidth !== requestedWidth || outputHeight !== requestedHeight,
    background: resolveBackground(source, request),
    quality: resolveQuality(request),
  });
}

function freezeSizing(sizing: RasterSizing): Readonly<RasterSizing> {
  switch (sizing.mode) {
    case "scale":
      requirePositiveFinite(sizing.scale, "raster scale");
      return Object.freeze({ ...sizing });
    case "width":
      requirePositiveInteger(sizing.width, "raster width");
      return Object.freeze({ ...sizing });
    case "height":
      requirePositiveInteger(sizing.height, "raster height");
      return Object.freeze({ ...sizing });
    case "fit":
      requirePositiveInteger(sizing.width, "raster fit width");
      requirePositiveInteger(sizing.height, "raster fit height");
      return Object.freeze({ ...sizing });
  }
}

function requestedScaleForSizing(
  baseWidth: number,
  baseHeight: number,
  sizing: RasterSizing,
): number {
  switch (sizing.mode) {
    case "scale":
      return sizing.scale;
    case "width":
      return sizing.width / baseWidth;
    case "height":
      return sizing.height / baseHeight;
    case "fit":
      return Math.min(1, sizing.width / baseWidth, sizing.height / baseHeight);
  }
}

function resolveBackground(
  source: Readonly<RasterExportSource>,
  request: Readonly<RasterExportRequest>,
): Readonly<PlannedRasterBackground> {
  const background = request.background;
  if (background.mode === "transparent") {
    if (request.format !== "png") {
      throw new Error("JPEG export requires an opaque background");
    }
    return Object.freeze({ mode: "transparent", color: null, opaque: false });
  }
  if (background.mode === "custom") {
    if (!/^#[0-9a-f]{6}$/iu.test(background.color)) {
      throw new Error("custom raster background must be a six-digit hex color");
    }
    return Object.freeze({
      mode: "custom",
      color: background.color.toLowerCase(),
      opaque: true,
    });
  }

  const original = source.originalBackground;
  if (request.format === "jpeg" && (!original?.opaque || !original.color.trim())) {
    throw new Error("JPEG Original requires an opaque SVG root background");
  }
  return Object.freeze({
    mode: "original",
    color: original?.color ?? null,
    opaque: original?.opaque ?? false,
  });
}

function resolveQuality(request: Readonly<RasterExportRequest>): number | null {
  if (request.format === "png") return null;
  const quality = request.quality ?? 90;
  if (!Number.isInteger(quality) || quality < 1 || quality > 100) {
    throw new Error("JPEG quality must be an integer from 1 through 100");
  }
  return quality;
}

function limitedDimensions(
  baseWidth: number,
  baseHeight: number,
  scale: number,
  maxSide: number,
): [number, number] {
  return [
    Math.min(maxSide, rasterDimension(baseWidth * scale)),
    Math.min(maxSide, rasterDimension(baseHeight * scale)),
  ];
}

function rasterDimension(value: number): number {
  requirePositiveFinite(value, "computed raster dimension");
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
