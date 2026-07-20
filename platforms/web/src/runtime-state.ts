import type {
  AsciiCapability,
  AsciiDiagramType,
  DiagramFamilyCapability,
  DiagramType,
  EditorSemanticTokenDescriptor,
  HostThemePresetName,
  LintRuleCatalogEntry,
  MermanWasmLoader,
  MermanWasmModule,
  ThemeName,
} from "./index.js";

export interface MermanRuntimeState {
  defaultLoader: MermanWasmLoader;
  wasmModule: MermanWasmModule | null;
  initPromise: Promise<MermanWasmModule> | null;
  supportedDiagramsCache: DiagramType[] | null;
  asciiSupportedDiagramsCache: AsciiDiagramType[] | null;
  asciiCapabilitiesCache: AsciiCapability[] | null;
  diagramFamilyCapabilitiesCache: DiagramFamilyCapability[] | null;
  diagramMetadataBySyntaxIdCache: ReadonlyMap<string, DiagramType | null> | null;
  editorSemanticTokenDescriptorCache: EditorSemanticTokenDescriptor | null;
  lintRuleCatalogCache: LintRuleCatalogEntry[] | null;
  supportedHostThemePresetsCache: HostThemePresetName[] | null;
  supportedThemesCache: ThemeName[] | null;
}

let activeRuntimeState: MermanRuntimeState | null = null;

export function createMermanRuntimeState(
  defaultLoader: MermanWasmLoader
): MermanRuntimeState {
  return {
    defaultLoader,
    wasmModule: null,
    initPromise: null,
    supportedDiagramsCache: null,
    asciiSupportedDiagramsCache: null,
    asciiCapabilitiesCache: null,
    diagramFamilyCapabilitiesCache: null,
    diagramMetadataBySyntaxIdCache: null,
    editorSemanticTokenDescriptorCache: null,
    lintRuleCatalogCache: null,
    supportedHostThemePresetsCache: null,
    supportedThemesCache: null,
  };
}

export function currentMermanRuntimeState(
  defaultState: MermanRuntimeState
): MermanRuntimeState {
  return activeRuntimeState ?? defaultState;
}

export function withMermanRuntimeState<T>(
  state: MermanRuntimeState,
  run: () => T
): T {
  const previous = activeRuntimeState;
  activeRuntimeState = state;
  try {
    return run();
  } finally {
    activeRuntimeState = previous;
  }
}
