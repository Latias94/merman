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

const FOREIGN_OBJECT_LABEL_ELEMENTS = new Set([
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
  selfClosing: boolean;
}

interface SvgAttribute {
  name: string;
  value: string;
}

export function assertSafeSvgWithMessagePrefix(svg: string, messagePrefix: string): void {
  const scanner = new SvgSafetyScanner(svg, messagePrefix);
  scanner.assertSafe();
}

class SvgSafetyScanner {
  private cursor = 0;
  private sawRoot = false;
  private rootDepth = 0;
  private rootClosedAt: number | null = null;
  private foreignObjectDepth = 0;

  constructor(
    private readonly source: string,
    private readonly messagePrefix: string,
  ) {}

  assertSafe(): void {
    while (this.cursor < this.source.length) {
      const tag = this.nextTag();
      if (!tag) {
        break;
      }
      if (this.rootClosedAt !== null) {
        throw this.error("malformed SVG output.");
      }
      if (tag.kind === "end") {
        if (localName(tag.name) === "foreignobject" && this.foreignObjectDepth > 0) {
          this.foreignObjectDepth -= 1;
        }
        if (!this.sawRoot || this.rootDepth === 0) {
          throw this.error("malformed SVG output.");
        }
        this.rootDepth -= 1;
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
          this.rootDepth = 1;
        } else {
          this.rootClosedAt = tag.end;
        }
      } else if (!tag.selfClosing) {
        this.rootDepth += 1;
      }
      this.assertSafeElementName(elementName, inForeignObject);
      this.assertSafeAttributes(tag.attributes, inForeignObject);

      if (elementName === "foreignobject" && !tag.selfClosing) {
        this.foreignObjectDepth += 1;
      }

      if (elementName === "style") {
        const styleEnd = findClosingStyle(this.source, tag.end);
        const styleText = this.source.slice(tag.end, styleEnd ?? this.source.length);
        this.assertSafeCss(styleText);
        if (styleEnd !== null) {
          this.cursor = styleEnd;
        }
      }
    }

    if (!this.sawRoot) {
      throw this.error("non-SVG output.");
    }
    if (this.rootClosedAt === null || this.rootDepth !== 0) {
      throw this.error("malformed SVG output.");
    }
    this.assertOnlyIgnorableRootTail(this.rootClosedAt);
    if (this.foreignObjectDepth !== 0) {
      throw this.error("malformed SVG output.");
    }
  }

  private nextTag(): SvgTag | null {
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
      return this.nextTag();
    }
    if (this.source.startsWith("<?", start)) {
      this.cursor = this.consumeUntil(start + 2, "?>");
      return this.nextTag();
    }
    if (this.source.startsWith("<![CDATA[", start)) {
      this.cursor = this.consumeUntil(start + 9, "]]>");
      return this.nextTag();
    }
    if (this.source.startsWith("<!", start)) {
      throw this.error("SVG with unsupported declarations.");
    }

    const tag = this.parseTag(start);
    this.cursor = tag.end;
    return tag;
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
    if (inForeignObject && !FOREIGN_OBJECT_LABEL_ELEMENTS.has(name)) {
      throw this.error("SVG with unsupported foreignObject content.");
    }
    if (ACTIVE_SVG_ELEMENTS.has(name)) {
      throw this.error("SVG with active embedded content.");
    }
  }

  private assertSafeAttributes(attributes: SvgAttribute[], inForeignObject: boolean): void {
    for (const attribute of attributes) {
      const name = attribute.name.toLowerCase();
      const nameWithoutNamespace = localName(name);
      const value = decodeXmlEntities(attribute.value);
      if (nameWithoutNamespace.startsWith("on")) {
        throw this.error("SVG with event handler attributes.");
      }
      if (nameWithoutNamespace === "base") {
        throw this.error("SVG with base URL attributes.");
      }
      if (nameWithoutNamespace === "srcset") {
        this.assertSafeSrcset(value);
      }
      if (RAW_URL_ATTRIBUTES.has(nameWithoutNamespace)) {
        this.assertSafeUrl(value, "attribute");
      }
      if (SVG_URL_REFERENCE_ATTRIBUTES.has(nameWithoutNamespace)) {
        this.assertSafeUrlReferences(value, "attribute");
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

  private assertSafeUrl(value: string, source: "attribute" | "css"): void {
    const normalized = decodeCssEscapes(decodeXmlEntities(value));
    const compact = removeAsciiWhitespaceAndControl(normalized).toLowerCase();
    const trimmed = normalized.trim().toLowerCase();
    if (compact.startsWith("#") || SAFE_RASTER_DATA_IMAGE_URL.test(compact)) {
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

  private assertSafeUrlReferences(value: string, source: "attribute" | "css"): void {
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
      this.assertSafeUrl(unquoted, source);
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
      this.assertSafeUrl(normalized, source);
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
