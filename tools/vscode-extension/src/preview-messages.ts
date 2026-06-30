import {
  type PreviewBackground,
  type PreviewDiagramTheme,
  type PreviewDisplayMode,
  type PreviewDiagnostics,
  type PreviewSnapshot,
  type PreviewSourceKey,
} from "./preview-model.js";
import type { PreviewInput } from "./preview-source.js";

export interface PreviewSourceOption {
  sourceId: string;
  title: string;
  subtitle: string;
  kind: PreviewInput["kind"];
}

export interface PreviewSnapshotMessagePayload {
  documentUri: string;
  sourceId: string;
  title: string;
  subtitle: string;
  selectionLine: number;
  diagramTheme: PreviewDiagramTheme;
  displayMode: PreviewDisplayMode;
  background: PreviewBackground;
  sourceKey: PreviewSourceKey;
  sources: PreviewSourceOption[];
  diagnostics?: PreviewDiagnostics;
}

export type PreviewToWebviewMessage =
  | {
      type: "showEmpty";
      heading: string;
      detail: string;
    }
  | {
      type: "sourceListUpdated";
      snapshot: PreviewSnapshotMessagePayload;
    }
  | {
      type: "selectionChanged";
      snapshot: PreviewSnapshotMessagePayload;
    }
  | {
      type: "diagnosticsUpdated";
      snapshot: PreviewSnapshotMessagePayload;
    }
  | {
      type: "settingsUpdated";
      snapshot: PreviewSnapshotMessagePayload;
    }
  | {
      type: "renderStarted";
      requestId: number;
      reason: string;
      snapshot: PreviewSnapshotMessagePayload;
    }
  | {
      type: "renderSucceeded";
      requestId: number;
      snapshot: PreviewSnapshotMessagePayload;
      content: string;
    }
  | {
      type: "renderFailed";
      requestId: number;
      snapshot: PreviewSnapshotMessagePayload;
      error: string;
    };

export type PreviewFromWebviewMessage =
  | { type: "ready" }
  | { type: "copySvg"; svg: string }
  | { type: "exportRendered"; format: "svg" | "png" }
  | { type: "revealDiagnostic"; target: string }
  | { type: "showDiagnosticFixes"; target: string }
  | { type: "selectSource"; sourceId: string }
  | { type: "setDiagramTheme"; theme: PreviewDiagramTheme }
  | { type: "setDisplayMode"; mode: PreviewDisplayMode }
  | { type: "setBackground"; background: PreviewBackground };

export function snapshotMessagePayload(snapshot: PreviewSnapshot): PreviewSnapshotMessagePayload {
  return {
    documentUri: snapshot.documentUri,
    sourceId: snapshot.input.sourceId,
    title: snapshot.input.title,
    subtitle: snapshot.input.subtitle,
    selectionLine: snapshot.selectionLine,
    diagramTheme: snapshot.diagramTheme,
    displayMode: snapshot.displayMode,
    background: snapshot.background,
    sourceKey: snapshot.sourceKey,
    diagnostics: snapshot.diagnostics,
    sources: snapshot.sources.map((source) => ({
      sourceId: source.sourceId,
      title: source.title,
      subtitle: source.subtitle,
      kind: source.kind,
    })),
  };
}

export function isPreviewFromWebviewMessage(value: unknown): value is PreviewFromWebviewMessage {
  if (!value || typeof value !== "object") {
    return false;
  }
  const record = value as Record<string, unknown>;
  switch (record.type) {
    case "ready":
      return true;
    case "copySvg":
      return typeof record.svg === "string";
    case "exportRendered":
      return record.format === "svg" || record.format === "png";
    case "revealDiagnostic":
    case "showDiagnosticFixes":
      return typeof record.target === "string";
    case "selectSource":
      return typeof record.sourceId === "string";
    case "setDiagramTheme":
      return isPreviewDiagramTheme(record.theme);
    case "setDisplayMode":
      return isPreviewDisplayMode(record.mode);
    case "setBackground":
      return record.background === "transparent" || record.background === "paper" || record.background === "dark";
    default:
      return false;
  }
}

export function isPreviewDisplayMode(value: unknown): value is PreviewDisplayMode {
  return value === "svg" || value === "ascii" || value === "unicode";
}

export function isPreviewDiagramTheme(value: unknown): value is PreviewDiagramTheme {
  return (
    value === "source" ||
    value === "default" ||
    value === "dark" ||
    value === "forest" ||
    value === "neutral" ||
    value === "base"
  );
}
