import {
  assertNavigableSvgForDom,
  type NavigableSvgDomAdmission,
} from "@mermanjs/web";

const NAVIGABLE_INLINE_SVG: unique symbol = Symbol("NavigableInlineSvg");
const NAVIGABLE_INLINE_SVG_ARTIFACTS = new WeakSet<object>();

export interface NavigableInlineSvg {
  readonly [NAVIGABLE_INLINE_SVG]: true;
  readonly kind: "navigable-inline-svg";
  readonly mountAdmission: NavigableSvgDomAdmission;
  readonly svg: string;
}

export function projectNavigableInlineSvg(svg: string): NavigableInlineSvg {
  const mountAdmission = assertNavigableSvgForDom(svg);
  const artifact: NavigableInlineSvg = {
    [NAVIGABLE_INLINE_SVG]: true,
    kind: "navigable-inline-svg",
    mountAdmission,
    svg,
  };
  const frozenArtifact = Object.freeze(artifact);
  NAVIGABLE_INLINE_SVG_ARTIFACTS.add(frozenArtifact);
  return frozenArtifact;
}

export function assertNavigableInlineSvgArtifact(artifact: NavigableInlineSvg): void {
  if (!NAVIGABLE_INLINE_SVG_ARTIFACTS.has(artifact)) {
    throw new Error("SVG artifact was not created by the navigable inline SVG projector.");
  }
}
