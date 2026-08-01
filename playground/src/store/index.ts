import { create } from "zustand";
import type {
  HostThemePresetName,
  ThemeName,
} from "@mermanjs/web";
import type { DiagramFont } from "@/src/lib/diagram-font";
import { DEFAULT_MERMAID_CONFIG } from "@/src/lib/mermaid-config";
import type { MermanTextMeasurementMode } from "@/src/runtime/merman-core";

export type Theme = ThemeName;
export type HostThemePreset = "none" | HostThemePresetName;
export type UITheme = "light" | "dark" | "system";
export type ResolvedUITheme = Exclude<UITheme, "system">;
export type EditorMode = "code" | "config";
export type PreviewMode = "svg" | "ascii" | "compare" | "diagnostics";
export type WorkspacePane = "editor" | "preview";
export type TextMeasurementMode = MermanTextMeasurementMode;
export type { DiagramFont };

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
  hostThemePreset: HostThemePreset;
  setHostThemePreset: (preset: HostThemePreset) => void;
  textMeasurementMode: TextMeasurementMode;
  setTextMeasurementMode: (mode: TextMeasurementMode) => void;
  diagramFont: DiagramFont;
  setDiagramFont: (font: DiagramFont) => void;

  // Workbench theme
  uiTheme: UITheme;
  resolvedTheme: ResolvedUITheme;
  setUITheme: (theme: UITheme) => void;

  // Workspace navigation
  workspacePane: WorkspacePane;
  setWorkspacePane: (pane: WorkspacePane) => void;
  previewMode: PreviewMode;
  setPreviewMode: (mode: PreviewMode) => void;
  showExamples: boolean;
  setShowExamples: (open: boolean) => void;
  toggleExamples: () => void;
}

// Default source
const DEFAULT_CODE = `flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D`;

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
  code: DEFAULT_CODE,
  setCode: (code) => set({ code }),
  mermaidConfig: DEFAULT_MERMAID_CONFIG,
  setMermaidConfig: (mermaidConfig) => set({ mermaidConfig }),
  editorMode: "code",
  setEditorMode: (editorMode) => set({ editorMode }),

  // Diagram presentation
  diagramTheme: "default",
  setDiagramTheme: (diagramTheme) =>
    set({ diagramTheme, hostThemePreset: "none" }),
  hostThemePreset: "none",
  setHostThemePreset: (hostThemePreset) =>
    set((state) => ({
      hostThemePreset,
      diagramTheme: hostThemePreset === "none" ? state.diagramTheme : "default",
    })),
  textMeasurementMode: "browser",
  setTextMeasurementMode: (textMeasurementMode) => set({ textMeasurementMode }),
  diagramFont: "trebuchet",
  setDiagramFont: (diagramFont) => set({ diagramFont }),

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
  showExamples: false,
  setShowExamples: (showExamples) => set({ showExamples }),
  toggleExamples: () => set((state) => ({ showExamples: !state.showExamples })),
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
