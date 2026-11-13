import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  FakeButtonElement,
  loadPreviewApp,
  type FakeElement,
  type PreviewAppHarness,
} from "./preview-webview-test-harness.js";

describe("preview webview app", () => {
  it("preserves viewport state across same-source SVG replacement and resets for a new source", () => {
    const sourceIdentity = previewSourceIdentity("file:///workspace/notes.md", "fence-1", "hash-a");
    const app = loadPreviewApp({
      zoom: 2,
      panX: 40,
      panY: 20,
      autoFit: false,
      sourceIdentityKey: sourceIdentity,
    });

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a", diagramTheme: "forest" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });

    assert.equal(app.persistedState.zoom, 2);
    assert.equal(app.persistedState.panX, 40);
    assert.equal(app.persistedState.panY, 20);
    assert.equal(app.persistedState.autoFit, false);
    assert.equal(app.persistedState.sourceIdentityKey, sourceIdentity);

    app.dispatch({
      type: "renderSucceeded",
      requestId: 2,
      snapshot: snapshot({ sourceHash: "hash-b" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });

    assert.equal(app.persistedState.zoom, 1);
    assert.equal(app.persistedState.panX, 0);
    assert.equal(app.persistedState.panY, 0);
    assert.equal(app.persistedState.autoFit, true);
    assert.equal(
      app.persistedState.sourceIdentityKey,
      previewSourceIdentity("file:///workspace/notes.md", "fence-1", "hash-b"),
    );
  });

  it("marks the previous SVG as stale when the same source edit fails", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });
    const initialSvg = renderedSvg(app);

    app.dispatch({
      type: "renderStarted",
      requestId: 2,
      reason: "document-change",
      snapshot: snapshot({ sourceHash: "hash-b" }),
    });

    assert.equal(app.document.frame.dataset.renderState, "loading");
    assert.equal(app.document.copySvg.disabled, true);
    assert.equal(app.document.exportSvg.disabled, true);
    assert.equal(app.document.exportPng.disabled, true);

    const postedBeforeLoadingClicks = app.postedMessages.length;
    app.click(app.document.copySvg);
    app.click(app.document.exportSvg);
    app.click(app.document.exportPng);
    assert.equal(app.postedMessages.length, postedBeforeLoadingClicks);

    app.dispatch({
      type: "renderFailed",
      requestId: 2,
      snapshot: snapshot({ sourceHash: "hash-b" }),
      error: "syntax issue",
    });

    assert.equal(renderedSvg(app), initialSvg);
    assert.equal(app.document.status.hidden, false);
    assert.equal(
      app.document.status.textContent,
      "Render failed. Showing last successful preview.\nsyntax issue",
    );
    assert.equal(app.document.status.dataset.kind, "error");
    assert.equal(app.document.frame.dataset.renderState, "stale");
    assert.equal(app.document.copySvg.disabled, true);
    assert.equal(app.document.exportSvg.disabled, true);
    assert.equal(app.document.exportPng.disabled, true);
    assert.equal(
      app.document.copySvg.title,
      "Copy SVG is disabled while the preview shows the last successful render",
    );
  });

  it("blocks stale output actions until the preview renders successfully again", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });
    app.dispatch({
      type: "renderStarted",
      requestId: 2,
      reason: "document-change",
      snapshot: snapshot({ sourceHash: "hash-b" }),
    });
    app.dispatch({
      type: "renderFailed",
      requestId: 2,
      snapshot: snapshot({ sourceHash: "hash-b" }),
      error: "syntax issue",
    });

    const postedBeforeStaleClicks = app.postedMessages.length;
    app.click(app.document.copySvg);
    app.click(app.document.exportSvg);
    app.click(app.document.exportPng);

    assert.equal(app.postedMessages.length, postedBeforeStaleClicks);

    app.dispatch({
      type: "renderSucceeded",
      requestId: 3,
      snapshot: snapshot({ sourceHash: "hash-c" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });

    assert.equal(app.document.copySvg.disabled, false);
    assert.equal(app.document.exportSvg.disabled, false);
    assert.equal(app.document.exportPng.disabled, false);
    assert.equal(app.document.copySvg.title, "Copy rendered SVG");
    assert.equal(app.document.exportSvg.title, "Export SVG");
  });

  it("posts export commands with the rendered source key", () => {
    const app = loadPreviewApp();
    const renderedSnapshot = snapshot({ sourceHash: "hash-a" });

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: renderedSnapshot,
      content: '<svg viewBox="0 0 100 50"></svg>',
    });

    app.click(app.document.exportSvg);
    app.click(app.document.exportPng);

    const posted = JSON.parse(JSON.stringify(app.postedMessages.slice(-2))) as unknown;
    assert.deepEqual(posted, [
      {
        type: "exportRendered",
        format: "svg",
        sourceKey: renderedSnapshot.sourceKey,
      },
      {
        type: "exportRendered",
        format: "png",
        sourceKey: renderedSnapshot.sourceKey,
      },
    ]);
  });

  it("clears the previous SVG when a different source render fails", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({
        documentUri: "file:///workspace/one.mmd",
        sourceId: "document",
        sourceHash: "hash-a",
      }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });

    app.dispatch({
      type: "renderStarted",
      requestId: 2,
      reason: "active-editor",
      snapshot: snapshot({
        documentUri: "file:///workspace/two.mmd",
        sourceId: "document",
        sourceHash: "hash-b",
      }),
    });
    app.dispatch({
      type: "renderFailed",
      requestId: 2,
      snapshot: snapshot({
        documentUri: "file:///workspace/two.mmd",
        sourceId: "document",
        sourceHash: "hash-b",
      }),
      error: "syntax issue",
    });

    assert.equal(renderedSvg(app), null);
    assert.equal(app.document.empty.hidden, true);
    assert.equal(app.document.status.hidden, false);
    assert.equal(app.document.status.textContent, "syntax issue");
    assert.equal(app.document.status.dataset.kind, "error");
    assert.equal(app.document.frame.dataset.renderState, "error");
    assert.equal(
      app.persistedState.sourceLocationKey,
      previewSourceLocation("file:///workspace/two.mmd", "document"),
    );
  });

  it("hides the empty placeholder as soon as rendering starts", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderStarted",
      requestId: 1,
      reason: "manual-open",
      snapshot: snapshot({ sourceHash: "hash-a" }),
    });

    assert.equal(app.document.empty.hidden, true);
    assert.equal(app.document.status.hidden, false);
    assert.equal(app.document.status.textContent, "Rendering SVG preview: Mermaid fence 1");
    assert.equal(app.document.frame.dataset.renderState, "loading");
  });

  it("keeps the empty placeholder hidden when the first render fails for an identified source", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderStarted",
      requestId: 1,
      reason: "manual-open",
      snapshot: snapshot({ sourceHash: "hash-a" }),
    });
    app.dispatch({
      type: "renderFailed",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      error: "syntax issue",
    });

    assert.equal(app.document.empty.hidden, true);
    assert.equal(app.document.status.hidden, false);
    assert.equal(app.document.status.textContent, "syntax issue");
    assert.equal(app.document.status.dataset.kind, "error");
    assert.equal(app.document.frame.dataset.renderState, "error");
  });

  it("ignores stale render success messages after a newer render starts", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });
    const initialSvg = renderedSvg(app);

    app.dispatch({
      type: "renderStarted",
      requestId: 3,
      reason: "document-change",
      snapshot: snapshot({ sourceHash: "hash-c" }),
    });
    app.dispatch({
      type: "renderSucceeded",
      requestId: 2,
      snapshot: snapshot({ sourceHash: "hash-b" }),
      content: '<svg viewBox="0 0 300 150"></svg>',
    });

    assert.equal(renderedSvg(app), initialSvg);
    assert.equal(app.persistedState.sourceIdentityKey, previewSourceIdentity("file:///workspace/notes.md", "fence-1", "hash-a"));
  });

  it("ignores stale empty messages after a newer render succeeds", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 3,
      snapshot: snapshot({ sourceHash: "hash-c" }),
      content: '<svg viewBox="0 0 200 100"></svg>',
    });
    const visibleSvg = renderedSvg(app);

    app.dispatch({
      type: "showEmpty",
      requestId: 2,
      heading: "No Mermaid source",
      detail: "Open a supported file.",
    });

    assert.equal(renderedSvg(app), visibleSvg);
    assert.equal(app.document.empty.hidden, true);
    assert.equal(app.document.frame.dataset.renderState, "ready");
    assert.equal(app.persistedState.sourceIdentityKey, previewSourceIdentity("file:///workspace/notes.md", "fence-1", "hash-c"));
  });

  it("ignores lifecycle messages older than the latest render start", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });
    const initialSvg = renderedSvg(app);

    app.dispatch({
      type: "renderStarted",
      requestId: 3,
      reason: "document-change",
      snapshot: snapshot({ sourceHash: "hash-c" }),
    });
    app.dispatch({
      type: "renderStarted",
      requestId: 2,
      reason: "document-change",
      snapshot: snapshot({ sourceHash: "hash-b" }),
    });
    app.dispatch({
      type: "renderSucceeded",
      requestId: 2,
      snapshot: snapshot({ sourceHash: "hash-b" }),
      content: '<svg viewBox="0 0 300 150"></svg>',
    });

    assert.equal(renderedSvg(app), initialSvg);
    assert.equal(app.document.frame.dataset.renderState, "loading");

    app.dispatch({
      type: "renderSucceeded",
      requestId: 3,
      snapshot: snapshot({ sourceHash: "hash-c" }),
      content: '<svg viewBox="0 0 200 100"></svg>',
    });

    assert.notEqual(renderedSvg(app), initialSvg);
    assert.equal(app.persistedState.sourceIdentityKey, previewSourceIdentity("file:///workspace/notes.md", "fence-1", "hash-c"));
  });

  it("keeps render invalidation as a tombstone for older terminal messages", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });
    app.dispatch({
      type: "renderStarted",
      requestId: 2,
      reason: "document-change",
      snapshot: snapshot({ sourceHash: "hash-b" }),
    });
    app.dispatch({
      type: "renderInvalidated",
      requestId: 3,
      reason: "document-change",
    });
    app.dispatch({
      type: "renderSucceeded",
      requestId: 2,
      snapshot: snapshot({ sourceHash: "hash-b" }),
      content: '<svg viewBox="0 0 300 150"></svg>',
    });

    assert.equal(app.document.frame.dataset.renderState, "stale");
    assert.equal(app.persistedState.sourceIdentityKey, previewSourceIdentity("file:///workspace/notes.md", "fence-1", "hash-a"));
  });

  it("patches diagnostics without replacing the rendered SVG", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });
    const initialSvg = renderedSvg(app);

    app.dispatch({
      type: "diagnosticsUpdated",
      snapshot: snapshot({
        sourceHash: "hash-a",
        diagnostics: {
          summary: "1 errors, 0 warnings, 0 infos, 0 hints",
          totalCount: 1,
          firstTarget: {
            uri: "file:///workspace/notes.md",
            startLine: 1,
            startCharacter: 0,
            endLine: 1,
            endCharacter: 1,
          },
        },
      }),
    });

    assert.equal(renderedSvg(app), initialSvg);
    assert.equal(app.document.diagnostics.hidden, false);
    assert.match(app.document.diagnostics.textContentTree(), /1 errors/);
    assert.doesNotMatch(app.document.diagnostics.textContentTree(), /syntax issue/);
    assert.equal(app.document.diagnostics.querySelector('[data-action="quick-fix"]'), null);
  });

  it("hides diagnostics when there are no issues", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "diagnosticsUpdated",
      snapshot: snapshot({
        diagnostics: {
          summary: "0 errors, 0 warnings, 0 infos, 0 hints",
          totalCount: 0,
        },
      }),
    });

    assert.equal(app.document.diagnostics.hidden, true);
  });

  it("renders ASCII and Unicode modes as text content", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ displayMode: "ascii" }),
      content: "A --> B",
    });

    const textPreview = app.document.canvas.querySelector(".text-preview");
    assert.ok(textPreview);
    assert.equal(textPreview.textContent, "A --> B");
    assert.equal(renderedSvg(app), null);
    assert.equal(app.persistedState.displayMode, "ascii");
    assert.equal(app.document.outputControls.hidden, true);
  });

  it("discards shadow-hosted SVG before text and empty renders", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });
    assert.ok(renderedSvg(app));
    assert.ok(renderedSvgHost(app));
    assert.equal(app.document.canvas.shadowRoot, null);

    app.dispatch({
      type: "renderSucceeded",
      requestId: 2,
      snapshot: snapshot({ displayMode: "ascii", sourceHash: "hash-b" }),
      content: "A --> B",
    });

    const textPreview = app.document.canvas.querySelector(".text-preview");
    assert.ok(textPreview);
    assert.equal(textPreview.textContent, "A --> B");
    assert.equal(renderedSvg(app), null);
    assert.equal(renderedSvgHost(app), null);

    app.dispatch({
      type: "renderSucceeded",
      requestId: 3,
      snapshot: snapshot({ sourceHash: "hash-c" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });
    assert.ok(renderedSvg(app));

    app.dispatch({
      type: "showEmpty",
      heading: "No Mermaid source available",
      detail: "Focus a Mermaid source.",
    });

    assert.equal(renderedSvg(app), null);
    assert.equal(renderedSvgHost(app), null);
    assert.equal(app.document.canvas.children.length, 0);
    assert.equal(app.document.empty.hidden, false);
  });

  it("defaults the preview to paper background and keeps output controls visible for SVG", () => {
    const app = loadPreviewApp();

    assert.equal(app.persistedState.background, "paper");
    assert.equal(app.document.frame.dataset.background, "paper");

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ background: "paper" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });

    assert.equal(app.document.outputControls.hidden, false);
    assert.equal(app.document.background.value, "paper");
  });

  it("copies source-sized SVG even after preview zoom changes DOM dimensions", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      content: '<svg viewBox="0 0 100 50"><path d="M0 0h100"/></svg>',
    });
    const svg = renderedSvg(app);
    assert.ok(svg);
    assert.equal(svg.getAttribute("width"), "100");
    assert.equal(svg.getAttribute("height"), "50");

    svg.setAttribute("width", "250");
    svg.setAttribute("height", "125");
    app.click(app.document.copySvg);

    const message = app.postedMessages.at(-1) as { type?: string; svg?: string; sourceKey?: unknown };
    assert.equal(message.type, "copySvg");
    assert.deepEqual(message.sourceKey, snapshot({ sourceHash: "hash-a" }).sourceKey);
    assert.match(message.svg ?? "", /<svg\b/);
    assert.match(message.svg ?? "", /width="100"/);
    assert.match(message.svg ?? "", /height="50"/);
    assert.doesNotMatch(message.svg ?? "", /width="250"/);
    assert.doesNotMatch(message.svg ?? "", /data-base-width/);
  });

  it("isolates rendered SVG styles from preview chrome", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      content:
        '<svg viewBox="0 0 100 50"><style>.toolbar{display:none!important}</style><rect width="100" height="50"/></svg>',
    });

    assert.equal(app.document.canvas.querySelector("svg"), null);
    assert.ok(renderedSvg(app));

    app.click(app.document.copySvg);

    const message = app.postedMessages.at(-1) as { type?: string; svg?: string };
    assert.equal(message.type, "copySvg");
    assert.match(message.svg ?? "", /<svg\b/);
  });

  it("supports drag pan, wheel zoom, fit, and actual-size controls in SVG mode", () => {
    const app = loadPreviewApp({
      zoom: 1,
      panX: 0,
      panY: 0,
      autoFit: true,
      sourceIdentityKey: previewSourceIdentity("file:///workspace/notes.md", "fence-1", "hash-a"),
    });

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      content: '<svg viewBox="0 0 100 50"></svg>',
    });

    app.dispatchViewport("pointerdown", {
      target: app.document.canvas,
      button: 0,
      pointerId: 7,
      clientX: 100,
      clientY: 100,
    });
    app.dispatchDocument("pointermove", {
      pointerId: 7,
      clientX: 128,
      clientY: 116,
    });
    app.dispatchDocument("pointerup", {
      pointerId: 7,
    });

    assert.equal(app.persistedState.autoFit, false);
    assert.equal(app.persistedState.panX, 28);
    assert.equal(app.persistedState.panY, 16);

    let wheelPrevented = false;
    app.dispatchViewport("wheel", {
      target: app.document.canvas,
      deltaY: -120,
      clientX: 400,
      clientY: 300,
      preventDefault: () => {
        wheelPrevented = true;
      },
    });

    assert.equal(wheelPrevented, true);
    assert.ok((app.persistedState.zoom as number) > 1);

    app.click(app.document.fit);

    assert.equal(app.persistedState.autoFit, true);
    assert.equal(app.persistedState.zoom, 1);
    assert.equal(app.persistedState.panX, 0);
    assert.equal(app.persistedState.panY, 0);

    app.click(app.document.zoomIn);
    assert.ok((app.persistedState.zoom as number) > 1);

    app.click(app.document.reset);
    assert.equal(app.persistedState.autoFit, false);
    assert.equal(app.persistedState.zoom, 1);
    assert.equal(app.persistedState.panX, 0);
    assert.equal(app.persistedState.panY, 0);
  });

  it("hides the preview source bar unless a Markdown document has multiple Mermaid fences", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "settingsUpdated",
      snapshot: snapshot({
        sources: [
          sourceOption("document", "example.mmd", "Mermaid source file", "mermaid-file"),
        ],
      }),
    });
    assert.equal(app.document.sourceBar.hidden, true);
    assert.equal(app.document.viewport.classList.contains("has-sourcebar"), false);

    app.dispatch({
      type: "settingsUpdated",
      snapshot: snapshot({
        sources: [
          sourceOption("fence-1", "notes.md", "Mermaid fence 1", "markdown-fence"),
          sourceOption("fence-2", "notes.md", "Mermaid fence 2", "markdown-fence"),
        ],
      }),
    });
    assert.equal(app.document.sourceBar.hidden, false);
    assert.equal(app.document.viewport.classList.contains("has-sourcebar"), true);
  });

  it("shows and toggles the preview lock state", () => {
    const app = loadPreviewApp();

    assert.equal(app.document.lock.disabled, true);

    app.dispatch({
      type: "settingsUpdated",
      snapshot: snapshot({ locked: true }),
    });

    assert.equal(app.document.lock.textContent, "Locked");
    assert.equal(app.document.lock.getAttribute("aria-pressed"), "true");
    assert.equal(app.document.lock.disabled, false);
    assert.equal(app.persistedState.locked, true);

    app.click(app.document.lock);

    const message = app.postedMessages.at(-1) as { type?: string; locked?: boolean };
    assert.equal(message.type, "setLocked");
    assert.equal(message.locked, false);

    app.dispatch({
      type: "showEmpty",
      heading: "No Mermaid source available",
      detail: "Focus a Mermaid source.",
    });

    assert.equal(app.document.lock.textContent, "Follow");
    assert.equal(app.document.lock.disabled, true);
    assert.equal(app.persistedState.locked, false);
  });

  it("posts preview instance commands from toolbar actions", () => {
    const app = loadPreviewApp();

    app.click(app.document.refresh);
    app.click(app.document.showSource);

    assert.deepEqual(app.postedMessages.slice(-2).map((message) => (message as { type?: string }).type), [
      "refresh",
      "showSource",
    ]);
  });

  it("ignores data-action elements inside rendered preview content", () => {
    const app = loadPreviewApp();
    const renderedButton = new FakeButtonElement({ dataset: { action: "copy-svg" } });
    app.document.canvas.appendChild(renderedButton);

    app.click(renderedButton);

    assert.deepEqual(app.postedMessages.map((message) => (message as { type?: string }).type), [
      "ready",
    ]);
  });
});

function snapshot(options: {
  documentUri?: string;
  sourceId?: string;
  sourceHash?: string;
  diagramTheme?: string;
  diagnostics?: unknown;
  displayMode?: string;
  background?: string;
  locked?: boolean;
  sources?: Array<{
    sourceId: string;
    title: string;
    subtitle: string;
    kind: string;
  }>;
}): Record<string, unknown> {
  const documentUri = options.documentUri ?? "file:///workspace/notes.md";
  const sourceId = options.sourceId ?? "fence-1";
  const sourceHash = options.sourceHash ?? "hash-a";
  const diagramTheme = options.diagramTheme ?? "source";
  const displayMode = options.displayMode ?? "svg";
  const background = options.background ?? "paper";
  const locked = options.locked ?? false;
  return {
    documentUri,
    sourceId,
    title: "notes.md",
    subtitle: "Mermaid fence 1",
    selectionLine: 1,
    selected: false,
    diagramTheme,
    displayMode,
    background,
    locked,
    sourceKey: {
      documentUri,
      sourceId,
      sourceHash,
      diagramTheme,
      displayMode,
      background,
    },
    sources: options.sources ?? [
      sourceOption("fence-1", "notes.md", "Mermaid fence 1", "markdown-fence"),
    ],
    diagnostics: options.diagnostics,
  };
}

function sourceOption(sourceId: string, title: string, subtitle: string, kind: string): {
  sourceId: string;
  title: string;
  subtitle: string;
  kind: string;
} {
  return {
    sourceId,
    title,
    subtitle,
    kind,
  };
}

function renderedSvg(app: PreviewAppHarness): FakeElement | null {
  return renderedSvgHost(app)?.shadowRoot?.querySelector("svg") ?? null;
}

function renderedSvgHost(app: PreviewAppHarness): FakeElement | null {
  return app.document.canvas.children.find((child) => child.shadowRoot?.querySelector("svg")) ?? null;
}

function previewSourceIdentity(documentUri: string, sourceId: string, sourceHash: string): string {
  return [documentUri, sourceId, sourceHash].join("\u0000");
}

function previewSourceLocation(documentUri: string, sourceId: string): string {
  return [documentUri, sourceId].join("\u0000");
}
