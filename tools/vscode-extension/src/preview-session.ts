import type * as vscode from "vscode";

import {
  createPreviewSnapshot,
  type PreviewBackground,
  type PreviewDiagramTheme,
  type PreviewDisplayMode,
  type PreviewDiagnostics,
  type PreviewSnapshot,
} from "./preview-model.js";
import {
  getPreviewSourceSnapshot,
  type PreviewInput,
  previewSourceIdentity,
  resolvePreviewInputIdentity,
  selectPreviewInput,
  type PreviewSourceIdentity,
} from "./preview-source.js";

export type PreviewDiagnosticsProvider = (
  uri: vscode.Uri,
  diagnosticRange: { startLine: number; endLine: number },
  documentVersion: number,
) => PreviewDiagnostics;

export interface PreviewSessionDefaults {
  diagramTheme: PreviewDiagramTheme;
  displayMode: PreviewDisplayMode;
  background: PreviewBackground;
}

export interface PreviewSourceResolution {
  editor: vscode.TextEditor;
  inputs: readonly PreviewInput[];
  input: PreviewInput;
  selected: boolean;
}

const DEFAULT_PREVIEW_SESSION_DEFAULTS: PreviewSessionDefaults = {
  diagramTheme: "source",
  displayMode: "svg",
  background: "paper",
};

export class PreviewSession {
  private readonly defaults: PreviewSessionDefaults;
  private currentSnapshot: PreviewSnapshot | undefined;
  private lastPreviewEditorUri: string | undefined;
  private preferredEditorUri: string | undefined;
  private selectedSource: PreviewSourceSelection | undefined;
  private lockedSourceSelection: PreviewSourceSelection | undefined;
  private trackedLockedMarkdownSource: TrackedMarkdownSource | undefined;
  private theme: PreviewDiagramTheme;
  private displayMode: PreviewDisplayMode;
  private background: PreviewBackground;
  private locked = false;

  constructor(defaults: Partial<PreviewSessionDefaults> = {}) {
    this.defaults = {
      ...DEFAULT_PREVIEW_SESSION_DEFAULTS,
      ...defaults,
    };
    this.theme = this.defaults.diagramTheme;
    this.displayMode = this.defaults.displayMode;
    this.background = this.defaults.background;
  }

  get snapshot(): PreviewSnapshot | undefined {
    return this.currentSnapshot;
  }

  get diagramTheme(): PreviewDiagramTheme {
    return this.theme;
  }

  get previewDisplayMode(): PreviewDisplayMode {
    return this.displayMode;
  }

  get previewBackground(): PreviewBackground {
    return this.background;
  }

  get isLocked(): boolean {
    return this.locked;
  }

  reset(): void {
    this.currentSnapshot = undefined;
    this.lastPreviewEditorUri = undefined;
    this.preferredEditorUri = undefined;
    this.selectedSource = undefined;
    this.lockedSourceSelection = undefined;
    this.trackedLockedMarkdownSource = undefined;
    this.theme = this.defaults.diagramTheme;
    this.displayMode = this.defaults.displayMode;
    this.background = this.defaults.background;
    this.locked = false;
  }

  clearSource(): void {
    this.currentSnapshot = undefined;
    this.preferredEditorUri = undefined;
    this.selectedSource = undefined;
    this.lockedSourceSelection = undefined;
    this.trackedLockedMarkdownSource = undefined;
  }

  rememberResource(uri: vscode.Uri, options: { preferOnce?: boolean } = {}): void {
    this.lastPreviewEditorUri = uri.toString();
    if (options.preferOnce) {
      this.preferredEditorUri = this.lastPreviewEditorUri;
    }
  }

  clearSelectedSource(): void {
    this.selectedSource = undefined;
    this.lockedSourceSelection = undefined;
    this.trackedLockedMarkdownSource = undefined;
  }

  setLocked(locked: boolean): boolean {
    if (this.locked === locked) {
      return false;
    }
    this.locked = locked;
    if (locked) {
      this.lockCurrentSnapshotSource();
    } else if (this.lockedSourceSelection && sameSelection(this.selectedSource, this.lockedSourceSelection)) {
      this.selectedSource = undefined;
      this.lockedSourceSelection = undefined;
      this.trackedLockedMarkdownSource = undefined;
    } else if (!locked) {
      this.trackedLockedMarkdownSource = undefined;
    }
    return true;
  }

  rememberSnapshot(snapshot: PreviewSnapshot): void {
    this.lastPreviewEditorUri = snapshot.documentUri;
    this.currentSnapshot = snapshot;
  }

  trackDocumentChange(
    event: Pick<vscode.TextDocumentChangeEvent, "document"> &
      Partial<Pick<vscode.TextDocumentChangeEvent, "contentChanges">>,
  ): void {
    if (
      !this.locked ||
      !this.trackedLockedMarkdownSource ||
      this.trackedLockedMarkdownSource.uri !== event.document.uri.toString()
    ) {
      return;
    }
    if (event.document.version < this.trackedLockedMarkdownSource.documentVersion) {
      return;
    }

    const contentChanges = event.contentChanges ?? [];
    if (contentChanges.length === 0) {
      return;
    }
    if (event.document.version !== this.trackedLockedMarkdownSource.documentVersion + 1) {
      this.trackedLockedMarkdownSource = invalidateTrackedMarkdownSource(
        this.trackedLockedMarkdownSource,
        event.document.version,
      );
      return;
    }
    const tracked = trackMarkdownSourceChanges(
      this.trackedLockedMarkdownSource,
      contentChanges,
      event.document.version,
    );
    this.trackedLockedMarkdownSource =
      tracked ?? invalidateTrackedMarkdownSource(this.trackedLockedMarkdownSource, event.document.version);
  }

  createSnapshot(
    activeEditor: vscode.TextEditor | undefined,
    visibleEditors: readonly vscode.TextEditor[],
    diagnosticsProvider: PreviewDiagnosticsProvider,
  ): PreviewSnapshot | undefined {
    const source =
      this.takePreferredSource(visibleEditors) ?? this.resolveSource(activeEditor, visibleEditors);
    if (!source) {
      return this.locked ? this.createSnapshotFromCurrentState() : undefined;
    }

    const { editor, input, inputs, selected } = source;
    const diagnostics = diagnosticsProvider(
      editor.document.uri,
      input.diagnosticRange,
      editor.document.version,
    );
    return createPreviewSnapshot({
      documentUri: editor.document.uri.toString(),
      documentVersion: editor.document.version,
      input,
      sources: inputs,
      diagnostics,
      selectionLine: editor.selection.active.line,
      selected,
      diagramTheme: this.theme,
      displayMode: this.displayMode,
      background: this.background,
      locked: this.locked,
    });
  }

  resolveSource(
    activeEditor: vscode.TextEditor | undefined,
    visibleEditors: readonly vscode.TextEditor[],
  ): PreviewSourceResolution | undefined {
    if (!this.locked && activeEditor) {
      const source = this.resolveEditorSource(activeEditor);
      if (source) {
        return source;
      }
    }

    if (!this.lastPreviewEditorUri) {
      return undefined;
    }

    for (const editor of visibleEditors) {
      if (editor.document.uri.toString() === this.lastPreviewEditorUri) {
        const source = this.resolveEditorSource(editor);
        if (source) {
          return source;
        }
      }
    }
    return undefined;
  }

  selectSource(
    activeEditor: vscode.TextEditor | undefined,
    visibleEditors: readonly vscode.TextEditor[],
    source: string | PreviewSourceIdentity,
  ): boolean {
    if (typeof source === "string" && source.length === 0) {
      return false;
    }
    const resolved = this.resolveSource(activeEditor, visibleEditors);
    if (!resolved) {
      return false;
    }
    const { editor, inputs } = resolved;
    const input = selectPreviewInput(
      inputs,
      typeof source === "string" ? { sourceId: source } : { sourceIdentity: source },
    );
    if (!input) {
      return false;
    }
    this.selectedSource = {
      uri: editor.document.uri.toString(),
      identity: previewSourceIdentity(input),
    };
    this.lockedSourceSelection = undefined;
    this.trackedLockedMarkdownSource = this.locked
      ? trackedMarkdownSourceForInput(this.selectedSource.uri, input, editor.document.version)
      : undefined;
    return true;
  }

  setDiagramTheme(theme: PreviewDiagramTheme): boolean {
    if (this.theme === theme) {
      return false;
    }
    this.theme = theme;
    return true;
  }

  setDisplayMode(displayMode: PreviewDisplayMode): boolean {
    if (this.displayMode === displayMode) {
      return false;
    }
    this.displayMode = displayMode;
    return true;
  }

  setBackground(background: PreviewBackground): boolean {
    if (this.background === background) {
      return false;
    }
    this.background = background;
    return true;
  }

  private resolveEditorSource(editor: vscode.TextEditor): PreviewSourceResolution | undefined {
    const { inputs } = getPreviewSourceSnapshot(editor.document);
    const editorUri = editor.document.uri.toString();
    if (this.selectedSource?.uri === editorUri) {
      if (this.trackedLockedMarkdownSource?.invalid) {
        return undefined;
      }
      const hadTrackedContentChange = this.trackedLockedMarkdownSource?.contentChanged === true;
      const tracked = this.resolveTrackedMarkdownInput(editorUri, editor.document.version, inputs);
      const selected =
        tracked ??
        (hadTrackedContentChange
          ? null
          : resolvePreviewInputIdentity(inputs, this.selectedSource.identity));
      if (selected) {
        this.rememberResolvedSelectedInput(selected, editor.document.version);
        return {
          editor,
          inputs,
          input: selected,
          selected: this.lastPreviewEditorUri === editorUri,
        };
      }
      if (this.locked) {
        return undefined;
      }
      this.selectedSource = undefined;
    }
    const input = selectPreviewInput(inputs, { activeLine: editor.selection.active.line });
    return input ? { editor, inputs, input, selected: false } : undefined;
  }

  private takePreferredSource(
    visibleEditors: readonly vscode.TextEditor[],
  ): PreviewSourceResolution | undefined {
    if (this.locked || !this.preferredEditorUri) {
      return undefined;
    }

    const preferredEditorUri = this.preferredEditorUri;
    this.preferredEditorUri = undefined;
    for (const editor of visibleEditors) {
      if (editor.document.uri.toString() === preferredEditorUri) {
        const source = this.resolveEditorSource(editor);
        if (source) {
          return source;
        }
      }
    }
    return undefined;
  }

  private createSnapshotFromCurrentState(): PreviewSnapshot | undefined {
    if (!this.currentSnapshot) {
      return undefined;
    }

    return createPreviewSnapshot({
      documentUri: this.currentSnapshot.documentUri,
      documentVersion: this.currentSnapshot.documentVersion,
      input: this.currentSnapshot.input,
      sources: this.currentSnapshot.sources,
      diagnostics: this.currentSnapshot.diagnostics,
      selectionLine: this.currentSnapshot.selectionLine,
      selected: this.currentSnapshot.selected,
      diagramTheme: this.theme,
      displayMode: this.displayMode,
      background: this.background,
      locked: this.locked,
    });
  }

  private lockCurrentSnapshotSource(): void {
    if (!this.currentSnapshot) {
      return;
    }

    const selection = {
      uri: this.currentSnapshot.documentUri,
      identity: previewSourceIdentity(this.currentSnapshot.input),
    };
    if (!this.selectedSource) {
      this.selectedSource = selection;
      this.lockedSourceSelection = selection;
    }
    this.trackedLockedMarkdownSource = trackedMarkdownSourceForInput(
      selection.uri,
      this.currentSnapshot.input,
      this.currentSnapshot.documentVersion,
    );
    this.lastPreviewEditorUri = selection.uri;
  }

  private rememberResolvedSelectedInput(input: PreviewInput, documentVersion: number): void {
    if (!this.selectedSource) {
      return;
    }
    const previousSelection = this.selectedSource;
    const nextSelection = {
      uri: previousSelection.uri,
      identity: previewSourceIdentity(input),
    };
    this.selectedSource = nextSelection;
    if (sameSelection(this.lockedSourceSelection, previousSelection)) {
      this.lockedSourceSelection = nextSelection;
    }
    if (this.locked) {
      this.trackedLockedMarkdownSource = trackedMarkdownSourceForInput(
        nextSelection.uri,
        input,
        documentVersion,
      );
    }
  }

  private resolveTrackedMarkdownInput(
    editorUri: string,
    documentVersion: number,
    inputs: readonly PreviewInput[],
  ): PreviewInput | null {
    const tracked = this.trackedLockedMarkdownSource;
    if (
      !tracked ||
      tracked.uri !== editorUri ||
      tracked.documentVersion !== documentVersion ||
      !tracked.contentChanged
    ) {
      return null;
    }

    const input =
      inputs.find(
        (candidate) =>
          candidate.kind === "markdown-fence" &&
          candidate.sourceRange.startLine === tracked.sourceRange.startLine &&
          candidate.sourceRange.endLine === tracked.sourceRange.endLine,
      ) ?? null;
    if (!input) {
      this.trackedLockedMarkdownSource = undefined;
    }
    return input;
  }
}

interface PreviewSourceSelection {
  uri: string;
  identity: PreviewSourceIdentity;
}

interface LineRange {
  startLine: number;
  endLine: number;
}

interface TrackedMarkdownSource {
  uri: string;
  sourceRange: LineRange;
  contentRange: LineRange;
  documentVersion: number;
  contentChanged: boolean;
  invalid?: boolean;
}

function sameSelection(
  first: PreviewSourceSelection | undefined,
  second: PreviewSourceSelection | undefined,
): boolean {
  return (
    !!first &&
    !!second &&
    first.uri === second.uri &&
    first.identity.sourceId === second.identity.sourceId &&
    first.identity.sourceHash === second.identity.sourceHash &&
    first.identity.kind === second.identity.kind &&
    first.identity.sourceRange.startLine === second.identity.sourceRange.startLine &&
    first.identity.sourceRange.endLine === second.identity.sourceRange.endLine
  );
}

function trackedMarkdownSourceForInput(
  uri: string,
  input: PreviewInput,
  documentVersion: number,
): TrackedMarkdownSource | undefined {
  if (input.kind !== "markdown-fence") {
    return undefined;
  }

  return {
    uri,
    sourceRange: {
      startLine: input.sourceRange.startLine,
      endLine: input.sourceRange.endLine,
    },
    contentRange: {
      startLine: input.diagnosticRange.startLine,
      endLine: input.diagnosticRange.endLine,
    },
    documentVersion,
    contentChanged: false,
  };
}

function invalidateTrackedMarkdownSource(
  tracked: TrackedMarkdownSource,
  documentVersion: number,
): TrackedMarkdownSource {
  return {
    ...tracked,
    documentVersion,
    invalid: true,
  };
}

function trackMarkdownSourceChanges(
  tracked: TrackedMarkdownSource,
  changes: readonly vscode.TextDocumentContentChangeEvent[],
  documentVersion: number,
): TrackedMarkdownSource | undefined {
  let next = { ...tracked, documentVersion };
  for (const change of changes) {
    const trackedAfterChange = trackMarkdownSourceChange(next, change);
    if (!trackedAfterChange) {
      return undefined;
    }
    next = trackedAfterChange;
  }
  return next;
}

function trackMarkdownSourceChange(
  tracked: TrackedMarkdownSource,
  change: vscode.TextDocumentContentChangeEvent,
): TrackedMarkdownSource | undefined {
  const lineDelta = insertedLineCount(change.text) - removedLineCount(change.range);

  if (isChangeBeforeRange(change, tracked.sourceRange)) {
    return shiftTrackedMarkdownSource(tracked, lineDelta);
  }
  if (isChangeAfterRange(change, tracked.sourceRange)) {
    return tracked;
  }
  if (!isSafeMarkdownFenceBodyChange(change, tracked.sourceRange, tracked.contentRange)) {
    return undefined;
  }

  return {
    ...tracked,
    sourceRange: {
      startLine: tracked.sourceRange.startLine,
      endLine: tracked.sourceRange.endLine + lineDelta,
    },
    contentRange: {
      startLine: tracked.contentRange.startLine,
      endLine: tracked.contentRange.endLine + lineDelta,
    },
    contentChanged: true,
  };
}

function shiftTrackedMarkdownSource(
  tracked: TrackedMarkdownSource,
  lineDelta: number,
): TrackedMarkdownSource {
  if (lineDelta === 0) {
    return tracked;
  }

  return {
    ...tracked,
    sourceRange: shiftRange(tracked.sourceRange, lineDelta),
    contentRange: shiftRange(tracked.contentRange, lineDelta),
  };
}

function shiftRange(range: LineRange, lineDelta: number): LineRange {
  return {
    startLine: range.startLine + lineDelta,
    endLine: range.endLine + lineDelta,
  };
}

function isChangeBeforeRange(
  change: vscode.TextDocumentContentChangeEvent,
  range: LineRange,
): boolean {
  if (change.range.end.line < range.startLine) {
    return true;
  }
  return (
    isZeroLengthChange(change) &&
    insertedLineCount(change.text) > 0 &&
    change.range.start.line === range.startLine &&
    change.range.start.character === 0
  );
}

function isChangeAfterRange(
  change: vscode.TextDocumentContentChangeEvent,
  range: LineRange,
): boolean {
  return change.range.start.line > range.endLine;
}

function isSafeMarkdownFenceBodyChange(
  change: vscode.TextDocumentContentChangeEvent,
  sourceRange: LineRange,
  contentRange: LineRange,
): boolean {
  if (change.range.start.line < contentRange.startLine) {
    return false;
  }
  if (change.range.end.line < sourceRange.endLine) {
    return true;
  }

  return (
    change.range.end.line === sourceRange.endLine &&
    change.range.end.character === 0
  );
}

function isZeroLengthChange(change: vscode.TextDocumentContentChangeEvent): boolean {
  return (
    change.range.start.line === change.range.end.line &&
    change.range.start.character === change.range.end.character
  );
}

function insertedLineCount(text: string): number {
  let count = 0;
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (char === "\n") {
      count += 1;
    }
  }
  return count;
}

function removedLineCount(range: vscode.Range): number {
  return range.end.line - range.start.line;
}
