import { createHash } from "node:crypto";

import { hashPreviewSource, type PreviewInput } from "./preview-source.js";
import type { DiagramTheme, ResolvedTheme, ThemeAppearancePreference, ThemeChoice } from "./theme-selection.js";

export type PreviewDiagramTheme = DiagramTheme;
export type PreviewDisplayMode = "svg" | "ascii" | "unicode";
export type PreviewBackground = "theme" | "transparent" | "paper" | "dark";
export type PreviewViewerBackground = "auto" | "transparent" | "paper" | "dark";

export interface PreviewDiagnostics {
  summary: string;
  totalCount: number;
  firstTarget?: PreviewDiagnosticTarget;
}

export interface PreviewDiagnosticTarget {
  uri: string;
  startLine: number;
  startCharacter: number;
  endLine: number;
  endCharacter: number;
}

export interface PreviewSourceKey {
  documentUri: string;
  sourceId: string;
  sourceHash: string;
  diagramTheme: PreviewDiagramTheme;
  displayMode: PreviewDisplayMode;
  background: PreviewBackground;
  themeSelection?: string;
  fitWidth?: number;
}

export interface PreviewSnapshot {
  documentUri: string;
  documentVersion: number;
  input: PreviewInput;
  sources: readonly PreviewInput[];
  diagnostics?: PreviewDiagnostics;
  selectionLine: number;
  selected: boolean;
  diagramTheme: PreviewDiagramTheme;
  displayMode: PreviewDisplayMode;
  background: PreviewBackground;
  locked: boolean;
  sourceKey: PreviewSourceKey;
  appearancePreference?: ThemeAppearancePreference;
  resolvedTheme?: ResolvedTheme;
  themeChoices?: ThemeChoice[];
  themeNotice?: string;
  viewerBackground?: PreviewViewerBackground;
}

export interface CreatePreviewSnapshotRequest {
  documentUri: string;
  documentVersion: number;
  input: PreviewInput;
  sources: readonly PreviewInput[];
  diagnostics?: PreviewDiagnostics;
  selectionLine: number;
  selected: boolean;
  diagramTheme: PreviewDiagramTheme;
  displayMode: PreviewDisplayMode;
  background: PreviewBackground;
  locked: boolean;
  fitWidth?: number;
  appearancePreference?: ThemeAppearancePreference;
  resolvedTheme?: ResolvedTheme;
  themeChoices?: ThemeChoice[];
  themeNotice?: string;
  viewerBackground?: PreviewViewerBackground;
}

export function createPreviewSnapshot({ fitWidth, ...request }: CreatePreviewSnapshotRequest): PreviewSnapshot {
  return {
    ...request,
    sourceKey: {
      documentUri: request.documentUri,
      sourceId: request.input.sourceId,
      sourceHash: hashPreviewSource(request.input.source),
      diagramTheme: request.diagramTheme,
      displayMode: request.displayMode,
      background: request.background,
      ...(request.displayMode === "svg" && fitWidth !== undefined ? { fitWidth } : {}),
      themeSelection: request.resolvedTheme ? createHash("sha256").update(JSON.stringify({
        requested: request.diagramTheme,
        preference: request.appearancePreference,
        effective: request.resolvedTheme.effectiveAppearance,
        theme: request.resolvedTheme.theme,
        file: request.resolvedTheme.themeFileContent,
      })).digest("hex") : undefined,
    },
  };
}

export function samePreviewSource(a: PreviewSnapshot, b: PreviewSnapshot): boolean {
  return a.documentUri === b.documentUri && a.input.sourceId === b.input.sourceId;
}

export function samePreviewRenderKey(a: PreviewSnapshot, b: PreviewSnapshot): boolean {
  return samePreviewSourceKey(a.sourceKey, b.sourceKey);
}

export function samePreviewSourceKey(a: PreviewSourceKey, b: PreviewSourceKey): boolean {
  return (
    a.documentUri === b.documentUri &&
    a.sourceId === b.sourceId &&
    a.sourceHash === b.sourceHash &&
    a.diagramTheme === b.diagramTheme &&
    a.displayMode === b.displayMode &&
    a.background === b.background &&
    a.themeSelection === b.themeSelection &&
    a.fitWidth === b.fitWidth
  );
}

export function previewSourceKeyId(key: PreviewSourceKey): string {
  return [
    key.documentUri,
    key.sourceId,
    key.sourceHash,
    key.diagramTheme,
    key.displayMode,
    key.background,
    key.themeSelection ?? "",
    ...(key.fitWidth === undefined ? [] : [key.fitWidth]),
  ].join("\u0000");
}
