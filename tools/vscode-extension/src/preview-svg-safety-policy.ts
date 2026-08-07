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

const SVG_ELEMENTS = new Set([
  "a",
  "circle",
  "clippath",
  "defs",
  "desc",
  "ellipse",
  "feblend",
  "fecolormatrix",
  "fecomponenttransfer",
  "fecomposite",
  "feconvolvematrix",
  "fediffuselighting",
  "fedisplacementmap",
  "fedistantlight",
  "fedropshadow",
  "feflood",
  "fefunca",
  "fefuncb",
  "fefuncg",
  "fefuncr",
  "fegaussianblur",
  "feimage",
  "femerge",
  "femergenode",
  "femorphology",
  "feoffset",
  "fepointlight",
  "fespecularlighting",
  "fespotlight",
  "fetile",
  "feturbulence",
  "filter",
  "foreignobject",
  "g",
  "image",
  "line",
  "lineargradient",
  "marker",
  "mask",
  "metadata",
  "path",
  "pattern",
  "polygon",
  "polyline",
  "radialgradient",
  "rect",
  "stop",
  "style",
  "svg",
  "switch",
  "symbol",
  "text",
  "textpath",
  "title",
  "tspan",
  "use",
  "view",
]);

const FOREIGN_OBJECT_LABEL_ELEMENTS = new Set([
  "a",
  "div",
  "span",
  "p",
  "br",
  "b",
  "strong",
  "i",
  "em",
  "s",
  "u",
  "small",
  "sub",
  "sup",
  "code",
  "pre",
]);

const FOREIGN_OBJECT_INTERACTIVE_ATTRIBUTES = new Set([
  "autofocus",
  "contenteditable",
  "draggable",
  "tabindex",
]);

const MAX_EMBEDDED_IMAGE_BYTES = 16 * 1024 * 1024;
const MAX_TOTAL_EMBEDDED_IMAGE_BYTES = 32 * 1024 * 1024;
const MAX_EMBEDDED_IMAGE_ENCODED_BYTES = 24 * 1024 * 1024;
const MAX_TOTAL_EMBEDDED_IMAGE_ENCODED_BYTES = 44 * 1024 * 1024;
const MAX_EMBEDDED_IMAGE_PIXELS = 16 * 1024 * 1024;
const MAX_TOTAL_EMBEDDED_IMAGE_PIXELS = 32 * 1024 * 1024;
const MAX_SAFE_SVG_SOURCE_CODE_UNITS = 64 * 1024 * 1024;
const MAX_SAFE_SVG_SOURCE_UTF8_BYTES = 64 * 1024 * 1024;
const MAX_SAFE_SVG_ATTRIBUTE_CODE_UNITS = 25 * 1024 * 1024;
const PNG_ANIMATION_CHUNKS = new Set(["acTL", "fcTL", "fdAT"]);
const PNG_STATIC_CHUNKS = new Set([
  "IHDR",
  "PLTE",
  "IDAT",
  "IEND",
  "tRNS",
  "cHRM",
  "gAMA",
  "iCCP",
  "sBIT",
  "sRGB",
  "cICP",
  "mDCV",
  "iTXt",
  "tEXt",
  "zTXt",
  "bKGD",
  "hIST",
  "pHYs",
  "sPLT",
  "eXIf",
  "tIME",
]);
const PNG_COMPRESSED_METADATA_CHUNKS = new Set(["iCCP", "zTXt"]);
const RASTER_DATA_IMAGE_URL = /^data:image\/(png|gif|jpe?g|webp);base64,([a-z0-9+/]*={0,2})$/i;
const CSS_HEX_COLOR = /^#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;
const URL_SCHEME = /^[a-z][a-z0-9+.-]*:/;
const MERMAID_NAVIGATION_SCHEMES = new Set([
  "callto:",
  "cid:",
  "ftp:",
  "ftps:",
  "http:",
  "https:",
  "mailto:",
  "matrix:",
  "sms:",
  "tel:",
  "xmpp:",
]);
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
  selfClosing: boolean;
}

interface SvgAttribute {
  name: string;
  value: string;
}

interface SvgSafetyPolicy {
  readonly anchorNavigation: "fragment-only" | "mermaid-compatible";
}

export interface SvgSafetyInspection {
  readonly hasFragmentReferences: boolean;
}

const SELF_CONTAINED_SVG_POLICY: SvgSafetyPolicy = Object.freeze({
  anchorNavigation: "fragment-only",
});

const NAVIGABLE_SVG_POLICY: SvgSafetyPolicy = Object.freeze({
  anchorNavigation: "mermaid-compatible",
});

export function assertSelfContainedSvgWithMessagePrefix(
  svg: string,
  messagePrefix: string,
): SvgSafetyInspection {
  const scanner = new SvgSafetyScanner(svg, messagePrefix, SELF_CONTAINED_SVG_POLICY);
  return scanner.assertSafe();
}

export function assertNavigableSvgWithMessagePrefix(
  svg: string,
  messagePrefix: string,
): SvgSafetyInspection {
  const scanner = new SvgSafetyScanner(svg, messagePrefix, NAVIGABLE_SVG_POLICY);
  return scanner.assertSafe();
}

class SvgSafetyScanner {
  private cursor = 0;
  private sawRoot = false;
  private rootDepth = 0;
  private rootClosedAt: number | null = null;
  private foreignObjectDepth = 0;
  private embeddedImageEncodedBytes = 0;
  private embeddedImageBytes = 0;
  private embeddedImagePixels = 0;
  private hasFragmentReferences = false;
  private readonly elementStack: string[] = [];

  constructor(
    private readonly source: string,
    private readonly messagePrefix: string,
    private readonly policy: SvgSafetyPolicy,
  ) {}

  assertSafe(): SvgSafetyInspection {
    if (
      this.source.length > MAX_SAFE_SVG_SOURCE_CODE_UNITS ||
      !utf8LengthAtMost(this.source, MAX_SAFE_SVG_SOURCE_UTF8_BYTES)
    ) {
      throw this.error("SVG output exceeds the source byte limit.");
    }
    while (this.cursor < this.source.length) {
      const tag = this.nextTag();
      if (!tag) {
        break;
      }
      if (this.rootClosedAt !== null) {
        throw this.error("malformed SVG output.");
      }
      if (tag.kind === "end") {
        const elementName = localName(tag.name);
        if (!this.sawRoot || this.elementStack.length === 0) {
          throw this.error("malformed SVG output.");
        }
        const openElementName = this.elementStack.pop();
        if (openElementName !== elementName) {
          throw this.error("malformed SVG output.");
        }
        if (elementName === "foreignobject" && this.foreignObjectDepth > 0) {
          this.foreignObjectDepth -= 1;
        }
        this.rootDepth = this.elementStack.length;
        if (this.rootDepth === 0) {
          this.rootClosedAt = tag.end;
        }
        continue;
      }

      const elementName = localName(tag.name);
      const inForeignObject = this.foreignObjectDepth > 0;
      if (!this.sawRoot) {
        this.sawRoot = true;
        if (elementName !== "svg") {
          throw this.error("non-SVG output.");
        }
        if (!tag.selfClosing) {
          this.elementStack.push(elementName);
          this.rootDepth = this.elementStack.length;
        } else {
          this.rootClosedAt = tag.end;
        }
      } else if (!tag.selfClosing) {
        this.elementStack.push(elementName);
        this.rootDepth = this.elementStack.length;
      }
      this.assertSafeElementName(elementName, inForeignObject);
      this.assertSafeAttributes(
        elementName,
        tag.attributes,
        inForeignObject || elementName === "foreignobject",
      );

      if (elementName === "foreignobject" && !tag.selfClosing) {
        this.foreignObjectDepth += 1;
      }

      if (elementName === "style" && !tag.selfClosing) {
        const styleEnd = findClosingStyle(this.source, tag.end);
        if (styleEnd === null) {
          throw this.error("malformed SVG output.");
        }
        const styleText = this.source.slice(tag.end, styleEnd);
        this.assertSafeCss(styleText);
        this.cursor = styleEnd;
      }
    }

    if (!this.sawRoot) {
      throw this.error("non-SVG output.");
    }
    if (this.rootClosedAt === null || this.rootDepth !== 0 || this.elementStack.length !== 0) {
      throw this.error("malformed SVG output.");
    }
    this.assertOnlyIgnorableRootTail(this.rootClosedAt);
    if (this.foreignObjectDepth !== 0) {
      throw this.error("malformed SVG output.");
    }
    return Object.freeze({ hasFragmentReferences: this.hasFragmentReferences });
  }

  private nextTag(): SvgTag | null {
    while (true) {
      const start = this.source.indexOf("<", this.cursor);
      if (start < 0) {
        this.cursor = this.source.length;
        return null;
      }
      if (!this.sawRoot && !isOnlyWhitespace(this.source.slice(this.cursor, start))) {
        throw this.error("non-SVG output.");
      }

      if (this.source.startsWith("<!--", start)) {
        this.cursor = this.consumeUntil(start + 4, "-->");
        continue;
      }
      if (this.source.startsWith("<?", start)) {
        this.cursor = this.consumeUntil(start + 2, "?>");
        continue;
      }
      if (this.source.startsWith("<![CDATA[", start)) {
        this.cursor = this.consumeUntil(start + 9, "]]>");
        continue;
      }
      if (this.source.startsWith("<!", start)) {
        throw this.error("SVG with unsupported declarations.");
      }

      const tag = this.parseTag(start);
      this.cursor = tag.end;
      return tag;
    }
  }

  private parseTag(start: number): SvgTag {
    let cursor = start + 1;
    let kind: SvgTag["kind"] = "start";
    if (this.source[cursor] === "/") {
      kind = "end";
      cursor += 1;
    }

    cursor = skipWhitespace(this.source, cursor);
    const nameStart = cursor;
    while (cursor < this.source.length && !isNameTerminator(this.source[cursor] ?? "")) {
      cursor += 1;
    }
    const name = this.source.slice(nameStart, cursor);
    if (!name) {
      throw this.error("malformed SVG output.");
    }

    const attributes: SvgAttribute[] = [];
    while (cursor < this.source.length) {
      cursor = skipWhitespace(this.source, cursor);
      const char = this.source[cursor];
      if (char === ">") {
        return { kind, name, attributes, end: cursor + 1, selfClosing: false };
      }
      if (char === "/" && this.source[cursor + 1] === ">") {
        return { kind, name, attributes, end: cursor + 2, selfClosing: true };
      }
      if (kind === "end") {
        throw this.error("malformed SVG output.");
      }

      const attributeStart = cursor;
      while (
        cursor < this.source.length &&
        !isAttributeNameTerminator(this.source[cursor] ?? "")
      ) {
        cursor += 1;
      }
      const attributeName = this.source.slice(attributeStart, cursor);
      if (!attributeName) {
        throw this.error("malformed SVG output.");
      }

      cursor = skipWhitespace(this.source, cursor);
      let value = "";
      if (this.source[cursor] === "=") {
        cursor += 1;
        cursor = skipWhitespace(this.source, cursor);
        const quote = this.source[cursor];
        if (quote === '"' || quote === "'") {
          const valueStart = cursor + 1;
          const valueEnd = this.source.indexOf(quote, valueStart);
          if (valueEnd < 0) {
            throw this.error("malformed SVG output.");
          }
          value = this.source.slice(valueStart, valueEnd);
          cursor = valueEnd + 1;
        } else {
          const valueStart = cursor;
          while (
            cursor < this.source.length &&
            !isUnquotedValueTerminator(this.source[cursor] ?? "")
          ) {
            cursor += 1;
          }
          value = this.source.slice(valueStart, cursor);
        }
      }
      attributes.push({ name: attributeName, value });
    }

    throw this.error("malformed SVG output.");
  }

  private assertSafeElementName(name: string, inForeignObject: boolean): void {
    if (ACTIVE_SVG_ELEMENTS.has(name)) {
      throw this.error("SVG with active embedded content.");
    }
    if (inForeignObject) {
      if (!FOREIGN_OBJECT_LABEL_ELEMENTS.has(name)) {
        throw this.error("SVG with unsupported foreignObject content.");
      }
      return;
    }
    if (!SVG_ELEMENTS.has(name)) {
      throw this.error("SVG with unsupported element content.");
    }
  }

  private assertSafeAttributes(
    elementName: string,
    attributes: SvgAttribute[],
    inForeignObject: boolean,
  ): void {
    for (const attribute of attributes) {
      if (attribute.value.length > MAX_SAFE_SVG_ATTRIBUTE_CODE_UNITS) {
        throw this.error("SVG attribute exceeds the raw value limit.");
      }
      const name = attribute.name.toLowerCase();
      const nameWithoutNamespace = localName(name);
      const value = decodeXmlEntities(attribute.value);
      if (nameWithoutNamespace.startsWith("on")) {
        throw this.error("SVG with event handler attributes.");
      }
      if (elementName === "a") {
        this.assertSafeAnchorInteractionAttribute(nameWithoutNamespace, value);
      }
      if (nameWithoutNamespace === "base") {
        throw this.error("SVG with base URL attributes.");
      }
      if (nameWithoutNamespace === "srcset") {
        this.assertSafeSrcset(value);
      }
      if (RAW_URL_ATTRIBUTES.has(nameWithoutNamespace)) {
        this.assertSafeUrl(attribute.value, "attribute", elementName, nameWithoutNamespace);
      }
      if (SVG_URL_REFERENCE_ATTRIBUTES.has(nameWithoutNamespace)) {
        this.assertSafeUrlReferences(value, nameWithoutNamespace);
      }
      if (nameWithoutNamespace === "style") {
        this.assertSafeCss(value);
      }
      if (
        inForeignObject &&
        FOREIGN_OBJECT_INTERACTIVE_ATTRIBUTES.has(nameWithoutNamespace)
      ) {
        throw this.error("SVG with interactive foreignObject attributes.");
      }
    }
  }

  private assertSafeAnchorInteractionAttribute(name: string, value: string): void {
    if (name === "download") {
      throw this.error("SVG with unsupported navigation download attributes.");
    }
    if (name === "ping" || name === "attributionsrc") {
      throw this.error("SVG with unsupported navigation tracking attributes.");
    }
    if (name === "target") {
      const trimmedTarget = value.trim();
      if (value !== trimmedTarget) {
        throw this.error("SVG with malformed navigation target attributes.");
      }
      const target = trimmedTarget.toLowerCase();
      const allowedTarget =
        target === "" ||
        (this.policy.anchorNavigation === "mermaid-compatible" &&
          (target === "_self" ||
            target === "_blank" ||
            target === "_parent" ||
            target === "_top"));
      if (!allowedTarget) {
        throw this.error("SVG with unsupported navigation target attributes.");
      }
    }
    if (
      name === "rel" &&
      value
        .toLowerCase()
        .split(/\s+/u)
        .includes("opener")
    ) {
      throw this.error("SVG with unsafe navigation relationship attributes.");
    }
  }

  private assertSafeUrl(
    value: string,
    source: "attribute" | "css",
    elementName?: string,
    attributeName?: string,
  ): void {
    const normalized = source === "attribute" ? decodeXmlEntities(value) : value;
    const decoded = normalized;
    const compact = removeAsciiWhitespaceAndControl(normalized).toLowerCase();
    const trimmed = normalized.trim().toLowerCase();
    if (compact.startsWith("#")) {
      this.hasFragmentReferences = true;
      return;
    }
    if (
      source === "attribute" &&
      elementName === "a" &&
      attributeName === "href" &&
      this.policy.anchorNavigation === "mermaid-compatible"
    ) {
      this.assertSafeAnchorNavigationUrl(value, decoded, normalized, compact, trimmed);
      return;
    }
    if (compact.startsWith("data:")) {
      if (
        source !== "attribute" ||
        attributeName !== "href" ||
        (elementName !== "image" && elementName !== "feimage")
      ) {
        throw this.error("SVG with unsafe embedded resource references.");
      }
      this.assertSafeEmbeddedRasterDataUrl(normalized);
      return;
    }
    if (containsCharacterReference(normalized)) {
      throw this.error(
        source === "css"
          ? "SVG with unsafe CSS character references."
          : "SVG with unsafe URL character references.",
      );
    }
    if (
      trimmed.startsWith("http://") ||
      trimmed.startsWith("https://") ||
      trimmed.startsWith("//") ||
      !URL_SCHEME.test(compact)
    ) {
      throw this.error(
        source === "css"
          ? "SVG with external CSS resource references."
          : "SVG with external resource references.",
      );
    }
    throw this.error(
      source === "css"
        ? "SVG with unsafe CSS URL references."
        : "SVG with unsafe URL attributes.",
    );
  }

  private assertSafeAnchorNavigationUrl(
    rawValue: string,
    decodedValue: string,
    normalized: string,
    compact: string,
    trimmed: string,
  ): void {
    // Navigation URLs are intentionally stricter than ordinary URL parsing:
    // whitespace, CSS escapes, and character references must not be able to
    // disguise the scheme or turn a relative URL into an ambient navigation.
    if (
      decodedValue !== normalized ||
      containsCharacterReference(rawValue.replace(/&amp;/gi, "")) ||
      normalized !== normalized.trim() ||
      compact !== normalized.toLowerCase()
    ) {
      throw this.error("SVG with unsafe navigation URL attributes.");
    }
    if (!trimmed) {
      throw this.error("SVG with malformed navigation URL attributes.");
    }
    if (trimmed.startsWith("//") || trimmed.includes("\\")) {
      throw this.error("SVG with ambient navigation URL attributes.");
    }

    const schemeMatch = URL_SCHEME.exec(trimmed);
    if (!schemeMatch) {
      return;
    }

    const scheme = schemeMatch[0].toLowerCase();
    if (!MERMAID_NAVIGATION_SCHEMES.has(scheme)) {
      throw this.error("SVG with unsafe navigation URL attributes.");
    }

    if (scheme !== "http:" && scheme !== "https:" && scheme !== "ftp:" && scheme !== "ftps:") {
      return;
    }

    let parsed: URL;
    try {
      parsed = new URL(trimmed);
    } catch {
      throw this.error("SVG with malformed navigation URL attributes.");
    }
    if (!parsed.hostname) {
      throw this.error("SVG with malformed navigation URL attributes.");
    }
  }

  private assertSafeEmbeddedRasterDataUrl(value: string): void {
    const parsed = parseRasterDataUrl(value);
    if (!parsed) {
      throw this.error("SVG with malformed embedded raster data URL.");
    }
    if (parsed.encodedBytes > MAX_EMBEDDED_IMAGE_ENCODED_BYTES) {
      throw this.error("SVG embedded raster exceeds the per-image encoded byte limit.");
    }
    if (
      parsed.encodedBytes >
      MAX_TOTAL_EMBEDDED_IMAGE_ENCODED_BYTES - this.embeddedImageEncodedBytes
    ) {
      throw this.error("SVG embedded rasters exceed the aggregate encoded byte limit.");
    }
    if (parsed.decodedBytes > MAX_EMBEDDED_IMAGE_BYTES) {
      throw this.error("SVG embedded raster exceeds the per-image byte limit.");
    }
    if (parsed.decodedBytes > MAX_TOTAL_EMBEDDED_IMAGE_BYTES - this.embeddedImageBytes) {
      throw this.error("SVG embedded rasters exceed the aggregate byte limit.");
    }

    const bytes = decodeBase64(parsed.payload, parsed.decodedBytes);
    if (!bytes) {
      throw this.error("SVG with malformed embedded raster data URL.");
    }
    const inspected = inspectRasterImage(bytes);
    if (!inspected) {
      throw this.error("SVG with malformed embedded raster image.");
    }
    if (inspected.format !== parsed.declaredFormat) {
      throw this.error("SVG embedded raster MIME type does not match its file format.");
    }
    if (inspected.animated) {
      throw this.error("SVG with animated or multi-frame embedded raster image.");
    }
    if (
      inspected.width > Math.floor(MAX_EMBEDDED_IMAGE_PIXELS / inspected.height)
    ) {
      throw this.error("SVG embedded raster exceeds the per-image pixel limit.");
    }
    const pixels = inspected.width * inspected.height;
    if (pixels > MAX_TOTAL_EMBEDDED_IMAGE_PIXELS - this.embeddedImagePixels) {
      throw this.error("SVG embedded rasters exceed the aggregate pixel limit.");
    }

    this.embeddedImageEncodedBytes += parsed.encodedBytes;
    this.embeddedImageBytes += parsed.decodedBytes;
    this.embeddedImagePixels += pixels;
  }

  private assertSafeSrcset(value: string): void {
    if (value.trim().length === 0) {
      return;
    }
    throw this.error("SVG with srcset resource references.");
  }

  private assertSafeCss(css: string): void {
    const normalized = decodeCssEscapes(decodeXmlEntities(css));
    const withoutComments = stripCssComments(normalized);
    const lower = withoutComments.toLowerCase();
    if (containsCharacterReference(withoutComments)) {
      throw this.error("SVG with unsafe CSS character references.");
    }
    if (lower.includes("@import")) {
      throw this.error("SVG with external CSS resource references.");
    }
    if (containsShadowScopingSelector(lower)) {
      throw this.error("SVG with unsafe shadow CSS selectors.");
    }
    if (
      containsCssFunction(lower, "image-set") ||
      containsCssFunction(lower, "-webkit-image-set")
    ) {
      throw this.error("SVG with external CSS resource references.");
    }
    if (containsViewportEscapingCssDeclaration(lower)) {
      throw this.error("SVG with viewport-escaping CSS.");
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
        throw this.error("malformed SVG CSS.");
      }
      const rawValue = withoutComments.slice(valueStart, valueEnd).trim();
      const unquoted =
        (rawValue.startsWith('"') && rawValue.endsWith('"')) ||
        (rawValue.startsWith("'") && rawValue.endsWith("'"))
          ? rawValue.slice(1, -1)
          : rawValue;
      this.assertSafeUrl(unquoted, "css");
      cursor = valueEnd + 1;
    }
  }

  private assertSafeUrlReferences(value: string, attributeName: string): void {
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
        throw this.error("malformed SVG URL references.");
      }
      const rawValue = normalized.slice(valueStart, valueEnd).trim();
      const unquoted =
        (rawValue.startsWith('"') && rawValue.endsWith('"')) ||
        (rawValue.startsWith("'") && rawValue.endsWith("'"))
          ? rawValue.slice(1, -1)
          : rawValue;
      this.assertSafeUrl(unquoted, "css");
      cursor = valueEnd + 1;
    }

    if (sawUrlReference) {
      return;
    }

    const compact = removeAsciiWhitespaceAndControl(normalized).toLowerCase();
    if (!compact || compact === "none") {
      return;
    }
    if (compact.startsWith("#")) {
      if (attributeName === "fill" || attributeName === "stroke") {
        if (!CSS_HEX_COLOR.test(compact)) {
          throw this.error("SVG with malformed hexadecimal paint values.");
        }
        return;
      }
      this.hasFragmentReferences = true;
      return;
    }
    if (compact.startsWith("//") || compact.startsWith("/") || URL_SCHEME.test(compact)) {
      this.assertSafeUrl(normalized, "css");
    }
  }

  private assertOnlyIgnorableRootTail(start: number): void {
    let cursor = start;
    while (cursor < this.source.length) {
      cursor = skipWhitespace(this.source, cursor);
      if (cursor >= this.source.length) {
        return;
      }
      if (this.source.startsWith("<!--", cursor)) {
        cursor = this.consumeUntil(cursor + 4, "-->");
        continue;
      }
      throw this.error("malformed SVG output.");
    }
  }

  private consumeUntil(start: number, terminator: string): number {
    const end = this.source.indexOf(terminator, start);
    if (end < 0) {
      throw this.error("malformed SVG output.");
    }
    return end + terminator.length;
  }

  private error(suffix: string): Error {
    return new Error(`${this.messagePrefix} ${suffix}`);
  }
}

type RasterFormat = "png" | "gif" | "jpeg" | "webp";

interface ParsedRasterDataUrl {
  declaredFormat: RasterFormat;
  payload: string;
  encodedBytes: number;
  decodedBytes: number;
}

interface RasterImageInspection {
  format: RasterFormat;
  width: number;
  height: number;
  animated: boolean;
}

function parseRasterDataUrl(value: string): ParsedRasterDataUrl | null {
  const match = RASTER_DATA_IMAGE_URL.exec(value);
  if (!match) {
    return null;
  }
  const mime = match[1]?.toLowerCase();
  const payload = match[2];
  if (!mime || !payload || payload.length % 4 !== 0) {
    return null;
  }
  const firstPadding = payload.indexOf("=");
  if (firstPadding >= 0 && firstPadding < payload.length - 2) {
    return null;
  }
  const padding = payload.endsWith("==") ? 2 : payload.endsWith("=") ? 1 : 0;
  const decodedBytes = (payload.length / 4) * 3 - padding;
  if (!Number.isSafeInteger(decodedBytes) || decodedBytes <= 0) {
    return null;
  }
  return {
    declaredFormat: mime === "jpg" ? "jpeg" : (mime as RasterFormat),
    payload,
    encodedBytes: payload.length,
    decodedBytes,
  };
}

function decodeBase64(payload: string, decodedBytes: number): Uint8Array | null {
  const output = new Uint8Array(decodedBytes);
  let outputCursor = 0;

  for (let cursor = 0; cursor < payload.length; cursor += 4) {
    const a = base64Value(payload[cursor] ?? "");
    const b = base64Value(payload[cursor + 1] ?? "");
    const cChar = payload[cursor + 2] ?? "";
    const dChar = payload[cursor + 3] ?? "";
    const isLast = cursor + 4 === payload.length;
    if (a < 0 || b < 0) {
      return null;
    }

    output[outputCursor] = (a << 2) | (b >> 4);
    outputCursor += 1;
    if (cChar === "=") {
      if (!isLast || dChar !== "=" || (b & 0x0f) !== 0) {
        return null;
      }
      continue;
    }

    const c = base64Value(cChar);
    if (c < 0 || outputCursor >= decodedBytes) {
      return null;
    }
    output[outputCursor] = ((b & 0x0f) << 4) | (c >> 2);
    outputCursor += 1;
    if (dChar === "=") {
      if (!isLast || (c & 0x03) !== 0) {
        return null;
      }
      continue;
    }

    const d = base64Value(dChar);
    if (d < 0 || outputCursor >= decodedBytes) {
      return null;
    }
    output[outputCursor] = ((c & 0x03) << 6) | d;
    outputCursor += 1;
  }

  return outputCursor === decodedBytes ? output : null;
}

function base64Value(char: string): number {
  const code = char.charCodeAt(0);
  if (code >= 65 && code <= 90) {
    return code - 65;
  }
  if (code >= 97 && code <= 122) {
    return code - 97 + 26;
  }
  if (code >= 48 && code <= 57) {
    return code - 48 + 52;
  }
  if (char === "+") {
    return 62;
  }
  if (char === "/") {
    return 63;
  }
  return -1;
}

function inspectRasterImage(bytes: Uint8Array): RasterImageInspection | null {
  if (hasBytes(bytes, 0, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) {
    return inspectPng(bytes);
  }
  if (asciiAt(bytes, 0, "GIF87a") || asciiAt(bytes, 0, "GIF89a")) {
    return inspectGif(bytes);
  }
  if (hasBytes(bytes, 0, [0xff, 0xd8])) {
    return inspectJpeg(bytes);
  }
  if (asciiAt(bytes, 0, "RIFF") && asciiAt(bytes, 8, "WEBP")) {
    return inspectWebp(bytes);
  }
  return null;
}

function inspectPng(bytes: Uint8Array): RasterImageInspection | null {
  let cursor = 8;
  let width = 0;
  let height = 0;
  let sawHeader = false;
  let sawPalette = false;
  let sawImageData = false;
  let imageDataEnded = false;
  let sawEnd = false;
  let animated = false;

  while (cursor < bytes.length) {
    if (bytes.length - cursor < 12) {
      return null;
    }
    const length = readU32Be(bytes, cursor);
    const type = asciiSlice(bytes, cursor + 4, 4);
    if (length === null || type === null || length > bytes.length - cursor - 12) {
      return null;
    }
    if (!PNG_STATIC_CHUNKS.has(type) && !PNG_ANIMATION_CHUNKS.has(type)) {
      return null;
    }
    const dataStart = cursor + 8;
    const next = dataStart + length + 4;

    if (!sawHeader) {
      if (type !== "IHDR" || length !== 13) {
        return null;
      }
      const parsedWidth = readU32Be(bytes, dataStart);
      const parsedHeight = readU32Be(bytes, dataStart + 4);
      if (
        parsedWidth === null ||
        parsedHeight === null ||
        parsedWidth === 0 ||
        parsedHeight === 0 ||
        !validPngHeader(bytes.subarray(dataStart, dataStart + length))
      ) {
        return null;
      }
      width = parsedWidth;
      height = parsedHeight;
      sawHeader = true;
    } else if (type === "IHDR") {
      return null;
    }

    if (PNG_ANIMATION_CHUNKS.has(type)) {
      animated = true;
    }
    if (PNG_COMPRESSED_METADATA_CHUNKS.has(type)) {
      return null;
    }
    if (type === "iTXt") {
      const compressed = inspectPngInternationalTextCompression(bytes, dataStart, length);
      if (compressed === null || compressed) {
        return null;
      }
    }
    if (type === "PLTE") {
      if (sawPalette || sawImageData || length === 0 || length % 3 !== 0 || length > 768) {
        return null;
      }
      sawPalette = true;
    } else if (type === "IDAT") {
      if (imageDataEnded) {
        return null;
      }
      sawImageData = true;
    } else if (type === "IEND") {
      if (length !== 0 || next !== bytes.length) {
        return null;
      }
      sawEnd = true;
      cursor = next;
      break;
    } else if (sawImageData) {
      imageDataEnded = true;
    }
    cursor = next;
  }

  return sawHeader && sawImageData && sawEnd && cursor === bytes.length
    ? { format: "png", width, height, animated }
    : null;
}

function validPngHeader(header: Uint8Array): boolean {
  const bitDepth = header[8];
  const colorType = header[9];
  const allowedDepths =
    colorType === 0
      ? [1, 2, 4, 8, 16]
      : colorType === 2
        ? [8, 16]
        : colorType === 3
          ? [1, 2, 4, 8]
          : colorType === 4 || colorType === 6
            ? [8, 16]
            : [];
  return (
    bitDepth !== undefined &&
    allowedDepths.includes(bitDepth) &&
    header[10] === 0 &&
    header[11] === 0 &&
    (header[12] === 0 || header[12] === 1)
  );
}

function inspectPngInternationalTextCompression(
  bytes: Uint8Array,
  start: number,
  length: number,
): boolean | null {
  const end = start + length;
  const keywordEnd = findByte(bytes, start, end, 0);
  if (keywordEnd === null || keywordEnd === start || keywordEnd - start > 79) {
    return null;
  }
  const compressionFlag = bytes[keywordEnd + 1];
  const compressionMethod = bytes[keywordEnd + 2];
  if (
    (compressionFlag !== 0 && compressionFlag !== 1) ||
    compressionMethod !== 0
  ) {
    return null;
  }
  const languageEnd = findByte(bytes, keywordEnd + 3, end, 0);
  if (languageEnd === null) {
    return null;
  }
  const translatedKeywordEnd = findByte(bytes, languageEnd + 1, end, 0);
  if (translatedKeywordEnd === null) {
    return null;
  }
  return compressionFlag === 1;
}

function findByte(
  bytes: Uint8Array,
  start: number,
  end: number,
  expected: number,
): number | null {
  for (let index = start; index < end; index += 1) {
    if (bytes[index] === expected) {
      return index;
    }
  }
  return null;
}

function inspectGif(bytes: Uint8Array): RasterImageInspection | null {
  if (bytes.length < 14) {
    return null;
  }
  const width = readU16Le(bytes, 6);
  const height = readU16Le(bytes, 8);
  const packed = bytes[10];
  if (width === null || height === null || packed === undefined || width === 0 || height === 0) {
    return null;
  }

  let cursor = 13;
  const hasGlobalColorTable = (packed & 0x80) !== 0;
  if (hasGlobalColorTable) {
    const colorTableBytes = 3 * 2 ** ((packed & 0x07) + 1);
    if (colorTableBytes > bytes.length - cursor) {
      return null;
    }
    cursor += colorTableBytes;
  }

  let imageDescriptors = 0;
  let animationSignaled = false;
  let pendingGraphicControl = false;
  let sawTrailer = false;
  while (cursor < bytes.length) {
    const block = bytes[cursor];
    cursor += 1;
    if (block === 0x3b) {
      sawTrailer = cursor === bytes.length;
      break;
    }
    if (block === 0x21) {
      const label = bytes[cursor];
      if (label === undefined) {
        return null;
      }
      cursor += 1;
      if (label === 0xf9) {
        const control = bytes[cursor + 1];
        if (
          pendingGraphicControl ||
          bytes[cursor] !== 4 ||
          control === undefined ||
          (control & 0xe0) !== 0 ||
          ((control >> 2) & 0x07) > 3 ||
          (control & 0x02) !== 0 ||
          bytes[cursor + 5] !== 0
        ) {
          return null;
        }
        pendingGraphicControl = true;
        cursor += 6;
        continue;
      }
      if (label === 0xfe) {
        const next = skipGifSubBlocks(bytes, cursor);
        if (next === null) {
          return null;
        }
        cursor = next;
        continue;
      }
      if (label !== 0xff || bytes[cursor] !== 11) {
        return null;
      }
      const application = asciiSlice(bytes, cursor + 1, 11);
      const next = skipGifSubBlocks(bytes, cursor);
      if (
        next === null ||
        (application !== "NETSCAPE2.0" && application !== "ANIMEXTS1.0")
      ) {
        return null;
      }
      animationSignaled = true;
      cursor = next;
      continue;
    }
    if (block !== 0x2c || bytes.length - cursor < 9) {
      return null;
    }

    imageDescriptors += 1;
    const imageLeft = readU16Le(bytes, cursor);
    const imageTop = readU16Le(bytes, cursor + 2);
    const imageWidth = readU16Le(bytes, cursor + 4);
    const imageHeight = readU16Le(bytes, cursor + 6);
    const imagePacked = bytes[cursor + 8];
    if (
      imageLeft === null ||
      imageTop === null ||
      imageWidth === null ||
      imageHeight === null ||
      imagePacked === undefined ||
      (imagePacked & 0x18) !== 0 ||
      imageWidth === 0 ||
      imageHeight === 0 ||
      imageLeft + imageWidth > width ||
      imageTop + imageHeight > height
    ) {
      return null;
    }
    cursor += 9;
    const hasLocalColorTable = (imagePacked & 0x80) !== 0;
    if (hasLocalColorTable) {
      const colorTableBytes = 3 * 2 ** ((imagePacked & 0x07) + 1);
      if (colorTableBytes > bytes.length - cursor) {
        return null;
      }
      cursor += colorTableBytes;
    }
    const minimumCodeSize = bytes[cursor];
    if (
      (!hasGlobalColorTable && !hasLocalColorTable) ||
      minimumCodeSize === undefined ||
      minimumCodeSize < 2 ||
      minimumCodeSize > 8
    ) {
      return null;
    }
    cursor += 1;
    const next = skipGifSubBlocks(bytes, cursor);
    if (next === null) {
      return null;
    }
    cursor = next;
    pendingGraphicControl = false;
  }

  return sawTrailer && imageDescriptors > 0 && !pendingGraphicControl
    ? {
        format: "gif",
        width,
        height,
        animated: animationSignaled || imageDescriptors > 1,
      }
    : null;
}

function skipGifSubBlocks(bytes: Uint8Array, start: number): number | null {
  let cursor = start;
  while (cursor < bytes.length) {
    const length = bytes[cursor];
    if (length === undefined) {
      return null;
    }
    cursor += 1;
    if (length === 0) {
      return cursor;
    }
    if (length > bytes.length - cursor) {
      return null;
    }
    cursor += length;
  }
  return null;
}

function inspectJpeg(bytes: Uint8Array): RasterImageInspection | null {
  let cursor = 2;
  let width = 0;
  let height = 0;
  let sawFrame = false;
  let sawScan = false;

  while (cursor < bytes.length) {
    if (bytes[cursor] !== 0xff) {
      return null;
    }
    const markerStart = cursor;
    while (bytes[cursor] === 0xff) {
      cursor += 1;
    }
    const marker = bytes[cursor];
    if (marker === undefined || marker === 0x00 || marker === 0xff) {
      return null;
    }
    cursor += 1;
    if (marker === 0xd9) {
      return sawFrame && sawScan && height > 0 && cursor === bytes.length
        ? { format: "jpeg", width, height, animated: false }
        : null;
    }
    if (marker === 0xd8 || marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) {
      continue;
    }

    const segmentLength = readU16Be(bytes, cursor);
    if (segmentLength === null || segmentLength < 2 || segmentLength > bytes.length - cursor) {
      return null;
    }
    const segmentStart = cursor + 2;
    const segmentEnd = cursor + segmentLength;
    if (isJpegStartOfFrame(marker)) {
      if (sawFrame || segmentLength < 8) {
        return null;
      }
      const parsedHeight = readU16Be(bytes, segmentStart + 1);
      const parsedWidth = readU16Be(bytes, segmentStart + 3);
      if (
        parsedWidth === null ||
        parsedHeight === null ||
        parsedWidth === 0
      ) {
        return null;
      }
      width = parsedWidth;
      height = parsedHeight;
      sawFrame = true;
    } else if (marker === 0xdc) {
      const definedHeight = readU16Be(bytes, segmentStart);
      if (
        segmentLength !== 4 ||
        !sawFrame ||
        !sawScan ||
        height !== 0 ||
        definedHeight === null ||
        definedHeight === 0
      ) {
        return null;
      }
      height = definedHeight;
    }
    cursor = segmentEnd;
    if (marker !== 0xda) {
      continue;
    }

    sawScan = true;
    while (cursor < bytes.length) {
      const markerPrefix = bytes.indexOf(0xff, cursor);
      if (markerPrefix < 0 || markerPrefix + 1 >= bytes.length) {
        return null;
      }
      const next = bytes[markerPrefix + 1];
      if (next === 0x00 || (next !== undefined && next >= 0xd0 && next <= 0xd7)) {
        cursor = markerPrefix + 2;
        continue;
      }
      cursor = markerPrefix;
      break;
    }
    if (cursor === markerStart) {
      return null;
    }
  }
  return null;
}

function isJpegStartOfFrame(marker: number): boolean {
  return (
    marker >= 0xc0 &&
    marker <= 0xcf &&
    marker !== 0xc4 &&
    marker !== 0xc8 &&
    marker !== 0xcc
  );
}

function inspectWebp(bytes: Uint8Array): RasterImageInspection | null {
  if (bytes.length < 20) {
    return null;
  }
  const riffLength = readU32Le(bytes, 4);
  if (riffLength === null || riffLength + 8 !== bytes.length) {
    return null;
  }

  let cursor = 12;
  let width = 0;
  let height = 0;
  let sawDimensions = false;
  let sawExtendedHeader = false;
  let sawImageData = false;
  let sawAlpha = false;
  let animated = false;
  while (cursor < bytes.length) {
    if (bytes.length - cursor < 8) {
      return null;
    }
    const chunkType = asciiSlice(bytes, cursor, 4);
    const chunkLength = readU32Le(bytes, cursor + 4);
    if (chunkType === null || chunkLength === null || chunkLength > bytes.length - cursor - 8) {
      return null;
    }
    const dataStart = cursor + 8;
    const paddedLength = chunkLength + (chunkLength & 1);
    if (paddedLength > bytes.length - dataStart) {
      return null;
    }
    if ((chunkLength & 1) !== 0 && bytes[dataStart + chunkLength] !== 0) {
      return null;
    }
    if (
      cursor === 12 &&
      chunkType !== "VP8X" &&
      chunkType !== "VP8 " &&
      chunkType !== "VP8L"
    ) {
      return null;
    }
    if (cursor > 12 && !sawExtendedHeader) {
      return null;
    }

    if (chunkType === "VP8X") {
      if (cursor !== 12 || sawDimensions || chunkLength !== 10) {
        return null;
      }
      const flags = bytes[dataStart];
      const parsedWidth = readU24Le(bytes, dataStart + 4);
      const parsedHeight = readU24Le(bytes, dataStart + 7);
      if (
        flags === undefined ||
        (flags & 0xc1) !== 0 ||
        bytes[dataStart + 1] !== 0 ||
        bytes[dataStart + 2] !== 0 ||
        bytes[dataStart + 3] !== 0 ||
        parsedWidth === null ||
        parsedHeight === null ||
        (parsedWidth + 1) * (parsedHeight + 1) > 0xffffffff
      ) {
        return null;
      }
      width = parsedWidth + 1;
      height = parsedHeight + 1;
      animated = (flags & 0x02) !== 0;
      sawDimensions = true;
      sawExtendedHeader = true;
    } else if (chunkType === "VP8 ") {
      if (chunkLength < 10 || sawImageData) {
        return null;
      }
      const frameTag = bytes[dataStart];
      if (
        frameTag === undefined ||
        (frameTag & 0x01) !== 0 ||
        !hasBytes(bytes, dataStart + 3, [0x9d, 0x01, 0x2a])
      ) {
        return null;
      }
      const parsedWidth = readU16Le(bytes, dataStart + 6);
      const parsedHeight = readU16Le(bytes, dataStart + 8);
      if (parsedWidth === null || parsedHeight === null) {
        return null;
      }
      const frameWidth = parsedWidth & 0x3fff;
      const frameHeight = parsedHeight & 0x3fff;
      if (frameWidth === 0 || frameHeight === 0) {
        return null;
      }
      if (sawExtendedHeader) {
        if (frameWidth !== width || frameHeight !== height) {
          return null;
        }
      } else if (!sawDimensions) {
        width = frameWidth;
        height = frameHeight;
        sawDimensions = true;
      }
      sawImageData = true;
    } else if (chunkType === "VP8L") {
      if (chunkLength < 5 || sawImageData || sawAlpha || bytes[dataStart] !== 0x2f) {
        return null;
      }
      const b1 = bytes[dataStart + 1];
      const b2 = bytes[dataStart + 2];
      const b3 = bytes[dataStart + 3];
      const b4 = bytes[dataStart + 4];
      if (
        b1 === undefined ||
        b2 === undefined ||
        b3 === undefined ||
        b4 === undefined ||
        (b4 & 0xe0) !== 0
      ) {
        return null;
      }
      const frameWidth = 1 + b1 + ((b2 & 0x3f) << 8);
      const frameHeight = 1 + (b2 >> 6) + (b3 << 2) + ((b4 & 0x0f) << 10);
      if (sawExtendedHeader) {
        if (frameWidth !== width || frameHeight !== height) {
          return null;
        }
      } else if (!sawDimensions) {
        width = frameWidth;
        height = frameHeight;
        sawDimensions = true;
      }
      sawImageData = true;
    } else if (chunkType === "ANIM" || chunkType === "ANMF") {
      animated = true;
    } else if (chunkType === "ALPH") {
      if (!sawExtendedHeader || sawAlpha || sawImageData || chunkLength === 0) {
        return null;
      }
      sawAlpha = true;
    } else if (
      chunkType !== "ICCP" &&
      chunkType !== "EXIF" &&
      chunkType !== "XMP "
    ) {
      return null;
    }

    cursor = dataStart + paddedLength;
  }

  return cursor === bytes.length && sawDimensions && (sawImageData || animated)
    ? { format: "webp", width, height, animated }
    : null;
}

function readU16Be(bytes: Uint8Array, offset: number): number | null {
  const a = bytes[offset];
  const b = bytes[offset + 1];
  return a === undefined || b === undefined ? null : a * 0x100 + b;
}

function readU16Le(bytes: Uint8Array, offset: number): number | null {
  const a = bytes[offset];
  const b = bytes[offset + 1];
  return a === undefined || b === undefined ? null : a + b * 0x100;
}

function readU24Le(bytes: Uint8Array, offset: number): number | null {
  const a = bytes[offset];
  const b = bytes[offset + 1];
  const c = bytes[offset + 2];
  return a === undefined || b === undefined || c === undefined
    ? null
    : a + b * 0x100 + c * 0x10000;
}

function readU32Be(bytes: Uint8Array, offset: number): number | null {
  const a = bytes[offset];
  const b = bytes[offset + 1];
  const c = bytes[offset + 2];
  const d = bytes[offset + 3];
  return a === undefined || b === undefined || c === undefined || d === undefined
    ? null
    : a * 0x1000000 + b * 0x10000 + c * 0x100 + d;
}

function readU32Le(bytes: Uint8Array, offset: number): number | null {
  const a = bytes[offset];
  const b = bytes[offset + 1];
  const c = bytes[offset + 2];
  const d = bytes[offset + 3];
  return a === undefined || b === undefined || c === undefined || d === undefined
    ? null
    : a + b * 0x100 + c * 0x10000 + d * 0x1000000;
}

function hasBytes(bytes: Uint8Array, offset: number, expected: readonly number[]): boolean {
  return expected.every((value, index) => bytes[offset + index] === value);
}

function asciiAt(bytes: Uint8Array, offset: number, expected: string): boolean {
  return asciiSlice(bytes, offset, expected.length) === expected;
}

function asciiSlice(bytes: Uint8Array, offset: number, length: number): string | null {
  if (offset < 0 || length < 0 || length > bytes.length - offset) {
    return null;
  }
  let value = "";
  for (let index = 0; index < length; index += 1) {
    const byte = bytes[offset + index];
    if (byte === undefined || byte > 0x7f) {
      return null;
    }
    value += String.fromCharCode(byte);
  }
  return value;
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

function containsCssFunction(css: string, name: string): boolean {
  let cursor = 0;
  while (cursor < css.length) {
    const index = css.indexOf(name, cursor);
    if (index < 0) {
      return false;
    }
    cursor = index + name.length;
    const before = index === 0 ? "" : css[index - 1] ?? "";
    const after = css[cursor] ?? "";
    if (
      !isCssIdentifierChar(before) &&
      !isCssIdentifierChar(after) &&
      css[skipWhitespace(css, cursor)] === "("
    ) {
      return true;
    }
  }
  return false;
}

function containsShadowScopingSelector(css: string): boolean {
  return css.includes(":host") || css.includes("::slotted");
}

function containsViewportEscapingCssDeclaration(css: string): boolean {
  return cssDeclarationBlocks(css).some((block) => {
    if (/(?:^|[;{\s])position\s*:\s*(?:fixed|sticky)\b/.test(block)) {
      return true;
    }
    return (
      /(?:^|[;{\s])position\s*:\s*absolute\b/.test(block) &&
      /(?:^|[;{\s])(?:inset(?:-(?:block|inline)(?:-(?:start|end))?)?|top|right|bottom|left)\s*:/.test(
        block,
      )
    );
  });
}

function cssDeclarationBlocks(css: string): string[] {
  const blocks: string[] = [];
  let cursor = 0;
  while (cursor < css.length) {
    const open = css.indexOf("{", cursor);
    if (open < 0) {
      break;
    }
    const close = css.indexOf("}", open + 1);
    if (close < 0) {
      break;
    }
    blocks.push(css.slice(open + 1, close));
    cursor = close + 1;
  }
  return blocks.length > 0 ? blocks : [css];
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
      if (value[hexCursor] === "\r" && value[hexCursor + 1] === "\n") {
        hexCursor += 2;
      } else if (isWhitespace(value[hexCursor] ?? "")) {
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
  let output = "";
  let cursor = 0;

  while (cursor < value.length) {
    const ampersand = value.indexOf("&", cursor);
    if (ampersand < 0) {
      output += value.slice(cursor);
      break;
    }

    output += value.slice(cursor, ampersand);
    const next = value[ampersand + 1] ?? "";
    if (next === "#") {
      const parsed = decodeNumericCharacterReference(value, ampersand);
      if (parsed) {
        output += parsed.value;
        cursor = parsed.end;
        continue;
      }
    }

    const parsed = decodeNamedCharacterReference(value, ampersand);
    if (parsed) {
      output += parsed.value;
      cursor = parsed.end;
      continue;
    }

    output += "&";
    cursor = ampersand + 1;
  }

  return output;
}

function decodeNumericCharacterReference(
  value: string,
  ampersand: number,
): { value: string; end: number } | null {
  let cursor = ampersand + 2;
  let radix = 10;
  if (value[cursor]?.toLowerCase() === "x") {
    radix = 16;
    cursor += 1;
  }

  const digitsStart = cursor;
  while (
    cursor < value.length &&
    (radix === 16 ? isHexDigit(value[cursor] ?? "") : isAsciiDigit(value[cursor] ?? ""))
  ) {
    cursor += 1;
  }

  if (cursor === digitsStart) {
    return null;
  }

  const body = value.slice(digitsStart, cursor);
  const end = value[cursor] === ";" ? cursor + 1 : cursor;
  return {
    value: codePointToString(Number.parseInt(body, radix), value.slice(ampersand, end)),
    end,
  };
}

const NAMED_CHARACTER_REFERENCES = new Map<string, string>([
  ["amp", "&"],
  ["lt", "<"],
  ["gt", ">"],
  ["quot", '"'],
  ["apos", "'"],
  ["colon", ":"],
  ["sol", "/"],
  ["lpar", "("],
  ["rpar", ")"],
  ["newline", "\n"],
  ["tab", "\t"],
]);

function decodeNamedCharacterReference(
  value: string,
  ampersand: number,
): { value: string; end: number } | null {
  let cursor = ampersand + 1;
  while (cursor < value.length && isAsciiAlphanumeric(value[cursor] ?? "")) {
    cursor += 1;
  }
  if (value[cursor] !== ";") {
    return null;
  }

  const replacement = NAMED_CHARACTER_REFERENCES.get(
    value.slice(ampersand + 1, cursor).toLowerCase(),
  );
  if (replacement === undefined) {
    return null;
  }

  return { value: replacement, end: cursor + 1 };
}

function containsCharacterReference(value: string): boolean {
  return /&(?:#x[0-9a-f]+|#\d+|[a-z][a-z0-9]+);?/i.test(value);
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

function isAsciiDigit(value: string): boolean {
  return value >= "0" && value <= "9";
}

function isAsciiAlphanumeric(value: string): boolean {
  return (
    (value >= "0" && value <= "9") ||
    (value >= "A" && value <= "Z") ||
    (value >= "a" && value <= "z")
  );
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

function utf8LengthAtMost(value: string, limit: number): boolean {
  let bytes = 0;
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit <= 0x7f) {
      bytes += 1;
    } else if (codeUnit <= 0x7ff) {
      bytes += 2;
    } else if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (next >= 0xdc00 && next <= 0xdfff) {
        bytes += 4;
        index += 1;
      } else {
        bytes += 3;
      }
    } else {
      bytes += 3;
    }
    if (bytes > limit) {
      return false;
    }
  }
  return true;
}

function localName(name: string): string {
  const lower = name.toLowerCase();
  const separator = lower.lastIndexOf(":");
  return separator >= 0 ? lower.slice(separator + 1) : lower;
}

function findClosingStyle(source: string, start: number): number | null {
  let index = source.indexOf("<", start);
  while (index >= 0) {
    if (matchesAsciiCaseInsensitive(source, index, "</style")) {
      return index;
    }
    index = source.indexOf("<", index + 1);
  }
  return null;
}

function matchesAsciiCaseInsensitive(
  source: string,
  start: number,
  lowerAscii: string,
): boolean {
  if (start + lowerAscii.length > source.length) {
    return false;
  }
  for (let offset = 0; offset < lowerAscii.length; offset += 1) {
    const code = source.charCodeAt(start + offset);
    const normalized = code >= 0x41 && code <= 0x5a ? code + 0x20 : code;
    if (normalized !== lowerAscii.charCodeAt(offset)) {
      return false;
    }
  }
  return true;
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

function isOnlyWhitespace(value: string): boolean {
  for (const char of value) {
    if (!isWhitespace(char)) {
      return false;
    }
  }
  return true;
}

function isHexDigit(char: string): boolean {
  return (
    (char >= "0" && char <= "9") ||
    (char >= "a" && char <= "f") ||
    (char >= "A" && char <= "F")
  );
}

function isCssIdentifierChar(char: string): boolean {
  return (
    (char >= "a" && char <= "z") ||
    (char >= "A" && char <= "Z") ||
    (char >= "0" && char <= "9") ||
    char === "_" ||
    char === "-"
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
