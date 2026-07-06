import { assertSafeSvgWithMessagePrefix } from "./svg-safety-policy.js";

export function assertSafeSvgForDom(svg: string): void {
  assertSafeSvgWithMessagePrefix(svg, "Merman rendered");
}
