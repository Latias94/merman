import { create } from "zustand";
import type { ThemeName } from "@mermanjs/web";
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
import type { RealmViewport } from "../runtime/realm/channel-protocol.ts";
import {
  resolveRenderViewport,
  type LockedRenderEnvironment,
  type RenderViewportMode,
} from "../runtime/render-viewport.ts";

export type Theme = ThemeName;
export type UITheme = "light" | "dark" | "system";
export type ResolvedUITheme = Exclude<UITheme, "system">;
export type EditorMode = "code" | "config";
export type PreviewMode = "svg" | "ascii" | "compare" | "diagnostics";
export type WorkspacePane = "editor" | "preview";
export type TextMeasurementMode = MermanTextMeasurementMode;
export type SvgPipeline = MermanSvgPipeline;
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

  // Diagram presentation
  diagramTheme: Theme;
  setDiagramTheme: (theme: Theme) => void;
  presentationThemePresetId: string | null;
  setPresentationThemePresetId: (presetId: string | null) => void;
  presentationProfileId: string | null;
  setPresentationProfileId: (profileId: string | null) => void;
  renderViewportMode: RenderViewportMode;
  setRenderViewportMode: (mode: RenderViewportMode) => void;
  liveHostRenderViewport: Readonly<RealmViewport> | null;
  setLiveHostRenderViewport: (viewport: RealmViewport) => void;
  svgPipeline: SvgPipeline;
  setSvgPipeline: (pipeline: SvgPipeline) => void;
  textMeasurementMode: TextMeasurementMode;
  setTextMeasurementMode: (mode: TextMeasurementMode) => void;
  diagramFont: DiagramFont;
  setDiagramFont: (font: DiagramFont) => void;
  applyWorkspaceSnapshot: (snapshot: WorkspaceSnapshot) => void;
  sharedRenderEnvironmentLock: Readonly<LockedRenderEnvironment> | null;
  clearSharedRenderEnvironmentLock: () => void;
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
}

export function selectWorkspaceSnapshot(
  state: Pick<AppState, keyof WorkspaceSnapshot>
): WorkspaceSnapshot {
  return {
    code: state.code,
    mermaidConfig: state.mermaidConfig,
    diagramTheme: state.diagramTheme,
    presentationThemePresetId: state.presentationThemePresetId,
    presentationProfileId: state.presentationProfileId,
    renderViewportMode: state.renderViewportMode,
    svgPipeline: state.svgPipeline,
    textMeasurementMode: state.textMeasurementMode,
    diagramFont: state.diagramFont,
  };
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
  systemDark = systemPrefersDark()
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

export const useAppStore = create<AppState>((set) => ({
  // Editor state
  code: DEFAULT_WORKSPACE_SNAPSHOT.code,
  setCode: (code) => set({ code }),
  mermaidConfig: DEFAULT_WORKSPACE_SNAPSHOT.mermaidConfig,
  setMermaidConfig: (mermaidConfig) => set({ mermaidConfig }),
  editorMode: "code",
  setEditorMode: (editorMode) => set({ editorMode }),

  // Diagram presentation
  diagramTheme: DEFAULT_WORKSPACE_SNAPSHOT.diagramTheme,
  setDiagramTheme: (diagramTheme) => set({ diagramTheme }),
  presentationThemePresetId:
    DEFAULT_WORKSPACE_SNAPSHOT.presentationThemePresetId,
  setPresentationThemePresetId: (presentationThemePresetId) =>
    set({ presentationThemePresetId }),
  presentationProfileId: DEFAULT_WORKSPACE_SNAPSHOT.presentationProfileId,
  setPresentationProfileId: (presentationProfileId) =>
    set({ presentationProfileId }),
  renderViewportMode: DEFAULT_WORKSPACE_SNAPSHOT.renderViewportMode,
  setRenderViewportMode: (renderViewportMode) => set({ renderViewportMode }),
  liveHostRenderViewport: null,
  setLiveHostRenderViewport: (candidate) =>
    set((state) => {
      const resolved = resolveRenderViewport("host", candidate);
      if (resolved.status !== "host") return state;
      const previous = state.liveHostRenderViewport;
      if (
        previous?.width === resolved.viewport.width &&
        previous.height === resolved.viewport.height
      ) {
        return state;
      }
      return { liveHostRenderViewport: resolved.viewport };
    }),
  svgPipeline: DEFAULT_WORKSPACE_SNAPSHOT.svgPipeline,
  setSvgPipeline: (svgPipeline) => set({ svgPipeline }),
  textMeasurementMode: DEFAULT_WORKSPACE_SNAPSHOT.textMeasurementMode,
  setTextMeasurementMode: (textMeasurementMode) => set({ textMeasurementMode }),
  diagramFont: DEFAULT_WORKSPACE_SNAPSHOT.diagramFont,
  setDiagramFont: (diagramFont) => set({ diagramFont }),
  applyWorkspaceSnapshot: (snapshot) => set({ ...snapshot }),
  sharedRenderEnvironmentLock: null,
  clearSharedRenderEnvironmentLock: () =>
    set((state) =>
      state.sharedRenderEnvironmentLock
        ? { sharedRenderEnvironmentLock: null }
        : state,
    ),
  shareViewWarning: null,
  applyStartupShareHydration: ({ workspace, view, warning }) =>
    set({
      ...workspace,
      workspacePane: view.workspacePane,
      editorMode: view.editorMode,
      previewMode: view.previewMode,
      sharedRenderEnvironmentLock: view.lockedEnvironment,
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
