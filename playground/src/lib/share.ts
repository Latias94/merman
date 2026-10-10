import { Unzlib, zlibSync } from "fflate";
import { isThemeName } from "@mermanjs/web";

import { isMermaidThemeSelection } from "./mermaid-theme-name.ts";
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
const SHARE_THEME_PRESET_ID_BYTES = 16 * 1024;
const SHARE_V2_ENCODED_BYTES = 512 * 1024;
export const SHARE_THEME_RECIPE_BYTES = 256 * 1024;
const SHARE_DECOMPRESSION_CHUNK_BYTES = 512;

export const SHARE_LIMITS = Object.freeze({
  encodedBytes: SHARE_V2_ENCODED_BYTES,
  jsonBytes:
    SHARE_SOURCE_BYTES + SHARE_CONFIG_BYTES + SHARE_JSON_OVERHEAD_BYTES,
  sourceBytes: SHARE_SOURCE_BYTES,
  configBytes: SHARE_CONFIG_BYTES,
  idBytes: SHARE_THEME_PRESET_ID_BYTES,
});

export const SHARE_V2_PREFIX = "#s2:" as const;
export const SHARE_V3_PREFIX = "#s3:" as const;

export const SHARE_V3_LIMITS = Object.freeze({
  ...SHARE_LIMITS,
  encodedBytes: SHARE_LIMITS.encodedBytes + SHARE_THEME_RECIPE_BYTES,
  jsonBytes: SHARE_LIMITS.jsonBytes + SHARE_THEME_RECIPE_BYTES,
});

// This is the complete default snapshot owned by the s2 wire format. Keep it
// independent from application defaults so future UI changes cannot reinterpret
// fields omitted by an already-shared URL. The historical omitted/default theme
// is represented by `auto` because it never injected a Mermaid theme override.
export const WORKSPACE_V2_DEFAULTS: Readonly<WorkspaceSnapshot> = Object.freeze({
  code: `flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D`,
  mermaidConfig: "{\n}\n",
  diagramTheme: "auto",
  themePresetId: null,
  themeRecipeJson: null,
  svgPipeline: "parity",
  textMeasurementMode: "browser",
  diagramFont: "trebuchet",
});

// The s3 wire format owns its defaults independently of both the UI and s2.
export const WORKSPACE_V3_DEFAULTS: Readonly<WorkspaceSnapshot> = Object.freeze({
  code: `flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D`,
  mermaidConfig: "{\n}\n",
  diagramTheme: "auto",
  themePresetId: null,
  themeRecipeJson: null,
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
  "themeSelection",
  "config",
  "themePresetId",
  LEGACY_RENDER_VIEWPORT_KEY,
  "svgPipeline",
  "textMeasurementMode",
  "diagramFont",
]);
const WORKSPACE_V3_KEYS = new Set([...WORKSPACE_V2_KEYS, "themeRecipeJson"]);
const utf8Encoder = new TextEncoder();
const utf8Decoder = new TextDecoder("utf-8", { fatal: true });

export function encodeShareHash(data: WorkspaceSnapshot): string {
  if (!isValidShareSnapshot(data)) {
    throw new RangeError("Workspace exceeds the share URL contract.");
  }

  const customTheme = data.themeRecipeJson !== null;
  const prefix = customTheme ? SHARE_V3_PREFIX : SHARE_V2_PREFIX;
  const limits = customTheme ? SHARE_V3_LIMITS : SHARE_LIMITS;
  const defaults = customTheme ? WORKSPACE_V3_DEFAULTS : WORKSPACE_V2_DEFAULTS;
  const payload = encodeWorkspacePayload(data, defaults);
  if (customTheme) payload.themeRecipeJson = data.themeRecipeJson;
  const json = JSON.stringify(payload);
  if (utf8ByteLength(json) > limits.jsonBytes) {
    throw new RangeError("Workspace exceeds the share URL contract.");
  }

  const encoded = encodeBase64Url(zlibSync(utf8Encoder.encode(json)));
  if (prefix.length + encoded.length > limits.encodedBytes) {
    throw new RangeError("Workspace exceeds the share URL contract.");
  }
  return `${prefix}${encoded}`;
}

export function decodeShareHash(
  hash: string,
  legacyDefaults: Readonly<WorkspaceSnapshot> = DEFAULT_WORKSPACE_SNAPSHOT
): WorkspaceSnapshot | null {
  const fragment = hash.startsWith("#") ? hash : `#${hash}`;
  if (fragment.startsWith(SHARE_V2_PREFIX)) {
    return decodeCompressedWorkspace(fragment.slice(SHARE_V2_PREFIX.length), false);
  }
  if (fragment.startsWith(SHARE_V3_PREFIX)) {
    return decodeCompressedWorkspace(fragment.slice(SHARE_V3_PREFIX.length), true);
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

function encodeWorkspacePayload(
  data: WorkspaceSnapshot,
  defaults: Readonly<WorkspaceSnapshot>,
): Record<string, unknown> {
  const payload: Record<string, unknown> = {};
  if (data.code !== defaults.code) payload.code = data.code;
  if (data.diagramTheme !== defaults.diagramTheme) {
    payload.themeSelection = data.diagramTheme;
  }
  if (data.mermaidConfig !== defaults.mermaidConfig) {
    payload.config = data.mermaidConfig;
  }
  if (data.themePresetId !== defaults.themePresetId) {
    payload.themePresetId = data.themePresetId;
  }
  if (data.svgPipeline !== defaults.svgPipeline) {
    payload.svgPipeline = data.svgPipeline;
  }
  if (data.textMeasurementMode !== defaults.textMeasurementMode) {
    payload.textMeasurementMode = data.textMeasurementMode;
  }
  if (data.diagramFont !== defaults.diagramFont) {
    payload.diagramFont = data.diagramFont;
  }
  return payload;
}

function decodeCompressedWorkspace(
  encoded: string,
  customThemes: boolean,
): WorkspaceSnapshot | null {
  const limits = customThemes ? SHARE_V3_LIMITS : SHARE_LIMITS;
  const prefix = customThemes ? SHARE_V3_PREFIX : SHARE_V2_PREFIX;
  const defaults = customThemes ? WORKSPACE_V3_DEFAULTS : WORKSPACE_V2_DEFAULTS;
  const keys = customThemes ? WORKSPACE_V3_KEYS : WORKSPACE_V2_KEYS;
  try {
    if (
      encoded.length === 0 ||
      prefix.length + encoded.length > limits.encodedBytes ||
      !/^[A-Za-z0-9_-]+$/u.test(encoded)
    ) {
      return null;
    }
    const compressed = decodeBase64Url(encoded);
    const jsonBytes = decompressWithinBudget(compressed, limits.jsonBytes);
    const value: unknown = JSON.parse(utf8Decoder.decode(jsonBytes));
    if (!isRecord(value) || !hasOnlyKeys(value, keys)) return null;
    return decodeWorkspaceRecord(value, defaults);
  } catch {
    return null;
  }
}

function decodeWorkspaceRecord(
  value: Record<string, unknown>,
  defaults: Readonly<WorkspaceSnapshot>,
): WorkspaceSnapshot | null {
  const code = optionalBoundedString(
    value,
    "code",
    defaults.code,
    SHARE_LIMITS.sourceBytes
  );
  const diagramTheme = decodeThemeSelection(value, defaults.diagramTheme);
  const mermaidConfig = optionalBoundedString(
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
  const renderOptions = decodeCurrentRenderOptions(value, defaults);
  const themeRecipeJson = optionalNullableString(
    value,
    "themeRecipeJson",
    defaults.themeRecipeJson,
    SHARE_THEME_RECIPE_BYTES,
  );
  if (
    code === null ||
    diagramTheme === null ||
    mermaidConfig === null ||
    textMeasurementMode === null ||
    diagramFont === null ||
    !hasValidLegacyRenderViewportMode(value) ||
    renderOptions === null ||
    themeRecipeJson === undefined ||
    !isThemeRecipeJson(themeRecipeJson) ||
    (themeRecipeJson !== null && renderOptions.themePresetId !== null)
  ) {
    return null;
  }

  return {
    code,
    diagramTheme,
    mermaidConfig,
    ...renderOptions,
    themeRecipeJson,
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
      !hasValidLegacyRenderViewportMode(value) ||
      Object.hasOwn(value, "hostThemePreset") ||
      Object.hasOwn(value, "themeRecipeJson")
    ) {
      return null;
    }

    const renderOptions = ["themePresetId", "svgPipeline"].some((key) =>
      Object.hasOwn(value, key)
    )
      ? decodeCurrentRenderOptions(value, defaults)
      : selectDefaultRenderOptions(defaults);
    if (!renderOptions) return null;

    return {
      code: value.code,
      diagramTheme: value.theme === "default" ? "auto" : value.theme,
      mermaidConfig: config,
      ...renderOptions,
      themeRecipeJson: null,
      textMeasurementMode,
      diagramFont,
    };
  } catch {
    return null;
  }
}

function decompressWithinBudget(
  compressed: Uint8Array,
  maxBytes: number,
): Uint8Array {
  const chunks: Uint8Array[] = [];
  let totalBytes = 0;
  let complete = false;
  const decompressor = new Unzlib((chunk, final) => {
    if (totalBytes + chunk.byteLength > maxBytes) {
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

function decodeCurrentRenderOptions(
  value: Record<string, unknown>,
  defaults: Readonly<WorkspaceSnapshot>
): Pick<WorkspaceSnapshot, "themePresetId" | "svgPipeline"> | null {
  const themePresetId = optionalNullableString(
    value,
    "themePresetId",
    defaults.themePresetId,
    SHARE_LIMITS.idBytes
  );
  const svgPipeline = optionalEnum(
    value,
    "svgPipeline",
    defaults.svgPipeline,
    isMermanSvgPipeline
  );
  if (themePresetId === undefined || svgPipeline === null) {
    return null;
  }
  return {
    themePresetId,
    svgPipeline,
  };
}

function selectDefaultRenderOptions(
  defaults: Readonly<WorkspaceSnapshot>
): Pick<WorkspaceSnapshot, "themePresetId" | "svgPipeline"> {
  return {
    themePresetId: defaults.themePresetId,
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
    isMermaidThemeSelection(value.diagramTheme) &&
    isOptionalId(value.themePresetId) &&
    isThemeRecipeJson(value.themeRecipeJson) &&
    (value.themeRecipeJson === null || value.themePresetId === null) &&
    isMermanSvgPipeline(value.svgPipeline) &&
    isTextMeasurementMode(value.textMeasurementMode) &&
    isDiagramFontValue(value.diagramFont)
  );
}

// Rust owns recipe schema validation. Sharing checks only the bounded envelope.
function isThemeRecipeJson(value: unknown): value is string | null {
  if (value === null) return true;
  if (!isBoundedString(value, SHARE_THEME_RECIPE_BYTES)) return false;
  try {
    const recipe: unknown = JSON.parse(value);
    return (
      isRecord(recipe) &&
      recipe.schema_version === 1 &&
      (recipe.kind === "definition" || recipe.kind === "complete_spec")
    );
  } catch {
    return false;
  }
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

// Historical `theme: default` meant no override. New selections have their own
// field so explicit classic default and automatic appearance remain distinct.
function decodeThemeSelection(
  value: Record<string, unknown>,
  fallback: WorkspaceSnapshot["diagramTheme"],
): WorkspaceSnapshot["diagramTheme"] | null {
  if (Object.hasOwn(value, "themeSelection")) {
    if (Object.hasOwn(value, "theme")) return null;
    return isMermaidThemeSelection(value.themeSelection)
      ? value.themeSelection
      : null;
  }
  if (!Object.hasOwn(value, "theme")) return fallback;
  if (typeof value.theme !== "string" || !isThemeName(value.theme)) return null;
  return value.theme === "default" ? "auto" : value.theme;
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
