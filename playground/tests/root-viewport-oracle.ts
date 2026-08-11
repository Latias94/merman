export const ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX = 1 / 64;

export type RootViewportAudit = {
  root: RectSnapshot | null;
  paintedUnion: RectSnapshot | null;
  paintedElementCount: number;
  violations: PaintedElementViolation[];
};

export type RectSnapshot = {
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
};

export type PaintedElementViolation = {
  element: string;
  rect: RectSnapshot;
};

export type RootViewportAuditRequest = {
  svgSource: string;
  quantizationEpsilon: number;
};

export function auditMountedSvg(request: RootViewportAuditRequest): RootViewportAudit {
  const { svgSource, quantizationEpsilon } = request;
  function quantizedRect(rect: DOMRect): RectSnapshot {
    const left = quantize(rect.left);
    const top = quantize(rect.top);
    const right = quantize(rect.right);
    const bottom = quantize(rect.bottom);
    return {
      left,
      top,
      right,
      bottom,
      width: quantize(Math.max(0, right - left)),
      height: quantize(Math.max(0, bottom - top)),
    };
  }

  function quantize(value: number): number {
    return Math.round(value / quantizationEpsilon) * quantizationEpsilon;
  }

  function contains(outer: RectSnapshot, inner: RectSnapshot): boolean {
    return (
      inner.left >= outer.left &&
      inner.top >= outer.top &&
      inner.right <= outer.right &&
      inner.bottom <= outer.bottom
    );
  }

  function unionRects(rects: RectSnapshot[]): RectSnapshot | null {
    if (rects.length === 0) return null;
    const left = Math.min(...rects.map((rect) => rect.left));
    const top = Math.min(...rects.map((rect) => rect.top));
    const right = Math.max(...rects.map((rect) => rect.right));
    const bottom = Math.max(...rects.map((rect) => rect.bottom));
    return {
      left,
      top,
      right,
      bottom,
      width: right - left,
      height: bottom - top,
    };
  }

  function elementDescriptor(element: Element): string {
    const id = element.getAttribute("id");
    const classes = element
      .getAttribute("class")
      ?.trim()
      .split(/\s+/u)
      .filter(Boolean)
      .slice(0, 3)
      .join(".");
    return `${element.localName}${id ? `#${id}` : ""}${classes ? `.${classes}` : ""}`;
  }

  if (!Number.isFinite(quantizationEpsilon) || quantizationEpsilon <= 0) {
    throw new Error("Root viewport quantization epsilon must be positive and finite.");
  }

  document.documentElement.style.background = "white";
  document.body.replaceChildren();
  document.body.style.margin = "0";
  document.body.style.padding = "0";

  const host = document.createElement("div");
  host.style.position = "absolute";
  host.style.left = "0";
  host.style.top = "0";
  host.style.overflow = "visible";
  host.innerHTML = svgSource;
  document.body.append(host);

  const svg = host.querySelector(":scope > svg");
  if (!(svg instanceof SVGSVGElement)) {
    return {
      root: null,
      paintedUnion: null,
      paintedElementCount: 0,
      violations: [],
    };
  }

  const viewBox = svg.viewBox.baseVal;
  const sourceWidth =
    Number.isFinite(viewBox.width) && viewBox.width > 0 ? viewBox.width : 800;
  const sourceHeight =
    Number.isFinite(viewBox.height) && viewBox.height > 0 ? viewBox.height : 600;
  const scale = Math.min(1, 1200 / sourceWidth, 1200 / sourceHeight);
  const viewportWidth = Math.max(1, sourceWidth * scale);
  const viewportHeight = Math.max(1, sourceHeight * scale);
  host.style.width = `${viewportWidth}px`;
  host.style.height = `${viewportHeight}px`;
  svg.style.setProperty("width", `${viewportWidth}px`, "important");
  svg.style.setProperty("height", `${viewportHeight}px`, "important");
  svg.style.setProperty("max-width", "none", "important");
  svg.style.setProperty("display", "block", "important");

  const root = quantizedRect(svg.getBoundingClientRect());
  const paintedRects: RectSnapshot[] = [];
  const violations: PaintedElementViolation[] = [];
  const paintedSelectors = [
    "circle",
    "ellipse",
    "foreignObject",
    "image",
    "line",
    "path",
    "polygon",
    "polyline",
    "rect",
    "text",
    "use",
  ].join(",");

  for (const element of svg.querySelectorAll<SVGGraphicsElement>(paintedSelectors)) {
    if (element.closest("defs,clipPath,mask,marker,pattern,symbol")) continue;
    const style = getComputedStyle(element);
    if (
      style.display === "none" ||
      style.visibility === "hidden" ||
      Number.parseFloat(style.opacity || "1") === 0
    ) {
      continue;
    }
    const rect = quantizedRect(element.getBoundingClientRect());
    if (rect.width === 0 && rect.height === 0) continue;
    paintedRects.push(rect);
    if (!contains(root, rect)) {
      violations.push({ element: elementDescriptor(element), rect });
    }

    if (element instanceof SVGForeignObjectElement) {
      for (const htmlElement of element.querySelectorAll<HTMLElement>("*")) {
        const htmlStyle = getComputedStyle(htmlElement);
        if (
          htmlStyle.display === "none" ||
          htmlStyle.visibility === "hidden" ||
          Number.parseFloat(htmlStyle.opacity || "1") === 0
        ) {
          continue;
        }
        const htmlRect = quantizedRect(htmlElement.getBoundingClientRect());
        if (htmlRect.width === 0 && htmlRect.height === 0) continue;
        paintedRects.push(htmlRect);
        if (!contains(root, htmlRect)) {
          violations.push({ element: elementDescriptor(htmlElement), rect: htmlRect });
        }
      }
    }
  }

  return {
    root,
    paintedUnion: unionRects(paintedRects),
    paintedElementCount: paintedRects.length,
    violations,
  };
}
