import { createHash } from "node:crypto";

import type { Locator, Page } from "@playwright/test";

const MAX_OBSERVATION_PIXELS = 4_000_000;

export interface ThemeTextSurfaceProbe {
  role: string;
  occurrenceId: string;
  textSelector: string;
  surfaceSelector: string;
  surfacePaintProperty?: "fill" | "background-color";
}

export interface ThemeTextSurfaceBounds {
  xMilliPx: number;
  yMilliPx: number;
  widthMilliPx: number;
  heightMilliPx: number;
}

export interface ThemeTextSurfaceObservation {
  role: string;
  occurrenceId: string;
  finalTextPaint: [number, number, number];
  backgroundPaint: [number, number, number];
  finalFontIdentity: string;
  textBounds: ThemeTextSurfaceBounds;
  surfaceBounds: ThemeTextSurfaceBounds;
  artifactDigest: string;
}

interface BrowserTextSurfaceObservation {
  role: string;
  occurrenceId: string;
  finalTextPaint: string;
  backgroundPaint: string;
  finalFontIdentity: string;
  textBounds: ThemeTextSurfaceBounds;
  surfaceBounds: ThemeTextSurfaceBounds;
}

/**
 * Observes the browser-owned cascade winner and geometry for exact semantic occurrences.
 *
 * This helper is intentionally not a CSS engine or a heuristic surface finder. The family proof
 * must provide exact text and surface selectors; Rust owns the bounded contract validation and
 * receipt sealing.
 */
export async function observeThemeTextSurfaces(
  page: Page,
  svgMarkup: string,
  probes: readonly ThemeTextSurfaceProbe[],
): Promise<ThemeTextSurfaceObservation[]> {
  await page.setContent(svgMarkup);
  return observeMountedThemeTextSurfaces(page.locator("svg"), probes);
}

/** Observes an SVG already mounted by the production Playground/runtime path. */
export async function observeMountedThemeTextSurfaces(
  svgLocator: Locator,
  probes: readonly ThemeTextSurfaceProbe[],
): Promise<ThemeTextSurfaceObservation[]> {
  const svgMarkup = await svgLocator.evaluate((svg) => {
    if (!(svg instanceof SVGSVGElement)) {
      throw new Error("Theme text/surface observation requires one mounted SVG root.");
    }
    return new XMLSerializer().serializeToString(svg);
  });
  const artifactDigest = createHash("sha256").update(svgMarkup).digest("hex");
  const browserObservations = await svgLocator.evaluate(
    (svg, requestedProbes): BrowserTextSurfaceObservation[] => {
      const bounds = (element: Element): ThemeTextSurfaceBounds => {
        let rect = element.getBoundingClientRect();
        if (element instanceof SVGTextContentElement) {
          // Measure laid-out glyph cells instead of browser-dependent whole-element bounds.
          // SVG 2 defines getExtentOfChar in the element's user coordinate system.
          const transform = element.getScreenCTM();
          const count = element.getNumberOfChars();
          if (!transform || count === 0) {
            throw new Error("Terminal text requires visible character geometry.");
          }
          let left = Infinity;
          let top = Infinity;
          let right = -Infinity;
          let bottom = -Infinity;
          for (let index = 0; index < count; index += 1) {
            const cell = element.getExtentOfChar(index);
            for (const [x, y] of [
              [cell.x, cell.y], [cell.x + cell.width, cell.y],
              [cell.x, cell.y + cell.height], [cell.x + cell.width, cell.y + cell.height],
            ]) {
              const point = new DOMPoint(x, y).matrixTransform(transform);
              left = Math.min(left, point.x);
              top = Math.min(top, point.y);
              right = Math.max(right, point.x);
              bottom = Math.max(bottom, point.y);
            }
          }
          rect = new DOMRect(left, top, right - left, bottom - top);
        }
        return {
          xMilliPx: Math.round(rect.x * 1_000),
          yMilliPx: Math.round(rect.y * 1_000),
          widthMilliPx: Math.round(rect.width * 1_000),
          heightMilliPx: Math.round(rect.height * 1_000),
        };
      };
      const exactlyOne = (selector: string, kind: string): Element => {
        if (selector === ":scope" && kind === "surface") {
          return svg;
        }
        const matches = svg.querySelectorAll(selector);
        if (matches.length !== 1) {
          throw new Error(
            `Expected one terminal ${kind} for ${selector}; got ${matches.length}`,
          );
        }
        return matches[0];
      };

      const requireVisibleOpaque = (element: Element, kind: string, selector: string) => {
        if (getComputedStyle(element).visibility !== "visible") {
          throw new Error(`Terminal ${kind} must be visible and opaque: ${selector}`);
        }
        for (let ancestor: Element | null = element; ancestor;) {
          const style = getComputedStyle(ancestor);
          if (
            style.display === "none" ||
            Number(style.opacity) !== 1 || style.contentVisibility === "hidden"
          ) {
            throw new Error(`Terminal ${kind} must be visible and opaque: ${selector}`);
          }
          if (kind === "text" && (style.filter !== "none" || style.mixBlendMode !== "normal")) {
            throw new Error(`Terminal ${kind} requires unfiltered, normally blended paint: ${selector}`);
          }
          if (kind === "text" && style.maskImage !== "none") {
            throw new Error(`Terminal text requires unmasked paint: ${selector}`);
          }
          const root = ancestor.getRootNode();
          ancestor = ancestor.parentElement ?? (root instanceof ShadowRoot ? root.host : null);
        }
      };

      return requestedProbes.map((probe) => {
        const text = exactlyOne(probe.textSelector, "text");
        const surface = exactlyOne(probe.surfaceSelector, "surface");
        const textStyle = getComputedStyle(text);
        const surfaceStyle = getComputedStyle(surface);
        if (!text.textContent?.trim()) {
          throw new Error(`Terminal text must be visible: ${probe.textSelector}`);
        }
        requireVisibleOpaque(text, "text", probe.textSelector);
        requireVisibleOpaque(surface, "surface", probe.surfaceSelector);
        const textNodes = document.createTreeWalker(text, NodeFilter.SHOW_TEXT);
        const textParents = new Set<Element>([text]);
        for (let node = textNodes.nextNode(); node; node = textNodes.nextNode()) {
          if (node.textContent?.trim() && node.parentElement) textParents.add(node.parentElement);
        }
        for (const parent of textParents) {
          requireVisibleOpaque(parent, "text", probe.textSelector);
          const style = getComputedStyle(parent);
          if (parent instanceof SVGElement && Number(style.fillOpacity) !== 1) {
            throw new Error(`Terminal text fill must be visible and opaque: ${probe.textSelector}`);
          }
          if (parent instanceof SVGElement && style.stroke !== "none" &&
            Number(style.strokeOpacity) !== 0 && Number.parseFloat(style.strokeWidth) > 0) {
            throw new Error(`Terminal text requires fill-only paint: ${probe.textSelector}`);
          }
          const paint = parent instanceof SVGElement ? style.fill : style.color;
          const reportedPaint = text instanceof SVGElement ? textStyle.fill : textStyle.color;
          if (paint !== reportedPaint || style.fontFamily !== textStyle.fontFamily) {
            throw new Error(`Terminal text requires uniform resolved paint and font: ${probe.textSelector}`);
          }
        }
        const surfaceUsesFill = probe.surfacePaintProperty !== "background-color" &&
          surface instanceof SVGElement;
        if (surfaceUsesFill && Number(surfaceStyle.fillOpacity) !== 1) {
          throw new Error(`Terminal surface fill must be visible and opaque: ${probe.surfaceSelector}`);
        }
        const textBounds = bounds(text);
        const surfaceBounds = bounds(surface);
        if ([textBounds, surfaceBounds].some((rect) => rect.widthMilliPx <= 0 || rect.heightMilliPx <= 0)) {
          throw new Error(`Terminal text and surface require positive area: ${probe.textSelector}`);
        }
        return {
          role: probe.role,
          occurrenceId: probe.occurrenceId,
          finalTextPaint:
            text instanceof SVGElement ? textStyle.fill : textStyle.color,
          backgroundPaint:
            probe.surfacePaintProperty === "background-color"
              ? surfaceStyle.backgroundColor
              : surface instanceof SVGElement
                ? surfaceStyle.fill
                : surfaceStyle.backgroundColor,
          finalFontIdentity: textStyle.fontFamily,
          textBounds,
          surfaceBounds,
        };
      });
    },
    probes,
  );

  const observations = browserObservations.map((observation) => ({
    ...observation,
    finalTextPaint: parseComputedRgb(observation.finalTextPaint),
    backgroundPaint: parseComputedRgb(observation.backgroundPaint),
    artifactDigest,
  }));

  // Compare paint at the actual ink pixels, not elsewhere in the surface's bounding box.
  // A selected surface must change every text-suppressed ink pixel and supply its reported
  // opaque RGB there. Equal underlying paint is deliberately insufficient provenance.
  const rootBounds = await svgLocator.boundingBox();
  if (!rootBounds || Math.ceil(rootBounds.width) * Math.ceil(rootBounds.height) > MAX_OBSERVATION_PIXELS) {
    throw new Error("Theme text/surface observation exceeds the bounded pixel area.");
  }
  for (const [index, probe] of probes.entries()) {
    const text = svgLocator.locator(probe.textSelector);
    const surface = probe.surfaceSelector === ":scope" ? svgLocator : svgLocator.locator(probe.surfaceSelector);
    const textStyle = await text.getAttribute("style");
    const surfaceStyle = await surface.getAttribute("style");
    const visible = await svgLocator.screenshot({ animations: "disabled", caret: "hide", scale: "css" });
    let textSuppressed: Buffer;
    let bothSuppressed: Buffer;
    try {
      await text.evaluate((element) => {
        (element as SVGElement | HTMLElement).style.setProperty("opacity", "0", "important");
      });
      textSuppressed = await svgLocator.screenshot({ animations: "disabled", caret: "hide", scale: "css" });
      await surface.evaluate((element, paintProperty) => {
        const property = paintProperty === "background-color" || element instanceof HTMLElement
          ? "background-color" : "fill";
        (element as SVGElement | HTMLElement).style.setProperty(property, "transparent", "important");
      }, probe.surfacePaintProperty);
      bothSuppressed = await svgLocator.screenshot({ animations: "disabled", caret: "hide", scale: "css" });
    } finally {
      for (const [terminal, style] of [[surface, surfaceStyle], [text, textStyle]] as const) {
        await terminal.evaluate((element, authoredStyle) => {
          if (authoredStyle === null) element.removeAttribute("style");
          else element.setAttribute("style", authoredStyle);
        }, style);
      }
    }
    const evidence = await svgLocator.evaluate(async (_svg, input) => {
      const pixels = [];
      let width = 0;
      let height = 0;
      for (const encoded of input.images) {
        const bytes = Uint8Array.from(atob(encoded), (character) => character.charCodeAt(0));
        const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
        try {
          if (bitmap.width * bitmap.height > input.maxPixels ||
            (width !== 0 && (bitmap.width !== width || bitmap.height !== height))) {
            throw new Error("Theme text/surface observation requires bounded, matching pixel geometry.");
          }
          width = bitmap.width;
          height = bitmap.height;
          const canvas = document.createElement("canvas");
          canvas.width = width;
          canvas.height = height;
          const context = canvas.getContext("2d", { willReadFrequently: true });
          if (!context) throw new Error("Theme text/surface observation requires browser pixels.");
          context.drawImage(bitmap, 0, 0);
          pixels.push(context.getImageData(0, 0, width, height).data);
        } finally {
          bitmap.close();
        }
      }
      const [original, withoutText, withoutBoth] = pixels;
      const samePixel = (left: Uint8ClampedArray, right: Uint8ClampedArray, offset: number) =>
        left[offset] === right[offset] && left[offset + 1] === right[offset + 1] &&
        left[offset + 2] === right[offset + 2] && left[offset + 3] === right[offset + 3];
      let inkPixels = 0;
      for (let offset = 0; offset < original.length; offset += 4) {
        if (samePixel(original, withoutText, offset)) continue;
        inkPixels += 1;
        if (withoutText[offset + 3] !== 255 ||
          input.backgroundPaint.some((channel, component) => Math.abs(channel - withoutText[offset + component]) > 1) ||
          samePixel(withoutText, withoutBoth, offset)) {
          return { inkPixels, backgroundProven: false };
        }
      }
      return { inkPixels, backgroundProven: true };
    }, {
      images: [visible, textSuppressed, bothSuppressed].map((bytes) => bytes.toString("base64")),
      backgroundPaint: observations[index].backgroundPaint,
      maxPixels: MAX_OBSERVATION_PIXELS,
    });
    if (evidence.inkPixels === 0) {
      throw new Error(`Terminal text must contribute visible pixels: ${probe.textSelector}`);
    }
    if (!evidence.backgroundProven) {
      throw new Error(`Terminal surface must supply the opaque background at every visible text pixel: ${probe.surfaceSelector}`);
    }
  }

  return observations;
}

export function textSurfaceContrastRatio(
  observation: ThemeTextSurfaceObservation,
): number {
  const luminance = (rgb: [number, number, number]): number => {
    const [red, green, blue] = rgb.map(linearChannel);
    return red * 0.2126 + green * 0.7152 + blue * 0.0722;
  };
  const foreground = luminance(observation.finalTextPaint);
  const background = luminance(observation.backgroundPaint);
  return (
    (Math.max(foreground, background) + 0.05) /
    (Math.min(foreground, background) + 0.05)
  );
}

export function containsTextBounds(
  observation: ThemeTextSurfaceObservation,
  toleranceMilliPx: number,
): boolean {
  if (
    !Number.isFinite(toleranceMilliPx) || toleranceMilliPx < 0 ||
    [observation.textBounds, observation.surfaceBounds].some((bounds) =>
      Object.values(bounds).some((value) => !Number.isFinite(value)) ||
      bounds.widthMilliPx <= 0 || bounds.heightMilliPx <= 0
    )
  ) {
    return false;
  }
  const textRight =
    observation.textBounds.xMilliPx + observation.textBounds.widthMilliPx;
  const textBottom =
    observation.textBounds.yMilliPx + observation.textBounds.heightMilliPx;
  const surfaceRight =
    observation.surfaceBounds.xMilliPx + observation.surfaceBounds.widthMilliPx;
  const surfaceBottom =
    observation.surfaceBounds.yMilliPx +
    observation.surfaceBounds.heightMilliPx;
  return (
    observation.textBounds.xMilliPx >=
      observation.surfaceBounds.xMilliPx - toleranceMilliPx &&
    observation.textBounds.yMilliPx >=
      observation.surfaceBounds.yMilliPx - toleranceMilliPx &&
    textRight <= surfaceRight + toleranceMilliPx &&
    textBottom <= surfaceBottom + toleranceMilliPx
  );
}

function parseComputedRgb(value: string): [number, number, number] {
  const match = value.match(
    /^rgba?\(\s*(\d+(?:\.\d+)?)(?:\s*,\s*|\s+)(\d+(?:\.\d+)?)(?:\s*,\s*|\s+)(\d+(?:\.\d+)?)(?:(?:\s*,\s*|\s*\/\s*)(\d+(?:\.\d+)?))?\s*\)$/u,
  );
  if (!match) {
    throw new Error(`Unsupported terminal computed color: ${value}`);
  }
  if (match[4] !== undefined && Number(match[4]) !== 1) {
    throw new Error(
      `Terminal text/surface observation requires an opaque resolved color: ${value}`,
    );
  }
  const channels = match.slice(1, 4).map((channel) => Number(channel));
  if (
    channels.some(
      (channel) => !Number.isFinite(channel) || channel < 0 || channel > 255,
    )
  ) {
    throw new Error(`Terminal computed color is outside the RGB range: ${value}`);
  }
  return channels.map((channel) => Math.round(channel)) as [
    number,
    number,
    number,
  ];
}

function linearChannel(channel: number): number {
  const value = channel / 255;
  return value <= 0.04045
    ? value / 12.92
    : ((value + 0.055) / 1.055) ** 2.4;
}
