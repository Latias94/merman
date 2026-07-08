import { bindSurfaceRuntime } from "../surface-runtime.js";
import type { MermanWasmModule } from "../index.js";
export type * from "../index.js";
export {
  SUPPORTED_THEMES,
  SUPPORTED_HOST_THEME_PRESETS,
  SUPPORTED_DIAGRAMS,
  SUPPORTED_ASCII_DIAGRAMS,
  BINDING_STATUS_CODE_NAMES,
  RENDER_ONLY_BINDING_CAPABILITIES as DEFAULT_BINDING_CAPABILITIES,
  isThemeName,
  isHostThemePresetName,
  isDiagramType,
  isAsciiDiagramType,
  isBindingStatusCodeName,
  isBindingErrorPayload,
  normalizeThemeName,
  normalizeHostThemePresetName,
  encodeOptions,
  assertSafeSvgForDom,
  createBrowserTextMeasurer,
} from "../index.js";

function surfaceLoader(): Promise<MermanWasmModule> {
  // @ts-ignore -- generated wasm-bindgen artifact exists after build:surfaces runs.
  return import("../../pkg/render-only/merman_wasm.js");
}

const runtime = bindSurfaceRuntime(surfaceLoader);

export const {
  initMerman,
  getMerman,
  isMermanInitialized,
  bindingCapabilities,
  selectedRegistryProfile,
  supportedDiagrams,
  diagramFamilyCapabilities,
  supportedThemes,
  abiVersion,
  packageVersion,
  renderSvg,
  renderSvgWithTextMeasurer,
  layoutJsonWithTextMeasurer,
  renderSvgElement,
  renderSvgToElement,
  parseJson,
  parseObject,
  layoutJson,
  layoutObject,
  supportedHostThemePresets,
} = runtime;
