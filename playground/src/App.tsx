import { lazy, Suspense, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toolbar } from "./components/Toolbar";
import { StatusBar } from "./components/StatusBar";
import { useAppStore, type TextMeasurementMode } from "./store";
import { isDiagramFont } from "./lib/diagram-font";
import { useShare } from "./hooks/useShare";
import { prewarmWasmRenderer } from "./lib/wasm-loader";
import { normalizeHostThemePresetName, normalizeThemeName } from "@mermanjs/web";
import { cn } from "@/lib/utils";
import { useIsMobile } from "@/hooks/use-mobile";

const CodeEditor = lazy(() =>
  import("./components/Editor").then((module) => ({
    default: module.CodeEditor,
  }))
);
const ConfigEditor = lazy(() =>
  import("./components/ConfigEditor").then((module) => ({
    default: module.ConfigEditor,
  }))
);
const Preview = lazy(() =>
  import("./components/Preview").then((module) => ({
    default: module.Preview,
  }))
);
const ExampleGallery = lazy(() =>
  import("./components/ExampleGallery").then((module) => ({
    default: module.ExampleGallery,
  }))
);

const TEXT_MEASUREMENT_VALUES = new Set<TextMeasurementMode>([
  "browser",
  "headless",
]);
export default function App() {
  const { t, i18n } = useTranslation();
  const {
    setCode,
    setDiagramTheme,
    diagramTheme,
    hostThemePreset,
    setHostThemePreset,
    textMeasurementMode,
    setTextMeasurementMode,
    diagramFont,
    setDiagramFont,
    setMermaidConfig,
    mermaidConfig,
    editorMode,
    setEditorMode,
    uiTheme,
    showExamples,
  } = useAppStore();
  const { initialData } = useShare();
  const isMobile = useIsMobile();
  const [mobilePane, setMobilePane] = useState<"editor" | "preview">("editor");

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
      if (initialData.hostThemePreset) {
        const preset = normalizeHostThemePresetName(initialData.hostThemePreset);
        if (preset) {
          setHostThemePreset(preset);
        }
      }
      if (initialData.config !== undefined) {
        setMermaidConfig(initialData.config);
      }
      if (isTextMeasurementMode(initialData.textMeasurementMode)) {
        setTextMeasurementMode(initialData.textMeasurementMode);
      }
      if (isDiagramFont(initialData.diagramFont)) {
        setDiagramFont(initialData.diagramFont);
      }
    }
  }, [
    initialData,
    setCode,
    setDiagramFont,
    setDiagramTheme,
    setHostThemePreset,
    setMermaidConfig,
    setTextMeasurementMode,
  ]);

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

  // 页面级后台预热核心 WASM 渲染器；Mermaid JS 是可选对比引擎，按需加载。
  useEffect(() => {
    const timeout = window.setTimeout(() => {
      void prewarmWasmRenderer(
        diagramTheme,
        mermaidConfig,
        {
          hostThemePreset: hostThemePreset === "none" ? undefined : hostThemePreset,
          textMeasurementMode,
          diagramFont,
        }
      ).catch(() => undefined);
    }, 120);

    return () => window.clearTimeout(timeout);
  }, [diagramFont, diagramTheme, hostThemePreset, mermaidConfig, textMeasurementMode]);

  return (
    <TooltipProvider delayDuration={300}>
      <div className="h-screen flex flex-col bg-background">
        {/* 顶部工具栏 */}
        <Toolbar />

        {/* 主内容区 */}
        <main className="flex-1 overflow-hidden relative">
          {/* 示例库覆盖层 */}
          {showExamples && (
            <Suspense fallback={null}>
              <ExampleGallery />
            </Suspense>
          )}

          {isMobile ? (
            <div className="flex h-full flex-col overflow-hidden">
              <div className="flex h-10 shrink-0 items-center gap-1 border-b bg-muted/30 px-2">
                <MobilePaneButton
                  active={mobilePane === "editor"}
                  onClick={() => setMobilePane("editor")}
                >
                  {t("layout.editor")}
                </MobilePaneButton>
                <MobilePaneButton
                  active={mobilePane === "preview"}
                  onClick={() => setMobilePane("preview")}
                >
                  {t("layout.preview")}
                </MobilePaneButton>
              </div>
              <div className="min-h-0 flex-1">
                {mobilePane === "editor" ? (
                  <EditorPanel
                    editorMode={editorMode}
                    setEditorMode={setEditorMode}
                    t={t}
                  />
                ) : (
                  <PreviewPanel t={t} />
                )}
              </div>
            </div>
          ) : (
            /* 可调整大小的面板 */
            <ResizablePanelGroup direction="horizontal" className="h-full">
              {/* 编辑器面板 */}
              <ResizablePanel
                defaultSize={45}
                minSize={25}
                maxSize={75}
                className="bg-card"
              >
                <EditorPanel
                  editorMode={editorMode}
                  setEditorMode={setEditorMode}
                  t={t}
                />
              </ResizablePanel>

              {/* 拖拽手柄 */}
              <ResizableHandle withHandle />

              {/* 预览面板 */}
              <ResizablePanel defaultSize={55} minSize={25}>
                <PreviewPanel t={t} />
              </ResizablePanel>
            </ResizablePanelGroup>
          )}
        </main>

        {/* 底部状态栏 */}
        <StatusBar />
      </div>
    </TooltipProvider>
  );
}

function isTextMeasurementMode(
  value: string | undefined
): value is TextMeasurementMode {
  return Boolean(value && TEXT_MEASUREMENT_VALUES.has(value as TextMeasurementMode));
}

function EditorPanel({
  editorMode,
  setEditorMode,
  t,
}: {
  editorMode: "code" | "config";
  setEditorMode(mode: "code" | "config"): void;
  t(key: string): string;
}) {
  return (
    <div className="h-full min-h-0 flex flex-col bg-card">
      <div className="flex items-center justify-between px-3 sm:px-4 py-2 border-b bg-muted/30">
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
        <Suspense fallback={<PanelLoading label={t("editor.loading")} />}>
          <CodeEditor className="min-h-0 flex-1" />
        </Suspense>
      ) : (
        <Suspense fallback={<PanelLoading label={t("editor.loading")} />}>
          <ConfigEditor className="min-h-0 flex-1" />
        </Suspense>
      )}
    </div>
  );
}

function PreviewPanel({ t }: { t(key: string): string }) {
  return (
    <div className="h-full min-h-0 flex flex-col">
      <div className="flex items-center justify-between px-3 sm:px-4 py-2 border-b bg-muted/30">
        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
          {t("preview.title")}
        </span>
        <span className="hidden text-xs text-muted-foreground sm:inline">
          {t("preview.wheelZoom")}
        </span>
      </div>
      <Suspense fallback={<PanelLoading label={t("preview.loading")} />}>
        <Preview className="min-h-0 flex-1 bg-[repeating-conic-gradient(#80808010_0%_25%,transparent_0%_50%)] bg-[length:20px_20px]" />
      </Suspense>
    </div>
  );
}

function PanelLoading({ label }: { label: string }) {
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center text-sm text-muted-foreground">
      {label}
    </div>
  );
}

function MobilePaneButton({
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
        "flex-1 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
        active
          ? "bg-background text-foreground shadow-sm"
          : "text-muted-foreground hover:bg-background/60 hover:text-foreground"
      )}
    >
      {children}
    </button>
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
