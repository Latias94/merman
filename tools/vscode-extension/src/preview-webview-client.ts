import * as vscode from "vscode";

import { previewSourceKeyId, type PreviewSnapshot } from "./preview-model.js";
import {
  snapshotMessagePayload,
  type PreviewToWebviewMessage,
} from "./preview-messages.js";
import { renderPreviewHtml } from "./preview-html.js";

export class PreviewWebviewClient {
  private htmlInitialized = false;
  private ready = false;
  private pendingMessages: PreviewToWebviewMessage[] = [];
  private lastRenderedKeyId: string | undefined;
  private lastRenderedContent: string | undefined;

  constructor(private readonly extensionUri: vscode.Uri) {}

  reset(): void {
    this.htmlInitialized = false;
    this.ready = false;
    this.pendingMessages = [];
    this.lastRenderedKeyId = undefined;
    this.lastRenderedContent = undefined;
  }

  ensureHtml(panel: vscode.WebviewPanel): void {
    if (this.htmlInitialized) {
      return;
    }
    const webview = panel.webview;
    webview.html = renderPreviewHtml({
      resources: {
        cspSource: webview.cspSource,
        stylesUri: webviewResourceUri(webview, this.extensionUri, "preview.css"),
        scriptUri: webviewResourceUri(webview, this.extensionUri, "preview.js"),
      },
    });
    this.htmlInitialized = true;
    this.ready = false;
    this.pendingMessages = [];
  }

  async post(panel: vscode.WebviewPanel | undefined, message: PreviewToWebviewMessage): Promise<void> {
    if (!panel) {
      return;
    }
    if (!this.ready) {
      this.pendingMessages.push(message);
      return;
    }
    await panel.webview.postMessage(message);
  }

  markRendered(snapshot: PreviewSnapshot, content: string): void {
    this.lastRenderedKeyId = previewSourceKeyId(snapshot.sourceKey);
    this.lastRenderedContent = content;
  }

  async acceptReady(
    panel: vscode.WebviewPanel | undefined,
    currentSnapshot: PreviewSnapshot | undefined,
    replaySnapshotUi: (snapshot: PreviewSnapshot) => Promise<void>,
    rerenderSnapshot: (snapshot: PreviewSnapshot) => Promise<void>,
  ): Promise<void> {
    this.ready = true;
    if (!panel) {
      this.pendingMessages = [];
      return;
    }

    if (this.pendingMessages.length > 0) {
      const pending = this.pendingMessages;
      this.pendingMessages = [];
      for (const pendingMessage of pending) {
        await this.post(panel, pendingMessage);
      }
      return;
    }

    if (!currentSnapshot) {
      return;
    }

    await replaySnapshotUi(currentSnapshot);
    if (this.lastRenderedContent && this.lastRenderedKeyId === previewSourceKeyId(currentSnapshot.sourceKey)) {
      await this.post(panel, {
        type: "renderSucceeded",
        requestId: 0,
        snapshot: snapshotMessagePayload(currentSnapshot),
        content: this.lastRenderedContent,
      });
      return;
    }
    await rerenderSnapshot(currentSnapshot);
  }
}

function webviewResourceUri(
  webview: vscode.Webview,
  extensionUri: vscode.Uri,
  fileName: string,
): string {
  return webview.asWebviewUri(vscode.Uri.joinPath(extensionUri, "media", fileName)).toString();
}
