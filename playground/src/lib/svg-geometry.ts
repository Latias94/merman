export interface SvgDimensions {
  width: number;
  height: number;
}

export interface NormalizedSvgDimensions extends SvgDimensions {
  svg: string;
}

interface ParsedSvgRoot {
  root: Element;
}

interface ViewBox {
  width: number;
  height: number;
}

export function parseSvgDimensions(svg: string): SvgDimensions | null {
  const parsed = parseSvgRoot(svg);
  if (!parsed) return null;

  return resolveSvgDimensions(parsed.root);
}

export function normalizeSvgDimensions(
  svg: string
): NormalizedSvgDimensions | null {
  const parsed = parseSvgRoot(svg);
  if (!parsed) return null;

  const dimensions = resolveSvgDimensions(parsed.root);
  if (!dimensions) return null;

  parsed.root.setAttribute("width", formatSvgNumber(dimensions.width));
  parsed.root.setAttribute("height", formatSvgNumber(dimensions.height));

  return {
    svg: new XMLSerializer().serializeToString(parsed.root),
    ...dimensions,
  };
}

function parseSvgRoot(svg: string): ParsedSvgRoot | null {
  const parser = new DOMParser();
  const doc = parser.parseFromString(svg, "image/svg+xml");
  const root = doc.documentElement;

  if (
    root.localName.toLowerCase() !== "svg" ||
    doc.querySelector("parsererror")
  ) {
    return null;
  }

  return { root };
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
