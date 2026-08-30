import type { Page } from "playwright";

export const ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX = 1 / 64;
export const ROOT_VIEWPORT_PAINT_GUARD_CSS_PX = 16;
export const ROOT_VIEWPORT_MAX_CAPTURE_DIMENSION_CSS_PX = 16_384;
export const ROOT_VIEWPORT_MAX_CAPTURE_AREA_CSS_PX = 4096 * 4096;

const ROOT_VIEWPORT_HOST_WIDTH_CSS_PX = 1200;
const ROOT_VIEWPORT_AUDIT_PAGE_CSS_PX =
  ROOT_VIEWPORT_HOST_WIDTH_CSS_PX + ROOT_VIEWPORT_PAINT_GUARD_CSS_PX * 2;

export type RootViewportAudit = {
  root: RectSnapshot | null;
  geometryUnion: RectSnapshot | null;
  paintedElementCount: number;
  paintAudit: PaintAuditSnapshot;
  violations: PaintedPixelViolation[];
  structuralViolations: PaintedPixelViolation[];
  structuralPixelKeys: string[];
};

export type RectSnapshot = {
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
};

export type RootViewportEdge = "top" | "right" | "bottom" | "left";

export type PaintedPixelViolation = {
  edge: RootViewportEdge;
  paintedPixelCount: number;
  rect: RectSnapshot;
  reachesAuditBoundary: boolean;
};

export type PaintAuditIndeterminateReason =
  | "active-box-shadow"
  | "active-filter"
  | "active-text-shadow"
  | "capture-boundary"
  | "capture-limit"
  | "image-decode-failed"
  | "image-decode-unavailable"
  | "marker-capture-unbounded";

export type PaintAuditSnapshot = {
  status: "collected" | "indeterminate" | "missing-root";
  guardCssPx: number;
  captureWidthCssPx: number | null;
  captureHeightCssPx: number | null;
  indeterminateReasons: PaintAuditIndeterminateReason[];
};

export type RootViewportAuditRequest = {
  svgSource: string;
};

export type RootViewportContainmentClassification =
  | "blocking"
  | "browser-owned-diagnostic"
  | "contained"
  | "upstream-inherited";

type MountedSvgSnapshot = {
  root: RectSnapshot | null;
  geometryUnion: RectSnapshot | null;
  paintedElementCount: number;
  screenshotWidth: number;
  screenshotHeight: number;
  indeterminateReasons: PaintAuditIndeterminateReason[];
  rootPixelBounds: {
    left: number;
    top: number;
    right: number;
    bottom: number;
  } | null;
};

type PixelBounds = {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
};

type PixelAuditSnapshot = {
  edges: Array<{
    edge: RootViewportEdge;
    paintedPixelCount: number;
    rect: RectSnapshot | null;
    reachesAuditBoundary: boolean;
  }>;
  pixelKeys: string[];
};

export function classifyRootViewportContainment(
  local: RootViewportAudit,
  upstream: RootViewportAudit | null,
): RootViewportContainmentClassification {
  if (local.paintAudit.status === "missing-root") return "blocking";
  if (
    local.paintAudit.indeterminateReasons.includes("capture-boundary") ||
    local.paintAudit.indeterminateReasons.includes("marker-capture-unbounded")
  ) {
    return "blocking";
  }
  if (local.paintAudit.status === "collected" && local.violations.length === 0) {
    return "contained";
  }
  if (
    local.paintAudit.status === "collected" &&
    local.violations.length > 0 &&
    local.structuralViolations.length === 0
  ) {
    return "browser-owned-diagnostic";
  }
  if (upstream === null) return "blocking";
  if (local.paintAudit.status === "indeterminate") {
    return indeterminateEvidenceIsNoWorse(local, upstream)
      ? "upstream-inherited"
      : "blocking";
  }
  return structuralEdgeDepthIsNoWorse(local, upstream)
    ? "upstream-inherited"
    : "blocking";
}

export function exactRootViewportResidualEvidenceIsEligible(
  local: RootViewportAudit,
  upstream: RootViewportAudit | null,
): boolean {
  return (
    upstream !== null &&
    local.root !== null &&
    upstream.root !== null &&
    local.paintAudit.status === "collected" &&
    upstream.paintAudit.status === "collected" &&
    local.paintAudit.indeterminateReasons.length === 0 &&
    upstream.paintAudit.indeterminateReasons.length === 0
  );
}

function indeterminateEvidenceIsNoWorse(
  local: RootViewportAudit,
  upstream: RootViewportAudit,
): boolean {
  if (upstream.paintAudit.status !== "indeterminate") return false;
  if (local.paintAudit.indeterminateReasons.includes("capture-boundary")) return false;
  if (!sameValues(local.paintAudit.indeterminateReasons, upstream.paintAudit.indeterminateReasons)) {
    return false;
  }
  if (local.paintAudit.indeterminateReasons.includes("capture-limit")) {
    if (local.paintAudit.indeterminateReasons.length !== 1) return false;
    return captureLimitEvidenceIsNoWorse(local, upstream);
  }
  return structuralEdgeDepthIsNoWorse(local, upstream, true);
}

function captureLimitEvidenceIsNoWorse(
  local: RootViewportAudit,
  upstream: RootViewportAudit,
): boolean {
  if (
    local.root === null ||
    upstream.root === null ||
    local.paintAudit.guardCssPx !== upstream.paintAudit.guardCssPx ||
    local.paintAudit.captureWidthCssPx === null ||
    upstream.paintAudit.captureWidthCssPx === null ||
    local.paintAudit.captureWidthCssPx > upstream.paintAudit.captureWidthCssPx ||
    local.paintAudit.captureHeightCssPx === null ||
    upstream.paintAudit.captureHeightCssPx === null ||
    local.paintAudit.captureHeightCssPx > upstream.paintAudit.captureHeightCssPx
  ) {
    return false;
  }
  const localDepths = geometryOutwardDepths(local.root, local.geometryUnion);
  const upstreamDepths = geometryOutwardDepths(upstream.root, upstream.geometryUnion);
  return (Object.keys(localDepths) as RootViewportEdge[]).every(
    (edge) => localDepths[edge] <= upstreamDepths[edge],
  );
}

function geometryOutwardDepths(
  root: RectSnapshot,
  geometry: RectSnapshot | null,
): Record<RootViewportEdge, number> {
  return {
    top: Math.max(0, -(geometry?.top ?? 0)),
    right: Math.max(0, (geometry?.right ?? root.width) - root.width),
    bottom: Math.max(0, (geometry?.bottom ?? root.height) - root.height),
    left: Math.max(0, -(geometry?.left ?? 0)),
  };
}

function structuralEdgeDepthIsNoWorse(
  local: RootViewportAudit,
  upstream: RootViewportAudit,
  allowIndeterminateUpstream = false,
): boolean {
  if (local.root === null || upstream.root === null) return false;
  const localRoot = local.root;
  const upstreamRoot = upstream.root;
  if (
    (upstream.paintAudit.status !== "collected" &&
      !(allowIndeterminateUpstream && upstream.paintAudit.status === "indeterminate"))
  ) {
    return false;
  }
  const upstreamDepths = new Map<RootViewportEdge, number>();
  for (const violation of upstream.structuralViolations) {
    upstreamDepths.set(
      violation.edge,
      Math.max(
        upstreamDepths.get(violation.edge) ?? 0,
        violationDepth(upstreamRoot, violation),
      ),
    );
  }
  return local.structuralViolations.every(
    (violation) =>
      violationDepth(localRoot, violation) <=
      (upstreamDepths.get(violation.edge) ?? 0),
  );
}

function violationDepth(
  root: RectSnapshot,
  violation: PaintedPixelViolation,
): number {
  switch (violation.edge) {
    case "top":
      return Math.max(0, -violation.rect.top);
    case "right":
      return Math.max(0, violation.rect.right - Math.ceil(root.width));
    case "bottom":
      return Math.max(0, violation.rect.bottom - Math.ceil(root.height));
    case "left":
      return Math.max(0, -violation.rect.left);
  }
}

function sameValues<T extends string>(left: T[], right: T[]): boolean {
  const sortedLeft = [...left].sort();
  const sortedRight = [...right].sort();
  return (
    sortedLeft.length === sortedRight.length &&
    sortedLeft.every((value, index) => value === sortedRight[index])
  );
}

export async function auditMountedSvg(
  page: Page,
  request: RootViewportAuditRequest,
): Promise<RootViewportAudit> {
  const viewport = page.viewportSize();
  if (
    viewport === null ||
    viewport.width < ROOT_VIEWPORT_AUDIT_PAGE_CSS_PX ||
    viewport.height < ROOT_VIEWPORT_AUDIT_PAGE_CSS_PX
  ) {
    await page.setViewportSize({
      width: Math.max(viewport?.width ?? 0, ROOT_VIEWPORT_AUDIT_PAGE_CSS_PX),
      height: Math.max(viewport?.height ?? 0, ROOT_VIEWPORT_AUDIT_PAGE_CSS_PX),
    });
  }

  const mounted = await mountSvg(page, request);
  if (mounted.root === null) {
    return {
      root: null,
      geometryUnion: mounted.geometryUnion,
      paintedElementCount: mounted.paintedElementCount,
      paintAudit: {
        status: "missing-root",
        guardCssPx: ROOT_VIEWPORT_PAINT_GUARD_CSS_PX,
        captureWidthCssPx: null,
        captureHeightCssPx: null,
        indeterminateReasons: [],
      },
      violations: [],
      structuralViolations: [],
      structuralPixelKeys: [],
    };
  }

  if (mounted.rootPixelBounds === null) {
    return {
      root: mounted.root,
      geometryUnion: mounted.geometryUnion,
      paintedElementCount: mounted.paintedElementCount,
      paintAudit: {
        status: "indeterminate",
        guardCssPx: ROOT_VIEWPORT_PAINT_GUARD_CSS_PX,
        captureWidthCssPx: mounted.screenshotWidth,
        captureHeightCssPx: mounted.screenshotHeight,
        indeterminateReasons: mounted.indeterminateReasons,
      },
      violations: [],
      structuralViolations: [],
      structuralPixelKeys: [],
    };
  }

  const currentViewport = page.viewportSize();
  if (
    currentViewport === null ||
    currentViewport.width < mounted.screenshotWidth ||
    currentViewport.height < mounted.screenshotHeight
  ) {
    await page.setViewportSize({
      width: Math.max(currentViewport?.width ?? 0, mounted.screenshotWidth),
      height: Math.max(currentViewport?.height ?? 0, mounted.screenshotHeight),
    });
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
  }

  const rootPixelBounds = mounted.rootPixelBounds;
  const paint = await capturePaintEvidence(
    page,
    mounted.screenshotWidth,
    mounted.screenshotHeight,
    rootPixelBounds,
  );
  let structuralPaint = paint;
  if (paint.violations.length > 0) {
    await setBrowserOwnedPaintSuppressed(page, true);
    try {
      structuralPaint = await capturePaintEvidence(
        page,
        mounted.screenshotWidth,
        mounted.screenshotHeight,
        rootPixelBounds,
      );
    } finally {
      await setBrowserOwnedPaintSuppressed(page, false);
    }
  }
  const indeterminateReasons = [...mounted.indeterminateReasons];
  if (paint.violations.some((violation) => violation.reachesAuditBoundary)) {
    indeterminateReasons.push("capture-boundary");
  }

  return {
    root: mounted.root,
    geometryUnion: mounted.geometryUnion,
    paintedElementCount: mounted.paintedElementCount,
    paintAudit: {
      status: indeterminateReasons.length > 0 ? "indeterminate" : "collected",
      guardCssPx: ROOT_VIEWPORT_PAINT_GUARD_CSS_PX,
      captureWidthCssPx: mounted.screenshotWidth,
      captureHeightCssPx: mounted.screenshotHeight,
      indeterminateReasons,
    },
    violations: paint.violations,
    structuralViolations: structuralPaint.violations,
    structuralPixelKeys: structuralPaint.pixelKeys,
  };
}

async function capturePaintEvidence(
  page: Page,
  screenshotWidth: number,
  screenshotHeight: number,
  rootPixelBounds: NonNullable<MountedSvgSnapshot["rootPixelBounds"]>,
): Promise<{ violations: PaintedPixelViolation[]; pixelKeys: string[] }> {
  const screenshot = await page.screenshot({
    animations: "disabled",
    caret: "hide",
    clip: {
      x: 0,
      y: 0,
      width: screenshotWidth,
      height: screenshotHeight,
    },
    omitBackground: true,
    scale: "css",
    type: "png",
  });
  const pixelAudit = await auditScreenshotPixels(
    page,
    screenshot.toString("base64"),
    rootPixelBounds,
  );
  return {
    violations: pixelAudit.edges.filter(
      (edge): edge is PaintedPixelViolation => edge.rect !== null,
    ),
    pixelKeys: pixelAudit.pixelKeys,
  };
}

async function setBrowserOwnedPaintSuppressed(page: Page, suppressed: boolean): Promise<void> {
  await page.evaluate((shouldSuppress) => {
    const styleId = "root-viewport-audit-browser-owned-style";
    const roughPathAttribute = "data-root-viewport-audit-rough-path";

    if (shouldSuppress) {
      const roughPaths = document.querySelectorAll<SVGPathElement>(
        ".root-viewport-audit-host path[data-look='handDrawn'], .root-viewport-audit-host [data-look='handDrawn'] path",
      );
      for (const path of roughPaths) {
        if (path.closest(".label, foreignObject, .icon-shape, .label-icon")) continue;
        path.setAttribute(roughPathAttribute, "");
      }
      const style = document.createElement("style");
      style.id = styleId;
      style.textContent = `
        .root-viewport-audit-host text,
        .root-viewport-audit-host text * { visibility: hidden !important; }
        .root-viewport-audit-host foreignObject * {
          -webkit-text-fill-color: transparent !important;
          -webkit-text-stroke-color: transparent !important;
          text-decoration-color: transparent !important;
          text-shadow: none !important;
        }
        .root-viewport-audit-host .label foreignObject * {
          background-color: transparent !important;
          border-color: transparent !important;
          box-shadow: none !important;
          outline-color: transparent !important;
        }
        .root-viewport-audit-host [${roughPathAttribute}] {
          fill-opacity: 0 !important;
          stroke-opacity: 0 !important;
        }
      `;
      document.head.append(style);
      return;
    }
    document.getElementById(styleId)?.remove();
    for (const path of document.querySelectorAll(`[${roughPathAttribute}]`)) {
      path.removeAttribute(roughPathAttribute);
    }
  }, suppressed);
}

async function mountSvg(
  page: Page,
  request: RootViewportAuditRequest,
): Promise<MountedSvgSnapshot> {
  return page.evaluate(
    async ({
      svgSource,
      quantizationEpsilon,
      guardCssPx,
      hostWidthCssPx,
      maxCaptureDimensionCssPx,
      maxCaptureAreaCssPx,
    }) => {
      function quantize(value: number): number {
        return Math.round(value / quantizationEpsilon) * quantizationEpsilon;
      }

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

      function unionRects(rects: RectSnapshot[]): RectSnapshot | null {
        if (rects.length === 0) return null;
        let left = Number.POSITIVE_INFINITY;
        let top = Number.POSITIVE_INFINITY;
        let right = Number.NEGATIVE_INFINITY;
        let bottom = Number.NEGATIVE_INFINITY;
        for (const rect of rects) {
          left = Math.min(left, rect.left);
          top = Math.min(top, rect.top);
          right = Math.max(right, rect.right);
          bottom = Math.max(bottom, rect.bottom);
        }
        return {
          left,
          top,
          right,
          bottom,
          width: quantize(right - left),
          height: quantize(bottom - top),
        };
      }

      function translateRect(
        rect: RectSnapshot | null,
        deltaX: number,
        deltaY: number,
      ): RectSnapshot | null {
        if (rect === null) return null;
        return {
          left: quantize(rect.left + deltaX),
          top: quantize(rect.top + deltaY),
          right: quantize(rect.right + deltaX),
          bottom: quantize(rect.bottom + deltaY),
          width: rect.width,
          height: rect.height,
        };
      }

      document.documentElement.style.background = "transparent";
      document.documentElement.style.colorScheme = "light";
      document.body.replaceChildren();
      Object.assign(document.body.style, {
        background: "transparent",
        margin: "0",
        minHeight: "0",
        overflow: "hidden",
        padding: "0",
      });

      const host = document.createElement("div");
      host.className = "root-viewport-audit-host";
      Object.assign(host.style, {
        background: "transparent",
        left: `${guardCssPx}px`,
        overflow: "visible",
        position: "absolute",
        top: `${guardCssPx}px`,
      });
      host.innerHTML = svgSource;
      document.body.append(host);

      const svg = host.querySelector(":scope > svg");
      if (!(svg instanceof SVGSVGElement)) {
        return {
          root: null,
          geometryUnion: null,
          paintedElementCount: 0,
          screenshotWidth: guardCssPx * 2 + 1,
          screenshotHeight: guardCssPx * 2 + 1,
          indeterminateReasons: [],
          rootPixelBounds: null,
        };
      }
      const rootSvg = svg;

      host.style.width = `${hostWidthCssPx}px`;
      svg.style.setProperty("overflow", "visible", "important");

      await document.fonts.ready;
      const resourceReasons = new Set<PaintAuditIndeterminateReason>();
      const images = [
        ...svg.querySelectorAll<SVGImageElement>("image"),
        ...svg.querySelectorAll<HTMLImageElement>("foreignObject img"),
      ].filter((image) => {
        if (image instanceof SVGImageElement) return image.href.baseVal !== "";
        return image.currentSrc !== "" || image.src !== "";
      });
      await Promise.all(
        images.map(async (image) => {
          const decode = (image as typeof image & { decode?: () => Promise<void> }).decode;
          if (typeof decode !== "function") {
            resourceReasons.add("image-decode-unavailable");
            return;
          }
          try {
            await decode.call(image);
          } catch {
            resourceReasons.add("image-decode-failed");
          }
        }),
      );
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      );

      const rawHost = host.getBoundingClientRect();
      const rawRoot = svg.getBoundingClientRect();
      const measuredRoot = quantizedRect(rawRoot);
      const rootOffsetLeft = rawRoot.left - rawHost.left;
      const rootOffsetTop = rawRoot.top - rawHost.top;
      const geometryRects: RectSnapshot[] = [];
      const inspectedStyles = new Set<Element>();
      const indeterminateReasons = new Set(resourceReasons);
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

      function inspectPotentiallyUnboundedPaint(element: Element): void {
        if (inspectedStyles.has(element)) return;
        inspectedStyles.add(element);
        const style = getComputedStyle(element);
        if (style.filter && style.filter !== "none") {
          indeterminateReasons.add("active-filter");
        }
        if (style.boxShadow && style.boxShadow !== "none") {
          indeterminateReasons.add("active-box-shadow");
        }
        if (style.textShadow && style.textShadow !== "none") {
          indeterminateReasons.add("active-text-shadow");
        }
      }

      function expandedRect(rect: RectSnapshot, amount: number): RectSnapshot {
        const left = quantize(rect.left - amount);
        const top = quantize(rect.top - amount);
        const right = quantize(rect.right + amount);
        const bottom = quantize(rect.bottom + amount);
        return {
          left,
          top,
          right,
          bottom,
          width: quantize(right - left),
          height: quantize(bottom - top),
        };
      }

      function activeMarkerReachCssPx(
        element: SVGGraphicsElement,
        style: CSSStyleDeclaration,
      ): number | null {
        let maxReach = 0;
        for (const markerReference of [style.markerStart, style.markerMid, style.markerEnd]) {
          if (!markerReference || markerReference === "none") continue;
          const match = /^url\((["']?)(#[^)"']+)\1\)$/u.exec(markerReference.trim());
          if (match === null) return null;
          const marker = document.getElementById(match[2].slice(1));
          if (marker === null) continue;
          if (!(marker instanceof SVGMarkerElement) || !rootSvg.contains(marker)) return null;
          const reach = markerReachCssPx(marker, element, style);
          if (reach === null) return null;
          maxReach = Math.max(maxReach, reach);
        }
        return maxReach;
      }

      function markerReachCssPx(
        marker: SVGMarkerElement,
        referencingElement: SVGGraphicsElement,
        referencingStyle: CSSStyleDeclaration,
      ): number | null {
        if (getComputedStyle(marker).overflow !== "hidden") return null;
        const values = [
          marker.markerWidth.baseVal.value,
          marker.markerHeight.baseVal.value,
          marker.refX.baseVal.value,
          marker.refY.baseVal.value,
        ];
        if (values.some((value) => !Number.isFinite(value))) return null;
        const [markerWidth, markerHeight, refX, refY] = values;
        if (markerWidth <= 0 || markerHeight <= 0) return 0;

        const viewBox = marker.viewBox.baseVal;
        let minX = 0;
        let minY = 0;
        let userWidth = markerWidth;
        let userHeight = markerHeight;
        let viewportScale = 1;
        if (marker.hasAttribute("viewBox")) {
          if (
            !Number.isFinite(viewBox.x) ||
            !Number.isFinite(viewBox.y) ||
            !Number.isFinite(viewBox.width) ||
            !Number.isFinite(viewBox.height) ||
            viewBox.width <= 0 ||
            viewBox.height <= 0
          ) {
            return null;
          }
          minX = viewBox.x;
          minY = viewBox.y;
          userWidth = viewBox.width;
          userHeight = viewBox.height;
          viewportScale = Math.max(
            markerWidth / viewBox.width,
            markerHeight / viewBox.height,
          );
        }

        const unitsScale =
          marker.getAttribute("markerUnits") === "userSpaceOnUse"
            ? 1
            : Number.parseFloat(referencingStyle.strokeWidth || "");
        if (!Number.isFinite(unitsScale) || unitsScale < 0) return null;
        const screenMatrix = referencingElement.getScreenCTM();
        if (screenMatrix === null) return null;
        const matrixScale = Math.hypot(
          screenMatrix.a,
          screenMatrix.b,
          screenMatrix.c,
          screenMatrix.d,
        );
        if (!Number.isFinite(matrixScale) || matrixScale < 0) return null;
        const maxDx =
          Math.max(Math.abs(minX - refX), Math.abs(minX + userWidth - refX)) *
          viewportScale;
        const maxDy =
          Math.max(Math.abs(minY - refY), Math.abs(minY + userHeight - refY)) *
          viewportScale;
        const reach = (Math.hypot(maxDx, maxDy) + 1) * unitsScale * matrixScale;
        return Number.isFinite(reach) ? reach : null;
      }

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
        let styledElement: Element | null = element;
        while (styledElement !== null) {
          inspectPotentiallyUnboundedPaint(styledElement);
          if (styledElement === svg) break;
          styledElement = styledElement.parentElement;
        }
        const rect = quantizedRect(element.getBoundingClientRect());
        const markerReach = activeMarkerReachCssPx(element, style);
        if (markerReach === null) {
          indeterminateReasons.add("marker-capture-unbounded");
        }
        const captureRect = markerReach !== null && markerReach > 0
          ? expandedRect(rect, markerReach)
          : rect;
        if (captureRect.width === 0 && captureRect.height === 0) continue;
        geometryRects.push(captureRect);

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
            inspectPotentiallyUnboundedPaint(htmlElement);
            const htmlRect = quantizedRect(htmlElement.getBoundingClientRect());
            if (htmlRect.width === 0 && htmlRect.height === 0) continue;
            geometryRects.push(htmlRect);
          }
        }
      }

      const geometryUnion = translateRect(
        unionRects(geometryRects),
        -measuredRoot.left,
        -measuredRoot.top,
      );
      const root = {
        left: 0,
        top: 0,
        right: measuredRoot.width,
        bottom: measuredRoot.height,
        width: measuredRoot.width,
        height: measuredRoot.height,
      };
      const leftPadding =
        guardCssPx + Math.ceil(Math.max(0, -(geometryUnion?.left ?? 0)));
      const topPadding =
        guardCssPx + Math.ceil(Math.max(0, -(geometryUnion?.top ?? 0)));
      const rightPadding =
        guardCssPx +
        Math.ceil(Math.max(0, (geometryUnion?.right ?? root.right) - root.right));
      const bottomPadding =
        guardCssPx +
        Math.ceil(Math.max(0, (geometryUnion?.bottom ?? root.bottom) - root.bottom));
      const rootPixelWidth = Math.ceil(root.width);
      const rootPixelHeight = Math.ceil(root.height);
      const screenshotWidth = leftPadding + rootPixelWidth + rightPadding;
      const screenshotHeight = topPadding + rootPixelHeight + bottomPadding;
      const captureExceedsLimit =
        screenshotWidth > maxCaptureDimensionCssPx ||
        screenshotHeight > maxCaptureDimensionCssPx ||
        screenshotWidth * screenshotHeight > maxCaptureAreaCssPx;
      if (captureExceedsLimit) {
        indeterminateReasons.add("capture-limit");
      } else {
        host.style.left = `${leftPadding - rootOffsetLeft}px`;
        host.style.top = `${topPadding - rootOffsetTop}px`;
        document.body.style.width = `${screenshotWidth}px`;
        document.body.style.height = `${screenshotHeight}px`;
      }

      return {
        root,
        geometryUnion,
        paintedElementCount: geometryRects.length,
        screenshotWidth,
        screenshotHeight,
        indeterminateReasons: [...indeterminateReasons],
        rootPixelBounds: captureExceedsLimit
          ? null
          : {
              left: leftPadding,
              top: topPadding,
              right: leftPadding + rootPixelWidth,
              bottom: topPadding + rootPixelHeight,
            },
      };
    },
    {
      svgSource: request.svgSource,
      quantizationEpsilon: ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
      guardCssPx: ROOT_VIEWPORT_PAINT_GUARD_CSS_PX,
      hostWidthCssPx: ROOT_VIEWPORT_HOST_WIDTH_CSS_PX,
      maxCaptureDimensionCssPx: ROOT_VIEWPORT_MAX_CAPTURE_DIMENSION_CSS_PX,
      maxCaptureAreaCssPx: ROOT_VIEWPORT_MAX_CAPTURE_AREA_CSS_PX,
    },
  );
}

async function auditScreenshotPixels(
  page: Page,
  pngBase64: string,
  root: NonNullable<MountedSvgSnapshot["rootPixelBounds"]>,
): Promise<PixelAuditSnapshot> {
  return page.evaluate(
    async ({ pngBase64, root, quantizationEpsilon }) => {
      type EdgeAccumulator = {
        paintedPixelCount: number;
        bounds: PixelBounds | null;
        reachesAuditBoundary: boolean;
      };

      function quantize(value: number): number {
        return Math.round(value / quantizationEpsilon) * quantizationEpsilon;
      }

      function recordPixel(
        accumulator: EdgeAccumulator,
        x: number,
        y: number,
        reachesAuditBoundary: boolean,
      ): void {
        accumulator.paintedPixelCount += 1;
        accumulator.reachesAuditBoundary ||= reachesAuditBoundary;
        if (accumulator.bounds === null) {
          accumulator.bounds = { minX: x, minY: y, maxX: x, maxY: y };
          return;
        }
        accumulator.bounds.minX = Math.min(accumulator.bounds.minX, x);
        accumulator.bounds.minY = Math.min(accumulator.bounds.minY, y);
        accumulator.bounds.maxX = Math.max(accumulator.bounds.maxX, x);
        accumulator.bounds.maxY = Math.max(accumulator.bounds.maxY, y);
      }

      function rectFromBounds(bounds: PixelBounds | null): RectSnapshot | null {
        if (bounds === null) return null;
        const left = quantize(bounds.minX - root.left);
        const top = quantize(bounds.minY - root.top);
        const right = quantize(bounds.maxX + 1 - root.left);
        const bottom = quantize(bounds.maxY + 1 - root.top);
        return {
          left,
          top,
          right,
          bottom,
          width: quantize(right - left),
          height: quantize(bottom - top),
        };
      }

      const image = new Image();
      image.src = `data:image/png;base64,${pngBase64}`;
      await image.decode();
      const canvas = document.createElement("canvas");
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      if (context === null) throw new Error("Unable to create root viewport pixel audit canvas.");
      context.drawImage(image, 0, 0);
      const edges: Record<RootViewportEdge, EdgeAccumulator> = {
        top: { paintedPixelCount: 0, bounds: null, reachesAuditBoundary: false },
        right: { paintedPixelCount: 0, bounds: null, reachesAuditBoundary: false },
        bottom: { paintedPixelCount: 0, bounds: null, reachesAuditBoundary: false },
        left: { paintedPixelCount: 0, bounds: null, reachesAuditBoundary: false },
      };
      const pixelKeys = new Set<string>();

      const strips: Array<{
        edge: RootViewportEdge;
        x: number;
        y: number;
        width: number;
        height: number;
      }> = [
        { edge: "top", x: 0, y: 0, width: canvas.width, height: root.top },
        {
          edge: "right",
          x: root.right,
          y: 0,
          width: canvas.width - root.right,
          height: canvas.height,
        },
        {
          edge: "bottom",
          x: 0,
          y: root.bottom,
          width: canvas.width,
          height: canvas.height - root.bottom,
        },
        {
          edge: "left",
          x: 0,
          y: 0,
          width: root.left,
          height: canvas.height,
        },
      ];

      for (const strip of strips) {
        if (strip.width <= 0 || strip.height <= 0) continue;
        const pixels = context.getImageData(
          strip.x,
          strip.y,
          strip.width,
          strip.height,
        ).data;
        for (let y = 0; y < strip.height; y += 1) {
          for (let x = 0; x < strip.width; x += 1) {
            if (pixels[(y * strip.width + x) * 4 + 3] === 0) continue;
            const pageX = strip.x + x;
            const pageY = strip.y + y;
            const atAuditBoundary =
              pageX === 0 ||
              pageY === 0 ||
              pageX === canvas.width - 1 ||
              pageY === canvas.height - 1;
            recordPixel(edges[strip.edge], pageX, pageY, atAuditBoundary);
            pixelKeys.add(`${pageX - root.left},${pageY - root.top}`);
          }
        }
      }

      return {
        pixelKeys: [...pixelKeys],
        edges: strips.map(({ edge }) => ({
          edge,
          paintedPixelCount: edges[edge].paintedPixelCount,
          rect: rectFromBounds(edges[edge].bounds),
          reachesAuditBoundary: edges[edge].reachesAuditBoundary,
        })),
      };
    },
    {
      pngBase64,
      root,
      quantizationEpsilon: ROOT_VIEWPORT_QUANTIZATION_EPSILON_CSS_PX,
    },
  );
}
