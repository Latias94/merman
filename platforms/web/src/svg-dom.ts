import {
  assertNavigableSvgForDom,
  prepareNavigableSvgForDomMount,
  type NavigableSvgDomAdmission,
} from "./svg-safety.js";

export interface SvgDomCandidate {
  readonly root: SVGSVGElement;
  readonly admission: NavigableSvgDomAdmission;
}

/** Build and admit the replacement before it can enter the host's DOM. */
export function prepareSvgDomCandidate(ownerDocument: Document, svgText: string): SvgDomCandidate {
  const Parser = ownerDocument.defaultView?.DOMParser ?? globalThis.DOMParser;
  if (!Parser) throw new Error("Rendering SVG to an element requires DOMParser support.");
  const admission = assertNavigableSvgForDom(svgText);
  const parsed = new Parser().parseFromString(svgText, "image/svg+xml");
  const parseError = parsed.querySelector("parsererror");
  if (parseError) throw new Error(parseError.textContent || "Merman rendered invalid SVG.");
  if (parsed.documentElement.localName !== "svg") {
    throw new Error("Merman render output did not contain an SVG root element.");
  }
  const root = ownerDocument.importNode(parsed.documentElement, true) as unknown as SVGSVGElement;
  prepareNavigableSvgForDomMount(admission, root, ownerDocument);
  return { root, admission };
}

/** Recheck the actual target document immediately before atomic publication. */
export function publishSvgDomCandidate(target: Element, candidate: SvgDomCandidate): void {
  prepareNavigableSvgForDomMount(candidate.admission, candidate.root, target.ownerDocument);
  target.replaceChildren(candidate.root);
}
