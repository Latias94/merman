import { assertSafeSvgForDom } from "@mermanjs/web/svg-safety";

const SAFE_INLINE_SVG: unique symbol = Symbol("SafeInlineSvg");

export interface SafeInlineSvg {
  readonly [SAFE_INLINE_SVG]: true;
  readonly exportFormats: {
    readonly png: true;
    readonly svg: true;
  };
  readonly kind: "safe-inline-svg";
  readonly svg: string;
}

export function projectSafeInlineSvg(svg: string): SafeInlineSvg {
  assertSafeSvgForDom(svg);
  const artifact: SafeInlineSvg = {
    [SAFE_INLINE_SVG]: true,
    kind: "safe-inline-svg",
    exportFormats: Object.freeze({ png: true, svg: true }),
    svg,
  };
  return Object.freeze(artifact);
}
