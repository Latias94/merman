import type { PreviewInput } from "./preview-source.js";

export type PreviewDiagramTheme = "source" | "default" | "dark" | "forest" | "neutral" | "base";
export type PreviewDisplayMode = "svg" | "ascii" | "unicode";
export type PreviewBackground = "transparent" | "paper" | "dark";

export interface PreviewDiagnosticItem {
  severityLabel: string;
  severityKey: "error" | "warning" | "info" | "hint";
  line: number;
  column: number;
  target: PreviewDiagnosticTarget;
  source?: string;
  code?: string;
  message: string;
  hasQuickFixes: boolean;
}

export interface PreviewDiagnostics {
  summary: string;
  visibleCount: number;
  totalCount: number;
  items: PreviewDiagnosticItem[];
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
}

export interface PreviewSnapshot {
  documentUri: string;
  documentVersion: number;
  input: PreviewInput;
  sources: readonly PreviewInput[];
  diagnostics?: PreviewDiagnostics;
  selectionLine: number;
  pinned: boolean;
  diagramTheme: PreviewDiagramTheme;
  displayMode: PreviewDisplayMode;
  background: PreviewBackground;
  sourceKey: PreviewSourceKey;
}

export interface CreatePreviewSnapshotRequest {
  documentUri: string;
  documentVersion: number;
  input: PreviewInput;
  sources: readonly PreviewInput[];
  diagnostics?: PreviewDiagnostics;
  selectionLine: number;
  pinned: boolean;
  diagramTheme: PreviewDiagramTheme;
  displayMode: PreviewDisplayMode;
  background: PreviewBackground;
}

export function createPreviewSnapshot(request: CreatePreviewSnapshotRequest): PreviewSnapshot {
  return {
    ...request,
    sourceKey: {
      documentUri: request.documentUri,
      sourceId: request.input.sourceId,
      sourceHash: hashSource(request.input.source),
      diagramTheme: request.diagramTheme,
      displayMode: request.displayMode,
      background: request.background,
    },
  };
}

export function samePreviewSource(a: PreviewSnapshot, b: PreviewSnapshot): boolean {
  return a.documentUri === b.documentUri && a.input.sourceId === b.input.sourceId;
}

export function samePreviewRenderKey(a: PreviewSnapshot, b: PreviewSnapshot): boolean {
  return (
    a.sourceKey.documentUri === b.sourceKey.documentUri &&
    a.sourceKey.sourceId === b.sourceKey.sourceId &&
    a.sourceKey.sourceHash === b.sourceKey.sourceHash &&
    a.sourceKey.diagramTheme === b.sourceKey.diagramTheme &&
    a.sourceKey.displayMode === b.sourceKey.displayMode &&
    a.sourceKey.background === b.sourceKey.background
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
  ].join("\u0000");
}

function hashSource(source: string): string {
  let hash = 2166136261;
  for (let index = 0; index < source.length; index += 1) {
    hash ^= source.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}
