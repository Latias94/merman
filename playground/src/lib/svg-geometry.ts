import { prepareNavigableSvgForDomMount } from "@mermanjs/web";

import {
  assertNavigableInlineSvgArtifact,
  projectNavigableInlineSvg,
  type NavigableInlineSvg,
} from "../runtime/render-artifact.ts";
import type {
  RasterExportPlan,
  RasterExportSource,
  RasterSourceBackground,
} from "./raster-export-plan.ts";

export interface SvgDimensions {
  width: number;
  height: number;
}

interface PreparedSvgPreview {
  readonly dimensions: SvgDimensions | null;
  readonly rootSizing: "responsive" | "intrinsic" | "unknown";
  takeNode(): Element;
}

interface ParsedSvgRoot {
  root: Element;
}

interface ViewBox {
  width: number;
  height: number;
}

const SVG_NAMESPACE = "http://www.w3.org/2000/svg";
const FALLBACK_RASTER_WIDTH = 300;
const FALLBACK_RASTER_HEIGHT = 150;

export function inspectSvgForRasterExport(
  artifact: NavigableInlineSvg,
): Readonly<RasterExportSource> {
  assertNavigableInlineSvgArtifact(artifact);
  const parsed = parseSvgRoot(artifact.svg);
  if (!parsed) {
    return Object.freeze({
      width: FALLBACK_RASTER_WIDTH,
      height: FALLBACK_RASTER_HEIGHT,
      originalBackground: null,
    });
  }
  const dimensions = resolveSvgDimensions(parsed.root) ?? {
    width: FALLBACK_RASTER_WIDTH,
    height: FALLBACK_RASTER_HEIGHT,
  };
  return Object.freeze({
    ...dimensions,
    originalBackground: inspectRootBackground(parsed.root),
  });
}

export function prepareSvgForResponsivePreview(
  artifact: NavigableInlineSvg,
  ownerDocument: Document
): PreparedSvgPreview | null {
  assertNavigableInlineSvgArtifact(artifact);
  const Parser = ownerDocument.defaultView?.DOMParser ?? globalThis.DOMParser;
  if (!Parser) return null;
  const parsed = parseSvgRootForHtmlMount(new Parser(), artifact.svg);
  if (!parsed) return null;
  const root = ownerDocument.importNode(parsed.root, true) as Element;

  removeDefaultPreviewBackground(root);
  const dimensions = resolveSvgDimensions(root);
  const hasViewBox = parseViewBox(root.getAttribute("viewBox")) !== null;
  let rootSizing: PreparedSvgPreview["rootSizing"] = "unknown";
  if (dimensions && hasViewBox) {
    rootSizing = "responsive";
    root.setAttribute("width", "100%");
    root.setAttribute("height", "100%");
    appendRootStyle(
      root,
      "display:block;width:100%!important;height:100%!important;max-width:100%!important;max-height:100%!important"
    );
  } else if (dimensions) {
    rootSizing = "intrinsic";
    const width = formatSvgNumber(dimensions.width);
    const height = formatSvgNumber(dimensions.height);
    root.setAttribute("width", width);
    root.setAttribute("height", height);
    appendRootStyle(
      root,
      `display:block;width:${width}px!important;height:${height}px!important;max-width:none!important;max-height:none!important`
    );
  }

  const template = root;
  template.remove();
  return Object.freeze({
    dimensions: dimensions ? Object.freeze({ ...dimensions }) : null,
    rootSizing,
    takeNode(): Element {
      const node = template.parentNode
        ? (template.cloneNode(true) as Element)
        : template;
      prepareNavigableSvgForDomMount(
        artifact.mountAdmission,
        node,
        ownerDocument
      );
      return node;
    },
  });
}

export function prepareSvgForRasterExport(
  artifact: NavigableInlineSvg,
  plan: Readonly<RasterExportPlan>,
): NavigableInlineSvg | null {
  assertNavigableInlineSvgArtifact(artifact);
  const parsed = parseSvgRoot(artifact.svg);
  if (!parsed) return null;
  const intrinsicDimensions = resolveSvgDimensions(parsed.root) ?? {
    width: FALLBACK_RASTER_WIDTH,
    height: FALLBACK_RASTER_HEIGHT,
  };

  ensureViewBox(parsed.root, intrinsicDimensions);
  const width = formatSvgNumber(plan.outputWidth);
  const height = formatSvgNumber(plan.outputHeight);
  parsed.root.setAttribute("width", width);
  parsed.root.setAttribute("height", height);
  appendRootStyle(
    parsed.root,
    `display:block;width:${width}px!important;height:${height}px!important;max-width:none!important;max-height:none!important`,
  );
  if (plan.background.mode !== "original") {
    setRootBackground(
      parsed.root,
      plan.background.color ?? "transparent",
    );
  }
  return projectNavigableInlineSvg(
    new XMLSerializer().serializeToString(parsed.root),
  );
}

function parseSvgRoot(svg: string): ParsedSvgRoot | null {
  const parser = new DOMParser();
  const xml = parser.parseFromString(svg, "image/svg+xml");
  const xmlRoot = xml.documentElement;

  if (isSvgRoot(xmlRoot) && !xml.querySelector("parsererror")) {
    return { root: xmlRoot };
  }

  return parseSvgRootAsHtml(parser, svg);
}

function parseSvgRootForHtmlMount(
  parser: DOMParser,
  svg: string
): ParsedSvgRoot | null {
  return parseSvgRootAsHtml(parser, svg);
}

function parseSvgRootAsHtml(
  parser: DOMParser,
  svg: string
): ParsedSvgRoot | null {
  // HTML parsing preserves the namespace and case fixups of the former innerHTML sink,
  // including XHTML descendants inside foreignObject.
  const html = parser.parseFromString(svg, "text/html");
  const htmlRoot = html.body?.firstElementChild ?? null;
  if (html.body?.childElementCount !== 1 || !isSvgRoot(htmlRoot)) {
    return null;
  }

  return { root: htmlRoot };
}

function isSvgRoot(root: Element | null): root is Element {
  return (
    root?.localName.toLowerCase() === "svg" &&
    root.namespaceURI === SVG_NAMESPACE
  );
}

function ensureViewBox(root: Element, dimensions: SvgDimensions): void {
  if (parseViewBox(root.getAttribute("viewBox"))) return;
  root.setAttribute(
    "viewBox",
    `0 0 ${formatSvgNumber(dimensions.width)} ${formatSvgNumber(dimensions.height)}`
  );
}

function appendRootStyle(root: Element, declarations: string): void {
  const existing = root.getAttribute("style")?.trim();
  const prefix = existing ? `${existing.replace(/;+$/u, "")};` : "";
  root.setAttribute("style", `${prefix}${declarations}`);
}

function removeDefaultPreviewBackground(root: Element): void {
  const style = (root as SVGElement).style;
  if (style?.backgroundColor.trim().toLowerCase() === "white") {
    style.removeProperty("background-color");
  }
}

function setRootBackground(root: Element, color: string): void {
  const style = (root as SVGElement).style;
  if (style?.setProperty) {
    style.removeProperty("background");
    style.setProperty("background-color", color, "important");
    style.setProperty("background-image", "none", "important");
    return;
  }
  appendRootStyle(
    root,
    `background-color:${color}!important;background-image:none!important`,
  );
}

function inspectRootBackground(root: Element): RasterSourceBackground | null {
  const color = (root as SVGElement).style?.backgroundColor.trim();
  if (!color) return null;
  const canvas = document.createElement("canvas");
  canvas.width = 1;
  canvas.height = 1;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) return null;
  if (typeof CSS !== "undefined" && !CSS.supports("color", color)) return null;
  context.clearRect(0, 0, 1, 1);
  context.fillStyle = color;
  context.fillRect(0, 0, 1, 1);
  const alpha = context.getImageData(0, 0, 1, 1).data[3] ?? 0;
  return Object.freeze({ color, opaque: alpha === 255 });
}

function resolveSvgDimensions(root: Element): SvgDimensions | null {
  const viewBox = parseViewBox(root.getAttribute("viewBox"));
  const explicitWidth = parseSvgLength(root.getAttribute("width"));
  const explicitHeight = parseSvgLength(root.getAttribute("height"));
  const maxWidth = parseStyleMaxWidth(root.getAttribute("style"));

  let width = explicitWidth;
  let height = explicitHeight;

  if (viewBox && !width && !height) {
    width = maxWidth ?? viewBox.width;
    height = width * (viewBox.height / viewBox.width);
  }
  if (viewBox && width && !height) {
    height = width * (viewBox.height / viewBox.width);
  }
  if (viewBox && height && !width) {
    width = height * (viewBox.width / viewBox.height);
  }

  if (!isPositiveFinite(width) || !isPositiveFinite(height)) {
    return null;
  }

  return { width, height };
}

function parseViewBox(value: string | null): ViewBox | null {
  if (!value) return null;

  const parts = value
    .trim()
    .split(/[\s,]+/)
    .map((part) => Number(part));

  if (
    parts.length !== 4 ||
    parts.some((part) => !Number.isFinite(part)) ||
    parts[2] <= 0 ||
    parts[3] <= 0
  ) {
    return null;
  }

  return {
    width: parts[2],
    height: parts[3],
  };
}

function parseSvgLength(value: string | null): number | undefined {
  if (!value) return undefined;

  const trimmed = value.trim();
  if (trimmed.endsWith("%")) return undefined;

  const match = trimmed.match(
    /^([+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?)(px)?$/i
  );
  if (!match) return undefined;

  const parsed = Number(match[1]);
  return isPositiveFinite(parsed) ? parsed : undefined;
}

function parseStyleMaxWidth(style: string | null): number | undefined {
  if (!style) return undefined;

  for (const declaration of style.split(";")) {
    const [name, value] = declaration.split(":", 2);
    if (name?.trim().toLowerCase() !== "max-width") continue;

    return parseSvgLength(value);
  }

  return undefined;
}

function formatSvgNumber(value: number): string {
  return Number(value.toFixed(6)).toString();
}

function isPositiveFinite(value: number | undefined): value is number {
  return value !== undefined && Number.isFinite(value) && value > 0;
}
