import { Unzlib, zlibSync } from "fflate";
import { isBundledThemePresetName, isThemeName } from "@mermanjs/web";

import { isDiagramFont } from "./diagram-font.ts";
import { exceedsUtf8ByteBudget, utf8ByteLength } from "./utf8.ts";
import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  type WorkspaceSnapshot,
} from "./workspace-snapshot.ts";
import { isMermanSvgPipeline } from "../runtime/merman-core.ts";

const SHARE_SOURCE_BYTES = 2 * 1024 * 1024;
const SHARE_CONFIG_BYTES = 1024 * 1024;
const SHARE_JSON_OVERHEAD_BYTES = 16 * 1024;
const SHARE_PRESENTATION_ID_BYTES = 16 * 1024;
const SHARE_V2_ENCODED_BYTES = 512 * 1024;
const SHARE_DECOMPRESSION_CHUNK_BYTES = 512;

export const SHARE_LIMITS = Object.freeze({
  encodedBytes: SHARE_V2_ENCODED_BYTES,
  jsonBytes:
    SHARE_SOURCE_BYTES + SHARE_CONFIG_BYTES + SHARE_JSON_OVERHEAD_BYTES,
  sourceBytes: SHARE_SOURCE_BYTES,
  configBytes: SHARE_CONFIG_BYTES,
  idBytes: SHARE_PRESENTATION_ID_BYTES,
});

export const SHARE_V2_PREFIX = "#s2:" as const;

// This is the complete default snapshot owned by the s2 wire format. Keep it
// independent from application defaults so future UI changes cannot reinterpret
// fields omitted by an already-shared URL.
export const WORKSPACE_V2_DEFAULTS: Readonly<WorkspaceSnapshot> = Object.freeze({
  code: `flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D`,
  mermaidConfig: "{\n}\n",
  diagramTheme: "default",
  presentationThemePresetId: null,
  presentationProfileId: null,
  svgPipeline: "parity",
  textMeasurementMode: "browser",
  diagramFont: "trebuchet",
});

const MAX_LEGACY_ENCODED_HASH_CHARS = SHARE_LIMITS.jsonBytes * 4 + 4;
// Decoder-only compatibility for Host-bearing links created on the open branch.
const LEGACY_RENDER_VIEWPORT_KEY = "renderViewportMode";
const WORKSPACE_V2_KEYS = new Set([
  "code",
  "theme",
  "config",
  "presentationThemePresetId",
  "presentationProfileId",
  LEGACY_RENDER_VIEWPORT_KEY,
  "svgPipeline",
  "textMeasurementMode",
  "diagramFont",
]);
const utf8Encoder = new TextEncoder();
const utf8Decoder = new TextDecoder("utf-8", { fatal: true });

export function encodeShareHash(data: WorkspaceSnapshot): string {
  if (!isValidShareSnapshot(data)) {
    throw new RangeError("Workspace exceeds the share URL contract.");
  }

  const json = JSON.stringify(encodeWorkspaceV2Payload(data));
  if (utf8ByteLength(json) > SHARE_LIMITS.jsonBytes) {
    throw new RangeError("Workspace exceeds the share URL contract.");
  }

  const encoded = encodeBase64Url(zlibSync(utf8Encoder.encode(json)));
  if (SHARE_V2_PREFIX.length + encoded.length > SHARE_LIMITS.encodedBytes) {
    throw new RangeError("Workspace exceeds the share URL contract.");
  }
  return `${SHARE_V2_PREFIX}${encoded}`;
}

export function decodeShareHash(
  hash: string,
  legacyDefaults: Readonly<WorkspaceSnapshot> = DEFAULT_WORKSPACE_SNAPSHOT
): WorkspaceSnapshot | null {
  const fragment = hash.startsWith("#") ? hash : `#${hash}`;
  if (fragment.startsWith(SHARE_V2_PREFIX)) {
    return decodeWorkspaceV2(fragment.slice(SHARE_V2_PREFIX.length));
  }
  return decodeLegacyWorkspaceHash(hash, legacyDefaults);
}

export interface ShareLocation {
  readonly origin: string;
  readonly pathname: string;
}

export interface ShareCommandEnvironment extends ShareLocation {
  writeClipboardText(value: string): Promise<void>;
}

export function createWorkspaceShareUrl(
  data: WorkspaceSnapshot,
  location: ShareLocation = window.location
): string {
  return `${location.origin}${location.pathname}${encodeShareHash(data)}`;
}

export const createShareUrl = createWorkspaceShareUrl;

export async function copyWorkspaceShareUrl(
  data: WorkspaceSnapshot,
  environment: ShareCommandEnvironment = browserShareEnvironment()
): Promise<void> {
  await environment.writeClipboardText(createWorkspaceShareUrl(data, environment));
}

export const copyShareUrl = copyWorkspaceShareUrl;

export function migrateLegacyHostTheme(value: unknown): Pick<
  WorkspaceSnapshot,
  "presentationThemePresetId" | "presentationProfileId" | "svgPipeline"
> {
  if (typeof value !== "string" || value === "none" || value === "mermaid") {
    return {
      presentationThemePresetId: null,
      presentationProfileId: null,
      svgPipeline: "parity",
    };
  }
  if (value === "merman-modern") {
    return {
      presentationThemePresetId: null,
      presentationProfileId: value,
      svgPipeline: "parity",
    };
  }
  return {
    presentationThemePresetId: value,
    presentationProfileId: null,
    svgPipeline: isBundledThemePresetName(value) ? "resvg-safe" : "parity",
  };
}

function encodeWorkspaceV2Payload(
  data: WorkspaceSnapshot
): Record<string, unknown> {
  const payload: Record<string, unknown> = {};
  if (data.code !== WORKSPACE_V2_DEFAULTS.code) payload.code = data.code;
  if (data.diagramTheme !== WORKSPACE_V2_DEFAULTS.diagramTheme) {
    payload.theme = data.diagramTheme;
  }
  if (data.mermaidConfig !== WORKSPACE_V2_DEFAULTS.mermaidConfig) {
    payload.config = data.mermaidConfig;
  }
  if (
    data.presentationThemePresetId !==
    WORKSPACE_V2_DEFAULTS.presentationThemePresetId
  ) {
    payload.presentationThemePresetId = data.presentationThemePresetId;
  }
  if (
    data.presentationProfileId !== WORKSPACE_V2_DEFAULTS.presentationProfileId
  ) {
    payload.presentationProfileId = data.presentationProfileId;
  }
  if (data.svgPipeline !== WORKSPACE_V2_DEFAULTS.svgPipeline) {
    payload.svgPipeline = data.svgPipeline;
  }
  if (
    data.textMeasurementMode !== WORKSPACE_V2_DEFAULTS.textMeasurementMode
  ) {
    payload.textMeasurementMode = data.textMeasurementMode;
  }
  if (data.diagramFont !== WORKSPACE_V2_DEFAULTS.diagramFont) {
    payload.diagramFont = data.diagramFont;
  }
  return payload;
}

function decodeWorkspaceV2(encoded: string): WorkspaceSnapshot | null {
  try {
    if (
      encoded.length === 0 ||
      SHARE_V2_PREFIX.length + encoded.length > SHARE_LIMITS.encodedBytes ||
      !/^[A-Za-z0-9_-]+$/u.test(encoded)
    ) {
      return null;
    }
    const compressed = decodeBase64Url(encoded);
    const jsonBytes = decompressWithinBudget(compressed);
    const value: unknown = JSON.parse(utf8Decoder.decode(jsonBytes));
    if (!isRecord(value) || !hasOnlyKeys(value, WORKSPACE_V2_KEYS)) return null;
    return decodeWorkspaceV2Record(value);
  } catch {
    return null;
  }
}

function decodeWorkspaceV2Record(
  value: Record<string, unknown>
): WorkspaceSnapshot | null {
  const code = optionalBoundedString(
    value,
    "code",
    WORKSPACE_V2_DEFAULTS.code,
    SHARE_LIMITS.sourceBytes
  );
  const diagramTheme = optionalEnum(
    value,
    "theme",
    WORKSPACE_V2_DEFAULTS.diagramTheme,
    isThemeNameValue
  );
  const mermaidConfig = optionalBoundedString(
    value,
    "config",
    WORKSPACE_V2_DEFAULTS.mermaidConfig,
    SHARE_LIMITS.configBytes
  );
  const textMeasurementMode = optionalEnum(
    value,
    "textMeasurementMode",
    WORKSPACE_V2_DEFAULTS.textMeasurementMode,
    isTextMeasurementMode
  );
  const diagramFont = optionalEnum(
    value,
    "diagramFont",
    WORKSPACE_V2_DEFAULTS.diagramFont,
    isDiagramFontValue
  );
  const presentation = decodeCurrentPresentation(value, WORKSPACE_V2_DEFAULTS);
  if (
    code === null ||
    diagramTheme === null ||
    mermaidConfig === null ||
    textMeasurementMode === null ||
    diagramFont === null ||
    !hasValidLegacyRenderViewportMode(value) ||
    presentation === null
  ) {
    return null;
  }

  return {
    code,
    diagramTheme,
    mermaidConfig,
    ...presentation,
    textMeasurementMode,
    diagramFont,
  };
}

function decodeLegacyWorkspaceHash(
  hash: string,
  defaults: Readonly<WorkspaceSnapshot>
): WorkspaceSnapshot | null {
  try {
    const base64 = hash.startsWith("#") ? hash.slice(1) : hash;
    if (!base64 || base64.length > MAX_LEGACY_ENCODED_HASH_CHARS) return null;
    const json = decodeURIComponent(atob(base64));
    if (utf8ByteLength(json) > SHARE_LIMITS.jsonBytes) return null;
    const value: unknown = JSON.parse(json);
    if (!isRecord(value)) return null;
    if (
      !isBoundedString(value.code, SHARE_LIMITS.sourceBytes) ||
      typeof value.theme !== "string" ||
      !isThemeName(value.theme)
    ) {
      return null;
    }

    const config = optionalBoundedString(
      value,
      "config",
      defaults.mermaidConfig,
      SHARE_LIMITS.configBytes
    );
    const textMeasurementMode = optionalEnum(
      value,
      "textMeasurementMode",
      defaults.textMeasurementMode,
      isTextMeasurementMode
    );
    const diagramFont = optionalEnum(
      value,
      "diagramFont",
      defaults.diagramFont,
      isDiagramFontValue
    );
    if (
      config === null ||
      textMeasurementMode === null ||
      diagramFont === null ||
      !hasValidLegacyRenderViewportMode(value)
    ) {
      return null;
    }

    const hasNewPresentation = [
      "presentationThemePresetId",
      "presentationProfileId",
      "svgPipeline",
    ].some((key) => Object.hasOwn(value, key));
    const presentation = hasNewPresentation
      ? decodeCurrentPresentation(value, defaults)
      : Object.hasOwn(value, "hostThemePreset")
        ? decodeLegacyPresentation(value.hostThemePreset)
        : selectDefaultPresentation(defaults);
    if (!presentation) return null;

    return {
      code: value.code,
      diagramTheme: value.theme,
      mermaidConfig: config,
      ...presentation,
      textMeasurementMode,
      diagramFont,
    };
  } catch {
    return null;
  }
}

function decompressWithinBudget(compressed: Uint8Array): Uint8Array {
  const chunks: Uint8Array[] = [];
  let totalBytes = 0;
  let complete = false;
  const decompressor = new Unzlib((chunk, final) => {
    if (totalBytes + chunk.byteLength > SHARE_LIMITS.jsonBytes) {
      throw new RangeError("Workspace exceeds the share URL contract.");
    }
    chunks.push(chunk);
    totalBytes += chunk.byteLength;
    complete = final;
  });
  for (
    let offset = 0;
    offset < compressed.byteLength;
    offset += SHARE_DECOMPRESSION_CHUNK_BYTES
  ) {
    const end = Math.min(
      offset + SHARE_DECOMPRESSION_CHUNK_BYTES,
      compressed.byteLength
    );
    decompressor.push(compressed.subarray(offset, end), end === compressed.byteLength);
  }
  if (!complete) throw new Error("Workspace share envelope is incomplete.");

  const result = new Uint8Array(totalBytes);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return result;
}

function encodeBase64Url(value: Uint8Array): string {
  let binary = "";
  const chunkSize = 32 * 1024;
  for (let offset = 0; offset < value.length; offset += chunkSize) {
    binary += String.fromCharCode(...value.subarray(offset, offset + chunkSize));
  }
  return btoa(binary)
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replace(/=+$/u, "");
}

function decodeBase64Url(value: string): Uint8Array {
  if (value.length % 4 === 1) {
    throw new Error("Workspace share envelope is malformed.");
  }
  const normalized = value.replaceAll("-", "+").replaceAll("_", "/");
  const binary = atob(normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "="));
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

function decodeCurrentPresentation(
  value: Record<string, unknown>,
  defaults: Readonly<WorkspaceSnapshot>
): Pick<
  WorkspaceSnapshot,
  "presentationThemePresetId" | "presentationProfileId" | "svgPipeline"
> | null {
  const presentationThemePresetId = optionalNullableString(
    value,
    "presentationThemePresetId",
    defaults.presentationThemePresetId,
    SHARE_LIMITS.idBytes
  );
  const presentationProfileId = optionalNullableString(
    value,
    "presentationProfileId",
    defaults.presentationProfileId,
    SHARE_LIMITS.idBytes
  );
  const svgPipeline = optionalEnum(
    value,
    "svgPipeline",
    defaults.svgPipeline,
    isMermanSvgPipeline
  );
  if (
    presentationThemePresetId === undefined ||
    presentationProfileId === undefined ||
    svgPipeline === null
  ) {
    return null;
  }
  return {
    presentationThemePresetId,
    presentationProfileId,
    svgPipeline,
  };
}

function decodeLegacyPresentation(
  value: unknown
): Pick<
  WorkspaceSnapshot,
  "presentationThemePresetId" | "presentationProfileId" | "svgPipeline"
> | null {
  if (value !== null && value !== undefined && !isOptionalId(value)) {
    return null;
  }
  return migrateLegacyHostTheme(value);
}

function selectDefaultPresentation(
  defaults: Readonly<WorkspaceSnapshot>
): Pick<
  WorkspaceSnapshot,
  "presentationThemePresetId" | "presentationProfileId" | "svgPipeline"
> {
  return {
    presentationThemePresetId: defaults.presentationThemePresetId,
    presentationProfileId: defaults.presentationProfileId,
    svgPipeline: defaults.svgPipeline,
  };
}

function optionalBoundedString(
  record: Record<string, unknown>,
  key: string,
  fallback: string,
  maxBytes: number
): string | null {
  if (!Object.hasOwn(record, key)) return fallback;
  return isBoundedString(record[key], maxBytes) ? record[key] : null;
}

function optionalNullableString(
  record: Record<string, unknown>,
  key: string,
  fallback: string | null,
  maxBytes: number
): string | null | undefined {
  if (!Object.hasOwn(record, key)) return fallback;
  const value = record[key];
  return value === null ||
    (typeof value === "string" && value.length > 0 && isBoundedString(value, maxBytes))
    ? value
    : undefined;
}

function isValidShareSnapshot(value: WorkspaceSnapshot): boolean {
  return (
    isBoundedString(value.code, SHARE_LIMITS.sourceBytes) &&
    isBoundedString(value.mermaidConfig, SHARE_LIMITS.configBytes) &&
    isThemeName(value.diagramTheme) &&
    isOptionalId(value.presentationThemePresetId) &&
    isOptionalId(value.presentationProfileId) &&
    isMermanSvgPipeline(value.svgPipeline) &&
    isTextMeasurementMode(value.textMeasurementMode) &&
    isDiagramFontValue(value.diagramFont)
  );
}

function isOptionalId(value: unknown): value is string | null {
  return (
    value === null ||
    (typeof value === "string" &&
      value.length > 0 &&
      isBoundedString(value, SHARE_LIMITS.idBytes))
  );
}

function optionalEnum<T extends string>(
  record: Record<string, unknown>,
  key: string,
  fallback: T,
  guard: (value: unknown) => value is T
): T | null {
  if (!Object.hasOwn(record, key)) return fallback;
  return guard(record[key]) ? record[key] : null;
}

function isTextMeasurementMode(
  value: unknown
): value is WorkspaceSnapshot["textMeasurementMode"] {
  return value === "browser" || value === "headless";
}

function isThemeNameValue(
  value: unknown
): value is WorkspaceSnapshot["diagramTheme"] {
  return typeof value === "string" && isThemeName(value);
}

function isDiagramFontValue(
  value: unknown
): value is WorkspaceSnapshot["diagramFont"] {
  return typeof value === "string" && isDiagramFont(value);
}

function hasValidLegacyRenderViewportMode(
  value: Record<string, unknown>,
): boolean {
  if (!Object.hasOwn(value, LEGACY_RENDER_VIEWPORT_KEY)) return true;
  const mode = value[LEGACY_RENDER_VIEWPORT_KEY];
  return mode === "canonical" || mode === "host";
}

function isBoundedString(value: unknown, maxBytes: number): value is string {
  return typeof value === "string" && !exceedsUtf8ByteBudget(value, maxBytes);
}

export function browserShareEnvironment(): ShareCommandEnvironment {
  return {
    origin: window.location.origin,
    pathname: window.location.pathname,
    writeClipboardText: (value) => navigator.clipboard.writeText(value),
  };
}

function hasOnlyKeys(
  value: Record<string, unknown>,
  allowed: ReadonlySet<string>
): boolean {
  return Object.keys(value).every((key) => allowed.has(key));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
