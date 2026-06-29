import {
  type PreviewDiagramTheme,
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
  pinned: boolean;
  diagramTheme: PreviewDiagramTheme;
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
      svg: string;
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
  | { type: "revealDiagnostic"; target: string }
  | { type: "showDiagnosticFixes"; target: string }
  | { type: "togglePin" }
  | { type: "selectSource"; sourceId: string }
  | { type: "setDiagramTheme"; theme: PreviewDiagramTheme }
  | { type: "setBackground"; background: "transparent" | "paper" | "dark" };

export function snapshotMessagePayload(snapshot: PreviewSnapshot): PreviewSnapshotMessagePayload {
  return {
    documentUri: snapshot.documentUri,
    sourceId: snapshot.input.sourceId,
    title: snapshot.input.title,
    subtitle: snapshot.input.subtitle,
    selectionLine: snapshot.selectionLine,
    pinned: snapshot.pinned,
    diagramTheme: snapshot.diagramTheme,
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
    case "togglePin":
      return true;
    case "copySvg":
      return typeof record.svg === "string";
    case "revealDiagnostic":
    case "showDiagnosticFixes":
      return typeof record.target === "string";
    case "selectSource":
      return typeof record.sourceId === "string";
    case "setDiagramTheme":
      return isPreviewDiagramTheme(record.theme);
    case "setBackground":
      return record.background === "transparent" || record.background === "paper" || record.background === "dark";
    default:
      return false;
  }
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
