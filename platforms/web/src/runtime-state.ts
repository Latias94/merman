import type {
  DiagramFamilyCapability,
  DiagramType,
  ThemeName,
} from "./public-catalog.js";
import type {
  MermanWasmLoader,
  MermanWasmModule,
  RuntimeCatalog,
  ThemeCatalog,
} from "./public-types.js";

export interface MermanRuntimeState {
  defaultLoader: MermanWasmLoader;
  wasmModule: MermanWasmModule | null;
  initPromise: Promise<MermanWasmModule> | null;
  supportedDiagramsCache: DiagramType[] | null;
  diagramFamilyCapabilitiesCache: DiagramFamilyCapability[] | null;
  runtimeCatalogCache: RuntimeCatalog | null;
  themeCatalogCache: ThemeCatalog | null;
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
    diagramFamilyCapabilitiesCache: null,
    runtimeCatalogCache: null,
    themeCatalogCache: null,
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
