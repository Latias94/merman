import {
  DEFAULT_MERMAID_CONFIG,
  buildMermaidConfig,
  sourceWithConfig,
} from "@/src/lib/mermaid-config";
import type { DiagramFont } from "@/src/lib/diagram-font";
import { normalizeThemeName } from "@mermanjs/web";

export const MERMAID_JS_VERSION = "11.15.0";
export const MERMAID_ZENUML_VERSION = "0.2.2";
export const MERMAID_LAYOUT_ELK_VERSION = "0.2.1";
export const MERMAID_CDN_URL =
  import.meta.env.VITE_MERMAID_CDN_URL?.trim() ||
  `https://cdn.jsdelivr.net/npm/mermaid@${MERMAID_JS_VERSION}/dist/mermaid.esm.min.mjs`;
export const MERMAID_CDN_LOAD_ERROR = "__mermaid_cdn_load_failed__";
const MERMAID_FALLBACK_CDN_URL =
  import.meta.env.VITE_MERMAID_FALLBACK_CDN_URL?.trim() ||
  `https://unpkg.com/mermaid@${MERMAID_JS_VERSION}/dist/mermaid.esm.min.mjs`;
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

export interface MermaidRenderResult {
  svg: string | null;
  error: string | null;
  prepareTime: number;
  renderTime: number;
}

interface MermaidApi {
  initialize(config: MermaidConfig): void;
  render(id: string, source: string): Promise<{ svg: string }> | { svg: string };
  registerExternalDiagrams?(
    diagrams: unknown[],
    options?: { lazyLoad?: boolean }
  ): Promise<void>;
  registerLayoutLoaders?(loaders: unknown[]): void;
}

interface MermaidConfig {
  startOnLoad: boolean;
  securityLevel?: "strict" | "loose" | "antiscript" | "sandbox";
  theme?: string;
  [key: string]: unknown;
}

let mermaidPromise: Promise<MermaidApi> | null = null;
let mermaidLoaded = false;
let mermaidLoadSource: "cdn" | null = null;
let renderSerial = 0;
let initializedConfigSignature: string | null = null;
let warmupConfigSignature: string | null = null;
let warmupPromise: Promise<void> | null = null;
let zenumlPromise: Promise<unknown> | null = null;
let zenumlRegisteredMermaid: MermaidApi | null = null;
let elkLayoutsPromise: Promise<unknown[]> | null = null;
let elkLayoutsRegisteredMermaid: MermaidApi | null = null;

const WARMUP_SOURCE = "flowchart TD\n  warmupA[Warmup] --> warmupB[Ready]";
const CDN_ENABLED = import.meta.env.VITE_MERMAID_CDN !== "false";

export async function renderMermaidSvg(
  source: string,
  theme: string,
  configJson = DEFAULT_MERMAID_CONFIG,
  options: { diagramFont?: DiagramFont } = {}
): Promise<MermaidRenderResult> {
  const prepareStartTime = performance.now();

  try {
    const prepared = await prepareMermaid(theme, configJson, {
      warmup: true,
      elkLayouts: needsElkLayouts(source, configJson),
      zenuml: isZenUmlSource(source),
      diagramFont: options.diagramFont,
    });
    const prepareTime = performance.now() - prepareStartTime;
    const preparedSource = sourceWithConfig(
      source,
      prepared.normalizedTheme,
      configJson,
      { diagramFont: options.diagramFont }
    );
    const startTime = performance.now();

    const id = `mermaid-compare-${++renderSerial}`;
    const result = await prepared.mermaid.render(id, preparedSource);
    return {
      svg: result.svg,
      error: null,
      prepareTime,
      renderTime: performance.now() - startTime,
    };
  } catch (error) {
    return {
      svg: null,
      error: error instanceof Error ? error.message : String(error),
      prepareTime: performance.now() - prepareStartTime,
      renderTime: 0,
    };
  }
}

export async function preloadMermaid(): Promise<void> {
  await loadMermaid().catch(() => undefined);
}

export async function prewarmMermaidRenderer(
  theme: string,
  configJson = DEFAULT_MERMAID_CONFIG,
  options: { diagramFont?: DiagramFont } = {}
): Promise<void> {
  await prepareMermaid(theme, configJson, {
    warmup: true,
    elkLayouts: needsElkLayouts(WARMUP_SOURCE, configJson),
    diagramFont: options.diagramFont,
  }).catch(() => undefined);
}

export function isMermaidLoaded(): boolean {
  return mermaidLoaded;
}

export function getMermaidLoadSource(): "cdn" | null {
  return mermaidLoadSource;
}

export function mermaidRuntimeErrorI18nKey(message: string | null | undefined) {
  if (message?.startsWith(MERMAID_CDN_LOAD_ERROR)) {
    return "preview.mermaidCdnLoadFailed";
  }
  return null;
}

async function loadMermaid(): Promise<MermaidApi> {
  if (mermaidPromise) {
    return mermaidPromise;
  }

  mermaidPromise = loadMermaidModule()
    .then((mermaid) => {
      mermaidLoaded = true;
      return mermaid;
    })
    .catch((error) => {
      mermaidPromise = null;
      mermaidLoaded = false;
      mermaidLoadSource = null;
      throw error;
    });
  return mermaidPromise;
}

async function loadMermaidModule(): Promise<MermaidApi> {
  if (!CDN_ENABLED) {
    throw new Error("Mermaid CDN loading is disabled.");
  }

  const urls = Array.from(new Set([MERMAID_CDN_URL, MERMAID_FALLBACK_CDN_URL]));
  let lastError: unknown = null;

  for (const url of urls) {
    try {
      const module = await import(/* @vite-ignore */ url);
      mermaidLoadSource = "cdn";
      return module.default as MermaidApi;
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

async function prepareMermaid(
  theme: string,
  configJson: string,
  options: {
    warmup: boolean;
    elkLayouts?: boolean;
    zenuml?: boolean;
    diagramFont?: DiagramFont;
  }
): Promise<{
  mermaid: MermaidApi;
  normalizedTheme: string;
  configSignature: string;
}> {
  const mermaid = await loadMermaid();
  if (options.elkLayouts) {
    await ensureElkLayoutsRegistered(mermaid);
  }
  if (options.zenuml) {
    await ensureZenUmlRegistered(mermaid);
  }
  const normalizedTheme = normalizeThemeName(theme);
  const effectiveConfig = buildMermaidConfig(configJson, normalizedTheme, {
    diagramFont: options.diagramFont,
  });
  const runtimeConfig: MermaidConfig = {
    ...effectiveConfig,
    startOnLoad: false,
    securityLevel:
      (effectiveConfig.securityLevel as MermaidConfig["securityLevel"]) ??
      "strict",
  };
  const configSignature = stableConfigSignature(runtimeConfig);

  if (initializedConfigSignature !== configSignature) {
    mermaid.initialize(runtimeConfig);
    initializedConfigSignature = configSignature;
    warmupConfigSignature = null;
    warmupPromise = null;
  }

  if (options.warmup) {
    await warmupMermaid(
      mermaid,
      normalizedTheme,
      configJson,
      configSignature,
      options.diagramFont
    );
  }

  return { mermaid, normalizedTheme, configSignature };
}

async function ensureZenUmlRegistered(mermaid: MermaidApi): Promise<void> {
  if (zenumlRegisteredMermaid === mermaid) return;
  if (typeof mermaid.registerExternalDiagrams !== "function") {
    throw new Error("Loaded Mermaid runtime does not support external diagrams.");
  }

  const zenuml = await loadZenUmlDiagram();
  // Keep ZenUML lazy-loaded; eager loading can fail during compare preparation
  // before Mermaid reaches the actual zenuml render path.
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

function isZenUmlSource(source: string): boolean {
  return /^\s*zenuml\b/i.test(source);
}

function needsElkLayouts(source: string, configJson: string): boolean {
  if (/^\s*flowchart-elk\b/i.test(source)) {
    return true;
  }
  if (sourceRequestsElkLayout(source)) {
    return true;
  }

  try {
    const config = buildMermaidConfig(configJson, "default");
    return (
      config.layout === "elk" ||
      (typeof config.layout === "string" && config.layout.startsWith("elk.")) ||
      getNestedString(config, ["flowchart", "defaultRenderer"]) === "elk"
    );
  } catch {
    return false;
  }
}

function sourceRequestsElkLayout(source: string): boolean {
  return (
    /(?:^|\n)\s*layout\s*:\s*["']?elk(?:\.[\w-]+)?["']?\s*(?:\n|$)/i.test(
      source
    ) ||
    /["']layout["']\s*:\s*["']elk(?:\.[\w-]+)?["']/i.test(source) ||
    /["']defaultRenderer["']\s*:\s*["']elk["']/i.test(source) ||
    /(?:^|\n)\s*defaultRenderer\s*:\s*["']?elk["']?\s*(?:\n|$)/i.test(source)
  );
}

function getNestedString(
  value: Record<string, unknown>,
  path: string[]
): string | null {
  let current: unknown = value;
  for (const key of path) {
    if (!current || typeof current !== "object" || Array.isArray(current)) {
      return null;
    }
    current = (current as Record<string, unknown>)[key];
  }
  return typeof current === "string" ? current : null;
}

async function warmupMermaid(
  mermaid: MermaidApi,
  normalizedTheme: string,
  configJson: string,
  configSignature: string,
  diagramFont: DiagramFont | undefined
): Promise<void> {
  if (warmupConfigSignature === configSignature) return;
  if (warmupPromise) {
    await warmupPromise;
    if (warmupConfigSignature === configSignature) return;
  }

  warmupPromise = Promise.resolve(
    mermaid.render(
      `mermaid-warmup-${++renderSerial}`,
      sourceWithConfig(WARMUP_SOURCE, normalizedTheme, configJson, {
        diagramFont,
      })
    )
  )
    .then(() => {
      if (initializedConfigSignature === configSignature) {
        warmupConfigSignature = configSignature;
      }
    })
    .finally(() => {
      warmupPromise = null;
    });

  await warmupPromise;
}

function stableConfigSignature(value: unknown): string {
  return JSON.stringify(sortJsonValue(value));
}

function sortJsonValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortJsonValue);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entry]) => [key, sortJsonValue(entry)])
    );
  }
  return value;
}
