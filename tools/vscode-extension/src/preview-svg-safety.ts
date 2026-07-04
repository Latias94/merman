const ACTIVE_SVG_ELEMENTS = new Set([
  "script",
  "iframe",
  "object",
  "embed",
  "applet",
  "form",
  "link",
  "audio",
  "video",
  "canvas",
  "animate",
  "animatemotion",
  "animatetransform",
  "discard",
  "mpath",
  "set",
]);

const SAFE_RASTER_DATA_IMAGE_URL = /^data:image\/(?:png|gif|jpe?g|webp);base64,[a-z0-9+/=]*$/;
const URL_SCHEME = /^[a-z][a-z0-9+.-]*:/;
const RAW_URL_ATTRIBUTES = new Set([
  "action",
  "background",
  "cite",
  "data",
  "formaction",
  "href",
  "longdesc",
  "manifest",
  "ping",
  "poster",
  "profile",
  "src",
  "usemap",
]);
const SVG_URL_REFERENCE_ATTRIBUTES = new Set([
  "clip-path",
  "color-profile",
  "cursor",
  "fill",
  "filter",
  "marker",
  "marker-end",
  "marker-mid",
  "marker-start",
  "mask",
  "stroke",
]);

interface SvgTag {
  kind: "start" | "end";
  name: string;
  attributes: SvgAttribute[];
  end: number;
}

interface SvgAttribute {
  name: string;
  value: string;
}

export function assertSafePreviewSvg(svg: string): void {
  const scanner = new SvgSafetyScanner(svg);
  scanner.assertSafe();
}

class SvgSafetyScanner {
  private cursor = 0;
  private sawRoot = false;

  constructor(private readonly source: string) {}

  assertSafe(): void {
    while (this.cursor < this.source.length) {
      const tag = this.nextTag();
      if (!tag) {
        break;
      }
      if (tag.kind === "end") {
        continue;
      }

      const elementName = localName(tag.name);
      if (!this.sawRoot) {
        this.sawRoot = true;
        if (elementName !== "svg") {
          throw new Error("Preview renderer returned non-SVG output.");
        }
      }
      assertSafeElementName(elementName);
      assertSafeAttributes(tag.attributes);

      if (elementName === "style") {
        const styleEnd = findClosingStyle(this.source, tag.end);
        const styleText = this.source.slice(tag.end, styleEnd ?? this.source.length);
        assertSafeCss(styleText);
        if (styleEnd !== null) {
          this.cursor = styleEnd;
        }
      }
    }

    if (!this.sawRoot) {
      throw new Error("Preview renderer returned non-SVG output.");
    }
  }

  private nextTag(): SvgTag | null {
    const start = this.source.indexOf("<", this.cursor);
    if (start < 0) {
      this.cursor = this.source.length;
      return null;
    }

    if (this.source.startsWith("<!--", start)) {
      this.cursor = consumeUntil(this.source, start + 4, "-->");
      return this.nextTag();
    }
    if (this.source.startsWith("<?", start)) {
      this.cursor = consumeUntil(this.source, start + 2, "?>");
      return this.nextTag();
    }
    if (this.source.startsWith("<![CDATA[", start)) {
      this.cursor = consumeUntil(this.source, start + 9, "]]>");
      return this.nextTag();
    }
    if (this.source.startsWith("<!", start)) {
      throw new Error("Preview renderer returned SVG with unsupported declarations.");
    }

    const tag = parseTag(this.source, start);
    this.cursor = tag.end;
    return tag;
  }
}

function parseTag(source: string, start: number): SvgTag {
  let cursor = start + 1;
  let kind: SvgTag["kind"] = "start";
  if (source[cursor] === "/") {
    kind = "end";
    cursor += 1;
  }

  cursor = skipWhitespace(source, cursor);
  const nameStart = cursor;
  while (cursor < source.length && !isNameTerminator(source[cursor] ?? "")) {
    cursor += 1;
  }
  const name = source.slice(nameStart, cursor);
  if (!name) {
    throw new Error("Preview renderer returned malformed SVG output.");
  }

  const attributes: SvgAttribute[] = [];
  while (cursor < source.length) {
    cursor = skipWhitespace(source, cursor);
    const char = source[cursor];
    if (char === ">") {
      return { kind, name, attributes, end: cursor + 1 };
    }
    if (char === "/" && source[cursor + 1] === ">") {
      return { kind, name, attributes, end: cursor + 2 };
    }
    if (kind === "end") {
      throw new Error("Preview renderer returned malformed SVG output.");
    }

    const attributeStart = cursor;
    while (cursor < source.length && !isAttributeNameTerminator(source[cursor] ?? "")) {
      cursor += 1;
    }
    const attributeName = source.slice(attributeStart, cursor);
    if (!attributeName) {
      throw new Error("Preview renderer returned malformed SVG output.");
    }

    cursor = skipWhitespace(source, cursor);
    let value = "";
    if (source[cursor] === "=") {
      cursor += 1;
      cursor = skipWhitespace(source, cursor);
      const quote = source[cursor];
      if (quote === '"' || quote === "'") {
        const valueStart = cursor + 1;
        const valueEnd = source.indexOf(quote, valueStart);
        if (valueEnd < 0) {
          throw new Error("Preview renderer returned malformed SVG output.");
        }
        value = source.slice(valueStart, valueEnd);
        cursor = valueEnd + 1;
      } else {
        const valueStart = cursor;
        while (cursor < source.length && !isUnquotedValueTerminator(source[cursor] ?? "")) {
          cursor += 1;
        }
        value = source.slice(valueStart, cursor);
      }
    }
    attributes.push({ name: attributeName, value });
  }

  throw new Error("Preview renderer returned malformed SVG output.");
}

function assertSafeElementName(name: string): void {
  if (ACTIVE_SVG_ELEMENTS.has(name)) {
    throw new Error("Preview renderer returned SVG with active embedded content.");
  }
}

function assertSafeAttributes(attributes: SvgAttribute[]): void {
  for (const attribute of attributes) {
    const name = attribute.name.toLowerCase();
    const nameWithoutNamespace = localName(name);
    const value = decodeXmlEntities(attribute.value);
    if (nameWithoutNamespace.startsWith("on")) {
      throw new Error("Preview renderer returned SVG with event handler attributes.");
    }
    if (nameWithoutNamespace === "srcset") {
      assertSafeSrcset(value);
    }
    if (RAW_URL_ATTRIBUTES.has(nameWithoutNamespace)) {
      assertSafeUrl(value, "attribute");
    }
    if (SVG_URL_REFERENCE_ATTRIBUTES.has(nameWithoutNamespace)) {
      assertSafeUrlReferences(value, "attribute");
    }
    if (nameWithoutNamespace === "style") {
      assertSafeCss(value);
    }
  }
}

function assertSafeUrl(value: string, source: "attribute" | "css"): void {
  const compact = removeAsciiWhitespaceAndControl(value).toLowerCase();
  const trimmed = value.trim().toLowerCase();
  if (compact.startsWith("#") || SAFE_RASTER_DATA_IMAGE_URL.test(compact)) {
    return;
  }
  if (
    trimmed.startsWith("http://") ||
    trimmed.startsWith("https://") ||
    trimmed.startsWith("//") ||
    !URL_SCHEME.test(compact)
  ) {
    throw new Error(
      source === "css"
        ? "Preview renderer returned SVG with external CSS resource references."
        : "Preview renderer returned SVG with external resource references.",
    );
  }
  throw new Error(
    source === "css"
      ? "Preview renderer returned SVG with unsafe CSS URL references."
      : "Preview renderer returned SVG with unsafe URL attributes.",
  );
}

function assertSafeSrcset(value: string): void {
  if (value.trim().length === 0) {
    return;
  }
  throw new Error("Preview renderer returned SVG with srcset resource references.");
}

function assertSafeCss(css: string): void {
  const withoutComments = stripCssComments(css);
  const normalized = decodeCssEscapes(decodeXmlEntities(withoutComments));
  const lower = normalized.toLowerCase();
  if (lower.includes("@import")) {
    throw new Error("Preview renderer returned SVG with external CSS resource references.");
  }

  let cursor = 0;
  while (cursor < lower.length) {
    const urlIndex = lower.indexOf("url", cursor);
    if (urlIndex < 0) {
      return;
    }
    cursor = urlIndex + "url".length;
    cursor = skipWhitespace(lower, cursor);
    if (lower[cursor] !== "(") {
      continue;
    }
    const valueStart = cursor + 1;
    const valueEnd = lower.indexOf(")", valueStart);
    if (valueEnd < 0) {
      throw new Error("Preview renderer returned malformed SVG CSS.");
    }
    const rawValue = normalized.slice(valueStart, valueEnd).trim();
    const unquoted =
      (rawValue.startsWith('"') && rawValue.endsWith('"')) ||
      (rawValue.startsWith("'") && rawValue.endsWith("'"))
        ? rawValue.slice(1, -1)
        : rawValue;
    assertSafeUrl(unquoted, "css");
    cursor = valueEnd + 1;
  }
}

function assertSafeUrlReferences(value: string, source: "attribute" | "css"): void {
  const normalized = decodeCssEscapes(decodeXmlEntities(value));
  const lower = normalized.toLowerCase();
  let cursor = 0;
  let sawUrlReference = false;

  while (cursor < lower.length) {
    const urlIndex = lower.indexOf("url", cursor);
    if (urlIndex < 0) {
      break;
    }
    cursor = urlIndex + "url".length;
    cursor = skipWhitespace(lower, cursor);
    if (lower[cursor] !== "(") {
      continue;
    }
    sawUrlReference = true;
    const valueStart = cursor + 1;
    const valueEnd = lower.indexOf(")", valueStart);
    if (valueEnd < 0) {
      throw new Error("Preview renderer returned malformed SVG URL references.");
    }
    const rawValue = normalized.slice(valueStart, valueEnd).trim();
    const unquoted =
      (rawValue.startsWith('"') && rawValue.endsWith('"')) ||
      (rawValue.startsWith("'") && rawValue.endsWith("'"))
        ? rawValue.slice(1, -1)
        : rawValue;
    assertSafeUrl(unquoted, source);
    cursor = valueEnd + 1;
  }

  if (sawUrlReference) {
    return;
  }

  const compact = removeAsciiWhitespaceAndControl(normalized).toLowerCase();
  if (!compact || compact === "none" || compact.startsWith("#")) {
    return;
  }
  if (compact.startsWith("//") || compact.startsWith("/") || URL_SCHEME.test(compact)) {
    assertSafeUrl(normalized, source);
  }
}

function stripCssComments(css: string): string {
  let output = "";
  let cursor = 0;
  while (cursor < css.length) {
    const commentStart = css.indexOf("/*", cursor);
    if (commentStart < 0) {
      output += css.slice(cursor);
      break;
    }
    output += css.slice(cursor, commentStart);
    const commentEnd = css.indexOf("*/", commentStart + 2);
    if (commentEnd < 0) {
      break;
    }
    cursor = commentEnd + 2;
  }
  return output;
}

function decodeCssEscapes(value: string): string {
  let output = "";
  for (let cursor = 0; cursor < value.length; cursor += 1) {
    const char = value[cursor];
    if (char !== "\\") {
      output += char;
      continue;
    }

    const next = value[cursor + 1];
    if (next === undefined) {
      continue;
    }
    if (next === "\n" || next === "\r" || next === "\f") {
      cursor += next === "\r" && value[cursor + 2] === "\n" ? 2 : 1;
      continue;
    }

    let hex = "";
    let hexCursor = cursor + 1;
    while (hexCursor < value.length && hex.length < 6 && isHexDigit(value[hexCursor] ?? "")) {
      hex += value[hexCursor];
      hexCursor += 1;
    }
    if (hex.length > 0) {
      output += codePointToString(Number.parseInt(hex, 16), "");
      if (isWhitespace(value[hexCursor] ?? "")) {
        hexCursor += 1;
      }
      cursor = hexCursor - 1;
      continue;
    }

    output += next;
    cursor += 1;
  }
  return output;
}

function decodeXmlEntities(value: string): string {
  return value.replace(/&(#x[0-9a-f]+|#\d+|amp|lt|gt|quot|apos);/gi, (entity, body: string) => {
    const lower = body.toLowerCase();
    if (lower.startsWith("#x")) {
      return codePointToString(Number.parseInt(lower.slice(2), 16), entity);
    }
    if (lower.startsWith("#")) {
      return codePointToString(Number.parseInt(lower.slice(1), 10), entity);
    }
    switch (lower) {
      case "amp":
        return "&";
      case "lt":
        return "<";
      case "gt":
        return ">";
      case "quot":
        return '"';
      case "apos":
        return "'";
      default:
        return entity;
    }
  });
}

function codePointToString(codePoint: number, fallback: string): string {
  if (!Number.isFinite(codePoint)) {
    return fallback;
  }
  try {
    return String.fromCodePoint(codePoint);
  } catch {
    return fallback;
  }
}

function removeAsciiWhitespaceAndControl(value: string): string {
  let output = "";
  for (const char of value) {
    const codePoint = char.codePointAt(0) ?? 0;
    if (codePoint > 0x20) {
      output += char;
    }
  }
  return output;
}

function localName(name: string): string {
  const lower = name.toLowerCase();
  const separator = lower.lastIndexOf(":");
  return separator >= 0 ? lower.slice(separator + 1) : lower;
}

function findClosingStyle(source: string, start: number): number | null {
  const lower = source.toLowerCase();
  const index = lower.indexOf("</style", start);
  return index >= 0 ? index : null;
}

function consumeUntil(source: string, start: number, terminator: string): number {
  const end = source.indexOf(terminator, start);
  if (end < 0) {
    throw new Error("Preview renderer returned malformed SVG output.");
  }
  return end + terminator.length;
}

function skipWhitespace(source: string, cursor: number): number {
  while (cursor < source.length && isWhitespace(source[cursor] ?? "")) {
    cursor += 1;
  }
  return cursor;
}

function isWhitespace(char: string): boolean {
  return char === " " || char === "\n" || char === "\r" || char === "\t" || char === "\f";
}

function isHexDigit(char: string): boolean {
  return (
    (char >= "0" && char <= "9") ||
    (char >= "a" && char <= "f") ||
    (char >= "A" && char <= "F")
  );
}

function isNameTerminator(char: string): boolean {
  return isWhitespace(char) || char === ">" || char === "/";
}

function isAttributeNameTerminator(char: string): boolean {
  return isWhitespace(char) || char === "=" || char === ">" || char === "/";
}

function isUnquotedValueTerminator(char: string): boolean {
  return isWhitespace(char) || char === ">";
}
