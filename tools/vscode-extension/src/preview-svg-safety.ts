import { assertSelfContainedSvgWithMessagePrefix } from "./preview-svg-safety-policy.js";

export function assertSelfContainedPreviewSvg(svg: string): void {
  assertSelfContainedSvgWithMessagePrefix(svg, "Preview renderer returned");
}
