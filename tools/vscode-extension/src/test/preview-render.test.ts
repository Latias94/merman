import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { createPreviewSnapshot, type PreviewSnapshot } from "../preview-model.js";
import { PreviewRenderQueue, type PreviewRenderHost } from "../preview-render.js";

describe("preview render queue", () => {
  it("posts render lifecycle messages for a successful render", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];

    await queue.render(snapshot(), "manual-open", host(queue, messages, async () => "<svg></svg>"));

    assert.deepEqual(
      messages.map((message) => (message as { type: string }).type),
      ["renderStarted", "renderSucceeded"],
    );
  });

  it("ignores stale render success after a newer request cancels it", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];

    await queue.render(
      snapshot(),
      "document-change",
      host(queue, messages, async () => {
        queue.cancelPending();
        return "<svg></svg>";
      }),
    );

    assert.deepEqual(
      messages.map((message) => (message as { type: string }).type),
      ["renderStarted"],
    );
  });

  it("posts renderFailed without replacing the svg on current failures", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    const errors: string[] = [];

    await queue.render(
      snapshot(),
      "document-change",
      host(queue, messages, async () => {
        throw new Error("syntax issue");
      }, errors),
    );

    assert.deepEqual(
      messages.map((message) => (message as { type: string }).type),
      ["renderStarted", "renderFailed"],
    );
    assert.deepEqual(errors, ["syntax issue"]);
  });
});

function host(
  queue: PreviewRenderQueue,
  messages: unknown[],
  renderSvg: (source: string) => Promise<string>,
  errors: string[] = [],
): PreviewRenderHost {
  return {
    renderSvg,
    postMessage: async (message) => {
      messages.push(message);
    },
    info: () => {},
    error: (message) => {
      errors.push(message);
    },
    isCurrentRequest: (requestId) => queue.isCurrentRequest(requestId),
    markRendered: () => {},
  };
}

function snapshot(): PreviewSnapshot {
  const input = {
    sourceId: "document",
    source: "flowchart TD\nA --> B\n",
    title: "example.mmd",
    subtitle: "Mermaid source file",
    exportBaseName: "example",
    kind: "mermaid-file" as const,
    sourceRange: {
      startLine: 0,
      endLine: 1,
    },
    diagnosticRange: {
      startLine: 0,
      endLine: 1,
    },
  };
  return createPreviewSnapshot({
    documentUri: "file:///workspace/example.mmd",
    documentVersion: 1,
    input,
    sources: [input],
    selectionLine: 0,
    pinned: false,
    diagramTheme: "source",
  });
}
