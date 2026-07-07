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
  extractPreviewInput,
  listPreviewInputsFromDocument,
  type PreviewInput,
  previewSourceIdentity,
  resolvePreviewInputIdentity,
  type PreviewSourceIdentity,
} from "./preview-source.js";

export type PreviewDiagnosticsProvider = (
  uri: vscode.Uri,
  diagnosticRange: { startLine: number; endLine: number },
) => PreviewDiagnostics;

export interface PreviewSessionDefaults {
  diagramTheme: PreviewDiagramTheme;
  displayMode: PreviewDisplayMode;
  background: PreviewBackground;
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
    const editor = this.resolveSnapshotEditor(activeEditor, visibleEditors);
    if (!editor) {
      return this.locked ? this.createSnapshotFromCurrentState() : undefined;
    }

    const input = this.resolvePreviewInput(editor);
    if (!input) {
      return this.locked ? this.createSnapshotFromCurrentState() : undefined;
    }

    const sources = listPreviewInputsFromDocument(editor.document, editor.selection.active.line);
    const diagnostics = diagnosticsProvider(editor.document.uri, input.diagnosticRange);
    return createPreviewSnapshot({
      documentUri: editor.document.uri.toString(),
      documentVersion: editor.document.version,
      input,
      sources,
      diagnostics,
      selectionLine: editor.selection.active.line,
      selected: this.isSelectedInput(input),
      diagramTheme: this.theme,
      displayMode: this.displayMode,
      background: this.background,
      locked: this.locked,
    });
  }

  resolvePreviewEditor(
    activeEditor: vscode.TextEditor | undefined,
    visibleEditors: readonly vscode.TextEditor[],
  ): vscode.TextEditor | undefined {
    if (!this.locked && activeEditor && this.resolvePreviewInput(activeEditor)) {
      return activeEditor;
    }

    if (!this.lastPreviewEditorUri) {
      return undefined;
    }

    return visibleEditors.find(
      (editor) =>
        editor.document.uri.toString() === this.lastPreviewEditorUri &&
        this.resolvePreviewInput(editor) !== null,
    );
  }

  private resolveSnapshotEditor(
    activeEditor: vscode.TextEditor | undefined,
    visibleEditors: readonly vscode.TextEditor[],
  ): vscode.TextEditor | undefined {
    return (
      this.takePreferredEditor(visibleEditors) ??
      this.resolvePreviewEditor(activeEditor, visibleEditors)
    );
  }

  selectSource(
    activeEditor: vscode.TextEditor | undefined,
    visibleEditors: readonly vscode.TextEditor[],
    source: string | PreviewSourceIdentity,
  ): boolean {
    const editor = this.resolvePreviewEditor(activeEditor, visibleEditors);
    if (!editor || (typeof source === "string" && source.length === 0)) {
      return false;
    }
    const input =
      typeof source === "string"
        ? extractPreviewInput(editor, source)
        : resolvePreviewInputIdentity(
            listPreviewInputsFromDocument(editor.document, editor.selection.active.line),
            source,
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

  private resolvePreviewInput(editor: vscode.TextEditor): PreviewInput | null {
    const editorUri = editor.document.uri.toString();
    if (this.selectedSource?.uri === editorUri) {
      const inputs = listPreviewInputsFromDocument(editor.document, editor.selection.active.line);
      if (this.trackedLockedMarkdownSource?.invalid) {
        return null;
      }
      const tracked = this.resolveTrackedMarkdownInput(editorUri, editor.document.version, inputs);
      const selected =
        tracked ??
        (this.trackedLockedMarkdownSource?.contentChanged
          ? null
          : resolvePreviewInputIdentity(inputs, this.selectedSource.identity));
      if (selected) {
        this.rememberResolvedSelectedInput(selected, editor.document.version);
        return selected;
      }
      if (this.locked) {
        return null;
      }
      this.selectedSource = undefined;
    }
    return extractPreviewInput(editor);
  }

  private isSelectedInput(input: PreviewInput | undefined): boolean {
    return (
      !!input &&
      !!this.selectedSource &&
      this.lastPreviewEditorUri === this.selectedSource.uri &&
      inputMatchesSelection(input, this.selectedSource)
    );
  }

  private takePreferredEditor(visibleEditors: readonly vscode.TextEditor[]): vscode.TextEditor | undefined {
    if (this.locked || !this.preferredEditorUri) {
      return undefined;
    }

    const preferredEditorUri = this.preferredEditorUri;
    this.preferredEditorUri = undefined;
    return visibleEditors.find(
      (editor) =>
        editor.document.uri.toString() === preferredEditorUri &&
        this.resolvePreviewInput(editor) !== null,
    );
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

function inputMatchesSelection(input: PreviewInput, selection: PreviewSourceSelection): boolean {
  const identity = previewSourceIdentity(input);
  return (
    identity.kind === selection.identity.kind &&
    identity.sourceId === selection.identity.sourceId &&
    identity.sourceHash === selection.identity.sourceHash &&
    identity.sourceRange.startLine === selection.identity.sourceRange.startLine &&
    identity.sourceRange.endLine === selection.identity.sourceRange.endLine
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
  return change.range.end.line < range.startLine;
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
  if (change.range.end.line <= contentRange.endLine) {
    return true;
  }

  return (
    isZeroLengthChange(change) &&
    insertedLineCount(change.text) > 0 &&
    change.range.start.line === sourceRange.endLine &&
    change.range.start.character === 0 &&
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
