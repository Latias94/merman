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
      svg: '<svg viewBox="0 0 100 50"></svg>',
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
      svg: '<svg viewBox="0 0 100 50"></svg>',
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

  it("keeps the previous SVG visible when the current render fails", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      svg: '<svg viewBox="0 0 100 50"></svg>',
    });
    const initialSvg = app.document.canvas.querySelector("svg");

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

    assert.equal(app.document.canvas.querySelector("svg"), initialSvg);
    assert.equal(app.document.status.hidden, false);
    assert.equal(app.document.status.textContent, "syntax issue");
    assert.equal(app.document.status.dataset.kind, "error");
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
    assert.equal(app.document.title.textContent, "notes.md");
    assert.equal(app.document.status.hidden, false);
    assert.equal(app.document.status.textContent, "Rendering preview: Mermaid fence 1");
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
  });

  it("ignores stale render success messages after a newer render starts", () => {
    const app = loadPreviewApp();

    app.dispatch({
      type: "renderSucceeded",
      requestId: 1,
      snapshot: snapshot({ sourceHash: "hash-a" }),
      svg: '<svg viewBox="0 0 100 50"></svg>',
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
      svg: '<svg viewBox="0 0 300 150"></svg>',
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
      svg: '<svg viewBox="0 0 100 50"></svg>',
    });
    const initialSvg = app.document.canvas.querySelector("svg");

    app.dispatch({
      type: "diagnosticsUpdated",
      snapshot: snapshot({
        sourceHash: "hash-a",
        diagnostics: {
          summary: "1 errors, 0 warnings, 0 infos, 0 hints",
          visibleCount: 1,
          totalCount: 1,
          items: [
            {
              severityLabel: "Error",
              severityKey: "error",
              line: 2,
              column: 1,
              target: {
                uri: "file:///workspace/notes.md",
                startLine: 1,
                startCharacter: 0,
                endLine: 1,
                endCharacter: 1,
              },
              message: "syntax issue",
              hasQuickFixes: true,
            },
          ],
        },
      }),
    });

    assert.equal(app.document.canvas.querySelector("svg"), initialSvg);
    assert.equal(app.document.diagnostics.hidden, false);
    assert.match(app.document.diagnostics.textContentTree(), /1 errors/);
    assert.match(app.document.diagnostics.textContentTree(), /syntax issue/);
  });
});

interface PreviewAppHarness {
  document: FakeDocument;
  dispatch(message: unknown): void;
  readonly persistedState: Record<string, unknown>;
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
    get persistedState(): Record<string, unknown> {
      return persistedState;
    },
  };
}

function snapshot(options: {
  documentUri?: string;
  sourceId?: string;
  sourceHash?: string;
  diagramTheme?: string;
  diagnostics?: unknown;
}): Record<string, unknown> {
  const documentUri = options.documentUri ?? "file:///workspace/notes.md";
  const sourceId = options.sourceId ?? "fence-1";
  const sourceHash = options.sourceHash ?? "hash-a";
  const diagramTheme = options.diagramTheme ?? "source";
  return {
    documentUri,
    sourceId,
    title: "notes.md",
    subtitle: "Mermaid fence 1",
    selectionLine: 1,
    pinned: false,
    diagramTheme,
    sourceKey: {
      documentUri,
      sourceId,
      sourceHash,
      diagramTheme,
    },
    sources: [
      {
        sourceId: "fence-1",
        title: "notes.md",
        subtitle: "Mermaid fence 1",
        kind: "markdown-fence",
      },
    ],
    diagnostics: options.diagnostics,
  };
}

function previewSourceIdentity(documentUri: string, sourceId: string, sourceHash: string): string {
  return [documentUri, sourceId, sourceHash].join("\u0000");
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
  readonly title = new FakeElement("span", { dataset: { previewTitle: "" } });
  readonly subtitle = new FakeElement("span", { dataset: { previewSubtitle: "" } });
  readonly diagnostics = new FakeElement("section", { dataset: { previewDiagnostics: "" } });
  readonly status = new FakeElement("p", { dataset: { previewStatus: "" } });
  readonly empty = new FakeElement("div", { dataset: { previewEmpty: "" } });
  readonly sourceList = new FakeSelectElement({ dataset: { previewSourceList: "", action: "source" } });
  readonly theme = new FakeSelectElement({ dataset: { action: "diagram-theme" } });
  readonly background = new FakeSelectElement({ dataset: { action: "background" } });
  readonly pin = new FakeButtonElement({ dataset: { action: "pin" } });
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
      case "[data-preview-title]":
        return this.title;
      case "[data-preview-subtitle]":
        return this.subtitle;
      case "[data-preview-diagnostics]":
        return this.diagnostics;
      case "[data-preview-status]":
        return this.status;
      case "[data-preview-empty]":
        return this.empty;
      case "[data-preview-source-list]":
        return this.sourceList;
      case '[data-action="diagram-theme"]':
        return this.theme;
      case '[data-action="background"]':
        return this.background;
      case '[data-action="pin"]':
        return this.pin;
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
}

class FakeElement {
  readonly tagName: string;
  readonly dataset: Record<string, string>;
  readonly style = new FakeStyleDeclaration();
  readonly classList = new FakeClassList();
  readonly children: FakeElement[] = [];
  hidden = false;
  className = "";
  textContent = "";
  title = "";
  type = "";
  value = "";
  selected = false;
  clientWidth = 1;
  clientHeight = 1;
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
    this.children.push(child);
    return child;
  }

  replaceChildren(...children: FakeElement[]): void {
    this.children.splice(0, this.children.length, ...children);
    this.html = "";
  }

  querySelector(selector: string): FakeElement | null {
    if (selector === this.tagName) {
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
    if (selector === "[data-action]" && this.dataset.action !== undefined) {
      return this;
    }
    return null;
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
      this.children.push(svg);
    }
  }

  get outerHTML(): string {
    const attrs = [...this.attributes.entries()]
      .map(([name, value]) => ` ${name}="${value}"`)
      .join("");
    return `<${this.tagName}${attrs}>${this.html}</${this.tagName}>`;
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
