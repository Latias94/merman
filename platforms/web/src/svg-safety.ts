import {
  assertNavigableSvgWithMessagePrefix,
  assertSelfContainedSvgWithMessagePrefix,
  type SvgSafetyInspection,
} from "./svg-safety-policy.js";

const SVG_DOM_ADMISSION_BRAND: unique symbol = Symbol("svg-dom-admission");
const SVG_DOM_ADMISSIONS = new WeakMap<object, SvgDomCapability>();

type SvgDomCapability = "self-contained" | "navigable";

/** An opaque admission created by one of the capability-specific SVG validators. */
export interface SvgDomAdmission<Capability extends SvgDomCapability> {
  readonly hasFragmentReferences: boolean;
  readonly [SVG_DOM_ADMISSION_BRAND]: Capability;
}

export type SelfContainedSvgDomAdmission = SvgDomAdmission<"self-contained">;
export type NavigableSvgDomAdmission = SvgDomAdmission<"navigable">;

/** Accepts only self-contained SVG without external navigation or rendering resources. */
export function assertSelfContainedSvgForDom(
  svg: string,
): SelfContainedSvgDomAdmission {
  return createSvgDomAdmission(
    assertSelfContainedSvgWithMessagePrefix(svg, "Merman rendered"),
    "self-contained",
  );
}

/**
 * Accepts self-contained SVG plus Mermaid-compatible, user-activated navigation
 * on SVG anchor elements. External rendering resources remain forbidden.
 */
export function assertNavigableSvgForDom(svg: string): NavigableSvgDomAdmission {
  return createSvgDomAdmission(
    assertNavigableSvgWithMessagePrefix(svg, "Merman rendered"),
    "navigable",
  );
}

/** Validates self-contained SVG again at the exact DOM mount boundary. */
export function prepareSelfContainedSvgForDomMount(
  admission: SelfContainedSvgDomAdmission,
  root: Element,
  ownerDocument: Document,
): void {
  const inspection = prepareSvgForDomMount(
    admission,
    "self-contained",
    root,
    ownerDocument,
    (svg) => assertSelfContainedSvgWithMessagePrefix(svg, "Merman mounted"),
  );
  assertSvgMountDocument(inspection.hasFragmentReferences, ownerDocument);
}

/**
 * Validates and hardens a navigable SVG at the exact DOM mount boundary.
 *
 * The admission and the parsed root must belong to the same Web package instance and document.
 * A string or a structured clone is never treated as an admission.
 */
export function prepareNavigableSvgForDomMount(
  admission: NavigableSvgDomAdmission,
  root: Element,
  ownerDocument: Document,
): void {
  const inspection = prepareSvgForDomMount(
    admission,
    "navigable",
    root,
    ownerDocument,
    (svg) => assertNavigableSvgWithMessagePrefix(svg, "Merman mounted"),
  );
  assertSvgMountDocument(inspection.hasFragmentReferences, ownerDocument);

  for (const anchor of Array.from(root.querySelectorAll("a"))) {
    const href =
      anchor.getAttribute("href") ??
      anchor.getAttributeNS("http://www.w3.org/1999/xlink", "href") ??
      anchor.getAttribute("xlink:href");
    if (!href) continue;

    if (href.trimStart().startsWith("#")) {
      anchor.setAttribute("target", "_self");
      continue;
    }

    anchor.setAttribute("target", "_blank");
    const rel = new Set(
      (anchor.getAttribute("rel") ?? "")
        .split(/\s+/u)
        .filter(Boolean)
        .map((token: string) => token.toLowerCase()),
    );
    rel.add("noopener");
    rel.add("noreferrer");
    anchor.setAttribute("rel", [...rel].join(" "));
  }
}

function prepareSvgForDomMount(
  admission: SelfContainedSvgDomAdmission | NavigableSvgDomAdmission,
  expectedCapability: SvgDomCapability,
  root: Element,
  ownerDocument: Document,
  inspect: (svg: string) => SvgSafetyInspection,
): SvgSafetyInspection {
  assertSvgDomAdmission(admission, expectedCapability);
  if (root.ownerDocument !== ownerDocument) {
    throw new Error("SVG root belongs to a different mount document.");
  }
  // `outerHTML` is an HTML serialization. In particular, HTML void elements such as
  // `<br />` are emitted as `<br>`, which is valid in the mounted foreignObject DOM but
  // looks like an unclosed XML element to the SVG safety scanner. Re-serialize the
  // already-admitted DOM as XML so the second check observes the same element structure
  // as the original SVG artifact.
  const Serializer = ownerDocument.defaultView?.XMLSerializer ?? globalThis.XMLSerializer;
  return inspect(
    Serializer ? new Serializer().serializeToString(root) : root.outerHTML,
  );
}

function assertSvgMountDocument(
  hasFragmentReferences: boolean,
  ownerDocument: Pick<Document, "URL" | "baseURI">,
): void {
  if (!hasFragmentReferences) return;

  let documentUrl: URL;
  let baseUrl: URL;
  try {
    documentUrl = new URL(ownerDocument.URL);
    baseUrl = new URL(ownerDocument.baseURI, documentUrl);
  } catch {
    throw new Error("SVG mount document has an invalid URL or base URI.");
  }
  documentUrl.hash = "";
  baseUrl.hash = "";
  if (baseUrl.href !== documentUrl.href) {
    throw new Error(
      "SVG fragment references would resolve outside the mount document because its base URI differs from its URL.",
    );
  }
}

function createSvgDomAdmission<Capability extends SvgDomCapability>(
  inspection: SvgSafetyInspection,
  capability: Capability,
): SvgDomAdmission<Capability> {
  const admission = Object.freeze({
    hasFragmentReferences: inspection.hasFragmentReferences,
    [SVG_DOM_ADMISSION_BRAND]: capability,
  }) as SvgDomAdmission<Capability>;
  SVG_DOM_ADMISSIONS.set(admission, capability);
  return admission;
}

function assertSvgDomAdmission(
  admission: SelfContainedSvgDomAdmission | NavigableSvgDomAdmission,
  expectedCapability?: SvgDomCapability,
): void {
  if (typeof admission !== "object" || admission === null) {
    throw new Error("SVG DOM admission was not created by a Merman SVG validator.");
  }
  const capability = SVG_DOM_ADMISSIONS.get(admission);
  if (
    capability === undefined ||
    capability !== admission[SVG_DOM_ADMISSION_BRAND]
  ) {
    throw new Error("SVG DOM admission was not created by a Merman SVG validator.");
  }
  if (expectedCapability !== undefined && capability !== expectedCapability) {
    throw new Error(
      `SVG DOM admission grants ${capability} capability, not ${expectedCapability} capability.`,
    );
  }
}
