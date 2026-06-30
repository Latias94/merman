import {
  samePreviewRenderKey,
  samePreviewSource,
  type PreviewDiagnosticTarget,
  type PreviewSnapshot,
} from "./preview-model.js";

export type PreviewUpdateReason =
  | "manual-open"
  | "active-editor"
  | "selection"
  | "document-change"
  | "diagnostics"
  | "panel-visible"
  | "source-select"
  | "diagram-theme"
  | "display-mode"
  | "background";

export type PreviewAction =
  | { type: "showEmpty" }
  | { type: "sourceListUpdated"; snapshot: PreviewSnapshot }
  | { type: "selectionChanged"; snapshot: PreviewSnapshot }
  | { type: "diagnosticsUpdated"; snapshot: PreviewSnapshot }
  | { type: "settingsUpdated"; snapshot: PreviewSnapshot }
  | { type: "renderRequested"; snapshot: PreviewSnapshot; reason: PreviewUpdateReason };

export function planPreviewUpdate(
  previous: PreviewSnapshot | undefined,
  next: PreviewSnapshot | undefined,
  reason: PreviewUpdateReason,
): PreviewAction[] {
  if (!next) {
    return [{ type: "showEmpty" }];
  }

  if (!previous) {
    return [
      { type: "sourceListUpdated", snapshot: next },
      { type: "diagnosticsUpdated", snapshot: next },
      { type: "settingsUpdated", snapshot: next },
      { type: "renderRequested", snapshot: next, reason },
    ];
  }

  const actions: PreviewAction[] = [];
  const sameSource = samePreviewSource(previous, next);
  const sameRenderKey = samePreviewRenderKey(previous, next);

  if (!sourceListsEqual(previous, next) || !sameSource) {
    actions.push({ type: "sourceListUpdated", snapshot: next });
  }

  if (
    previous.selected !== next.selected ||
    previous.diagramTheme !== next.diagramTheme ||
    previous.displayMode !== next.displayMode ||
    previous.background !== next.background
  ) {
    actions.push({ type: "settingsUpdated", snapshot: next });
  }

  if (!diagnosticsEqual(previous, next) && sameSource && sameRenderKey) {
    actions.push({ type: "diagnosticsUpdated", snapshot: next });
  }

  if (reason === "selection" && sameSource && sameRenderKey && previous.selectionLine !== next.selectionLine) {
    actions.push({ type: "selectionChanged", snapshot: next });
  }

  if (!sameRenderKey) {
    actions.push({ type: "renderRequested", snapshot: next, reason });
  }

  return actions;
}

function sourceListsEqual(previous: PreviewSnapshot, next: PreviewSnapshot): boolean {
  if (previous.sources.length !== next.sources.length) {
    return false;
  }
  return previous.sources.every((source, index) => {
    const other = next.sources[index];
    return (
      !!other &&
      source.sourceId === other.sourceId &&
      source.title === other.title &&
      source.subtitle === other.subtitle &&
      source.kind === other.kind &&
      source.sourceRange.startLine === other.sourceRange.startLine &&
      source.sourceRange.endLine === other.sourceRange.endLine
    );
  });
}

function diagnosticsEqual(previous: PreviewSnapshot, next: PreviewSnapshot): boolean {
  const previousDiagnostics = previous.diagnostics;
  const nextDiagnostics = next.diagnostics;
  if (!previousDiagnostics || !nextDiagnostics) {
    return previousDiagnostics === nextDiagnostics;
  }
  if (
    previousDiagnostics.summary !== nextDiagnostics.summary ||
    previousDiagnostics.totalCount !== nextDiagnostics.totalCount
  ) {
    return false;
  }

  return diagnosticTargetsEqual(previousDiagnostics.firstTarget, nextDiagnostics.firstTarget);
}

function diagnosticTargetsEqual(
  previous: PreviewDiagnosticTarget | undefined,
  next: PreviewDiagnosticTarget | undefined,
): boolean {
  if (!previous || !next) {
    return previous === next;
  }
  return (
    previous.uri === next.uri &&
    previous.startLine === next.startLine &&
    previous.startCharacter === next.startCharacter &&
    previous.endLine === next.endLine &&
    previous.endCharacter === next.endCharacter
  );
}
