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
} from "./preview-source.js";

export type PreviewDiagnosticsProvider = (
  uri: vscode.Uri,
  diagnosticRange: { startLine: number; endLine: number },
) => PreviewDiagnostics;

export class PreviewSession {
  private currentSnapshot: PreviewSnapshot | undefined;
  private lastPreviewEditorUri: string | undefined;
  private preferredEditorUri: string | undefined;
  private selectedSource: PreviewSourceSelection | undefined;
  private lockedSourceSelection: PreviewSourceSelection | undefined;
  private theme: PreviewDiagramTheme = "source";
  private displayMode: PreviewDisplayMode = "svg";
  private background: PreviewBackground = "paper";
  private locked = false;

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
    this.theme = "source";
    this.displayMode = "svg";
    this.background = "paper";
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
    sourceId: string,
  ): boolean {
    const editor = this.resolvePreviewEditor(activeEditor, visibleEditors);
    if (!editor || sourceId.length === 0) {
      return false;
    }
    const input = extractPreviewInput(editor, sourceId);
    if (!input) {
      return false;
    }
    this.selectedSource = {
      uri: editor.document.uri.toString(),
      sourceId: input.sourceId,
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
      const selected = extractPreviewInput(editor, this.selectedSource.sourceId);
      if (selected) {
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
      input.sourceId === this.selectedSource.sourceId
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
      sourceId: this.currentSnapshot.input.sourceId,
    };
    this.selectedSource = selection;
    this.lockedSourceSelection = selection;
    this.lastPreviewEditorUri = selection.uri;
  }
}

interface PreviewSourceSelection {
  uri: string;
  sourceId: string;
}

function sameSelection(
  first: PreviewSourceSelection | undefined,
  second: PreviewSourceSelection | undefined,
): boolean {
  return !!first && !!second && first.uri === second.uri && first.sourceId === second.sourceId;
}
