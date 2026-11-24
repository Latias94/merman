import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  createPreviewSnapshot,
  type CreatePreviewSnapshotRequest,
  type PreviewSnapshot,
} from "../preview-model.js";
import { PreviewRenderQueue, type PreviewRenderContent, type PreviewRenderHost } from "../preview-render.js";
import { listPreviewInputsFromText } from "../preview-source.js";

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

  it("reuses SVG frames across edits and clears them when changing display mode", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    const first = snapshot();
    const edited = snapshot({
      documentVersion: 2,
      input: { ...first.input, source: "flowchart TD\nA --> C" },
    });
    assert.notEqual(first.sourceKey.sourceHash, edited.sourceKey.sourceHash);
    const frame = Buffer.from('{"nodes":{}}');
    const result = { content: "<svg/>", previewLayoutFrame: frame };
    const observed: Array<Buffer | undefined> = [];

    await queue.render(first, "manual-open", host(queue, messages, async (_source, _signal, previous) => {
      observed.push(previous);
      return result;
    }));
    await queue.render(edited, "document-change", host(queue, messages, async (_source, _signal, previous) => {
      observed.push(previous);
      return result;
    }));
    await queue.render({ ...edited, displayMode: "ascii" }, "document-change", host(queue, messages, async (_source, _signal, previous) => {
      observed.push(previous);
      return { content: "text", previewLayoutFrame: Buffer.from("ignored") };
    }));
    await queue.render(edited, "document-change", host(queue, messages, async (_source, _signal, previous) => {
      observed.push(previous);
      return "<svg/>";
    }));
    assert.deepEqual(observed, [undefined, frame, undefined, undefined]);
    assert.equal(messages.some((message) => Object.hasOwn(message as object, "previewLayoutFrame")), false);
  });

  it("keeps the previous successful frame after cancelled and failed attempts", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    const frame = Buffer.from("accepted");
    const content = "<svg/>";
    await queue.render(snapshot(), "manual-open", host(queue, messages, async () => ({
      content, previewLayoutFrame: frame,
    })));
    await queue.render(snapshot(), "document-change", host(queue, messages, async () => {
      queue.cancelPending();
      return { content, previewLayoutFrame: Buffer.from("cancelled") };
    }));
    await queue.render(snapshot(), "document-change", host(queue, messages, async () => {
      throw new Error("invalid source");
    }));
    await queue.render(snapshot(), "document-change", {
      ...host(queue, messages, async () => ({ content, previewLayoutFrame: Buffer.from("rejected") })),
      postMessage: async (message) => {
        if (message.type === "renderSucceeded") {
          throw new Error("closed webview");
        }
      },
    });
    const observed: Array<Buffer | undefined> = [];
    await queue.render(snapshot(), "document-change", host(queue, messages, async (_source, _signal, previous) => {
      observed.push(previous);
      return content;
    }));
    assert.deepEqual(observed, [frame]);
  });

  it("discards frames when switching documents, reopening a file, moving a fence, or resetting a panel", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    const first = snapshot();
    const otherDocument = snapshot({ documentUri: "file:///workspace/other.mmd" });
    const reopened = snapshot({ input: { ...first.input, source: "flowchart LR\nC --> D" } });
    const markdown = snapshot({
      documentUri: "file:///workspace/example.md",
      input: { ...first.input, kind: "markdown-fence", sourceId: "fence-1" },
    });
    const movedFence = snapshot({
      documentUri: markdown.documentUri,
      input: { ...markdown.input, sourceRange: { startLine: 4, endLine: 6 } },
    });
    const observed: Array<Buffer | undefined> = [];
    const coldCapture = host(queue, messages, async (_source, _signal, previous) => {
      observed.push(previous);
      return { content: "<svg/>", previewLayoutFrame: Buffer.from("accepted") };
    });
    for (const source of [first, otherDocument, first, reopened, markdown, movedFence]) {
      await queue.render(source, "manual-open", coldCapture);
    }
    queue.reset();
    await queue.render(movedFence, "manual-open", coldCapture);
    assert.deepEqual(observed, Array.from({ length: 7 }, () => undefined));
  });

  it("reuses frames for Markdown fence edits and cold-starts an equal-length fence swap", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    const first = "flowchart LR\nA[first] --> B";
    const edited = "flowchart LR\nA[edits] --> B";
    const second = "flowchart LR\nA[other] --> B";
    const snapshots = [
      markdownSnapshot(1, first, second),
      markdownSnapshot(2, edited, second),
      markdownSnapshot(3, `${edited}\nB --> C`, second),
      markdownSnapshot(4, edited, second),
      markdownSnapshot(5, second, edited),
      markdownSnapshot(6, "flowchart LR\nA[after] --> B", edited),
    ];
    assert.equal(edited.length, second.length);
    assert.deepEqual(
      snapshots[3]!.sources.map((source) => [source.sourceId, source.sourceRange]),
      snapshots[4]!.sources.map((source) => [source.sourceId, source.sourceRange]),
    );
    const frames = snapshots.map((_, index) => Buffer.from(`frame ${index}`));
    const observed: Array<Buffer | undefined> = [];
    for (const [index, source] of snapshots.entries()) {
      await queue.render(source, "document-change", host(queue, messages, async (_source, _signal, previous) => {
        observed.push(previous);
        return { content: "<svg/>", previewLayoutFrame: frames[index] };
      }));
    }
    assert.deepEqual(observed, [undefined, frames[0], frames[1], frames[2], undefined, frames[4]]);
  });

  it("cold-starts when changed sibling fences make a Markdown edit ambiguous", async () => {
    const queue = new PreviewRenderQueue();
    const messages: unknown[] = [];
    const first = "flowchart LR\nA[first] --> B";
    const second = "flowchart LR\nA[other] --> B";
    const original = markdownSnapshot(1, first, second);
    const copied = markdownSnapshot(2, second, second);
    const bothChanged = markdownSnapshot(3, first, "flowchart LR\nA[third] --> B");
    const observed: Array<Buffer | undefined> = [];
    for (const source of [original, copied, bothChanged]) {
      await queue.render(source, "document-change", host(queue, messages, async (_source, _signal, previous) => {
        observed.push(previous);
        return { content: "<svg/>", previewLayoutFrame: Buffer.from("accepted") };
      }));
    }
    assert.deepEqual(observed, [undefined, undefined, undefined]);
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
  renderContent: (
    source: PreviewSnapshot,
    signal: AbortSignal,
    previousLayoutFrame: Buffer | undefined,
  ) => Promise<string | PreviewRenderContent>,
  errors: string[] = [],
): PreviewRenderHost {
  return {
    renderContent: async (...args) => {
      const rendered = await renderContent(...args);
      return typeof rendered === "string" ? { content: rendered } : rendered;
    },
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

function snapshot(overrides: Partial<CreatePreviewSnapshotRequest> = {}): PreviewSnapshot {
  const input = overrides.input ?? {
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
    ...overrides,
  });
}

function markdownSnapshot(documentVersion: number, first: string, second: string): PreviewSnapshot {
  const sources = listPreviewInputsFromText({
    fileName: "example.md",
    languageId: "markdown",
    text: ["```mermaid", first, "```", "", "```mermaid", second, "```"].join("\n"),
  });
  return snapshot({
    documentUri: "file:///workspace/example.md",
    documentVersion,
    input: sources[0]!,
    sources,
  });
}
