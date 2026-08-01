import { assertSafeSvgForDom } from "@mermanjs/web";

const SAFE_INLINE_SVG: unique symbol = Symbol("SafeInlineSvg");
const SAFE_INLINE_SVG_ARTIFACTS = new WeakSet<object>();

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
  const frozenArtifact = Object.freeze(artifact);
  SAFE_INLINE_SVG_ARTIFACTS.add(frozenArtifact);
  return frozenArtifact;
}

export function assertSafeInlineSvgArtifact(artifact: SafeInlineSvg): void {
  if (!SAFE_INLINE_SVG_ARTIFACTS.has(artifact)) {
    throw new Error("SVG artifact was not created by the safe inline SVG projector.");
  }
}
