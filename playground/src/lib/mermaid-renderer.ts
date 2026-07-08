import type { DiagramFont } from "@/src/lib/diagram-font";
import { assertSafeSvgForDom } from "@mermanjs/web";
import {
  ensureMermaidExternalDiagrams,
  isExternalDiagramLoadError,
  refreshZenUmlRegistration,
} from "@/src/lib/mermaid-external-diagrams";
import {
  loadMermaid,
  MERMAID_WARMUP_SOURCE,
  nextMermaidRenderId,
  prepareMermaidSession,
  type MermaidApi,
} from "@/src/lib/mermaid-runtime";
import {
  DEFAULT_MERMAID_CONFIG,
  buildMermaidConfig,
  sourceWithConfig,
} from "@/src/lib/mermaid-config";

export {
  MERMAID_LAYOUT_ELK_VERSION,
  MERMAID_ZENUML_VERSION,
} from "@/src/lib/mermaid-external-diagrams";
export {
  getMermaidLoadSource,
  isMermaidLoaded,
  MERMAID_CDN_LOAD_ERROR,
  MERMAID_CDN_URL,
  MERMAID_JS_VERSION,
  mermaidRuntimeErrorI18nKey,
} from "@/src/lib/mermaid-runtime";

export interface MermaidRenderResult {
  svg: string | null;
  error: string | null;
  prepareTime: number;
  renderTime: number;
}

export async function renderMermaidSvg(
  source: string,
  theme: string,
  configJson = DEFAULT_MERMAID_CONFIG,
  options: { diagramFont?: DiagramFont } = {}
): Promise<MermaidRenderResult> {
  const prepareStartTime = performance.now();
  const zenumlSource = isZenUmlSource(source);

  try {
    const prepared = await prepareMermaid(theme, configJson, {
      warmup: !zenumlSource,
      elkLayouts: needsElkLayouts(source, configJson),
      zenuml: zenumlSource,
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

    const result = await renderPreparedMermaid(
      prepared.mermaid,
      preparedSource,
      zenumlSource
    );
    assertSafeSvgForDom(result.svg);
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
    elkLayouts: needsElkLayouts(MERMAID_WARMUP_SOURCE, configJson),
    diagramFont: options.diagramFont,
  }).catch(() => undefined);
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
): Promise<{ mermaid: MermaidApi; normalizedTheme: string }> {
  const mermaid = await loadMermaid();
  await ensureMermaidExternalDiagrams(mermaid, {
    elkLayouts: options.elkLayouts,
    zenuml: options.zenuml,
  });
  return await prepareMermaidSession(theme, configJson, {
    warmup: options.warmup,
    diagramFont: options.diagramFont,
  });
}

async function renderPreparedMermaid(
  mermaid: MermaidApi,
  source: string,
  zenumlSource: boolean
): Promise<{ svg: string }> {
  try {
    return await mermaid.render(nextMermaidRenderId(), source);
  } catch (error) {
    if (!zenumlSource || !isExternalDiagramLoadError(error)) {
      throw error;
    }

    await refreshZenUmlRegistration(mermaid);
    return await mermaid.render(nextMermaidRenderId(), source);
  }
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
