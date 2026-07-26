import { createHash } from "node:crypto";

import { DOMParser } from "@xmldom/xmldom";

const GEOMETRY_ATTRIBUTES = new Set([
  "cx",
  "cy",
  "d",
  "dx",
  "dy",
  "height",
  "pathLength",
  "points",
  "r",
  "rx",
  "ry",
  "startOffset",
  "stroke-dasharray",
  "stroke-dashoffset",
  "stroke-width",
  "textLength",
  "transform",
  "viewBox",
  "width",
  "x",
  "x1",
  "x2",
  "y",
  "y1",
  "y2",
]);
const NUMERIC_STYLE_PROPERTIES = new Set([
  "height",
  "max-height",
  "max-width",
  "min-height",
  "min-width",
  "width",
]);
const FLOAT_TOKEN = /[-+]?(?:(?:\d+\.\d*|\.\d+|\d+)(?:[eE][-+]?\d+)?)/g;
const NUMBER_MARKER = "<number>";

export function svgTransportEvidence(svg) {
  const document = parseSvg(svg);
  const geometry = [];
  const structure = canonicalNode(document.documentElement, "0", geometry);
  return {
    structure_sha256: digest(JSON.stringify(structure)),
    geometry_sha256: digest(JSON.stringify(geometry)),
  };
}

export function equivalentTransportOutcome(left, right) {
  if (!left || !right || !equivalentOperationEvidence(left.semantic, right.semantic)) return false;
  if (left.ok !== right.ok) return false;
  if (left.ok) {
    return (
      left.operation_id === right.operation_id &&
      left.media_type === right.media_type &&
      left.svg_structure_sha256 === right.svg_structure_sha256
    );
  }
  return equivalentErrorEvidence(left, right);
}

function parseSvg(svg) {
  if (typeof svg !== "string") throw new TypeError("SVG transport comparison requires a string.");
  const errors = [];
  let document;
  try {
    document = new DOMParser({
      onError: (level, message) => errors.push(`${level}: ${message}`),
    }).parseFromString(svg, "image/svg+xml");
  } catch (cause) {
    throw new Error(
      `Cannot inspect rendered SVG: ${cause instanceof Error ? cause.message : String(cause)}`,
    );
  }
  if (
    errors.length > 0 ||
    !document.documentElement ||
    document.documentElement.nodeName === "parsererror" ||
    document.documentElement.localName !== "svg"
  ) {
    throw new Error(`Cannot inspect rendered SVG: ${errors[0] ?? "missing root element"}`);
  }
  return document;
}

function canonicalNode(node, path, geometry) {
  if (node.nodeType === node.ELEMENT_NODE) {
    const canonicalAttributes = Array.from(node.attributes ?? [])
      .map((attribute) => [attribute.name, canonicalAttribute(attribute.name, attribute.value)])
      .sort(([left], [right]) => left.localeCompare(right));
    const attributes = canonicalAttributes.map(([name, evidence]) => {
      if (evidence.geometry.length > 0) geometry.push([path, name, evidence.geometry]);
      return [name, evidence.structure];
    });
    const children = [];
    let elementIndex = 0;
    for (const child of node.childNodes) {
      const canonical = canonicalNode(child, `${path}.${elementIndex}`, geometry);
      if (canonical !== null) {
        children.push(canonical);
        elementIndex += 1;
      }
    }
    return ["element", node.namespaceURI ?? "", node.nodeName, attributes, children];
  }
  if (node.nodeType === node.TEXT_NODE) return /\S/u.test(node.data) ? ["text", node.data] : null;
  if (node.nodeType === node.CDATA_SECTION_NODE) return ["cdata", node.data];
  if (node.nodeType === node.COMMENT_NODE) return ["comment", node.data];
  if (node.nodeType === node.PROCESSING_INSTRUCTION_NODE) {
    return ["processing-instruction", node.target, node.data];
  }
  return null;
}

function canonicalAttribute(name, value) {
  if (name === "style") return canonicalNumericInlineStyle(value);
  if (name === "data-points") return canonicalDataPoints(value);
  if (GEOMETRY_ATTRIBUTES.has(name)) return canonicalGeometryString(value);
  return { structure: value, geometry: [] };
}

function canonicalGeometryString(value) {
  const geometry = [];
  const structure = value.replace(FLOAT_TOKEN, (raw) => {
    const numeric = Number(raw);
    if (!Number.isFinite(numeric)) return raw;
    geometry.push(normalizeNegativeZero(numeric));
    return NUMBER_MARKER;
  });
  return { structure, geometry };
}

function canonicalNumericInlineStyle(value) {
  const geometry = [];
  const structure = value
    .split(";")
    .map((declaration) => {
      const separator = declaration.indexOf(":");
      if (separator < 0) return declaration;
      const property = declaration.slice(0, separator).trim().toLowerCase();
      if (!NUMERIC_STYLE_PROPERTIES.has(property)) return declaration;
      const evidence = canonicalGeometryString(declaration.slice(separator + 1));
      geometry.push(...evidence.geometry);
      return `${declaration.slice(0, separator + 1)}${evidence.structure}`;
    })
    .join(";");
  return { structure, geometry };
}

function canonicalDataPoints(value) {
  try {
    const parsed = JSON.parse(Buffer.from(value, "base64").toString("utf8"));
    const geometry = [];
    const structure = replaceJsonNumbers(parsed, geometry);
    return { structure: ["base64-json", structure], geometry };
  } catch {
    return { structure: value, geometry: [] };
  }
}

function replaceJsonNumbers(value, geometry) {
  if (typeof value === "number") {
    geometry.push(normalizeNegativeZero(value));
    return NUMBER_MARKER;
  }
  if (Array.isArray(value)) return value.map((child) => replaceJsonNumbers(child, geometry));
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, child]) => [key, replaceJsonNumbers(child, geometry)]),
  );
}

function equivalentOperationEvidence(left, right) {
  if (!left || !right || left.ok !== right.ok) return false;
  if (left.ok) {
    return (
      left.operation_id === right.operation_id &&
      left.media_type === right.media_type &&
      left.sha256 === right.sha256
    );
  }
  return equivalentErrorEvidence(left, right);
}

function equivalentErrorEvidence(left, right) {
  return (
    left.code_name === right.code_name &&
    left.kind === right.kind &&
    left.capability_id === right.capability_id
  );
}

function normalizeNegativeZero(value) {
  return Object.is(value, -0) ? 0 : value;
}

function digest(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}
