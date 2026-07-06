import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import * as vm from "node:vm";
import { describe, it } from "node:test";

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
    const initialSvg = app.document.canvas.querySelector("svg");

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

    assert.equal(app.document.canvas.querySelector("svg"), initialSvg);
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

    assert.equal(app.document.canvas.querySelector("svg"), null);
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
    const initialSvg = app.document.canvas.querySelector("svg");

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

    assert.equal(app.document.canvas.querySelector("svg"), initialSvg);
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
    const initialSvg = app.document.canvas.querySelector("svg");

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

    assert.equal(app.document.canvas.querySelector("svg"), initialSvg);
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
    assert.equal(app.document.canvas.querySelector("svg"), null);
    assert.equal(app.persistedState.displayMode, "ascii");
    assert.equal(app.document.outputControls.hidden, true);
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
    const svg = app.document.canvas.querySelector("svg");
    assert.ok(svg);
    assert.equal(svg.getAttribute("width"), "100");
    assert.equal(svg.getAttribute("height"), "50");

    svg.setAttribute("width", "250");
    svg.setAttribute("height", "125");
    app.click(app.document.copySvg);

    const message = app.postedMessages.at(-1) as { type?: string; svg?: string };
    assert.equal(message.type, "copySvg");
    assert.match(message.svg ?? "", /<svg\b/);
    assert.match(message.svg ?? "", /width="100"/);
    assert.match(message.svg ?? "", /height="50"/);
    assert.doesNotMatch(message.svg ?? "", /width="250"/);
    assert.doesNotMatch(message.svg ?? "", /data-base-width/);
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

interface PreviewAppHarness {
  document: FakeDocument;
  dispatch(message: unknown): void;
  click(target: FakeElement): void;
  dispatchDocument(type: string, event: unknown): void;
  dispatchViewport(type: string, event: unknown): void;
  readonly persistedState: Record<string, unknown>;
  readonly postedMessages: readonly unknown[];
}

function loadPreviewApp(initialState: Record<string, unknown> = {}): PreviewAppHarness {
  const document = new FakeDocument();
  const windowListeners = new Map<string, Array<(event: { data: unknown }) => void>>();
  let persistedState = { ...initialState };
  const postedMessages: unknown[] = [];
  const context = vm.createContext({
    console,
    document,
    window: {
      addEventListener(type: string, listener: (event: { data: unknown }) => void): void {
        const listeners = windowListeners.get(type) ?? [];
        listeners.push(listener);
        windowListeners.set(type, listeners);
      },
    },
    acquireVsCodeApi: () => ({
      getState: () => persistedState,
      setState: (next: Record<string, unknown>) => {
        persistedState = { ...next };
      },
      postMessage: (message: unknown) => {
        postedMessages.push(message);
      },
    }),
    HTMLElement: FakeElement,
    SVGElement: FakeElement,
    HTMLSelectElement: FakeSelectElement,
    HTMLButtonElement: FakeButtonElement,
    ResizeObserver: class {
      observe(): void {}
    },
    requestAnimationFrame: (callback: () => void) => {
      callback();
      return 1;
    },
  });

  const script = fs.readFileSync(path.join(process.cwd(), "media", "preview.js"), "utf8");
  vm.runInContext(script, context);
  assert.equal((postedMessages.at(-1) as { type?: string } | undefined)?.type, "ready");

  return {
    document,
    dispatch(message: unknown): void {
      for (const listener of windowListeners.get("message") ?? []) {
        listener({ data: message });
      }
    },
    click(target: FakeElement): void {
      document.dispatch("click", {
        target,
      });
    },
    dispatchDocument(type: string, event: unknown): void {
      document.dispatch(type, event);
    },
    dispatchViewport(type: string, event: unknown): void {
      document.viewport.dispatch(type, event);
    },
    get persistedState(): Record<string, unknown> {
      return persistedState;
    },
    get postedMessages(): readonly unknown[] {
      return postedMessages;
    },
  };
}

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

function previewSourceIdentity(documentUri: string, sourceId: string, sourceHash: string): string {
  return [documentUri, sourceId, sourceHash].join("\u0000");
}

function previewSourceLocation(documentUri: string, sourceId: string): string {
  return [documentUri, sourceId].join("\u0000");
}

class FakeDocument {
  readonly frame = new FakeElement("section", { className: "frame" });
  readonly viewport = new FakeElement("section", {
    className: "viewport",
    clientWidth: 800,
    clientHeight: 600,
  });
  readonly stage = new FakeElement("div", { className: "stage" });
  readonly canvas = new FakeElement("div", { dataset: { previewCanvas: "" } });
  readonly zoomValue = new FakeElement("span", { dataset: { zoomValue: "" } });
  readonly diagnostics = new FakeElement("section", { dataset: { previewDiagnostics: "" } });
  readonly status = new FakeElement("p", { dataset: { previewStatus: "" } });
  readonly empty = new FakeElement("div", { dataset: { previewEmpty: "" } });
  readonly sourceBar = new FakeElement("div", { dataset: { previewSourcebar: "" } });
  readonly sourceList = new FakeSelectElement({ dataset: { previewSourceList: "", action: "source" } });
  readonly outputControls = new FakeElement("span", { dataset: { previewOutputControls: "" } });
  readonly displayMode = new FakeSelectElement({ dataset: { action: "display-mode" } });
  readonly theme = new FakeSelectElement({ dataset: { action: "diagram-theme" } });
  readonly background = new FakeSelectElement({ dataset: { action: "background" } });
  readonly copySvg = new FakeButtonElement({ dataset: { action: "copy-svg" } });
  readonly refresh = new FakeButtonElement({ dataset: { action: "refresh" } });
  readonly showSource = new FakeButtonElement({ dataset: { action: "show-source" } });
  readonly exportSvg = new FakeButtonElement({ dataset: { action: "export-svg" } });
  readonly exportPng = new FakeButtonElement({ dataset: { action: "export-png" } });
  readonly lock = new FakeButtonElement({ dataset: { previewLock: "", action: "lock" } });
  readonly fit = new FakeButtonElement({ dataset: { action: "fit" } });
  readonly reset = new FakeButtonElement({ dataset: { action: "reset" } });
  readonly zoomIn = new FakeButtonElement({ dataset: { action: "zoom-in" } });
  private readonly listeners = new Map<string, Array<(event: unknown) => void>>();

  constructor() {
    this.empty.appendChild(new FakeElement("h2"));
    this.empty.appendChild(new FakeElement("p"));
  }

  querySelector(selector: string): FakeElement | null {
    switch (selector) {
      case ".frame":
        return this.frame;
      case ".viewport":
        return this.viewport;
      case ".stage":
        return this.stage;
      case "[data-preview-canvas]":
        return this.canvas;
      case "[data-zoom-value]":
        return this.zoomValue;
      case "[data-preview-diagnostics]":
        return this.diagnostics;
      case "[data-preview-status]":
        return this.status;
      case "[data-preview-empty]":
        return this.empty;
      case "[data-preview-sourcebar]":
        return this.sourceBar;
      case "[data-preview-source-list]":
        return this.sourceList;
      case "[data-preview-output-controls]":
        return this.outputControls;
      case '[data-action="display-mode"]':
        return this.displayMode;
      case '[data-action="diagram-theme"]':
        return this.theme;
      case '[data-action="background"]':
        return this.background;
      case '[data-action="copy-svg"]':
        return this.copySvg;
      case '[data-action="refresh"]':
        return this.refresh;
      case '[data-action="show-source"]':
        return this.showSource;
      case '[data-action="export-svg"]':
        return this.exportSvg;
      case '[data-action="export-png"]':
        return this.exportPng;
      case "[data-preview-lock]":
        return this.lock;
      case '[data-action="fit"]':
        return this.fit;
      case '[data-action="reset"]':
        return this.reset;
      case '[data-action="zoom-in"]':
        return this.zoomIn;
      default:
        return null;
    }
  }

  createElement(tagName: string): FakeElement {
    if (tagName === "select") {
      return new FakeSelectElement();
    }
    if (tagName === "button") {
      return new FakeButtonElement();
    }
    return new FakeElement(tagName);
  }

  addEventListener(type: string, listener: (event: unknown) => void): void {
    const listeners = this.listeners.get(type) ?? [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }

  dispatch(type: string, event: unknown): void {
    for (const listener of this.listeners.get(type) ?? []) {
      listener(event);
    }
  }
}

class FakeElement {
  readonly tagName: string;
  readonly dataset: Record<string, string>;
  readonly style = new FakeStyleDeclaration();
  readonly classList = new FakeClassList();
  readonly children: FakeElement[] = [];
  hidden = false;
  disabled = false;
  className = "";
  textContent = "";
  title = "";
  type = "";
  value = "";
  selected = false;
  clientWidth = 1;
  clientHeight = 1;
  parentElement: FakeElement | null = null;
  private attributes = new Map<string, string>();
  private html = "";
  private readonly listeners = new Map<string, Array<(event: unknown) => void>>();

  constructor(
    tagName: string,
    options: {
      className?: string;
      dataset?: Record<string, string>;
      clientWidth?: number;
      clientHeight?: number;
    } = {},
  ) {
    this.tagName = tagName.toLowerCase();
    this.className = options.className ?? "";
    this.dataset = { ...(options.dataset ?? {}) };
    this.clientWidth = options.clientWidth ?? 1;
    this.clientHeight = options.clientHeight ?? 1;
  }

  appendChild<T extends FakeElement>(child: T): T {
    child.parentElement = this;
    this.children.push(child);
    return child;
  }

  cloneNode(deep = false): FakeElement {
    const clone = new FakeElement(this.tagName, {
      className: this.className,
      dataset: { ...this.dataset },
      clientWidth: this.clientWidth,
      clientHeight: this.clientHeight,
    });
    clone.textContent = this.textContent;
    clone.title = this.title;
    clone.type = this.type;
    clone.value = this.value;
    clone.selected = this.selected;
    clone.disabled = this.disabled;
    clone.html = this.html;
    for (const [name, value] of this.attributes.entries()) {
      clone.attributes.set(name, value);
    }
    if (deep) {
      clone.children.push(...this.children.map((child) => child.cloneNode(true)));
    }
    return clone;
  }

  replaceChildren(...children: FakeElement[]): void {
    for (const child of this.children) {
      child.parentElement = null;
    }
    for (const child of children) {
      child.parentElement = this;
    }
    this.children.splice(0, this.children.length, ...children);
    this.html = "";
  }

  querySelector(selector: string): FakeElement | null {
    if (selector === this.tagName || this.matchesClassSelector(selector)) {
      return this;
    }
    for (const child of this.children) {
      const match = child.querySelector(selector);
      if (match) {
        return match;
      }
    }
    return null;
  }

  closest(selector: string): FakeElement | null {
    let current: FakeElement | null = this;
    while (current) {
      if (selector === "[data-action]" && current.dataset.action !== undefined) {
        return current;
      }
      for (const part of selector.split(",").map((item) => item.trim())) {
        if (current.matchesClassSelector(part)) {
          return current;
        }
      }
      current = current.parentElement;
    }
    return null;
  }

  contains(target: FakeElement): boolean {
    return this === target || this.children.some((child) => child.contains(target));
  }

  private matchesClassSelector(selector: string): boolean {
    return selector.startsWith(".") && this.className.split(/\s+/).includes(selector.slice(1));
  }

  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }

  getAttribute(name: string): string | null {
    return this.attributes.get(name) ?? null;
  }

  hasAttribute(name: string): boolean {
    return this.attributes.has(name);
  }

  get innerHTML(): string {
    return this.html;
  }

  set innerHTML(value: string) {
    this.html = value;
    this.children.splice(0, this.children.length);
    const svg = parseSvg(value);
    if (svg) {
      svg.parentElement = this;
      this.children.push(svg);
    }
  }

  get outerHTML(): string {
    const attrs = [...this.attributes.entries()]
      .map(([name, value]) => ` ${name}="${value}"`)
      .join("");
    const body = this.html || this.children.map((child) => child.outerHTML).join("");
    return `<${this.tagName}${attrs}>${body}</${this.tagName}>`;
  }

  get offsetWidth(): number {
    const svg = this.querySelector("svg");
    if (svg) {
      return Number.parseFloat(svg.getAttribute("width") ?? "0") || 1;
    }
    return this.clientWidth;
  }

  get offsetHeight(): number {
    const svg = this.querySelector("svg");
    if (svg) {
      return Number.parseFloat(svg.getAttribute("height") ?? "0") || 1;
    }
    return this.clientHeight;
  }

  getBoundingClientRect(): { left: number; top: number; width: number; height: number } {
    return {
      left: 0,
      top: 0,
      width: this.clientWidth,
      height: this.clientHeight,
    };
  }

  addEventListener(type: string, listener: (event: unknown) => void): void {
    const listeners = this.listeners.get(type) ?? [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }

  dispatch(type: string, event: unknown): void {
    for (const listener of this.listeners.get(type) ?? []) {
      listener(event);
    }
  }

  setPointerCapture(): void {}

  hasPointerCapture(): boolean {
    return false;
  }

  releasePointerCapture(): void {}

  textContentTree(): string {
    return [this.textContent, ...this.children.map((child) => child.textContentTree())].join("");
  }
}

class FakeSelectElement extends FakeElement {
  constructor(options: ConstructorParameters<typeof FakeElement>[1] = {}) {
    super("select", options);
  }
}

class FakeButtonElement extends FakeElement {
  constructor(options: ConstructorParameters<typeof FakeElement>[1] = {}) {
    super("button", options);
  }
}

class FakeStyleDeclaration {
  private readonly properties = new Map<string, string>();

  setProperty(name: string, value: string): void {
    this.properties.set(name, value);
  }
}

class FakeClassList {
  private readonly names = new Set<string>();

  add(name: string): void {
    this.names.add(name);
  }

  remove(name: string): void {
    this.names.delete(name);
  }

  toggle(name: string, force?: boolean): boolean {
    const shouldHave = force ?? !this.names.has(name);
    if (shouldHave) {
      this.names.add(name);
    } else {
      this.names.delete(name);
    }
    return shouldHave;
  }

  contains(name: string): boolean {
    return this.names.has(name);
  }
}

function parseSvg(value: string): FakeElement | undefined {
  if (!/<svg\b/i.test(value)) {
    return undefined;
  }
  const svg = new FakeElement("svg");
  for (const match of value.matchAll(/([A-Za-z_:][-A-Za-z0-9_:.]*)="([^"]*)"/g)) {
    svg.setAttribute(match[1] ?? "", match[2] ?? "");
  }
  return svg;
}
