import * as assert from "node:assert/strict";
import Module from "node:module";
import type * as vscode from "vscode";
import { describe, it } from "node:test";

import type { PreviewSnapshot } from "../preview-model.js";
import type { PreviewToWebviewMessage } from "../preview-messages.js";

describe("preview webview client", () => {
  it("preserves unsent pending messages when ready replay fails", async () => {
    const { PreviewWebviewClient } = loadPreviewWebviewClient();
    const client = new PreviewWebviewClient({} as vscode.Uri);
    const posted: PreviewToWebviewMessage[] = [];
    let failSecondPost = true;
    const panel = {
      webview: {
        postMessage: async (message: PreviewToWebviewMessage) => {
          posted.push(message);
          return !(failSecondPost && posted.length === 2);
        },
      },
    } as unknown as vscode.WebviewPanel;

    await client.post(panel, message("sourceListUpdated"));
    await client.post(panel, message("renderStarted"));
    await client.post(panel, message("renderSucceeded"));

    await assert.rejects(
      client.acceptReady(panel, undefined, async () => {}, async () => {}),
      /Preview webview did not accept the message/,
    );
    assert.deepEqual(posted.map(({ type }) => type), ["sourceListUpdated", "renderStarted"]);

    failSecondPost = false;
    await client.acceptReady(panel, undefined, async () => {}, async () => {});

    assert.deepEqual(posted.map(({ type }) => type), [
      "sourceListUpdated",
      "renderStarted",
      "renderStarted",
      "renderSucceeded",
    ]);
  });

  it("coalesces pre-ready render messages to the latest lifecycle", async () => {
    const { PreviewWebviewClient } = loadPreviewWebviewClient();
    const client = new PreviewWebviewClient({} as vscode.Uri);
    const posted: PreviewToWebviewMessage[] = [];
    const panel = {
      webview: {
        postMessage: async (message: PreviewToWebviewMessage) => {
          posted.push(message);
          return true;
        },
      },
    } as unknown as vscode.WebviewPanel;

    await client.post(panel, renderMessage("renderStarted", 1));
    await client.post(panel, renderMessage("renderSucceeded", 1));
    await client.post(panel, renderMessage("renderStarted", 2));
    await client.post(panel, renderMessage("renderFailed", 2));

    await client.acceptReady(panel, undefined, async () => {}, async () => {});

    assert.deepEqual(posted.map(({ type }) => type), ["renderStarted", "renderFailed"]);
    assert.deepEqual(
      posted.map((postedMessage) =>
        "requestId" in postedMessage ? postedMessage.requestId : undefined,
      ),
      [2, 2],
    );
  });

  it("does not replay stale rendered content after invalidation", async () => {
    const { PreviewWebviewClient } = loadPreviewWebviewClient();
    const client = new PreviewWebviewClient({} as vscode.Uri);
    const posted: PreviewToWebviewMessage[] = [];
    const panel = {
      webview: {
        postMessage: async (message: PreviewToWebviewMessage) => {
          posted.push(message);
          return true;
        },
      },
    } as unknown as vscode.WebviewPanel;
    const snapshot = previewSnapshot();

    client.markRendered(snapshot, "<svg><text>old</text></svg>");
    client.invalidateRenderedOutput();

    let replayed = false;
    let rerendered = false;
    await client.acceptReady(
      panel,
      snapshot,
      async () => {
        replayed = true;
      },
      async (actualSnapshot) => {
        rerendered = actualSnapshot === snapshot;
      },
    );

    assert.equal(replayed, true);
    assert.equal(rerendered, true);
    assert.deepEqual(posted, []);
  });
});

function message(type: string): PreviewToWebviewMessage {
  return { type } as PreviewToWebviewMessage;
}

function renderMessage(type: string, requestId: number): PreviewToWebviewMessage {
  return { type, requestId } as PreviewToWebviewMessage;
}

function previewSnapshot(): PreviewSnapshot {
  const sourceKey = {
    documentUri: "file:///tmp/example.mmd",
    sourceId: "document",
    sourceHash: "hash",
    diagramTheme: "default" as const,
    displayMode: "svg" as const,
    background: "paper" as const,
  };
  return {
    documentUri: sourceKey.documentUri,
    documentVersion: 1,
    input: {
      sourceId: sourceKey.sourceId,
      source: "graph TD\nA-->B\n",
      title: "example.mmd",
      subtitle: "Mermaid source file",
      exportBaseName: "example",
      kind: "mermaid-file",
      sourceRange: {
        startLine: 0,
        endLine: 1,
      },
      diagnosticRange: {
        startLine: 0,
        endLine: 1,
      },
    },
    sources: [],
    selectionLine: 0,
    selected: true,
    diagramTheme: sourceKey.diagramTheme,
    displayMode: sourceKey.displayMode,
    background: sourceKey.background,
    locked: false,
    sourceKey,
  };
}

function loadPreviewWebviewClient(): typeof import("../preview-webview-client.js") {
  type LoadModule = (this: unknown, request: string, parent: unknown, isMain: boolean) => unknown;
  const moduleWithLoad = Module as typeof Module & { _load: LoadModule };
  const originalLoad = moduleWithLoad._load;
  moduleWithLoad._load = function patchedLoad(
    this: unknown,
    request: string,
    parent: unknown,
    isMain: boolean,
  ): unknown {
    if (request === "vscode") {
      return {};
    }
    return originalLoad.call(this, request, parent, isMain);
  };
  try {
    delete require.cache[require.resolve("../preview-webview-client.js")];
    return require("../preview-webview-client.js") as typeof import("../preview-webview-client.js");
  } finally {
    moduleWithLoad._load = originalLoad;
  }
}
