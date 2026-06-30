import * as assert from "node:assert/strict";
import type * as vscode from "vscode";
import { describe, it } from "node:test";

import { PreviewSession } from "../preview-session.js";

describe("preview session", () => {
  it("resolves a remembered Mermaid resource after the webview takes active editor focus", () => {
    const session = new PreviewSession();
    const editor = textEditor("file:///workspace/example.mmd", "example.mmd", "flowchart TD\nA --> B\n");

    session.rememberResource(editor.document.uri);

    const snapshot = session.createSnapshot(undefined, [editor], () => ({
      summary: "0 errors, 0 warnings, 0 infos, 0 hints",
      visibleCount: 0,
      totalCount: 0,
      items: [],
    }));

    assert.equal(snapshot?.input.kind, "mermaid-file");
    assert.equal(snapshot?.input.sourceId, "document");
    assert.equal(snapshot?.documentUri, "file:///workspace/example.mmd");
    assert.equal(snapshot?.background, "paper");
  });
});

function textEditor(uri: string, fileName: string, text: string): vscode.TextEditor {
  const lines = text.split(/\r?\n/);
  return {
    document: {
      uri: {
        fsPath: fileName,
        toString: () => uri,
      },
      languageId: "mermaid",
      fileName,
      version: 1,
      lineCount: lines.length,
      getText: () => text,
      lineAt: (lineIndex: number) => ({
        text: lines[lineIndex] ?? "",
      }),
    },
    selection: {
      active: {
        line: 0,
      },
    },
  } as unknown as vscode.TextEditor;
}
