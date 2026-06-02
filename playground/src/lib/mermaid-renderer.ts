import {
  DEFAULT_MERMAID_CONFIG,
  buildMermaidConfig,
  sourceWithConfig,
} from "@/src/lib/mermaid-config";
import { normalizeThemeName } from "@merman/web";

export const MERMAID_JS_VERSION = "11.15.0";

export interface MermaidRenderResult {
  svg: string | null;
  error: string | null;
  renderTime: number;
}

interface MermaidApi {
  initialize(config: MermaidConfig): void;
  render(id: string, source: string): Promise<{ svg: string }> | { svg: string };
}

interface MermaidConfig {
  startOnLoad: boolean;
  securityLevel?: "strict" | "loose" | "antiscript" | "sandbox";
  theme?: string;
  [key: string]: unknown;
}

let mermaidPromise: Promise<MermaidApi> | null = null;
let mermaidLoaded = false;
let renderSerial = 0;
let initializedConfigSignature: string | null = null;
let warmupConfigSignature: string | null = null;
let warmupPromise: Promise<void> | null = null;

const WARMUP_SOURCE = "flowchart TD\n  warmupA[Warmup] --> warmupB[Ready]";

export async function renderMermaidSvg(
  source: string,
  theme: string,
  configJson = DEFAULT_MERMAID_CONFIG
): Promise<MermaidRenderResult> {
  try {
    const prepared = await prepareMermaid(theme, configJson, { warmup: true });
    const preparedSource = sourceWithConfig(
      source,
      prepared.normalizedTheme,
      configJson
    );
    const startTime = performance.now();

    const id = `mermaid-compare-${++renderSerial}`;
    const result = await prepared.mermaid.render(id, preparedSource);
    return {
      svg: result.svg,
      error: null,
      renderTime: performance.now() - startTime,
    };
  } catch (error) {
    return {
      svg: null,
      error: error instanceof Error ? error.message : String(error),
      renderTime: 0,
    };
  }
}

export async function preloadMermaid(): Promise<void> {
  await loadMermaid().catch(() => undefined);
}

export async function prewarmMermaidRenderer(
  theme: string,
  configJson = DEFAULT_MERMAID_CONFIG
): Promise<void> {
  await prepareMermaid(theme, configJson, { warmup: true }).catch(() => undefined);
}

export function isMermaidLoaded(): boolean {
  return mermaidLoaded;
}

async function loadMermaid(): Promise<MermaidApi> {
  if (mermaidPromise) {
    return mermaidPromise;
  }

  mermaidPromise = import("mermaid")
    .then((module) => {
      mermaidLoaded = true;
      return module.default as MermaidApi;
    })
    .catch((error) => {
      mermaidPromise = null;
      mermaidLoaded = false;
      throw error;
    });
  return mermaidPromise;
}

async function prepareMermaid(
  theme: string,
  configJson: string,
  options: { warmup: boolean }
): Promise<{
  mermaid: MermaidApi;
  normalizedTheme: string;
  configSignature: string;
}> {
  const mermaid = await loadMermaid();
  const normalizedTheme = normalizeThemeName(theme);
  const effectiveConfig = buildMermaidConfig(configJson, normalizedTheme);
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
    await warmupMermaid(mermaid, normalizedTheme, configJson, configSignature);
  }

  return { mermaid, normalizedTheme, configSignature };
}

async function warmupMermaid(
  mermaid: MermaidApi,
  normalizedTheme: string,
  configJson: string,
  configSignature: string
): Promise<void> {
  if (warmupConfigSignature === configSignature) return;
  if (warmupPromise) {
    await warmupPromise;
    if (warmupConfigSignature === configSignature) return;
  }

  warmupPromise = Promise.resolve(
    mermaid.render(
      `mermaid-warmup-${++renderSerial}`,
      sourceWithConfig(WARMUP_SOURCE, normalizedTheme, configJson)
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
