import { createHash } from "node:crypto";

import type { Locator, Page } from "@playwright/test";

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

      return requestedProbes.map((probe) => {
        const text = exactlyOne(probe.textSelector, "text");
        const surface = exactlyOne(probe.surfaceSelector, "surface");
        const textStyle = getComputedStyle(text);
        const surfaceStyle = getComputedStyle(surface);
        if (textStyle.visibility !== "visible" || !text.textContent?.trim()) {
          throw new Error(`Terminal text must be visible: ${probe.textSelector}`);
        }
        for (let ancestor: Element | null = text; ancestor;) {
          const style = getComputedStyle(ancestor);
          if (
            style.display === "none" || Number(style.opacity) !== 1 ||
            style.contentVisibility === "hidden"
          ) {
            throw new Error(`Terminal text must be visible and opaque: ${probe.textSelector}`);
          }
          const root = ancestor.getRootNode();
          ancestor = ancestor.parentElement ?? (root instanceof ShadowRoot ? root.host : null);
        }
        if (text instanceof SVGElement && Number(textStyle.fillOpacity) !== 1) {
          throw new Error(`Terminal text fill must be visible and opaque: ${probe.textSelector}`);
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

  // Geometry and computed paint alone cannot prove that clipping or occlusion left any ink.
  // Compare actual browser pixels with only this occurrence's paint suppressed, then restore
  // its exact authored style before returning the observation of the original artifact.
  for (const probe of probes) {
    const text = svgLocator.locator(probe.textSelector);
    const before = await svgLocator.screenshot({ animations: "disabled", caret: "hide" });
    const authoredStyle = await text.getAttribute("style");
    let after: Buffer;
    try {
      await text.evaluate((element) => {
        if (!(element instanceof SVGElement || element instanceof HTMLElement)) {
          throw new Error("Terminal text requires a styleable element.");
        }
        element.style.setProperty("opacity", "0", "important");
      });
      after = await svgLocator.screenshot({ animations: "disabled", caret: "hide" });
    } finally {
      await text.evaluate((element, style) => {
        if (style === null) element.removeAttribute("style");
        else element.setAttribute("style", style);
      }, authoredStyle);
    }
    if (before.equals(after)) {
      throw new Error(`Terminal text must contribute visible pixels: ${probe.textSelector}`);
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
