import { prepareSvgForRasterExport } from "@/src/lib/svg-geometry";
import type { RasterExportPlan } from "@/src/lib/raster-export-plan";
import {
  assertNavigableInlineSvgArtifact,
  type NavigableInlineSvg,
} from "@/src/runtime/render-artifact";

class RasterExportError extends Error {
  readonly plan: Readonly<RasterExportPlan>;

  constructor(
    plan: Readonly<RasterExportPlan>,
    reason: string,
    options?: ErrorOptions,
  ) {
    super(
      `${plan.format.toUpperCase()} export failed at ${plan.outputWidth} × ${plan.outputHeight}: ${reason}. ` +
        "Try SVG or smaller raster dimensions.",
      options,
    );
    this.name = "RasterExportError";
    this.plan = plan;
  }
}

export function createSvgExportBlob(artifact: NavigableInlineSvg): Blob {
  assertNavigableInlineSvgArtifact(artifact);
  return new Blob([artifact.svg], { type: "image/svg+xml;charset=utf-8" });
}

export async function encodeRasterExport(
  artifact: NavigableInlineSvg,
  plan: Readonly<RasterExportPlan>,
): Promise<Blob> {
  assertNavigableInlineSvgArtifact(artifact);
  const rasterArtifact = prepareSvgForRasterExport(artifact, plan) ?? artifact;
  const canvas = document.createElement("canvas");
  const context = allocateCanvas(canvas, plan);

  if (plan.background.opaque && plan.background.color) {
    context.fillStyle = plan.background.color;
    context.fillRect(0, 0, plan.outputWidth, plan.outputHeight);
  }

  const image = new Image();
  image.crossOrigin = "anonymous";
  const sourceBlob = new Blob([rasterArtifact.svg], {
    type: "image/svg+xml;charset=utf-8",
  });
  const sourceUrl = URL.createObjectURL(sourceBlob);

  try {
    await loadImage(image, sourceUrl, plan);
    try {
      context.drawImage(image, 0, 0, plan.outputWidth, plan.outputHeight);
    } catch (error) {
      throw rasterFailure(
        plan,
        "the browser could not draw the planned canvas",
        error,
      );
    }
  } finally {
    URL.revokeObjectURL(sourceUrl);
  }

  return encodeCanvas(canvas, plan);
}

export function exportASCII(
  ascii: string,
  filename: string = "diagram",
): void {
  const blob = new Blob([ascii], { type: "text/plain;charset=utf-8" });
  downloadBlob(blob, `${filename}.txt`);
}

export async function copyASCIIToClipboard(ascii: string): Promise<void> {
  await navigator.clipboard.writeText(ascii);
}

export async function copySVGToClipboard(
  artifact: NavigableInlineSvg,
): Promise<void> {
  assertNavigableInlineSvgArtifact(artifact);
  await navigator.clipboard.writeText(artifact.svg);
}

export async function copyCodeToClipboard(code: string): Promise<void> {
  await navigator.clipboard.writeText(code);
}

export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

function allocateCanvas(
  canvas: HTMLCanvasElement,
  plan: Readonly<RasterExportPlan>,
): CanvasRenderingContext2D {
  try {
    canvas.width = plan.outputWidth;
    canvas.height = plan.outputHeight;
  } catch (error) {
    throw rasterFailure(
      plan,
      "the browser rejected the planned canvas size",
      error,
    );
  }
  if (
    canvas.width !== plan.outputWidth ||
    canvas.height !== plan.outputHeight
  ) {
    throw rasterFailure(plan, "the browser clamped the planned canvas size");
  }
  const context = canvas.getContext("2d");
  if (!context) {
    throw rasterFailure(
      plan,
      "the browser could not allocate a 2D canvas at this size",
    );
  }
  return context;
}

function loadImage(
  image: HTMLImageElement,
  url: string,
  plan: Readonly<RasterExportPlan>,
): Promise<void> {
  return new Promise((resolve, reject) => {
    image.onload = () => resolve();
    image.onerror = () =>
      reject(
        rasterFailure(plan, "the browser could not decode the prepared SVG"),
      );
    image.src = url;
  });
}

function encodeCanvas(
  canvas: HTMLCanvasElement,
  plan: Readonly<RasterExportPlan>,
): Promise<Blob> {
  return new Promise((resolve, reject) => {
    try {
      canvas.toBlob(
        (blob) => {
          if (!blob) {
            reject(rasterFailure(plan, "the browser could not encode the canvas"));
            return;
          }
          if (blob.type !== plan.mimeType) {
            reject(
              rasterFailure(
                plan,
                `the browser returned ${blob.type || "an unknown MIME type"}`,
              ),
            );
            return;
          }
          resolve(blob);
        },
        plan.mimeType,
        plan.quality === null ? undefined : plan.quality / 100,
      );
    } catch (error) {
      reject(
        rasterFailure(plan, "the browser could not encode the canvas", error),
      );
    }
  });
}

function rasterFailure(
  plan: Readonly<RasterExportPlan>,
  reason: string,
  cause?: unknown,
): RasterExportError {
  return new RasterExportError(
    plan,
    reason,
    cause === undefined ? undefined : { cause },
  );
}
