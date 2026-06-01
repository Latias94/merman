import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { useMerman } from "@/src/hooks/useMerman";
import { useAppStore } from "@/src/store";
import {
  preloadMermaid,
  renderMermaidSvg,
  MERMAID_JS_VERSION,
} from "@/src/lib/mermaid-renderer";
import { exportPNG, exportSVG } from "@/src/lib/export";
import {
  SvgViewport,
  useSvgViewport,
  type SvgViewportController,
} from "@/src/components/SvgViewport";
import { cn } from "@/lib/utils";
import {
  ZoomIn,
  ZoomOut,
  RotateCcw,
  Maximize2,
  Loader2,
  AlertCircle,
  Copy,
  Check,
  FileCode,
  ImageIcon,
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

type PreviewMode = "svg" | "ascii" | "compare" | "diagnostics";
type EngineKey = "merman" | "mermaid";
type DiagnosticKey = "parse" | "layout";

interface CompareArtifact {
  key: EngineKey;
  title: string;
  version: string;
  svg: string | null;
  error: string | null;
  renderTime: number | null;
  loading: boolean;
}

interface DiagnosticArtifact {
  json: string | null;
  error: string | null;
  elapsedMs: number | null;
}

const ASCII_SUPPORTED_TYPES = ["flowchart", "sequence", "class", "er", "xychart"];

const EMPTY_DIAGNOSTICS: Record<DiagnosticKey, DiagnosticArtifact> = {
  parse: { json: null, error: null, elapsedMs: null },
  layout: { json: null, error: null, elapsedMs: null },
};

export function Preview({ className }: PreviewProps) {
  const { t } = useTranslation();
  const {
    code,
    diagramTheme,
    mermaidConfig,
    setLastRenderTime,
    setDiagramType,
    isDarkMode,
  } = useAppStore();
  const { ready, loading, render, renderAscii, parseJson, layoutJson } = useMerman();
  const [svg, setSvg] = useState<string | null>(null);
  const [ascii, setAscii] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [previewMode, setPreviewMode] = useState<PreviewMode>("svg");
  const [copiedAscii, setCopiedAscii] = useState(false);
  const [copiedDiagnostic, setCopiedDiagnostic] = useState<DiagnosticKey | null>(null);
  const [copiedEngine, setCopiedEngine] = useState<EngineKey | null>(null);
  const [exportingEngine, setExportingEngine] = useState<EngineKey | null>(null);
  const [currentDiagramType, setCurrentDiagramType] = useState<string>("flowchart");
  const [diagnosticTab, setDiagnosticTab] = useState<DiagnosticKey>("parse");
  const [diagnostics, setDiagnostics] =
    useState<Record<DiagnosticKey, DiagnosticArtifact>>(EMPTY_DIAGNOSTICS);
  const [diagnosticsLoading, setDiagnosticsLoading] = useState(false);
  const [mermanRenderTime, setMermanRenderTime] = useState<number | null>(null);
  const [mermaidSvg, setMermaidSvg] = useState<string | null>(null);
  const [mermaidError, setMermaidError] = useState<string | null>(null);
  const [mermaidRenderTime, setMermaidRenderTime] = useState<number | null>(null);
  const [mermaidLoading, setMermaidLoading] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const diagnosticsDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const copyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const diagnosticCopyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const svgViewport = useSvgViewport({
    svg,
    enabled: previewMode === "svg",
  });
  const mermanCompareViewport = useSvgViewport({
    svg,
    enabled: previewMode === "compare",
  });
  const mermaidCompareViewport = useSvgViewport({
    svg: mermaidSvg,
    enabled: previewMode === "compare",
  });

  const detectDiagramType = useCallback((source: string): string => {
    const firstLine = source.trim().split("\n")[0]?.toLowerCase() || "";
    if (firstLine.startsWith("flowchart") || firstLine.startsWith("graph")) return "flowchart";
    if (firstLine.startsWith("sequencediagram")) return "sequence";
    if (firstLine.startsWith("classdiagram")) return "class";
    if (firstLine.startsWith("statediagram")) return "state";
    if (firstLine.startsWith("erdiagram")) return "er";
    if (firstLine.startsWith("gantt")) return "gantt";
    if (firstLine.startsWith("pie")) return "pie";
    if (firstLine.startsWith("mindmap")) return "mindmap";
    if (firstLine.startsWith("gitgraph")) return "gitgraph";
    if (firstLine.startsWith("timeline")) return "timeline";
    return "unknown";
  }, []);

  const isAsciiSupported = ASCII_SUPPORTED_TYPES.includes(currentDiagramType);
  const warmMermaidRenderer = useCallback(() => {
    void preloadMermaid();
  }, []);

  useEffect(() => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }

    debounceRef.current = setTimeout(() => {
      if (ready && code.trim()) {
        const diagramType = detectDiagramType(code);
        setCurrentDiagramType(diagramType);
        setDiagramType(diagramType);

        const result = render(code, diagramTheme, mermaidConfig);
        setSvg(result.svg);
        setError(result.error);
        setMermanRenderTime(result.error ? null : result.renderTime);
        setLastRenderTime(result.renderTime);

        if (ASCII_SUPPORTED_TYPES.includes(diagramType)) {
          setAscii(renderAscii(code, diagramTheme, mermaidConfig));
        } else {
          setAscii(null);
        }
      } else if (!code.trim()) {
        setSvg(null);
        setAscii(null);
        setError(null);
        setMermanRenderTime(null);
      }
    }, 300);

    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, [
    code,
    detectDiagramType,
    diagramTheme,
    mermaidConfig,
    ready,
    render,
    renderAscii,
    setDiagramType,
    setLastRenderTime,
  ]);

  useEffect(() => {
    if (!code.trim()) return;

    const timeout = window.setTimeout(warmMermaidRenderer, 600);
    return () => window.clearTimeout(timeout);
  }, [code, warmMermaidRenderer]);

  useEffect(() => {
    if (previewMode === "ascii" && !isAsciiSupported) {
      setPreviewMode("svg");
    }
  }, [isAsciiSupported, previewMode]);

  useEffect(() => {
    if (previewMode !== "compare" || !code.trim()) {
      setMermaidLoading(false);
      if (!code.trim()) {
        setMermaidSvg(null);
        setMermaidError(null);
        setMermaidRenderTime(null);
      }
      return;
    }

    let cancelled = false;
    setMermaidLoading(true);
    const timeout = setTimeout(() => {
      void renderMermaidSvg(code, diagramTheme, mermaidConfig).then((result) => {
        if (cancelled) return;
        setMermaidSvg(result.svg);
        setMermaidError(result.error);
        setMermaidRenderTime(result.renderTime);
        setMermaidLoading(false);
      });
    }, 300);

    return () => {
      cancelled = true;
      clearTimeout(timeout);
    };
  }, [code, diagramTheme, mermaidConfig, previewMode]);

  useEffect(() => {
    if (diagnosticsDebounceRef.current) {
      clearTimeout(diagnosticsDebounceRef.current);
    }

    if (previewMode !== "diagnostics") {
      setDiagnosticsLoading(false);
      return;
    }

    if (!code.trim()) {
      setDiagnostics(EMPTY_DIAGNOSTICS);
      setDiagnosticsLoading(false);
      return;
    }

    if (!ready) {
      setDiagnostics(
        diagnosticsError(
          loading ? t("preview.loading") : t("preview.diagnosticsUnavailable")
        )
      );
      setDiagnosticsLoading(false);
      return;
    }

    setDiagnosticsLoading(true);
    diagnosticsDebounceRef.current = setTimeout(() => {
      setDiagnostics({
        parse: collectDiagnostic(() => parseJson(code, diagramTheme, mermaidConfig)),
        layout: collectDiagnostic(() => layoutJson(code, diagramTheme, mermaidConfig)),
      });
      setDiagnosticsLoading(false);
    }, 300);

    return () => {
      if (diagnosticsDebounceRef.current) {
        clearTimeout(diagnosticsDebounceRef.current);
      }
    };
  }, [
    code,
    diagramTheme,
    layoutJson,
    loading,
    mermaidConfig,
    parseJson,
    previewMode,
    ready,
    t,
  ]);

  useEffect(() => {
    return () => {
      if (copyTimeoutRef.current) {
        clearTimeout(copyTimeoutRef.current);
      }
      if (diagnosticCopyTimeoutRef.current) {
        clearTimeout(diagnosticCopyTimeoutRef.current);
      }
    };
  }, []);

  const handleCopyAscii = useCallback(async () => {
    if (!ascii) return;

    try {
      await navigator.clipboard.writeText(ascii);
      setCopiedAscii(true);
      setTimeout(() => setCopiedAscii(false), 2000);
    } catch (err) {
      console.error("Failed to copy ASCII:", err);
    }
  }, [ascii]);

  const handleCopySvg = useCallback(async (engine: EngineKey, value: string | null) => {
    if (!value) return;

    try {
      await navigator.clipboard.writeText(value);
      setCopiedEngine(engine);
      if (copyTimeoutRef.current) {
        clearTimeout(copyTimeoutRef.current);
      }
      copyTimeoutRef.current = setTimeout(() => setCopiedEngine(null), 2000);
    } catch (err) {
      console.error("Failed to copy SVG:", err);
    }
  }, []);

  const handleCopyDiagnosticJson = useCallback(async () => {
    const json = diagnostics[diagnosticTab].json;
    if (!json) return;

    try {
      await navigator.clipboard.writeText(json);
      setCopiedDiagnostic(diagnosticTab);
      if (diagnosticCopyTimeoutRef.current) {
        clearTimeout(diagnosticCopyTimeoutRef.current);
      }
      diagnosticCopyTimeoutRef.current = setTimeout(
        () => setCopiedDiagnostic(null),
        2000
      );
    } catch (err) {
      console.error("Failed to copy diagnostics JSON:", err);
    }
  }, [diagnosticTab, diagnostics]);

  const handleExportSvg = useCallback((engine: EngineKey, value: string | null) => {
    if (!value) return;
    exportSVG(value, `merman-compare-${engine}`);
  }, []);

  const handleExportPng = useCallback(async (engine: EngineKey, value: string | null) => {
    if (!value) return;

    setExportingEngine(engine);
    try {
      await exportPNG(value, `merman-compare-${engine}`, 2);
    } catch (err) {
      console.error("Failed to export PNG:", err);
    } finally {
      setExportingEngine(null);
    }
  }, []);

  const renderTabBar = (rightContent?: ReactNode) => (
    <TabBar
      mode={previewMode}
      onModeChange={setPreviewMode}
      onCompareWarmup={warmMermaidRenderer}
      isAsciiSupported={isAsciiSupported}
      t={t}
      rightContent={rightContent}
    />
  );

  if (loading) {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        {renderTabBar()}
        <CenteredMessage icon={<Loader2 className="size-8 animate-spin" />}>
          {t("preview.loading")}
        </CenteredMessage>
      </div>
    );
  }

  if (!code.trim()) {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        {renderTabBar()}
        <div className="flex-1 flex items-center justify-center">
          <div className="text-center text-muted-foreground">
            <p className="text-sm">{t("preview.empty")}</p>
            <p className="text-xs mt-1">{t("preview.emptyHint")}</p>
          </div>
        </div>
      </div>
    );
  }

  if (error && previewMode !== "compare" && previewMode !== "diagnostics") {
    return (
      <div className={cn("flex flex-col h-full", className)}>
        {renderTabBar()}
        <RenderError message={error} t={t} />
      </div>
    );
  }

  const mermanArtifact: CompareArtifact = {
    key: "merman",
    title: t("preview.mermanEngine"),
    version: "WASM",
    svg,
    error,
    renderTime: svg ? mermanRenderTime : null,
    loading: false,
  };
  const mermaidArtifact: CompareArtifact = {
    key: "mermaid",
    title: t("preview.mermaidEngine"),
    version: MERMAID_JS_VERSION,
    svg: mermaidSvg,
    error: mermaidError,
    renderTime: mermaidRenderTime,
    loading: mermaidLoading,
  };

  return (
    <div className={cn("flex flex-col h-full", className)}>
      {renderTabBar(
        <>
          {previewMode === "svg" && (
            <ViewportControls controller={svgViewport} t={t} />
          )}
          {previewMode === "ascii" && ascii && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" size="icon-sm" onClick={handleCopyAscii}>
                  {copiedAscii ? (
                    <Check className="size-4 text-green-500" />
                  ) : (
                    <Copy className="size-4" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {copiedAscii ? t("preview.copied") : t("preview.copyAscii")}
              </TooltipContent>
            </Tooltip>
          )}
          {previewMode === "diagnostics" && (
            <IconButton
              label={
                copiedDiagnostic === diagnosticTab
                  ? t("preview.copied")
                  : t("preview.copyJson")
              }
              onClick={handleCopyDiagnosticJson}
              disabled={diagnosticsLoading || !diagnostics[diagnosticTab].json}
            >
              {copiedDiagnostic === diagnosticTab ? (
                <Check className="size-4 text-green-500" />
              ) : diagnosticsLoading ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <Copy className="size-4" />
              )}
            </IconButton>
          )}
        </>
      )}

      <div className="flex-1 min-h-0 relative overflow-hidden">
        {previewMode === "svg" && (
          <SvgViewport svg={svg} controller={svgViewport} />
        )}

        {previewMode === "compare" && (
          <CompareView
            mermanArtifact={mermanArtifact}
            mermaidArtifact={mermaidArtifact}
            mermanController={mermanCompareViewport}
            mermaidController={mermaidCompareViewport}
            copiedEngine={copiedEngine}
            exportingEngine={exportingEngine}
            onCopySvg={handleCopySvg}
            onExportSvg={handleExportSvg}
            onExportPng={handleExportPng}
            t={t}
          />
        )}

        {previewMode === "diagnostics" && (
          <DiagnosticsView
            activeTab={diagnosticTab}
            diagnostics={diagnostics}
            loading={diagnosticsLoading}
            isDarkMode={isDarkMode}
            onActiveTabChange={setDiagnosticTab}
            t={t}
          />
        )}

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

interface TabBarProps {
  mode: PreviewMode;
  onModeChange: (mode: PreviewMode) => void;
  onCompareWarmup: () => void;
  isAsciiSupported: boolean;
  t: (key: string) => string;
  rightContent?: ReactNode;
}

function TabBar({
  mode,
  onModeChange,
  onCompareWarmup,
  isAsciiSupported,
  t,
  rightContent,
}: TabBarProps) {
  return (
    <div className="flex items-center justify-between h-10 px-2 border-b bg-muted/30 shrink-0">
      <div className="flex items-center gap-1">
        <TabButton active={mode === "svg"} onClick={() => onModeChange("svg")}>
          SVG
        </TabButton>
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
                !isAsciiSupported &&
                  "opacity-50 cursor-not-allowed hover:bg-transparent hover:text-muted-foreground"
              )}
            >
              ASCII
            </button>
          </TooltipTrigger>
          {!isAsciiSupported && (
            <TooltipContent>{t("preview.asciiNotSupported")}</TooltipContent>
          )}
        </Tooltip>
        <TabButton
          active={mode === "compare"}
          onClick={() => onModeChange("compare")}
          onFocus={onCompareWarmup}
          onPointerEnter={onCompareWarmup}
        >
          {t("preview.compareMode")}
        </TabButton>
        <TabButton
          active={mode === "diagnostics"}
          onClick={() => onModeChange("diagnostics")}
        >
          {t("preview.diagnosticsMode")}
        </TabButton>
      </div>

      <div className="flex items-center gap-1">{rightContent}</div>
    </div>
  );
}

interface TabButtonProps {
  active: boolean;
  onClick(): void;
  onFocus?: () => void;
  onPointerEnter?: () => void;
  children: ReactNode;
}

function TabButton({
  active,
  onClick,
  onFocus,
  onPointerEnter,
  children,
}: TabButtonProps) {
  return (
    <button
      onClick={onClick}
      onFocus={onFocus}
      onPointerEnter={onPointerEnter}
      className={cn(
        "px-3 py-1.5 text-sm rounded-md transition-colors",
        active
          ? "bg-background text-foreground shadow-sm font-medium"
          : "text-muted-foreground hover:text-foreground hover:bg-background/50"
      )}
    >
      {children}
    </button>
  );
}

function DiagnosticsView({
  activeTab,
  diagnostics,
  loading,
  isDarkMode,
  onActiveTabChange,
  t,
}: {
  activeTab: DiagnosticKey;
  diagnostics: Record<DiagnosticKey, DiagnosticArtifact>;
  loading: boolean;
  isDarkMode: boolean;
  onActiveTabChange(tab: DiagnosticKey): void;
  t: (key: string) => string;
}) {
  const current = diagnostics[activeTab];

  return (
    <div className="flex h-full flex-col bg-background">
      <div className="flex min-h-10 items-center justify-between gap-2 border-b bg-muted/20 px-3 py-2">
        <div className="flex min-w-0 items-center gap-1 overflow-x-auto">
          <TabButton
            active={activeTab === "parse"}
            onClick={() => onActiveTabChange("parse")}
          >
            {t("preview.parseJson")}
          </TabButton>
          <TabButton
            active={activeTab === "layout"}
            onClick={() => onActiveTabChange("layout")}
          >
            {t("preview.layoutJson")}
          </TabButton>
        </div>
        <p className="shrink-0 text-xs tabular-nums text-muted-foreground">
          {loading
            ? t("preview.runningDiagnostics")
            : current.elapsedMs !== null
              ? `${current.elapsedMs.toFixed(1)}ms`
              : "-"}
        </p>
      </div>

      <div className="min-h-0 flex-1">
        {loading ? (
          <CenteredMessage icon={<Loader2 className="size-6 animate-spin" />}>
            {t("preview.runningDiagnostics")}
          </CenteredMessage>
        ) : current.error ? (
          <RenderError message={current.error} t={t} compact />
        ) : current.json ? (
          <Editor
            height="100%"
            language="json"
            value={current.json}
            theme={isDarkMode ? "vs-dark" : "light"}
            options={{
              readOnly: true,
              domReadOnly: true,
              minimap: { enabled: false },
              fontSize: 13,
              fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
              scrollBeyondLastLine: false,
              wordWrap: "on",
              renderLineHighlight: "none",
              selectionHighlight: false,
              occurrencesHighlight: "off",
              folding: true,
              automaticLayout: true,
              padding: { top: 16, bottom: 16 },
            }}
          />
        ) : (
          <CenteredMessage icon={<FileCode className="size-8" />}>
            {t("preview.diagnosticsEmpty")}
          </CenteredMessage>
        )}
      </div>
    </div>
  );
}

function ViewportControls({
  controller,
  t,
}: {
  controller: SvgViewportController;
  t: (key: string) => string;
}) {
  return (
    <div className="flex items-center gap-1">
      <IconButton label={t("preview.zoomOut")} onClick={controller.zoomOut}>
        <ZoomOut className="size-4" />
      </IconButton>
      <span className="text-xs text-muted-foreground w-12 text-center tabular-nums">
        {Math.round(controller.zoom * 100)}%
      </span>
      <IconButton label={t("preview.zoomIn")} onClick={controller.zoomIn}>
        <ZoomIn className="size-4" />
      </IconButton>
      <IconButton label={t("preview.fitToView")} onClick={controller.fitToView}>
        <Maximize2 className="size-4" />
      </IconButton>
      <IconButton label={t("preview.reset")} onClick={controller.reset}>
        <RotateCcw className="size-4" />
      </IconButton>
    </div>
  );
}

function CompareView({
  mermanArtifact,
  mermaidArtifact,
  mermanController,
  mermaidController,
  copiedEngine,
  exportingEngine,
  onCopySvg,
  onExportSvg,
  onExportPng,
  t,
}: {
  mermanArtifact: CompareArtifact;
  mermaidArtifact: CompareArtifact;
  mermanController: SvgViewportController;
  mermaidController: SvgViewportController;
  copiedEngine: EngineKey | null;
  exportingEngine: EngineKey | null;
  onCopySvg(engine: EngineKey, svg: string | null): void;
  onExportSvg(engine: EngineKey, svg: string | null): void;
  onExportPng(engine: EngineKey, svg: string | null): void;
  t: (key: string) => string;
}) {
  return (
    <div className="h-full overflow-auto p-3">
      <div className="grid min-h-full grid-cols-1 gap-3 xl:grid-cols-2">
        <ComparePane
          artifact={mermanArtifact}
          controller={mermanController}
          copied={copiedEngine === "merman"}
          exporting={exportingEngine === "merman"}
          onCopySvg={onCopySvg}
          onExportSvg={onExportSvg}
          onExportPng={onExportPng}
          t={t}
        />
        <ComparePane
          artifact={mermaidArtifact}
          controller={mermaidController}
          copied={copiedEngine === "mermaid"}
          exporting={exportingEngine === "mermaid"}
          onCopySvg={onCopySvg}
          onExportSvg={onExportSvg}
          onExportPng={onExportPng}
          t={t}
        />
      </div>
    </div>
  );
}

function ComparePane({
  artifact,
  controller,
  copied,
  exporting,
  onCopySvg,
  onExportSvg,
  onExportPng,
  t,
}: {
  artifact: CompareArtifact;
  controller: SvgViewportController;
  copied: boolean;
  exporting: boolean;
  onCopySvg(engine: EngineKey, svg: string | null): void;
  onExportSvg(engine: EngineKey, svg: string | null): void;
  onExportPng(engine: EngineKey, svg: string | null): void;
  t: (key: string) => string;
}) {
  const hasSvg = Boolean(artifact.svg);

  return (
    <section className="flex min-h-[320px] flex-col overflow-hidden rounded-md border bg-background xl:min-h-0">
      <div className="border-b bg-muted/30 px-3 py-2">
        <div className="flex items-center justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2">
            <span className="truncate text-sm font-medium">{artifact.title}</span>
            <span className="shrink-0 rounded-sm bg-muted px-1.5 py-0.5 text-[11px] text-muted-foreground">
              {artifact.version}
            </span>
          </div>
          <p className="shrink-0 text-xs text-muted-foreground">
            {artifact.loading
              ? t("preview.loadingMermaid")
              : artifact.renderTime !== null
                ? `${artifact.renderTime.toFixed(1)}ms`
                : "-"}
          </p>
        </div>
        <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
          {hasSvg && <ViewportControls controller={controller} t={t} />}
          {!hasSvg && <div />}
          <div className="flex items-center gap-1">
            <IconButton
              label={copied ? t("preview.copied") : t("preview.copySvg")}
              onClick={() => onCopySvg(artifact.key, artifact.svg)}
              disabled={!hasSvg}
            >
              {copied ? (
                <Check className="size-4 text-green-500" />
              ) : (
                <Copy className="size-4" />
              )}
            </IconButton>
            <IconButton
              label={t("preview.exportSvg")}
              onClick={() => onExportSvg(artifact.key, artifact.svg)}
              disabled={!hasSvg}
            >
              <FileCode className="size-4" />
            </IconButton>
            <IconButton
              label={t("preview.exportPng")}
              onClick={() => onExportPng(artifact.key, artifact.svg)}
              disabled={!hasSvg || exporting}
            >
              {exporting ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <ImageIcon className="size-4" />
              )}
            </IconButton>
          </div>
        </div>
      </div>
      <div className="min-h-0 flex-1">
        {artifact.loading ? (
          <CenteredMessage icon={<Loader2 className="size-6 animate-spin" />}>
            {t("preview.loadingMermaid")}
          </CenteredMessage>
        ) : artifact.error ? (
          <RenderError message={artifact.error} t={t} compact />
        ) : (
          <SvgViewport
            svg={artifact.svg}
            controller={controller}
            empty={
              <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
                {t("preview.empty")}
              </div>
            }
          />
        )}
      </div>
    </section>
  );
}

function IconButton({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick(): void;
  disabled?: boolean;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon-sm" onClick={onClick} disabled={disabled}>
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

function CenteredMessage({
  icon,
  children,
}: {
  icon: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full flex-1 items-center justify-center">
      <div className="flex flex-col items-center gap-3 text-muted-foreground">
        {icon}
        <span className="text-sm">{children}</span>
      </div>
    </div>
  );
}

function RenderError({
  message,
  t,
  compact = false,
}: {
  message: string;
  t: (key: string) => string;
  compact?: boolean;
}) {
  return (
    <div className="flex h-full flex-1 items-center justify-center p-6">
      <div className={cn("text-center", compact ? "max-w-sm" : "max-w-md")}>
        <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-destructive/10">
          <AlertCircle className="size-6 text-destructive" />
        </div>
        <h3 className="mb-2 font-medium text-foreground">{t("preview.error")}</h3>
        <p className="rounded-md bg-muted/50 p-3 font-mono text-sm text-muted-foreground">
          {message}
        </p>
      </div>
    </div>
  );
}

function collectDiagnostic(createJson: () => string): DiagnosticArtifact {
  const start = performance.now();

  try {
    const json = formatDiagnosticJson(createJson());
    return {
      json,
      error: null,
      elapsedMs: performance.now() - start,
    };
  } catch (err) {
    return {
      json: null,
      error: err instanceof Error ? err.message : String(err),
      elapsedMs: null,
    };
  }
}

function diagnosticsError(message: string): Record<DiagnosticKey, DiagnosticArtifact> {
  return {
    parse: { json: null, error: message, elapsedMs: null },
    layout: { json: null, error: message, elapsedMs: null },
  };
}

function formatDiagnosticJson(rawJson: string): string {
  try {
    return `${JSON.stringify(JSON.parse(rawJson), null, 2)}\n`;
  } catch {
    return rawJson;
  }
}
