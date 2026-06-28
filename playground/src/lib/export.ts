import { normalizeSvgDimensions } from "@/src/lib/svg-geometry";

/**
 * 导出 SVG 文件
 */
export function exportSVG(svg: string, filename: string = 'diagram'): void {
  const blob = new Blob([svg], { type: 'image/svg+xml;charset=utf-8' });
  downloadBlob(blob, `${filename}.svg`);
}

/**
 * 导出 PNG 文件
 */
export async function exportPNG(
  svg: string,
  filename: string = 'diagram',
  scale: number = 2
): Promise<void> {
  const blob = await rasterizeSvgToPngBlob(svg, scale);
  downloadBlob(blob, `${filename}.png`);
}

/**
 * 导出 ASCII 文件
 * 这个函数预留接口给 WASM 模块调用
 * 在 mock 模式下，返回一个简单的 ASCII 预览
 */
export function exportASCII(
  ascii: string,
  filename: string = 'diagram'
): void {
  const blob = new Blob([ascii], { type: 'text/plain;charset=utf-8' });
  downloadBlob(blob, `${filename}.txt`);
}

/**
 * 复制 ASCII 到剪贴板
 */
export async function copyASCIIToClipboard(ascii: string): Promise<void> {
  await navigator.clipboard.writeText(ascii);
}

/**
 * 复制 SVG 到剪贴板
 */
export async function copySVGToClipboard(svg: string): Promise<void> {
  await navigator.clipboard.writeText(svg);
}

/**
 * 复制 PNG 到剪贴板
 */
export async function copyPNGToClipboard(
  svg: string,
  scale: number = 2
): Promise<void> {
  const blob = await rasterizeSvgToPngBlob(svg, scale);
  await navigator.clipboard.write([
    new ClipboardItem({ 'image/png': blob }),
  ]);
}

/**
 * 复制代码到剪贴板
 */
export async function copyCodeToClipboard(code: string): Promise<void> {
  await navigator.clipboard.writeText(code);
}

/**
 * 下载 Blob 文件
 */
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
  svg: string;
  width: number;
  height: number;
}

const FALLBACK_RASTER_WIDTH = 300;
const FALLBACK_RASTER_HEIGHT = 150;

async function rasterizeSvgToPngBlob(
  svg: string,
  scale: number
): Promise<Blob> {
  const source = prepareSvgForRasterExport(svg);
  const effectiveScale = normalizeScale(scale);
  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d');
  if (!ctx) {
    throw new Error('Failed to get canvas context');
  }

  canvas.width = Math.max(1, Math.ceil(source.width * effectiveScale));
  canvas.height = Math.max(1, Math.ceil(source.height * effectiveScale));

  const img = new Image();
  img.crossOrigin = 'anonymous';

  const svgBlob = new Blob([source.svg], {
    type: 'image/svg+xml;charset=utf-8',
  });
  const url = URL.createObjectURL(svgBlob);

  try {
    await loadImage(img, url);
    ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
  } finally {
    URL.revokeObjectURL(url);
  }

  return canvasToPngBlob(canvas);
}

function prepareSvgForRasterExport(svg: string): RasterSvgSource {
  return normalizeSvgDimensions(svg) ?? fallbackRasterSvgSource(svg);
}

function fallbackRasterSvgSource(svg: string): RasterSvgSource {
  return {
    svg,
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

function loadImage(img: HTMLImageElement, url: string): Promise<void> {
  return new Promise((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error('Failed to load SVG image'));
    img.src = url;
  });
}

function canvasToPngBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => {
        if (blob) {
          resolve(blob);
        } else {
          reject(new Error('Failed to create PNG blob'));
        }
      },
      'image/png',
      1.0
    );
  });
}
