import { useEffect } from "react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toolbar } from "./components/Toolbar";
import { CodeEditor } from "./components/Editor";
import { Preview } from "./components/Preview";
import { StatusBar } from "./components/StatusBar";
import { ExampleGallery } from "./components/ExampleGallery";
import { useAppStore } from "./store";
import { useShare } from "./hooks/useShare";
import { normalizeThemeName } from "@merman/web";

export default function App() {
  const { setCode, setDiagramTheme, uiTheme } = useAppStore();
  const { initialData } = useShare();

  // 从 URL 加载分享的数据
  useEffect(() => {
    if (initialData) {
      setCode(initialData.code);
      if (initialData.theme) {
        setDiagramTheme(normalizeThemeName(initialData.theme));
      }
    }
  }, [initialData, setCode, setDiagramTheme]);

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
                  <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                    编辑器
                  </span>
                  <span className="text-xs text-muted-foreground">
                    Mermaid 语法
                  </span>
                </div>
                <CodeEditor className="flex-1" />
              </div>
            </ResizablePanel>

            {/* 拖拽手柄 */}
            <ResizableHandle withHandle />

            {/* 预览面板 */}
            <ResizablePanel defaultSize={55} minSize={25}>
              <div className="h-full flex flex-col">
                <div className="flex items-center justify-between px-4 py-2 border-b bg-muted/30">
                  <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                    预览
                  </span>
                  <span className="text-xs text-muted-foreground">
                    滚轮缩放
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
