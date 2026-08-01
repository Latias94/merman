import {
  parseSvgDimensions,
  sizeSvgForRasterization,
} from "@/src/lib/svg-geometry";
import {
  planPngRaster,
  type PngRasterPlan,
} from "@/src/lib/png-export-plan";
import {
  assertSafeInlineSvgArtifact,
  type SafeInlineSvg,
} from "@/src/runtime/render-artifact";

/** Download an SVG file. */
export function exportSVG(
  artifact: SafeInlineSvg,
  filename: string = 'diagram'
): void {
  assertSafeInlineSvgArtifact(artifact);
  const blob = new Blob([artifact.svg], { type: 'image/svg+xml;charset=utf-8' });
  downloadBlob(blob, `${filename}.svg`);
}

/** Download a PNG file and report its actual raster dimensions. */
export async function exportPNG(
  artifact: SafeInlineSvg,
  filename: string = 'diagram',
  scale: number = 2,
  hooks: PngExportHooks = {}
): Promise<PngRasterPlan> {
  const { blob, plan } = await rasterizeSvgToPngBlob(artifact, scale, hooks);
  downloadBlob(blob, `${filename}.png`);
  return plan;
}

/** Download an ASCII artifact produced by the WASM runtime. */
export function exportASCII(
  ascii: string,
  filename: string = 'diagram'
): void {
  const blob = new Blob([ascii], { type: 'text/plain;charset=utf-8' });
  downloadBlob(blob, `${filename}.txt`);
}

/** Copy an ASCII artifact to the clipboard. */
export async function copyASCIIToClipboard(ascii: string): Promise<void> {
  await navigator.clipboard.writeText(ascii);
}

/** Copy an SVG artifact to the clipboard. */
export async function copySVGToClipboard(artifact: SafeInlineSvg): Promise<void> {
  assertSafeInlineSvgArtifact(artifact);
  await navigator.clipboard.writeText(artifact.svg);
}

/** Copy a PNG artifact to the clipboard and report its raster dimensions. */
export async function copyPNGToClipboard(
  artifact: SafeInlineSvg,
  scale: number = 2
): Promise<PngRasterPlan> {
  const { blob, plan } = await rasterizeSvgToPngBlob(artifact, scale);
  await navigator.clipboard.write([
    new ClipboardItem({ 'image/png': blob }),
  ]);
  return plan;
}

/** Copy Mermaid source to the clipboard. */
export async function copyCodeToClipboard(code: string): Promise<void> {
  await navigator.clipboard.writeText(code);
}

/** Download a browser Blob. */
function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

interface RasterSvgSource {
  artifact: SafeInlineSvg;
  width: number;
  height: number;
}

interface RasterizedPng {
  blob: Blob;
  plan: PngRasterPlan;
}

export interface PngExportHooks {
  onPlan?(plan: PngRasterPlan): void;
}

export class PngExportError extends Error {
  readonly plan: PngRasterPlan;

  constructor(plan: PngRasterPlan, reason: string, options?: ErrorOptions) {
    super(
      `PNG export failed at ${plan.outputWidth} × ${plan.outputHeight}: ${reason}. ` +
        'Try exporting SVG or reducing the diagram size.',
      options
    );
    this.name = 'PngExportError';
    this.plan = plan;
  }
}

const FALLBACK_RASTER_WIDTH = 300;
const FALLBACK_RASTER_HEIGHT = 150;

async function rasterizeSvgToPngBlob(
  artifact: SafeInlineSvg,
  scale: number,
  hooks: PngExportHooks = {}
): Promise<RasterizedPng> {
  assertSafeInlineSvgArtifact(artifact);
  const source = prepareSvgForRasterExport(artifact);
  const plan = planPngRaster(source.width, source.height, normalizeScale(scale));
  hooks.onPlan?.(plan);

  const rasterArtifact =
    sizeSvgForRasterization(artifact, {
      width: plan.outputWidth,
      height: plan.outputHeight,
    }) ?? source.artifact;
  const canvas = document.createElement('canvas');
  const ctx = allocateCanvas(canvas, plan);

  const img = new Image();
  img.crossOrigin = 'anonymous';

  const svgBlob = new Blob([rasterArtifact.svg], {
    type: 'image/svg+xml;charset=utf-8',
  });
  const url = URL.createObjectURL(svgBlob);

  try {
    await loadImage(img, url, plan);
    try {
      ctx.drawImage(img, 0, 0, plan.outputWidth, plan.outputHeight);
    } catch (error) {
      throw pngFailure(
        plan,
        'the browser could not draw the planned canvas',
        error
      );
    }
  } finally {
    URL.revokeObjectURL(url);
  }

  return {
    blob: await canvasToPngBlob(canvas, plan),
    plan,
  };
}

function prepareSvgForRasterExport(artifact: SafeInlineSvg): RasterSvgSource {
  const dimensions = parseSvgDimensions(artifact.svg);
  return dimensions
    ? { artifact, ...dimensions }
    : fallbackRasterSvgSource(artifact);
}

function fallbackRasterSvgSource(artifact: SafeInlineSvg): RasterSvgSource {
  return {
    artifact,
    width: FALLBACK_RASTER_WIDTH,
    height: FALLBACK_RASTER_HEIGHT,
  };
}

function normalizeScale(scale: number): number {
  return isPositiveFinite(scale) ? scale : 1;
}

function isPositiveFinite(value: number | undefined): value is number {
  return value !== undefined && Number.isFinite(value) && value > 0;
}

function allocateCanvas(
  canvas: HTMLCanvasElement,
  plan: PngRasterPlan
): CanvasRenderingContext2D {
  try {
    canvas.width = plan.outputWidth;
    canvas.height = plan.outputHeight;
  } catch (error) {
    throw pngFailure(plan, 'the browser rejected the planned canvas size', error);
  }

  if (
    canvas.width !== plan.outputWidth ||
    canvas.height !== plan.outputHeight
  ) {
    throw pngFailure(plan, 'the browser clamped the planned canvas size');
  }

  const ctx = canvas.getContext('2d');
  if (!ctx) {
    throw pngFailure(
      plan,
      'the browser could not allocate a 2D canvas at this size'
    );
  }
  return ctx;
}

function loadImage(
  img: HTMLImageElement,
  url: string,
  plan: PngRasterPlan
): Promise<void> {
  return new Promise((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () =>
      reject(pngFailure(plan, 'the browser could not decode the resized SVG'));
    img.src = url;
  });
}

function canvasToPngBlob(
  canvas: HTMLCanvasElement,
  plan: PngRasterPlan
): Promise<Blob> {
  return new Promise((resolve, reject) => {
    try {
      canvas.toBlob(
        (blob) => {
          if (blob) {
            resolve(blob);
          } else {
            reject(
              pngFailure(plan, 'the browser could not encode the canvas')
            );
          }
        },
        'image/png',
        1.0
      );
    } catch (error) {
      reject(pngFailure(plan, 'the browser could not encode the canvas', error));
    }
  });
}

function pngFailure(
  plan: PngRasterPlan,
  reason: string,
  cause?: unknown
): Error {
  return new PngExportError(
    plan,
    reason,
    cause === undefined ? undefined : { cause }
  );
}
