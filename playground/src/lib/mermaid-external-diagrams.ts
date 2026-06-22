import { MERMAID_CDN_LOAD_ERROR, type MermaidApi } from "@/src/lib/mermaid-runtime";

export const MERMAID_ZENUML_VERSION = "0.2.2";
export const MERMAID_LAYOUT_ELK_VERSION = "0.2.1";

const MERMAID_ZENUML_CDN_URL =
  import.meta.env.VITE_MERMAID_ZENUML_CDN_URL?.trim() ||
  `https://cdn.jsdelivr.net/npm/@mermaid-js/mermaid-zenuml@${MERMAID_ZENUML_VERSION}/dist/mermaid-zenuml.core.mjs`;
const MERMAID_ZENUML_FALLBACK_CDN_URL =
  import.meta.env.VITE_MERMAID_ZENUML_FALLBACK_CDN_URL?.trim() ||
  `https://unpkg.com/@mermaid-js/mermaid-zenuml@${MERMAID_ZENUML_VERSION}/dist/mermaid-zenuml.core.mjs`;
const MERMAID_LAYOUT_ELK_CDN_URL =
  import.meta.env.VITE_MERMAID_LAYOUT_ELK_CDN_URL?.trim() ||
  `https://cdn.jsdelivr.net/npm/@mermaid-js/layout-elk@${MERMAID_LAYOUT_ELK_VERSION}/dist/mermaid-layout-elk.esm.min.mjs`;
const MERMAID_LAYOUT_ELK_FALLBACK_CDN_URL =
  import.meta.env.VITE_MERMAID_LAYOUT_ELK_FALLBACK_CDN_URL?.trim() ||
  `https://unpkg.com/@mermaid-js/layout-elk@${MERMAID_LAYOUT_ELK_VERSION}/dist/mermaid-layout-elk.esm.min.mjs`;
const MERMAID_EXTERNAL_DIAGRAM_LOAD_ERROR =
  /^Failed to load \d+ external diagrams$/;

let zenumlPromise: Promise<unknown> | null = null;
let zenumlRegisteredMermaid: MermaidApi | null = null;
let elkLayoutsPromise: Promise<unknown[]> | null = null;
let elkLayoutsRegisteredMermaid: MermaidApi | null = null;

export async function ensureMermaidExternalDiagrams(
  mermaid: MermaidApi,
  options: {
    elkLayouts?: boolean;
    zenuml?: boolean;
  }
): Promise<void> {
  if (options.elkLayouts) {
    await ensureElkLayoutsRegistered(mermaid);
  }
  if (options.zenuml) {
    await ensureZenUmlRegistered(mermaid);
  }
}

export async function refreshZenUmlRegistration(
  mermaid: MermaidApi
): Promise<void> {
  zenumlRegisteredMermaid = null;
  await ensureZenUmlRegistered(mermaid);
}

export function isExternalDiagramLoadError(error: unknown): boolean {
  return (
    error instanceof Error &&
    MERMAID_EXTERNAL_DIAGRAM_LOAD_ERROR.test(error.message)
  );
}

async function ensureZenUmlRegistered(mermaid: MermaidApi): Promise<void> {
  if (zenumlRegisteredMermaid === mermaid) return;
  if (typeof mermaid.registerExternalDiagrams !== "function") {
    throw new Error("Loaded Mermaid runtime does not support external diagrams.");
  }

  ensureZenUmlBrowserGlobals();
  const zenuml = await loadZenUmlDiagram();
  await mermaid.registerExternalDiagrams([zenuml], { lazyLoad: true });
  zenumlRegisteredMermaid = mermaid;
}

async function loadZenUmlDiagram(): Promise<unknown> {
  if (zenumlPromise) return zenumlPromise;

  zenumlPromise = loadZenUmlModule().catch((error) => {
    zenumlPromise = null;
    throw error;
  });
  return zenumlPromise;
}

async function loadZenUmlModule(): Promise<unknown> {
  ensureZenUmlBrowserGlobals();
  const urls = Array.from(
    new Set([MERMAID_ZENUML_CDN_URL, MERMAID_ZENUML_FALLBACK_CDN_URL])
  );
  let lastError: unknown = null;

  for (const url of urls) {
    try {
      const module = await import(/* @vite-ignore */ url);
      return module.default;
    } catch (error) {
      lastError = error;
    }
  }

  throw new Error(
    `${MERMAID_CDN_LOAD_ERROR}: ${
      lastError instanceof Error ? lastError.message : String(lastError)
    }`
  );
}

async function ensureElkLayoutsRegistered(mermaid: MermaidApi): Promise<void> {
  if (elkLayoutsRegisteredMermaid === mermaid) return;
  if (typeof mermaid.registerLayoutLoaders !== "function") {
    throw new Error("Loaded Mermaid runtime does not support layout loaders.");
  }

  const elkLayouts = await loadElkLayouts();
  mermaid.registerLayoutLoaders(elkLayouts);
  elkLayoutsRegisteredMermaid = mermaid;
}

async function loadElkLayouts(): Promise<unknown[]> {
  if (elkLayoutsPromise) return elkLayoutsPromise;

  elkLayoutsPromise = loadElkLayoutsModule().catch((error) => {
    elkLayoutsPromise = null;
    throw error;
  });
  return elkLayoutsPromise;
}

async function loadElkLayoutsModule(): Promise<unknown[]> {
  const urls = Array.from(
    new Set([MERMAID_LAYOUT_ELK_CDN_URL, MERMAID_LAYOUT_ELK_FALLBACK_CDN_URL])
  );
  let lastError: unknown = null;

  for (const url of urls) {
    try {
      const module = await import(/* @vite-ignore */ url);
      const layouts = module.default as unknown;
      if (!Array.isArray(layouts)) {
        throw new Error("Loaded Mermaid ELK layout module did not export loaders.");
      }
      return layouts;
    } catch (error) {
      lastError = error;
    }
  }

  throw new Error(
    `${MERMAID_CDN_LOAD_ERROR}: ${
      lastError instanceof Error ? lastError.message : String(lastError)
    }`
  );
}

function ensureZenUmlBrowserGlobals(): void {
  ensureZenUmlLocalStorage();
  ensureZenUmlOrigin();
  ensureZenUmlIntersectionObserver();
  ensureZenUmlCssStyleSheet();
}

function ensureZenUmlLocalStorage(): void {
  const globalObject = globalThis as typeof globalThis & {
    localStorage?: Storage;
  };

  if (readUsableLocalStorage(globalObject)) {
    return;
  }

  try {
    Object.defineProperty(globalObject, "localStorage", {
      value: createMemoryStorage(),
      configurable: true,
    });
  } catch {
    // If the host exposes a non-configurable broken localStorage, let ZenUML
    // surface its native error instead of masking it with another failure.
  }
}

function readUsableLocalStorage(
  globalObject: typeof globalThis & { localStorage?: Storage }
): Storage | null {
  try {
    const storage = globalObject.localStorage;
    return typeof storage?.getItem === "function" ? storage : null;
  } catch {
    return null;
  }
}

function createMemoryStorage(): Storage {
  const entries = new Map<string, string>();

  return {
    get length() {
      return entries.size;
    },
    clear() {
      entries.clear();
    },
    getItem(key: string) {
      return entries.get(String(key)) ?? null;
    },
    key(index: number) {
      return Array.from(entries.keys())[index] ?? null;
    },
    removeItem(key: string) {
      entries.delete(String(key));
    },
    setItem(key: string, value: string) {
      entries.set(String(key), String(value));
    },
  };
}

function ensureZenUmlOrigin(): void {
  const globalObject = globalThis as typeof globalThis & {
    origin?: string;
    location?: Location;
  };

  try {
    if (typeof globalObject.origin === "string") {
      return;
    }
  } catch {
    // Fall through to a best-effort origin shim.
  }

  try {
    const locationOrigin = globalObject.location?.origin;
    Object.defineProperty(globalObject, "origin", {
      value:
        typeof locationOrigin === "string" ? locationOrigin : "http://localhost",
      configurable: true,
    });
  } catch {
    // ZenUML can still run in normal browser contexts where origin exists.
  }
}

function ensureZenUmlIntersectionObserver(): void {
  const globalObject = globalThis as typeof globalThis & {
    IntersectionObserver?: typeof IntersectionObserver;
  };

  if (typeof globalObject.IntersectionObserver !== "undefined") {
    return;
  }

  class NoopIntersectionObserver implements IntersectionObserver {
    readonly root: Element | Document | null = null;
    readonly rootMargin = "0px";
    readonly thresholds: ReadonlyArray<number> = [];

    constructor(_callback: unknown) {}

    disconnect(): void {}
    observe(_target: Element): void {}
    takeRecords() {
      return [];
    }
    unobserve(_target: Element): void {}
  }

  try {
    Object.defineProperty(globalObject, "IntersectionObserver", {
      value: NoopIntersectionObserver,
      configurable: true,
    });
  } catch {
    // Ignore hosts that do not allow patching globals.
  }
}

function ensureZenUmlCssStyleSheet(): void {
  const globalObject = globalThis as typeof globalThis & {
    CSSStyleSheet?: typeof CSSStyleSheet;
  };

  if (typeof globalObject.CSSStyleSheet !== "undefined") {
    return;
  }

  class NoopCssStyleSheet {
    replace(_text: string): Promise<NoopCssStyleSheet> {
      return Promise.resolve(this);
    }

    replaceSync(_text: string): void {}

    insertRule(_rule: string, index = 0): number {
      return index;
    }

    deleteRule(_index: number): void {}
  }

  try {
    Object.defineProperty(globalObject, "CSSStyleSheet", {
      value: NoopCssStyleSheet,
      configurable: true,
    });
  } catch {
    // Ignore hosts that do not allow patching globals.
  }
}
