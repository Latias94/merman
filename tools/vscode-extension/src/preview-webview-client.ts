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
  private pendingMessages = new PendingPreviewMessages();
  private lastRenderedKeyId: string | undefined;
  private lastRenderedContent: string | undefined;

  constructor(private readonly extensionUri: vscode.Uri) {}

  reset(): void {
    this.htmlInitialized = false;
    this.ready = false;
    this.pendingMessages.clear();
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
    this.pendingMessages.clear();
  }

  async post(panel: vscode.WebviewPanel | undefined, message: PreviewToWebviewMessage): Promise<void> {
    if (!panel) {
      return;
    }
    if (!this.ready) {
      this.pendingMessages.enqueue(message);
      return;
    }
    const accepted = await panel.webview.postMessage(message);
    if (!accepted) {
      throw new Error("Preview webview did not accept the message.");
    }
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
      this.pendingMessages.clear();
      return;
    }

    if (this.pendingMessages.hasMessages()) {
      const pending = this.pendingMessages.drain();
      for (const [index, pendingMessage] of pending.entries()) {
        try {
          await this.post(panel, pendingMessage);
        } catch (error) {
          this.ready = false;
          this.pendingMessages.replace(pending.slice(index));
          throw error;
        }
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

class PendingPreviewMessages {
  private empty: PreviewToWebviewMessage | undefined;
  private sourceList: PreviewToWebviewMessage | undefined;
  private selection: PreviewToWebviewMessage | undefined;
  private diagnostics: PreviewToWebviewMessage | undefined;
  private settings: PreviewToWebviewMessage | undefined;
  private renderStarted: PreviewToWebviewMessage | undefined;
  private renderFinished: PreviewToWebviewMessage | undefined;

  clear(): void {
    this.empty = undefined;
    this.sourceList = undefined;
    this.selection = undefined;
    this.diagnostics = undefined;
    this.settings = undefined;
    this.renderStarted = undefined;
    this.renderFinished = undefined;
  }

  hasMessages(): boolean {
    return this.messages().length > 0;
  }

  enqueue(message: PreviewToWebviewMessage): void {
    switch (message.type) {
      case "showEmpty":
        this.clear();
        this.empty = message;
        return;
      case "sourceListUpdated":
        this.empty = undefined;
        this.sourceList = message;
        return;
      case "selectionChanged":
        this.empty = undefined;
        this.selection = message;
        return;
      case "diagnosticsUpdated":
        this.empty = undefined;
        this.diagnostics = message;
        return;
      case "settingsUpdated":
        this.empty = undefined;
        this.settings = message;
        return;
      case "renderStarted":
        this.empty = undefined;
        this.renderStarted = message;
        if (
          this.renderFinished &&
          requestId(this.renderFinished) < requestId(message)
        ) {
          this.renderFinished = undefined;
        }
        return;
      case "renderSucceeded":
      case "renderFailed":
        this.empty = undefined;
        if (!this.renderStarted || requestId(message) >= requestId(this.renderStarted)) {
          this.renderFinished = message;
        }
        return;
    }
  }

  drain(): PreviewToWebviewMessage[] {
    const messages = this.messages();
    this.clear();
    return messages;
  }

  replace(messages: PreviewToWebviewMessage[]): void {
    this.clear();
    for (const message of messages) {
      this.enqueue(message);
    }
  }

  private messages(): PreviewToWebviewMessage[] {
    if (this.empty) {
      return [this.empty];
    }
    return [
      this.sourceList,
      this.selection,
      this.diagnostics,
      this.settings,
      this.renderStarted,
      this.renderFinished,
    ].filter((message): message is PreviewToWebviewMessage => Boolean(message));
  }
}

function requestId(message: PreviewToWebviewMessage): number {
  return "requestId" in message && typeof message.requestId === "number" ? message.requestId : -1;
}

function webviewResourceUri(
  webview: vscode.Webview,
  extensionUri: vscode.Uri,
  fileName: string,
): string {
  return webview.asWebviewUri(vscode.Uri.joinPath(extensionUri, "media", fileName)).toString();
}
