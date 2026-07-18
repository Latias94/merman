import { create } from "zustand";
import type {
  DiagramDetectionFacts,
  DiagramType,
  HostThemePresetName,
  ThemeName,
} from "@mermanjs/web";
import type { DiagramFont } from "@/src/lib/diagram-font";
import { DEFAULT_MERMAID_CONFIG } from "@/src/lib/mermaid-config";
import type { MermanTextMeasurementMode } from "@/src/runtime/merman-core";
import {
  createDiagramDetectionKey,
  freshRenderArtifactValue,
  type RenderArtifact,
} from "@/src/lib/render-artifacts";

export type Theme = ThemeName;
export type HostThemePreset = "none" | HostThemePresetName;
export type UITheme = "light" | "dark" | "system";
export type EditorMode = "code" | "config";
export type TextMeasurementMode = MermanTextMeasurementMode;
export type LiveDiagramType = DiagramType | "unknown";
export type { DiagramFont };

export interface AppState {
  // 编辑器状态
  code: string;
  setCode: (code: string) => void;
  mermaidConfig: string;
  setMermaidConfig: (config: string) => void;
  editorMode: EditorMode;
  setEditorMode: (mode: EditorMode) => void;

  // Canonical parser facts for the latest detection request.
  diagramDetectionArtifact: RenderArtifact<DiagramDetectionFacts> | null;
  setDiagramDetectionArtifact: (
    artifact: RenderArtifact<DiagramDetectionFacts> | null
  ) => void;

  // Mermaid 主题
  diagramTheme: Theme;
  setDiagramTheme: (theme: Theme) => void;
  hostThemePreset: HostThemePreset;
  setHostThemePreset: (preset: HostThemePreset) => void;
  textMeasurementMode: TextMeasurementMode;
  setTextMeasurementMode: (mode: TextMeasurementMode) => void;
  diagramFont: DiagramFont;
  setDiagramFont: (font: DiagramFont) => void;

  // UI 主题
  uiTheme: UITheme;
  setUITheme: (theme: UITheme) => void;
  isDarkMode: boolean;

  // 面板状态
  showExamples: boolean;
  toggleExamples: () => void;

  // 渲染状态
  lastRenderTime: number;
  setLastRenderTime: (time: number) => void;
}

// 默认代码
const DEFAULT_CODE = `flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D`;

// 从 localStorage 读取 UI 主题
function getInitialUITheme(): UITheme {
  if (typeof window === "undefined") return "dark";
  const stored = localStorage.getItem("merman-ui-theme");
  if (stored === "light" || stored === "dark" || stored === "system") {
    return stored;
  }
  return "dark";
}

// 计算是否为深色模式
function getIsDarkMode(uiTheme: UITheme): boolean {
  if (uiTheme === "dark") return true;
  if (uiTheme === "light") return false;
  // system theme
  if (typeof window === "undefined") return true;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export const useAppStore = create<AppState>((set) => ({
  // 编辑器状态
  code: DEFAULT_CODE,
  setCode: (code) => set({ code }),
  mermaidConfig: DEFAULT_MERMAID_CONFIG,
  setMermaidConfig: (mermaidConfig) => set({ mermaidConfig }),
  editorMode: "code",
  setEditorMode: (editorMode) => set({ editorMode }),

  // Canonical parser facts for the latest detection request.
  diagramDetectionArtifact: null,
  setDiagramDetectionArtifact: (diagramDetectionArtifact) =>
    set((state) =>
      state.diagramDetectionArtifact === diagramDetectionArtifact
        ? state
        : { diagramDetectionArtifact }
    ),

  // Mermaid 主题
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

  // UI 主题
  uiTheme: getInitialUITheme(),
  isDarkMode: getIsDarkMode(getInitialUITheme()),
  setUITheme: (uiTheme) => {
    localStorage.setItem("merman-ui-theme", uiTheme);
    set({ uiTheme, isDarkMode: getIsDarkMode(uiTheme) });
  },

  // 面板状态
  showExamples: false,
  toggleExamples: () => set((state) => ({ showExamples: !state.showExamples })),

  // 渲染状态
  lastRenderTime: 0,
  setLastRenderTime: (lastRenderTime) => set({ lastRenderTime }),
}));

export function selectCurrentDiagramDetection(
  state: AppState
): DiagramDetectionFacts | null {
  return freshRenderArtifactValue(
    state.diagramDetectionArtifact,
    createDiagramDetectionKey({
      code: state.code,
      diagramTheme: state.diagramTheme,
      mermaidConfig: state.mermaidConfig,
      hostThemePreset:
        state.hostThemePreset === "none" ? null : state.hostThemePreset,
    })
  );
}

export function selectCurrentDiagramType(state: AppState): LiveDiagramType {
  const detection = selectCurrentDiagramDetection(state);
  return detection?.status === "available" ? detection.diagramType : "unknown";
}
