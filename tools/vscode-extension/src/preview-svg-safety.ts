import { assertSafeSvgWithMessagePrefix } from "./preview-svg-safety-policy.js";

export function assertSafePreviewSvg(svg: string): void {
  assertSafeSvgWithMessagePrefix(svg, "Preview renderer returned");
}
