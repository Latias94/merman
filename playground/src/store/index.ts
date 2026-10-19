import { create } from "zustand";
import type { MermaidThemeSelection } from "../lib/mermaid-theme-name.ts";
import type { DiagramFont } from "../lib/diagram-font.ts";
import type {
  ShareViewWarning,
  StartupShareHydration,
} from "../lib/share-view.ts";
import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  type WorkspaceSnapshot,
} from "../lib/workspace-snapshot.ts";
import type {
  MermanSvgPipeline,
  MermanTextMeasurementMode,
} from "../runtime/merman-core.ts";
import {
  DEFAULT_SVG_PRESENTATION_MODE,
  type SvgPresentationMode,
} from "../lib/svg-presentation.ts";
import {
  acceptThemeValidation,
  advanceThemeBaseline,
  beginThemeEditSession,
  beginThemeImport,
  completeThemeImport,
  failThemeImport,
  isThemeDraftChanged,
  rejectThemeValidation,
  selectThemeCandidate,
  themeCandidateFields,
  themeCandidateFromFields,
  themeValidationToken,
  type ThemeCandidate,
  type ThemeEditSession,
  type ThemeImportToken,
  type ThemeValidationToken,
} from "../lib/theme-edit-session.ts";

export type Theme = MermaidThemeSelection;
export type UITheme = "light" | "dark" | "system";
export type ResolvedUITheme = Exclude<UITheme, "system">;
export type EditorMode = "code" | "config";
export type PreviewMode = "svg" | "ascii" | "compare" | "diagnostics";
export type PreviewBacking = "automatic" | "light" | "dark" | "transparency";
export type WorkspacePane = "editor" | "preview";
export type TextMeasurementMode = MermanTextMeasurementMode;
export type SvgPipeline = MermanSvgPipeline;
export type { SvgPresentationMode };
export type { DiagramFont };
export { DEFAULT_WORKSPACE_SNAPSHOT, type WorkspaceSnapshot };

export interface AppState {
  // Editor state
  code: string;
  setCode: (code: string) => void;
  mermaidConfig: string;
  setMermaidConfig: (config: string) => void;
  editorMode: EditorMode;
  setEditorMode: (mode: EditorMode) => void;

  // Diagram rendering
  diagramTheme: Theme;
  setDiagramTheme: (theme: Theme) => void;
  themePresetId: string | null;
  setThemePresetId: (presetId: string | null) => void;
  themeRecipeJson: string | null;
  setThemeRecipeJson: (recipeJson: string | null) => void;
  themeEditSession: ThemeEditSession | null;
  themeEditGeneration: number;
  beginThemeEdit: () => ThemeValidationToken;
  acceptThemeValidation: (token: ThemeValidationToken, recipeJson: string) => boolean;
  rejectThemeValidation: (token: ThemeValidationToken, error: string) => boolean;
  selectThemeCandidate: (candidate: Exclude<ThemeCandidate, { kind: "custom" }>) => void;
  beginThemeImport: () => ThemeImportToken | null;
  completeThemeImport: (token: ThemeImportToken, rawJson: string) => ThemeValidationToken | null;
  failThemeImport: (token: ThemeImportToken, error: string) => boolean;
  applyThemeEdit: () => boolean;
  cancelThemeEdit: () => void;
  svgPipeline: SvgPipeline;
  setSvgPipeline: (pipeline: SvgPipeline) => void;
  textMeasurementMode: TextMeasurementMode;
  setTextMeasurementMode: (mode: TextMeasurementMode) => void;
  diagramFont: DiagramFont;
  setDiagramFont: (font: DiagramFont) => void;
  applyWorkspaceSnapshot: (snapshot: WorkspaceSnapshot) => void;
  shareViewWarning: Readonly<ShareViewWarning> | null;
  applyStartupShareHydration: (hydration: StartupShareHydration) => void;

  // Workbench theme
  uiTheme: UITheme;
  resolvedTheme: ResolvedUITheme;
  setUITheme: (theme: UITheme) => void;

  // Workspace navigation
  workspacePane: WorkspacePane;
  setWorkspacePane: (pane: WorkspacePane) => void;
  previewMode: PreviewMode;
  setPreviewMode: (mode: PreviewMode) => void;
  svgPresentationMode: SvgPresentationMode;
  setSvgPresentationMode: (mode: SvgPresentationMode) => void;
  showSvgBounds: boolean;
  setShowSvgBounds: (show: boolean) => void;
  previewBacking: PreviewBacking;
  setPreviewBacking: (backing: PreviewBacking) => void;
}

export function selectWorkspaceSnapshot(
  state: Pick<AppState, keyof WorkspaceSnapshot>,
): WorkspaceSnapshot {
  return {
    code: state.code,
    mermaidConfig: state.mermaidConfig,
    diagramTheme: state.diagramTheme,
    themePresetId: state.themePresetId,
    themeRecipeJson: state.themeRecipeJson,
    svgPipeline: state.svgPipeline,
    textMeasurementMode: state.textMeasurementMode,
    diagramFont: state.diagramFont,
  };
}

export function selectRenderInputSnapshot(
  state: Pick<AppState, keyof WorkspaceSnapshot | "themeEditSession">,
): WorkspaceSnapshot {
  const workspace = selectWorkspaceSnapshot(state);
  return state.themeEditSession
    ? { ...workspace, ...themeCandidateFields(state.themeEditSession.candidate) }
    : workspace;
}

export function selectThemeDraftChanged(state: Pick<AppState, "themeEditSession">): boolean {
  return isThemeDraftChanged(state.themeEditSession);
}

function getInitialUITheme(): UITheme {
  if (typeof window === "undefined") return "dark";
  try {
    const stored = localStorage.getItem("merman-ui-theme");
    if (stored === "light" || stored === "dark" || stored === "system") {
      return stored;
    }
  } catch {
    // Storage can be unavailable in hardened browsing contexts.
  }
  return "dark";
}

function resolveUITheme(
  uiTheme: UITheme,
  systemDark = systemPrefersDark(),
): ResolvedUITheme {
  return uiTheme === "system" ? (systemDark ? "dark" : "light") : uiTheme;
}

function systemPrefersDark(): boolean {
  return typeof window === "undefined"
    ? true
    : window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function persistUITheme(uiTheme: UITheme): void {
  try {
    localStorage.setItem("merman-ui-theme", uiTheme);
  } catch {
    // Theme selection still applies for this session when storage is unavailable.
  }
}

function applyResolvedTheme(resolvedTheme: ResolvedUITheme): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.classList.toggle("dark", resolvedTheme === "dark");
  root.classList.toggle("light", resolvedTheme === "light");
  root.dataset.theme = resolvedTheme;
  root.style.colorScheme = resolvedTheme;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", resolvedTheme === "dark" ? "#121816" : "#f7faf8");
}

const initialUITheme = getInitialUITheme();
const initialResolvedTheme = resolveUITheme(initialUITheme);

export const useAppStore = create<AppState>((set, get) => ({
  // Editor state
  code: DEFAULT_WORKSPACE_SNAPSHOT.code,
  setCode: (code) => set({ code }),
  mermaidConfig: DEFAULT_WORKSPACE_SNAPSHOT.mermaidConfig,
  setMermaidConfig: (mermaidConfig) => set({ mermaidConfig }),
  editorMode: "code",
  setEditorMode: (editorMode) => set({ editorMode }),

  // Diagram rendering
  diagramTheme: DEFAULT_WORKSPACE_SNAPSHOT.diagramTheme,
  setDiagramTheme: (diagramTheme) => set({ diagramTheme }),
  themePresetId: DEFAULT_WORKSPACE_SNAPSHOT.themePresetId,
  setThemePresetId: (themePresetId) => set({ themePresetId, themeRecipeJson: null, themeEditSession: null }),
  themeRecipeJson: DEFAULT_WORKSPACE_SNAPSHOT.themeRecipeJson,
  setThemeRecipeJson: (themeRecipeJson) =>
    set(
      themeRecipeJson === null
        ? { themeRecipeJson, themeEditSession: null }
        : { themeRecipeJson, themePresetId: null, themeEditSession: null },
    ),
  themeEditSession: null,
  themeEditGeneration: 0,
  beginThemeEdit: () => {
    const state = get();
    if (state.themeEditSession) return themeValidationToken(state.themeEditSession);
    const generation = state.themeEditGeneration + 1;
    const session = beginThemeEditSession(generation, themeCandidateFromFields(state));
    set({ themeEditSession: session, themeEditGeneration: generation });
    return themeValidationToken(session);
  },
  acceptThemeValidation: (token, recipeJson) => {
    const current = get().themeEditSession;
    if (!current) return false;
    const session = acceptThemeValidation(current, token, recipeJson);
    if (session === current) return false;
    set({ themeEditSession: session });
    return true;
  },
  rejectThemeValidation: (token, error) => {
    const current = get().themeEditSession;
    if (!current) return false;
    const session = rejectThemeValidation(current, token, error);
    if (session === current) return false;
    set({ themeEditSession: session });
    return true;
  },
  selectThemeCandidate: (candidate) => {
    const current = get().themeEditSession;
    if (current) set({ themeEditSession: selectThemeCandidate(current, candidate) });
  },
  beginThemeImport: () => {
    const current = get().themeEditSession;
    if (!current) return null;
    const session = beginThemeImport(current);
    set({ themeEditSession: session });
    return { ...themeValidationToken(session), generation: session.importGeneration };
  },
  completeThemeImport: (token, rawJson) => {
    const current = get().themeEditSession;
    if (!current) return null;
    const session = completeThemeImport(current, token, rawJson);
    if (session === current) return null;
    set({ themeEditSession: session });
    return themeValidationToken(session);
  },
  failThemeImport: (token, error) => {
    const current = get().themeEditSession;
    if (!current) return false;
    const session = failThemeImport(current, token, error);
    if (session === current) return false;
    set({ themeEditSession: session });
    return true;
  },
  applyThemeEdit: () => {
    const current = get().themeEditSession;
    if (!current || current.validation !== "valid" || current.importing) return false;
    const session = advanceThemeBaseline(current);
    set({ ...themeCandidateFields(session.candidate), themeEditSession: session });
    return true;
  },
  cancelThemeEdit: () => set({ themeEditSession: null }),
  svgPipeline: DEFAULT_WORKSPACE_SNAPSHOT.svgPipeline,
  setSvgPipeline: (svgPipeline) => set({ svgPipeline }),
  textMeasurementMode: DEFAULT_WORKSPACE_SNAPSHOT.textMeasurementMode,
  setTextMeasurementMode: (textMeasurementMode) => set({ textMeasurementMode }),
  diagramFont: DEFAULT_WORKSPACE_SNAPSHOT.diagramFont,
  setDiagramFont: (diagramFont) => set({ diagramFont }),
  applyWorkspaceSnapshot: (snapshot) => set({ ...snapshot, themeEditSession: null }),
  shareViewWarning: null,
  applyStartupShareHydration: ({ workspace, view, warning }) =>
    set({
      ...workspace,
      themeEditSession: null,
      workspacePane: view.workspacePane,
      editorMode: view.editorMode,
      previewMode: view.previewMode,
      svgPresentationMode: view.svgPresentationMode,
      showSvgBounds: view.showSvgBounds,
      shareViewWarning: warning,
    }),

  // Workbench theme
  uiTheme: initialUITheme,
  resolvedTheme: initialResolvedTheme,
  setUITheme: (uiTheme) => {
    const resolvedTheme = resolveUITheme(uiTheme);
    persistUITheme(uiTheme);
    applyResolvedTheme(resolvedTheme);
    set({ uiTheme, resolvedTheme });
  },

  // Workspace navigation
  workspacePane: "editor",
  setWorkspacePane: (workspacePane) => set({ workspacePane }),
  previewMode: "svg",
  setPreviewMode: (previewMode) => set({ previewMode }),
  svgPresentationMode: DEFAULT_SVG_PRESENTATION_MODE,
  setSvgPresentationMode: (svgPresentationMode) =>
    set((state) =>
      state.svgPresentationMode === svgPresentationMode
        ? state
        : { svgPresentationMode },
    ),
  showSvgBounds: false,
  setShowSvgBounds: (showSvgBounds) => set({ showSvgBounds }),
  previewBacking: "automatic",
  setPreviewBacking: (previewBacking) => set({ previewBacking }),
}));

applyResolvedTheme(initialResolvedTheme);

let themeLifecycleOwners = 0;
let disposeThemeListener: (() => void) | null = null;

export function installUIThemeLifecycle(): () => void {
  if (typeof window === "undefined") return () => undefined;
  themeLifecycleOwners += 1;

  if (!disposeThemeListener) {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const synchronize = () => {
      const state = useAppStore.getState();
      if (state.uiTheme !== "system") return;
      const resolvedTheme = resolveUITheme("system", mediaQuery.matches);
      applyResolvedTheme(resolvedTheme);
      if (state.resolvedTheme !== resolvedTheme) {
        useAppStore.setState({ resolvedTheme });
      }
    };
    mediaQuery.addEventListener("change", synchronize);
    disposeThemeListener = () =>
      mediaQuery.removeEventListener("change", synchronize);
    synchronize();
  } else {
    applyResolvedTheme(useAppStore.getState().resolvedTheme);
  }

  let active = true;
  return () => {
    if (!active) return;
    active = false;
    themeLifecycleOwners = Math.max(0, themeLifecycleOwners - 1);
    if (themeLifecycleOwners === 0) {
      disposeThemeListener?.();
      disposeThemeListener = null;
    }
  };
}
