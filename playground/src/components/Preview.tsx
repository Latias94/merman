import { useEffect, useState, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useMerman } from "@/src/hooks/useMerman";
import { useAppStore } from "@/src/store";
import { cn } from "@/lib/utils";
import {
  ZoomIn,
  ZoomOut,
  RotateCcw,
  Maximize2,
  Loader2,
  AlertCircle,
  Copy,
  Check
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import Editor from "@monaco-editor/react";

interface PreviewProps {
  className?: string;
}

type PreviewMode = "svg" | "ascii";

// ASCII 支持的图表类型
const ASCII_SUPPORTED_TYPES = ["flowchart", "sequence", "class", "er", "xychart"];

export function Preview({ className }: PreviewProps) {
  const { t } = useTranslation();
  const { code, diagramTheme, setLastRenderTime, setDiagramType, isDarkMode } = useAppStore();
  const { ready, loading, render, renderAscii } = useMerman();
  const [svg, setSvg] = useState<string | null>(null);
  const [ascii, setAscii] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const [isAutoFit, setIsAutoFit] = useState(true);
  const [previewMode, setPreviewMode] = useState<PreviewMode>("svg");
  const [copied, setCopied] = useState(false);
  const [currentDiagramType, setCurrentDiagramType] = useState<string>("flowchart");
  const containerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<NodeJS.Timeout | null>(null);

  // 检测图表类型
  const detectDiagramType = useCallback((code: string): string => {
    const firstLine = code.trim().split('\n')[0]?.toLowerCase() || '';
    if (firstLine.startsWith('flowchart') || firstLine.startsWith('graph')) return 'flowchart';
    if (firstLine.startsWith('sequencediagram')) return 'sequence';
    if (firstLine.startsWith('classdiagram')) return 'class';
    if (firstLine.startsWith('statediagram')) return 'state';
    if (firstLine.startsWith('erdiagram')) return 'er';
    if (firstLine.startsWith('gantt')) return 'gantt';
    if (firstLine.startsWith('pie')) return 'pie';
    if (firstLine.startsWith('mindmap')) return 'mindmap';
    if (firstLine.startsWith('gitgraph')) return 'gitgraph';
    if (firstLine.startsWith('timeline')) return 'timeline';
    return 'unknown';
  }, []);

  // 检查是否支持 ASCII
  const isAsciiSupported = ASCII_SUPPORTED_TYPES.includes(currentDiagramType);

  // 防抖渲染
  useEffect(() => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }

    debounceRef.current = setTimeout(() => {
      if (ready && code.trim()) {
        // 检测图表类型
        const diagramType = detectDiagramType(code);
        setCurrentDiagramType(diagramType);
        setDiagramType(diagramType);

        // 渲染 SVG
        const result = render(code, diagramTheme);
        setSvg(result.svg);
        setError(result.error);
        setLastRenderTime(result.renderTime);

        // 如果支持 ASCII，也渲染 ASCII
        if (ASCII_SUPPORTED_TYPES.includes(diagramType)) {
          const asciiResult = renderAscii(code);
          setAscii(asciiResult);
        } else {
          setAscii(null);
        }
      } else if (!code.trim()) {
        setSvg(null);
        setAscii(null);
        setError(null);
      }
    }, 300);

    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, [code, diagramTheme, ready, render, renderAscii, setLastRenderTime, setDiagramType, detectDiagramType]);

  // 如果切换到 ASCII 模式但不支持，自动切回 SVG
  useEffect(() => {
    if (previewMode === "ascii" && !isAsciiSupported) {
      setPreviewMode("svg");
    }
  }, [previewMode, isAsciiSupported]);

  const handleZoomIn = useCallback(() => {
    setIsAutoFit(false);
    setZoom((z) => Math.min(z * 1.2, 5));
  }, []);

  const handleZoomOut = useCallback(() => {
    setIsAutoFit(false);
    setZoom((z) => Math.max(z / 1.2, 0.1));
  }, []);

  const handleReset = useCallback(() => {
    setIsAutoFit(false);
    setZoom(1);
    setPosition({ x: 0, y: 0 });
  }, []);

  const fitToView = useCallback(() => {
    const container = containerRef.current;
    const content = contentRef.current;
    if (!container || !content) return;

    const contentWidth = content.offsetWidth;
    const contentHeight = content.offsetHeight;
    if (contentWidth <= 0 || contentHeight <= 0) return;

    const availableWidth = Math.max(container.clientWidth - 48, 1);
    const availableHeight = Math.max(container.clientHeight - 48, 1);
    const nextZoom = Math.max(
      0.1,
      Math.min(1, availableWidth / contentWidth, availableHeight / contentHeight)
    );

    setZoom(Number(nextZoom.toFixed(3)));
    setPosition({ x: 0, y: 0 });
  }, []);

  const handleFitToView = useCallback(() => {
    setIsAutoFit(true);
    fitToView();
  }, [fitToView]);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    setIsAutoFit(false);
    const delta = Math.exp(-e.deltaY * 0.001);
    setZoom((z) => Math.max(0.1, Math.min(5, z * delta)));
  }, []);

  const handlePointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (e.button === 0) {
        e.preventDefault();
        window.getSelection()?.removeAllRanges();
        e.currentTarget.setPointerCapture(e.pointerId);
        setIsAutoFit(false);
        setIsDragging(true);
        setDragStart({ x: e.clientX - position.x, y: e.clientY - position.y });
      }
    },
    [position]
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (isDragging) {
        e.preventDefault();
        window.getSelection()?.removeAllRanges();
        setPosition({
          x: e.clientX - dragStart.x,
          y: e.clientY - dragStart.y,
        });
      }
    },
    [isDragging, dragStart]
  );

  const handlePointerUp = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    if (isDragging && e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    setIsDragging(false);
  }, [isDragging]);

  useEffect(() => {
    if (previewMode !== "svg" || !svg) return;

    setIsAutoFit(true);
    const frame = requestAnimationFrame(fitToView);
    return () => cancelAnimationFrame(frame);
  }, [svg, previewMode, fitToView]);

  useEffect(() => {
    if (previewMode !== "svg" || !svg || !isAutoFit) return;

    const container = containerRef.current;
    if (!container || typeof ResizeObserver === "undefined") return;

    let frame = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(fitToView);
    });

    observer.observe(container);

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [svg, previewMode, isAutoFit, fitToView]);

  const handleCopyAscii = useCallback(async () => {
    if (ascii) {
      try {
        await navigator.clipboard.writeText(ascii);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      } catch (err) {
        console.error("Failed to copy:", err);
      }
    }
  }, [ascii]);

  // 加载状态
  if (loading) {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        <TabBar
          mode={previewMode}
          onModeChange={setPreviewMode}
          isAsciiSupported={false}
          t={t}
        />
        <div className="flex-1 flex items-center justify-center">
          <div className="flex flex-col items-center gap-3 text-muted-foreground">
            <Loader2 className="size-8 animate-spin" />
            <span className="text-sm">{t("preview.loading")}</span>
          </div>
        </div>
      </div>
    );
  }

  // 空状态
  if (!code.trim()) {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        <TabBar
          mode={previewMode}
          onModeChange={setPreviewMode}
          isAsciiSupported={false}
          t={t}
        />
        <div className="flex-1 flex items-center justify-center">
          <div className="text-center text-muted-foreground">
            <p className="text-sm">{t("preview.empty")}</p>
            <p className="text-xs mt-1">{t("preview.emptyHint")}</p>
          </div>
        </div>
      </div>
    );
  }

  // 错误状态
  if (error) {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        <TabBar
          mode={previewMode}
          onModeChange={setPreviewMode}
          isAsciiSupported={isAsciiSupported}
          t={t}
        />
        <div className="flex-1 flex items-center justify-center p-6">
          <div className="max-w-md text-center">
            <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-destructive/10">
              <AlertCircle className="size-6 text-destructive" />
            </div>
            <h3 className="font-medium text-foreground mb-2">{t("preview.error")}</h3>
            <p className="text-sm text-muted-foreground font-mono bg-muted/50 p-3 rounded-md">
              {error}
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={cn("flex flex-col h-full", className)}>
      {/* 顶部标签栏 */}
      <TabBar
        mode={previewMode}
        onModeChange={setPreviewMode}
        isAsciiSupported={isAsciiSupported}
        t={t}
        rightContent={
          <>
            {/* SVG 模式的缩放控制 */}
            {previewMode === "svg" && (
              <div className="flex items-center gap-1">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button variant="ghost" size="icon-sm" onClick={handleZoomOut}>
                      <ZoomOut className="size-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("preview.zoomOut")}</TooltipContent>
                </Tooltip>
                <span className="text-xs text-muted-foreground w-12 text-center tabular-nums">
                  {Math.round(zoom * 100)}%
                </span>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button variant="ghost" size="icon-sm" onClick={handleZoomIn}>
                      <ZoomIn className="size-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("preview.zoomIn")}</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button variant="ghost" size="icon-sm" onClick={handleFitToView}>
                      <Maximize2 className="size-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("preview.fitToView")}</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button variant="ghost" size="icon-sm" onClick={handleReset}>
                      <RotateCcw className="size-4" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("preview.reset")}</TooltipContent>
                </Tooltip>
              </div>
            )}

            {/* ASCII 模式的复制按钮 */}
            {previewMode === "ascii" && ascii && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="ghost" size="icon-sm" onClick={handleCopyAscii}>
                    {copied ? <Check className="size-4 text-green-500" /> : <Copy className="size-4" />}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  {copied ? t("preview.copied") : t("preview.copyAscii")}
                </TooltipContent>
              </Tooltip>
            )}
          </>
        }
      />

      {/* 预览内容区 */}
      <div className="flex-1 min-h-0 relative overflow-hidden">
        {/* SVG 预览模式 */}
        {previewMode === "svg" && (
          <div
            ref={containerRef}
            className={cn(
              "relative h-full w-full overflow-hidden cursor-grab select-none touch-none",
              isDragging && "cursor-grabbing"
            )}
            onWheel={handleWheel}
            onPointerDown={handlePointerDown}
            onPointerMove={handlePointerMove}
            onPointerUp={handlePointerUp}
            onPointerCancel={handlePointerUp}
            onDragStart={(event) => event.preventDefault()}
          >
            <div
              className="absolute left-1/2 top-1/2 will-change-transform"
              style={{
                transform: `translate3d(${position.x}px, ${position.y}px, 0)`,
              }}
            >
              <div
                className="will-change-transform"
                style={{
                  transform: `translate(-50%, -50%) scale(${zoom})`,
                  transformOrigin: "center center",
                }}
              >
                {svg && (
                  <div
                    ref={contentRef}
                    className="preview-container inline-flex bg-white rounded-lg shadow-sm p-4"
                    dangerouslySetInnerHTML={{ __html: svg }}
                  />
                )}
              </div>
            </div>
          </div>
        )}

        {/* ASCII 预览模式 */}
        {previewMode === "ascii" && (
          <div className="h-full w-full">
            {ascii ? (
              <Editor
                height="100%"
                language="plaintext"
                value={ascii}
                theme={isDarkMode ? "vs-dark" : "light"}
                options={{
                  readOnly: true,
                  minimap: { enabled: false },
                  fontSize: 13,
                  fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
                  lineNumbers: "off",
                  scrollBeyondLastLine: false,
                  wordWrap: "off",
                  renderLineHighlight: "none",
                  selectionHighlight: false,
                  occurrencesHighlight: "off",
                  folding: false,
                  padding: { top: 16, bottom: 16 },
                  domReadOnly: true,
                }}
              />
            ) : (
              <div className="flex items-center justify-center h-full text-muted-foreground">
                <p>{t("preview.asciiNotAvailable")}</p>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// 标签栏组件
interface TabBarProps {
  mode: PreviewMode;
  onModeChange: (mode: PreviewMode) => void;
  isAsciiSupported: boolean;
  t: (key: string) => string;
  rightContent?: React.ReactNode;
}

function TabBar({ mode, onModeChange, isAsciiSupported, t, rightContent }: TabBarProps) {
  return (
    <div className="flex items-center justify-between h-10 px-2 border-b bg-muted/30 shrink-0">
      {/* 左侧标签 */}
      <div className="flex items-center gap-1">
        <button
          onClick={() => onModeChange("svg")}
          className={cn(
            "px-3 py-1.5 text-sm rounded-md transition-colors",
            mode === "svg"
              ? "bg-background text-foreground shadow-sm font-medium"
              : "text-muted-foreground hover:text-foreground hover:bg-background/50"
          )}
        >
          SVG
        </button>
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              onClick={() => isAsciiSupported && onModeChange("ascii")}
              disabled={!isAsciiSupported}
              className={cn(
                "px-3 py-1.5 text-sm rounded-md transition-colors",
                mode === "ascii"
                  ? "bg-background text-foreground shadow-sm font-medium"
                  : "text-muted-foreground hover:text-foreground hover:bg-background/50",
                !isAsciiSupported && "opacity-50 cursor-not-allowed hover:bg-transparent hover:text-muted-foreground"
              )}
            >
              ASCII
            </button>
          </TooltipTrigger>
          {!isAsciiSupported && (
            <TooltipContent>{t("preview.asciiNotSupported")}</TooltipContent>
          )}
        </Tooltip>
      </div>

      {/* 右侧工具 */}
      <div className="flex items-center gap-1">
        {rightContent}
      </div>
    </div>
  );
}
