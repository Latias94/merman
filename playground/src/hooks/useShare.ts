import { useCallback, useEffect, useState } from "react";
import { isBundledThemePresetName } from "@mermanjs/web";

import {
  isMermanSvgPipeline,
  type MermanSvgPipeline,
} from "../runtime/merman-core.ts";

export interface ShareData {
  code: string;
  theme: string;
  config?: string;
  presentationThemePresetId: string | null;
  presentationProfileId: string | null;
  svgPipeline: MermanSvgPipeline;
  textMeasurementMode?: string;
  diagramFont?: string;
}

export function encodeShareHash(data: ShareData): string {
  const payload = {
    code: data.code,
    theme: data.theme,
    ...(data.config !== undefined ? { config: data.config } : {}),
    presentationThemePresetId: data.presentationThemePresetId,
    presentationProfileId: data.presentationProfileId,
    svgPipeline: data.svgPipeline,
    ...(data.textMeasurementMode !== undefined
      ? { textMeasurementMode: data.textMeasurementMode }
      : {}),
    ...(data.diagramFont !== undefined
      ? { diagramFont: data.diagramFont }
      : {}),
  };
  return btoa(encodeURIComponent(JSON.stringify(payload)));
}

export function decodeShareHash(hash: string): ShareData | null {
  try {
    const base64 = hash.startsWith("#") ? hash.slice(1) : hash;
    if (!base64) return null;
    const value: unknown = JSON.parse(decodeURIComponent(atob(base64)));
    if (!isRecord(value)) return null;
    if (typeof value.code !== "string" || typeof value.theme !== "string") {
      return null;
    }

    const hasNewPresentation = [
      "presentationThemePresetId",
      "presentationProfileId",
      "svgPipeline",
    ].some((key) => Object.hasOwn(value, key));
    const presentation = hasNewPresentation
      ? {
          presentationThemePresetId: nullableString(
            value.presentationThemePresetId,
          ),
          presentationProfileId: nullableString(value.presentationProfileId),
          svgPipeline: normalizeSvgPipeline(value.svgPipeline),
        }
      : migrateLegacyHostTheme(value.hostThemePreset);

    return {
      code: value.code,
      theme: value.theme,
      ...(typeof value.config === "string" ? { config: value.config } : {}),
      ...presentation,
      ...(typeof value.textMeasurementMode === "string"
        ? { textMeasurementMode: value.textMeasurementMode }
        : {}),
      ...(typeof value.diagramFont === "string"
        ? { diagramFont: value.diagramFont }
        : {}),
    };
  } catch {
    return null;
  }
}

export function useShare() {
  const [initialData, setInitialData] = useState<ShareData | null>(null);

  useEffect(() => {
    const data = decodeShareHash(window.location.hash);
    if (data) setInitialData(data);
  }, []);

  const createShareUrl = useCallback((data: ShareData): string => {
    const baseUrl = `${window.location.origin}${window.location.pathname}`;
    return `${baseUrl}#${encodeShareHash(data)}`;
  }, []);

  const copyShareUrl = useCallback(
    async (data: ShareData): Promise<void> => {
      const url = createShareUrl(data);
      await navigator.clipboard.writeText(url);
      window.history.replaceState(null, "", url);
    },
    [createShareUrl],
  );

  const clearShareUrl = useCallback(() => {
    const baseUrl = `${window.location.origin}${window.location.pathname}`;
    window.history.replaceState(null, "", baseUrl);
  }, []);

  return {
    initialData,
    createShareUrl,
    copyShareUrl,
    clearShareUrl,
  };
}

function migrateLegacyHostTheme(value: unknown): Pick<
  ShareData,
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

function nullableString(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function normalizeSvgPipeline(value: unknown): MermanSvgPipeline {
  return isMermanSvgPipeline(value) ? value : "parity";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
