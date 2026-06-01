/**
 * 支持 ASCII 导出的图表类型
 */
export const ASCII_SUPPORTED_TYPES = [
  'flowchart',
  'sequence',
  'class',
  'er',
  'xychart',
] as const;

export type AsciiSupportedType = (typeof ASCII_SUPPORTED_TYPES)[number];

/**
 * 检查图表类型是否支持 ASCII 导出
 */
export function isAsciiSupported(diagramType: string): boolean {
  return ASCII_SUPPORTED_TYPES.includes(diagramType as AsciiSupportedType);
}

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
  return new Promise((resolve, reject) => {
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      reject(new Error('Failed to get canvas context'));
      return;
    }

    const img = new Image();
    img.crossOrigin = 'anonymous';

    img.onload = () => {
      canvas.width = img.width * scale;
      canvas.height = img.height * scale;
      ctx.scale(scale, scale);
      ctx.drawImage(img, 0, 0);

      canvas.toBlob(
        (blob) => {
          if (blob) {
            downloadBlob(blob, `${filename}.png`);
            resolve();
          } else {
            reject(new Error('Failed to create PNG blob'));
          }
        },
        'image/png',
        1.0
      );
    };

    img.onerror = () => {
      reject(new Error('Failed to load SVG image'));
    };

    // 将 SVG 转换为 data URL
    const svgBlob = new Blob([svg], { type: 'image/svg+xml;charset=utf-8' });
    img.src = URL.createObjectURL(svgBlob);
  });
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
  return new Promise((resolve, reject) => {
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      reject(new Error('Failed to get canvas context'));
      return;
    }

    const img = new Image();
    img.crossOrigin = 'anonymous';

    img.onload = async () => {
      canvas.width = img.width * scale;
      canvas.height = img.height * scale;
      ctx.scale(scale, scale);
      ctx.drawImage(img, 0, 0);

      canvas.toBlob(async (blob) => {
        if (blob) {
          try {
            await navigator.clipboard.write([
              new ClipboardItem({ 'image/png': blob }),
            ]);
            resolve();
          } catch (err) {
            reject(err);
          }
        } else {
          reject(new Error('Failed to create PNG blob'));
        }
      }, 'image/png');
    };

    img.onerror = () => {
      reject(new Error('Failed to load SVG image'));
    };

    const svgBlob = new Blob([svg], { type: 'image/svg+xml;charset=utf-8' });
    img.src = URL.createObjectURL(svgBlob);
  });
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
