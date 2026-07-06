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
  locked: boolean;
  sourceKey: PreviewSourceKey;
  sources: PreviewSourceOption[];
  diagnostics?: PreviewDiagnostics;
}

export type PreviewToWebviewMessage =
  | {
      type: "showEmpty";
      requestId?: number;
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
      type: "renderInvalidated";
      requestId: number;
      reason: string;
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
  | { type: "refresh" }
  | { type: "showSource" }
  | { type: "copySvg"; svg: string; sourceKey: PreviewSourceKey }
  | { type: "exportRendered"; format: "svg" | "png"; sourceKey: PreviewSourceKey }
  | { type: "revealDiagnostic"; target: string }
  | { type: "selectSource"; sourceId: string }
  | { type: "setLocked"; locked: boolean }
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
    locked: snapshot.locked,
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
    case "refresh":
    case "showSource":
      return true;
    case "copySvg":
      return typeof record.svg === "string" && isPreviewSourceKey(record.sourceKey);
    case "exportRendered":
      return (
        (record.format === "svg" || record.format === "png") &&
        isPreviewSourceKey(record.sourceKey)
      );
    case "revealDiagnostic":
      return typeof record.target === "string";
    case "selectSource":
      return typeof record.sourceId === "string";
    case "setLocked":
      return typeof record.locked === "boolean";
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

function isPreviewSourceKey(value: unknown): value is PreviewSourceKey {
  if (!value || typeof value !== "object") {
    return false;
  }
  const record = value as Record<string, unknown>;
  return (
    typeof record.documentUri === "string" &&
    typeof record.sourceId === "string" &&
    typeof record.sourceHash === "string" &&
    isPreviewDiagramTheme(record.diagramTheme) &&
    isPreviewDisplayMode(record.displayMode) &&
    (record.background === "transparent" ||
      record.background === "paper" ||
      record.background === "dark")
  );
}
