import {
  samePreviewRenderKey,
  samePreviewSource,
  type PreviewSnapshot,
} from "./preview-model.js";

export type PreviewUpdateReason =
  | "manual-open"
  | "active-editor"
  | "selection"
  | "document-change"
  | "diagnostics"
  | "panel-visible"
  | "pin-toggle"
  | "source-select"
  | "diagram-theme";

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

  if (previous.pinned !== next.pinned || previous.diagramTheme !== next.diagramTheme) {
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
    previousDiagnostics.visibleCount !== nextDiagnostics.visibleCount ||
    previousDiagnostics.totalCount !== nextDiagnostics.totalCount ||
    previousDiagnostics.items.length !== nextDiagnostics.items.length
  ) {
    return false;
  }

  return previousDiagnostics.items.every((item, index) => {
    const other = nextDiagnostics.items[index];
    return (
      !!other &&
      item.severityLabel === other.severityLabel &&
      item.severityKey === other.severityKey &&
      item.line === other.line &&
      item.column === other.column &&
      item.source === other.source &&
      item.code === other.code &&
      item.message === other.message &&
      item.hasQuickFixes === other.hasQuickFixes &&
      item.target.uri === other.target.uri &&
      item.target.startLine === other.target.startLine &&
      item.target.startCharacter === other.target.startCharacter &&
      item.target.endLine === other.target.endLine &&
      item.target.endCharacter === other.target.endCharacter
    );
  });
}
