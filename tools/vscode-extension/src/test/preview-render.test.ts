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

  it("does not start rendering when cancellation happens while renderStarted is posting", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    let renderCalls = 0;

    await queue.render(
      snapshot(),
      "document-change",
      {
        ...host(queue, messages, async () => {
          renderCalls += 1;
          return "<svg></svg>";
        }),
        postMessage: async (message) => {
          messages.push(message);
          if ((message as { type: string }).type === "renderStarted") {
            await Promise.resolve();
            queue.cancelPending();
          }
        },
      },
    );

    assert.equal(renderCalls, 0);
    assert.deepEqual(
      messages.map((message) => (message as { type: string }).type),
      ["renderStarted"],
    );
  });

  it("aborts the previous render request when a newer render starts", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    const signals: AbortSignal[] = [];
    let resolveFirst: ((content: string) => void) | undefined;

    const first = queue.render(
      snapshot(),
      "document-change",
      host(queue, messages, (_source, signal) => {
        signals.push(signal);
        return new Promise<string>((resolve) => {
          resolveFirst = resolve;
        });
      }),
    );
    await waitUntil(() => signals.length === 1);

    await queue.render(snapshot(), "document-change", host(queue, messages, async () => "<svg id=\"new\"></svg>"));
    assert.equal(signals[0]?.aborted, true);

    resolveFirst?.("<svg id=\"old\"></svg>");
    await first;

    assert.deepEqual(
      messages.map((message) => (message as { type: string }).type),
      ["renderStarted", "renderStarted", "renderSucceeded"],
    );
  });

  it("aborts the current render request when pending work is cancelled", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    const signals: AbortSignal[] = [];
    let resolveRender: ((content: string) => void) | undefined;

    const render = queue.render(
      snapshot(),
      "document-change",
      host(queue, messages, (_source, signal) => {
        signals.push(signal);
        return new Promise<string>((resolve) => {
          resolveRender = resolve;
        });
      }),
    );
    await waitUntil(() => signals.length === 1);

    queue.cancelPending();
    assert.equal(signals[0]?.aborted, true);

    resolveRender?.("<svg></svg>");
    await render;

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

  it("reports renderStarted post failures through the render failure path", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    const errors: string[] = [];

    await queue.render(
      snapshot(),
      "document-change",
      {
        ...host(queue, messages, async () => {
          assert.fail("render content should not run when renderStarted cannot be posted");
        }, errors),
        postMessage: async (message) => {
          messages.push(message);
          if (messages.length === 1) {
            throw new Error("webview unavailable");
          }
        },
      },
    );

    assert.deepEqual(
      messages.map((message) => (message as { type: string }).type),
      ["renderStarted", "renderFailed"],
    );
    assert.deepEqual(errors, ["webview unavailable"]);
  });

  it("swallows renderFailed post failures after logging them", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    const errors: string[] = [];

    await queue.render(
      snapshot(),
      "document-change",
      {
        ...host(queue, messages, async () => {
          throw new Error("syntax issue");
        }, errors),
        postMessage: async (message) => {
          messages.push(message);
          if ((message as { type: string }).type === "renderFailed") {
            throw new Error("webview unavailable");
          }
        },
      },
    );

    assert.deepEqual(
      messages.map((message) => (message as { type: string }).type),
      ["renderStarted", "renderFailed"],
    );
    assert.deepEqual(errors, [
      "syntax issue",
      "failed to notify preview webview: webview unavailable",
    ]);
  });
});

function host(
  queue: PreviewRenderQueue,
  messages: unknown[],
  renderContent: (source: PreviewSnapshot, signal: AbortSignal) => Promise<string>,
  errors: string[] = [],
): PreviewRenderHost {
  return {
    renderContent,
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

async function waitUntil(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (predicate()) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  assert.fail("Condition was not met");
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
    selected: false,
    diagramTheme: "source",
    displayMode: "svg",
    background: "paper",
    locked: false,
  });
}
