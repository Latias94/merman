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
export const SHARE_LIMITS = Object.freeze({
  jsonBytes:
    SHARE_SOURCE_BYTES + SHARE_CONFIG_BYTES + SHARE_JSON_OVERHEAD_BYTES,
  sourceBytes: SHARE_SOURCE_BYTES,
  configBytes: SHARE_CONFIG_BYTES,
  idBytes: SHARE_PRESENTATION_ID_BYTES,
});
const MAX_ENCODED_HASH_CHARS = SHARE_LIMITS.jsonBytes * 4 + 4;

export function encodeShareHash(data: WorkspaceSnapshot): string {
  if (!isValidShareSnapshot(data)) {
    throw new RangeError("Workspace exceeds the share URL contract.");
  }
  const payload = {
    code: data.code,
    theme: data.diagramTheme,
    config: data.mermaidConfig,
    presentationThemePresetId: data.presentationThemePresetId,
    presentationProfileId: data.presentationProfileId,
    svgPipeline: data.svgPipeline,
    textMeasurementMode: data.textMeasurementMode,
    diagramFont: data.diagramFont,
  };
  const json = JSON.stringify(payload);
  if (utf8ByteLength(json) > SHARE_LIMITS.jsonBytes) {
    throw new RangeError("Workspace exceeds the share URL contract.");
  }
  return btoa(encodeURIComponent(json));
}

export function decodeShareHash(
  hash: string,
  defaults: Readonly<WorkspaceSnapshot> = DEFAULT_WORKSPACE_SNAPSHOT
): WorkspaceSnapshot | null {
  try {
    const base64 = hash.startsWith("#") ? hash.slice(1) : hash;
    if (!base64 || base64.length > MAX_ENCODED_HASH_CHARS) return null;
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
    if (config === null || textMeasurementMode === null || diagramFont === null) {
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

export interface ShareCommandEnvironment {
  readonly origin: string;
  readonly pathname: string;
  writeClipboardText(value: string): Promise<void>;
  replaceUrl(value: string): void;
}

export function createShareUrl(
  data: WorkspaceSnapshot,
  location: Pick<Location, "origin" | "pathname"> = window.location
): string {
  return `${location.origin}${location.pathname}#${encodeShareHash(data)}`;
}

export async function copyShareUrl(
  data: WorkspaceSnapshot,
  environment: ShareCommandEnvironment = browserShareEnvironment()
): Promise<void> {
  const url = createShareUrl(data, environment);
  await environment.writeClipboardText(url);
  environment.replaceUrl(url);
}

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
  if (
    value !== null &&
    value !== undefined &&
    !isOptionalId(value)
  ) {
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

function isDiagramFontValue(
  value: unknown
): value is WorkspaceSnapshot["diagramFont"] {
  return typeof value === "string" && isDiagramFont(value);
}

function isBoundedString(value: unknown, maxBytes: number): value is string {
  return typeof value === "string" && !exceedsUtf8ByteBudget(value, maxBytes);
}

function browserShareEnvironment(): ShareCommandEnvironment {
  return {
    origin: window.location.origin,
    pathname: window.location.pathname,
    writeClipboardText: (value) => navigator.clipboard.writeText(value),
    replaceUrl: (value) => window.history.replaceState(null, "", value),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
