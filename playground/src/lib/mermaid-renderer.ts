import { sourceWithTheme } from "@/src/lib/wasm-loader";

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
  theme: string;
  securityLevel: "strict";
}

let mermaidPromise: Promise<MermaidApi> | null = null;
let renderSerial = 0;

export async function renderMermaidSvg(
  source: string,
  theme: string
): Promise<MermaidRenderResult> {
  const startTime = performance.now();

  try {
    const mermaid = await loadMermaid();
    mermaid.initialize({
      startOnLoad: false,
      theme: normalizeTheme(theme),
      securityLevel: "strict",
    });

    const id = `mermaid-compare-${++renderSerial}`;
    const result = await mermaid.render(id, sourceWithTheme(source, theme));
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

async function loadMermaid(): Promise<MermaidApi> {
  if (mermaidPromise) {
    return mermaidPromise;
  }

  mermaidPromise = import("mermaid")
    .then((module) => module.default as MermaidApi)
    .catch((error) => {
      mermaidPromise = null;
      throw error;
    });
  return mermaidPromise;
}

function normalizeTheme(theme: string): string {
  if (theme === "dark" || theme === "forest" || theme === "neutral") {
    return theme;
  }
  return "default";
}
