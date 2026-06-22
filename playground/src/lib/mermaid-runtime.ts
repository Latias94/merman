import { buildMermaidConfig, sourceWithConfig } from "@/src/lib/mermaid-config";
import type { DiagramFont } from "@/src/lib/diagram-font";
import { normalizeThemeName } from "@mermanjs/web";

export const MERMAID_JS_VERSION = "11.15.0";
export const MERMAID_CDN_URL =
  import.meta.env.VITE_MERMAID_CDN_URL?.trim() ||
  `https://cdn.jsdelivr.net/npm/mermaid@${MERMAID_JS_VERSION}/dist/mermaid.esm.min.mjs`;
export const MERMAID_CDN_LOAD_ERROR = "__mermaid_cdn_load_failed__";
export const MERMAID_WARMUP_SOURCE =
  "flowchart TD\n  warmupA[Warmup] --> warmupB[Ready]";

const MERMAID_FALLBACK_CDN_URL =
  import.meta.env.VITE_MERMAID_FALLBACK_CDN_URL?.trim() ||
  `https://unpkg.com/mermaid@${MERMAID_JS_VERSION}/dist/mermaid.esm.min.mjs`;
const CDN_ENABLED = import.meta.env.VITE_MERMAID_CDN !== "false";

export interface MermaidApi {
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

export interface MermaidSession {
  mermaid: MermaidApi;
  normalizedTheme: string;
  configSignature: string;
}

let mermaidPromise: Promise<MermaidApi> | null = null;
let mermaidLoaded = false;
let mermaidLoadSource: "cdn" | null = null;
let renderSerial = 0;
let initializedConfigSignature: string | null = null;
let warmupConfigSignature: string | null = null;
let warmupPromise: Promise<void> | null = null;

export async function loadMermaid(): Promise<MermaidApi> {
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

export async function prepareMermaidSession(
  theme: string,
  configJson: string,
  options: {
    warmup: boolean;
    diagramFont?: DiagramFont;
  }
): Promise<MermaidSession> {
  const mermaid = await loadMermaid();
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

export function nextMermaidRenderId(prefix = "mermaid-compare"): string {
  return `${prefix}-${++renderSerial}`;
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
      nextMermaidRenderId("mermaid-warmup"),
      sourceWithConfig(MERMAID_WARMUP_SOURCE, normalizedTheme, configJson, {
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
