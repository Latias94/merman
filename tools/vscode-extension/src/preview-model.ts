import type { PreviewInput } from "./preview-source.js";

export type PreviewDiagramTheme = "source" | "default" | "dark" | "forest" | "neutral" | "base";

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
}

export function createPreviewSnapshot(request: CreatePreviewSnapshotRequest): PreviewSnapshot {
  return {
    ...request,
    sourceKey: {
      documentUri: request.documentUri,
      sourceId: request.input.sourceId,
      sourceHash: hashSource(request.input.source),
      diagramTheme: request.diagramTheme,
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
    a.sourceKey.diagramTheme === b.sourceKey.diagramTheme
  );
}

export function previewSourceKeyId(key: PreviewSourceKey): string {
  return `${key.documentUri}\u0000${key.sourceId}\u0000${key.sourceHash}\u0000${key.diagramTheme}`;
}

function hashSource(source: string): string {
  let hash = 2166136261;
  for (let index = 0; index < source.length; index += 1) {
    hash ^= source.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}
