import type { DiagramFont } from "@/src/lib/diagram-font";
import type { MermaidExternalRequirements } from "@/src/runtime/mermaid-requirements";
import { assertSafeSvgForDom } from "@mermanjs/web";
import {
  ensureMermaidExternalDiagrams,
  isExternalDiagramLoadError,
  refreshZenUmlRegistration,
} from "@/src/lib/mermaid-external-diagrams";
import {
  loadMermaid,
  nextMermaidRenderId,
  prepareMermaidSession,
  type MermaidApi,
} from "@/src/lib/mermaid-runtime";
import {
  DEFAULT_MERMAID_CONFIG,
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
  options: {
    diagramFont?: DiagramFont;
    externalRequirements: MermaidExternalRequirements;
  }
): Promise<MermaidRenderResult> {
  const prepareStartTime = performance.now();

  try {
    const prepared = await prepareMermaid(theme, configJson, {
      warmup: !options.externalRequirements.zenuml,
      externalRequirements: options.externalRequirements,
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
      options.externalRequirements.zenuml
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
  options: {
    diagramFont?: DiagramFont;
    externalRequirements: MermaidExternalRequirements;
  }
): Promise<void> {
  await prepareMermaid(theme, configJson, {
    warmup: !options.externalRequirements.zenuml,
    externalRequirements: options.externalRequirements,
    diagramFont: options.diagramFont,
  }).catch(() => undefined);
}

async function prepareMermaid(
  theme: string,
  configJson: string,
  options: {
    warmup: boolean;
    externalRequirements: MermaidExternalRequirements;
    diagramFont?: DiagramFont;
  }
): Promise<{ mermaid: MermaidApi; normalizedTheme: string }> {
  const mermaid = await loadMermaid();
  await ensureMermaidExternalDiagrams(mermaid, options.externalRequirements);
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
