import { bindSurfaceRuntime } from "../surface-runtime.js";
import type { MermanWasmModule } from "../index.js";
export * from "../index.js";

function surfaceLoader(): Promise<MermanWasmModule> {
  // @ts-ignore -- generated wasm-bindgen artifact exists after build:surfaces runs.
  return import("../../pkg/ascii/merman_wasm.js");
}

const runtime = bindSurfaceRuntime(surfaceLoader);

export const {
  initMerman,
  getMerman,
  isMermanInitialized,
  renderSvg,
  renderSvgWithTextMeasurer,
  layoutJsonWithTextMeasurer,
  renderSvgElement,
  renderSvgToElement,
  renderAscii,
  parseJson,
  parseObject,
  layoutJson,
  layoutObject,
  analyze,
  analyzeJson,
  analysisFacts,
  analyzeDocument,
  analyzeDocumentFacts,
  validate,
  editorDiagnostics,
  editorCodeActions,
  editorCompletions,
  editorHover,
  editorDocumentSymbols,
  editorWorkspaceSymbols,
  editorDefinition,
  editorReferences,
  editorPrepareRename,
  editorRename,
  editorSemanticTokenLegend,
  editorSemanticTokens,
  bindingCapabilities,
  selectedRegistryProfile,
  supportedDiagrams,
  diagramFamilyCapabilities,
  lintRuleCatalog,
  asciiSupportedDiagrams,
  asciiCapabilities,
  supportedThemes,
  supportedHostThemePresets,
  abiVersion,
  packageVersion,
} = runtime;
