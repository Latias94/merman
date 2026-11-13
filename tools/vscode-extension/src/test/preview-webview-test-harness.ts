import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import * as vm from "node:vm";

export interface PreviewAppHarness {
  document: FakeDocument;
  dispatch(message: unknown): void;
  click(target: FakeElement): void;
  dispatchDocument(type: string, event: unknown): void;
  dispatchViewport(type: string, event: unknown): void;
  readonly persistedState: Record<string, unknown>;
  readonly postedMessages: readonly unknown[];
}

export function loadPreviewApp(initialState: Record<string, unknown> = {}): PreviewAppHarness {
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

export class FakeElement {
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
  shadowRoot: FakeElement | null = null;
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

  attachShadow(_init: { mode: "open" }): FakeElement {
    this.shadowRoot = new FakeElement("shadow-root");
    return this.shadowRoot;
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
    const svg = this.queryShadowTree("svg") ?? this.querySelector("svg");
    if (svg) {
      return Number.parseFloat(svg.getAttribute("width") ?? "0") || 1;
    }
    return this.clientWidth;
  }

  get offsetHeight(): number {
    const svg = this.queryShadowTree("svg") ?? this.querySelector("svg");
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

  private queryShadowTree(selector: string): FakeElement | null {
    const ownShadowMatch = this.shadowRoot?.querySelector(selector);
    if (ownShadowMatch) {
      return ownShadowMatch;
    }
    for (const child of this.children) {
      const match = child.queryShadowTree(selector);
      if (match) {
        return match;
      }
    }
    return null;
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

export class FakeButtonElement extends FakeElement {
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
