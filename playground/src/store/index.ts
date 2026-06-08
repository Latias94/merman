import { create } from "zustand";
import type { ThemeName } from "@mermanjs/web";
import { DEFAULT_MERMAID_CONFIG } from "@/src/lib/mermaid-config";

export type Theme = ThemeName;
export type UITheme = "light" | "dark" | "system";
export type EditorMode = "code" | "config";

interface AppState {
  // 编辑器状态
  code: string;
  setCode: (code: string) => void;
  mermaidConfig: string;
  setMermaidConfig: (config: string) => void;
  editorMode: EditorMode;
  setEditorMode: (mode: EditorMode) => void;

  // 当前图表类型
  diagramType: string;
  setDiagramType: (type: string) => void;

  // Mermaid 主题
  diagramTheme: Theme;
  setDiagramTheme: (theme: Theme) => void;

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

  // 当前图表类型
  diagramType: "flowchart",
  setDiagramType: (diagramType) => set({ diagramType }),

  // Mermaid 主题
  diagramTheme: "default",
  setDiagramTheme: (diagramTheme) => set({ diagramTheme }),

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
