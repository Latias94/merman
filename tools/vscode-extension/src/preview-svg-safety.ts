const ACTIVE_SVG_ELEMENT_PATTERN = /<\s*(script|foreignObject|iframe|object|embed|audio|video|canvas)\b/i;
const EVENT_ATTRIBUTE_PATTERN = /\s(on[a-z][\w:-]*)\s*=/i;
const JAVASCRIPT_URL_PATTERN = /\b(?:href|xlink:href|src)\s*=\s*(['"]?)\s*javascript:/i;
const HTML_DATA_URL_PATTERN = /\b(?:href|xlink:href|src)\s*=\s*(['"]?)\s*data\s*:\s*text\/html/i;
const EXTERNAL_RESOURCE_PATTERN =
  /\b(?:href|xlink:href|src)\s*=\s*(['"]?)\s*(?:https?:)?\/\//i;

export function assertSafePreviewSvg(svg: string): void {
  const trimmed = svg.trimStart();
  if (!trimmed.startsWith("<svg")) {
    throw new Error("Preview renderer returned non-SVG output.");
  }
  if (ACTIVE_SVG_ELEMENT_PATTERN.test(svg)) {
    throw new Error("Preview renderer returned SVG with active embedded content.");
  }
  if (EVENT_ATTRIBUTE_PATTERN.test(svg)) {
    throw new Error("Preview renderer returned SVG with event handler attributes.");
  }
  if (JAVASCRIPT_URL_PATTERN.test(svg) || HTML_DATA_URL_PATTERN.test(svg)) {
    throw new Error("Preview renderer returned SVG with unsafe URL attributes.");
  }
  if (EXTERNAL_RESOURCE_PATTERN.test(svg)) {
    throw new Error("Preview renderer returned SVG with external resource references.");
  }
}
