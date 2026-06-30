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
  private pinnedSource: PreviewSourcePin | undefined;
  private theme: PreviewDiagramTheme = "source";
  private displayMode: PreviewDisplayMode = "svg";
  private background: PreviewBackground = "transparent";

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

  reset(): void {
    this.currentSnapshot = undefined;
    this.lastPreviewEditorUri = undefined;
    this.pinnedSource = undefined;
    this.theme = "source";
    this.displayMode = "svg";
    this.background = "transparent";
  }

  clearSource(): void {
    this.currentSnapshot = undefined;
    this.pinnedSource = undefined;
  }

  rememberResource(uri: vscode.Uri): void {
    this.lastPreviewEditorUri = uri.toString();
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
    const editor = this.resolvePreviewEditor(activeEditor, visibleEditors);
    const input = editor ? this.resolvePreviewInput(editor) : null;
    if (!editor || !input) {
      return undefined;
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
      pinned: this.isPinnedInput(input),
      diagramTheme: this.theme,
      displayMode: this.displayMode,
      background: this.background,
    });
  }

  resolvePreviewEditor(
    activeEditor: vscode.TextEditor | undefined,
    visibleEditors: readonly vscode.TextEditor[],
  ): vscode.TextEditor | undefined {
    if (activeEditor && this.resolvePreviewInput(activeEditor)) {
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

  togglePin(
    activeEditor: vscode.TextEditor | undefined,
    visibleEditors: readonly vscode.TextEditor[],
  ): boolean {
    const editor = this.resolvePreviewEditor(activeEditor, visibleEditors);
    const input = editor ? this.resolvePreviewInput(editor) : null;
    if (!editor || !input) {
      return false;
    }
    const editorUri = editor.document.uri.toString();
    if (this.pinnedSource?.uri === editorUri && this.pinnedSource.sourceId === input.sourceId) {
      this.pinnedSource = undefined;
    } else {
      this.pinnedSource = {
        uri: editorUri,
        sourceId: input.sourceId,
      };
    }
    return true;
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
    this.pinnedSource = {
      uri: editor.document.uri.toString(),
      sourceId: input.sourceId,
    };
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
    if (this.pinnedSource?.uri === editorUri) {
      const pinned = extractPreviewInput(editor, this.pinnedSource.sourceId);
      if (pinned) {
        return pinned;
      }
      this.pinnedSource = undefined;
    }
    return extractPreviewInput(editor);
  }

  private isPinnedInput(input: PreviewInput | undefined): boolean {
    return (
      !!input &&
      !!this.pinnedSource &&
      this.lastPreviewEditorUri === this.pinnedSource.uri &&
      input.sourceId === this.pinnedSource.sourceId
    );
  }
}

interface PreviewSourcePin {
  uri: string;
  sourceId: string;
}
