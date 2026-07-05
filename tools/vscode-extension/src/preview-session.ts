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
    }
    return true;
  }

  rememberSnapshot(snapshot: PreviewSnapshot): void {
    this.lastPreviewEditorUri = snapshot.documentUri;
    this.currentSnapshot = snapshot;
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
      const selected = resolvePreviewInputIdentity(
        listPreviewInputsFromDocument(editor.document, editor.selection.active.line),
        this.selectedSource.identity,
      );
      if (selected) {
        this.rememberResolvedSelectedInput(selected);
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
    if (!this.currentSnapshot || this.selectedSource) {
      return;
    }

    const selection = {
      uri: this.currentSnapshot.documentUri,
      identity: previewSourceIdentity(this.currentSnapshot.input),
    };
    this.selectedSource = selection;
    this.lockedSourceSelection = selection;
    this.lastPreviewEditorUri = selection.uri;
  }

  private rememberResolvedSelectedInput(input: PreviewInput): void {
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
  }
}

interface PreviewSourceSelection {
  uri: string;
  identity: PreviewSourceIdentity;
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
    (identity.sourceHash === selection.identity.sourceHash ||
      (identity.sourceId === selection.identity.sourceId &&
        identity.sourceRange.startLine === selection.identity.sourceRange.startLine &&
        identity.sourceRange.endLine === selection.identity.sourceRange.endLine))
  );
}
