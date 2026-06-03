import { useEffect, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toolbar } from "./components/Toolbar";
import { CodeEditor } from "./components/Editor";
import { ConfigEditor } from "./components/ConfigEditor";
import { Preview } from "./components/Preview";
import { StatusBar } from "./components/StatusBar";
import { ExampleGallery } from "./components/ExampleGallery";
import { useAppStore } from "./store";
import { useShare } from "./hooks/useShare";
import { prewarmWasmRenderer } from "./lib/wasm-loader";
import { prewarmMermaidRenderer } from "./lib/mermaid-renderer";
import { normalizeThemeName } from "@merman/web";
import { cn } from "@/lib/utils";

export default function App() {
  const { t, i18n } = useTranslation();
  const {
    setCode,
    setDiagramTheme,
    diagramTheme,
    setMermaidConfig,
    mermaidConfig,
    editorMode,
    setEditorMode,
    uiTheme,
  } = useAppStore();
  const { initialData } = useShare();

  useEffect(() => {
    const lang = i18n.language.startsWith("zh") ? "zh-CN" : "en";
    document.documentElement.lang = lang;
    document.title = t("app.title");
    document
      .querySelector('meta[name="description"]')
      ?.setAttribute("content", t("app.description"));
  }, [i18n.language, t]);

  // 从 URL 加载分享的数据
  useEffect(() => {
    if (initialData) {
      setCode(initialData.code);
      if (initialData.theme) {
        setDiagramTheme(normalizeThemeName(initialData.theme));
      }
      if (initialData.config !== undefined) {
        setMermaidConfig(initialData.config);
      }
    }
  }, [initialData, setCode, setDiagramTheme, setMermaidConfig]);

  // 应用 UI 主题
  useEffect(() => {
    const root = document.documentElement;
    if (uiTheme === "dark") {
      root.classList.add("dark");
    } else if (uiTheme === "light") {
      root.classList.remove("dark");
    } else {
      // system
      const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      const handleChange = (e: MediaQueryListEvent | MediaQueryList) => {
        if (e.matches) {
          root.classList.add("dark");
        } else {
          root.classList.remove("dark");
        }
      };
      handleChange(mediaQuery);
      mediaQuery.addEventListener("change", handleChange);
      return () => mediaQuery.removeEventListener("change", handleChange);
    }
  }, [uiTheme]);

  // 页面级后台预热，避免首次切到对比/预览时把资源准备算进渲染耗时。
  useEffect(() => {
    const timeout = window.setTimeout(() => {
      void prewarmWasmRenderer(diagramTheme, mermaidConfig).catch(() => undefined);
      void prewarmMermaidRenderer(diagramTheme, mermaidConfig);
    }, 120);

    return () => window.clearTimeout(timeout);
  }, [diagramTheme, mermaidConfig]);

  return (
    <TooltipProvider delayDuration={300}>
      <div className="h-screen flex flex-col bg-background">
        {/* 顶部工具栏 */}
        <Toolbar />

        {/* 主内容区 */}
        <main className="flex-1 overflow-hidden relative">
          {/* 示例库覆盖层 */}
          <ExampleGallery />

          {/* 可调整大小的面板 */}
          <ResizablePanelGroup
            direction="horizontal"
            className="h-full"
          >
            {/* 编辑器面板 */}
            <ResizablePanel
              defaultSize={45}
              minSize={25}
              maxSize={75}
              className="bg-card"
            >
              <div className="h-full flex flex-col">
                <div className="flex items-center justify-between px-4 py-2 border-b bg-muted/30">
                  <div className="flex items-center gap-1">
                    <EditorModeButton
                      active={editorMode === "code"}
                      onClick={() => setEditorMode("code")}
                    >
                      {t("editor.codeMode")}
                    </EditorModeButton>
                    <EditorModeButton
                      active={editorMode === "config"}
                      onClick={() => setEditorMode("config")}
                    >
                      {t("editor.configMode")}
                    </EditorModeButton>
                  </div>
                  <span className="text-xs text-muted-foreground">
                    {editorMode === "code" ? "Mermaid" : "JSON"}
                  </span>
                </div>
                {editorMode === "code" ? (
                  <CodeEditor className="flex-1" />
                ) : (
                  <ConfigEditor className="flex-1" />
                )}
              </div>
            </ResizablePanel>

            {/* 拖拽手柄 */}
            <ResizableHandle withHandle />

            {/* 预览面板 */}
            <ResizablePanel defaultSize={55} minSize={25}>
              <div className="h-full flex flex-col">
                <div className="flex items-center justify-between px-4 py-2 border-b bg-muted/30">
                  <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                    {t("preview.title")}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {t("preview.wheelZoom")}
                  </span>
                </div>
                <Preview className="flex-1 bg-[repeating-conic-gradient(#80808010_0%_25%,transparent_0%_50%)] bg-[length:20px_20px]" />
              </div>
            </ResizablePanel>
          </ResizablePanelGroup>
        </main>

        {/* 底部状态栏 */}
        <StatusBar />
      </div>
    </TooltipProvider>
  );
}

function EditorModeButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick(): void;
  children: ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "rounded-md px-2.5 py-1 text-xs font-medium transition-colors",
        active
          ? "bg-background text-foreground shadow-sm"
          : "text-muted-foreground hover:bg-background/60 hover:text-foreground"
      )}
    >
      {children}
    </button>
  );
}
