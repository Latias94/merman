import {
  lazy,
  Suspense,
  useEffect,
  useRef,
  useSyncExternalStore,
  type KeyboardEvent,
} from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Toolbar } from "./components/Toolbar";
import { StatusBar } from "./components/StatusBar";
import {
  useAppStore,
  type TextMeasurementMode,
  type WorkspacePane,
} from "./store";
import { isDiagramFont } from "./lib/diagram-font";
import { useShare } from "./hooks/useShare";
import { normalizeHostThemePresetName, normalizeThemeName } from "@mermanjs/web";
import { RenderCoordinatorBridge } from "@/src/runtime/RenderCoordinatorBridge";

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
    setHostThemePreset,
    setTextMeasurementMode,
    setDiagramFont,
    setMermaidConfig,
    editorMode,
    setEditorMode,
    workspacePane,
    setWorkspacePane,
  } = useAppStore(
    useShallow((state) => ({
      editorMode: state.editorMode,
      setCode: state.setCode,
      setDiagramFont: state.setDiagramFont,
      setDiagramTheme: state.setDiagramTheme,
      setEditorMode: state.setEditorMode,
      setHostThemePreset: state.setHostThemePreset,
      setMermaidConfig: state.setMermaidConfig,
      setTextMeasurementMode: state.setTextMeasurementMode,
      setWorkspacePane: state.setWorkspacePane,
      workspacePane: state.workspacePane,
    }))
  );
  const { initialData } = useShare();
  const isNarrowLayout = useNarrowLayout();

  useEffect(() => {
    const lang = i18n.language.startsWith("zh") ? "zh-CN" : "en";
    document.documentElement.lang = lang;
    document.title = t("app.title");
    document
      .querySelector('meta[name="description"]')
      ?.setAttribute("content", t("app.description"));
  }, [i18n.language, t]);

  // Apply shared inputs only after the URL payload has been decoded and validated.
  useEffect(() => {
    if (initialData) {
      setCode(initialData.code);
      if (initialData.theme) {
        setDiagramTheme(normalizeThemeName(initialData.theme));
      }
      if (initialData.hostThemePreset) {
        if (initialData.hostThemePreset === "none") {
          setHostThemePreset("none");
        } else {
          const preset = normalizeHostThemePresetName(initialData.hostThemePreset);
          if (preset) {
            setHostThemePreset(preset);
          }
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

  return (
    <TooltipProvider delayDuration={300}>
      <RenderCoordinatorBridge />
      <div className="flex h-[100dvh] min-h-0 flex-col overflow-hidden bg-background pt-[env(safe-area-inset-top)] pr-[env(safe-area-inset-right)] pb-[env(safe-area-inset-bottom)] pl-[env(safe-area-inset-left)]">
        <Toolbar />

        <main className="relative min-h-0 flex-1 overflow-hidden">
          <Suspense fallback={null}>
            <ExampleGallery />
          </Suspense>

          <div className="flex h-full min-h-0 flex-col overflow-hidden">
            {isNarrowLayout && (
              <WorkspaceTabs
                value={workspacePane}
                onValueChange={setWorkspacePane}
                editorLabel={t("layout.editor")}
                previewLabel={t("layout.preview")}
              />
            )}
            <ResizablePanelGroup direction="horizontal" className="min-h-0 flex-1">
              <ResizablePanel
                defaultSize={45}
                minSize={25}
                maxSize={75}
                className="bg-card"
                id={isNarrowLayout ? "workspace-editor-panel" : undefined}
                role={isNarrowLayout ? "tabpanel" : undefined}
                aria-labelledby={isNarrowLayout ? "workspace-editor-tab" : undefined}
                hidden={isNarrowLayout && workspacePane !== "editor"}
              >
                <EditorPanel
                  editorMode={editorMode}
                  setEditorMode={setEditorMode}
                  t={t}
                />
              </ResizablePanel>

              <ResizableHandle
                withHandle
                className={isNarrowLayout ? "hidden" : undefined}
              />

              <ResizablePanel
                defaultSize={55}
                minSize={25}
                id={isNarrowLayout ? "workspace-preview-panel" : undefined}
                role={isNarrowLayout ? "tabpanel" : undefined}
                aria-labelledby={isNarrowLayout ? "workspace-preview-tab" : undefined}
                hidden={isNarrowLayout && workspacePane !== "preview"}
              >
                <PreviewPanel t={t} />
              </ResizablePanel>
            </ResizablePanelGroup>
          </div>
        </main>

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
    <Tabs
      value={editorMode}
      onValueChange={(value) => setEditorMode(value as "code" | "config")}
      activationMode="manual"
      className="h-full min-h-0 gap-0 bg-card"
    >
      <div className="flex h-11 shrink-0 items-center justify-between border-b bg-muted/20 px-3 sm:px-4">
        <TabsList
          aria-label={t("layout.editor")}
          className="h-8 rounded-md bg-muted/70 p-0.5"
        >
          <TabsTrigger value="code" className="px-2.5 text-xs">
            {t("editor.codeMode")}
          </TabsTrigger>
          <TabsTrigger value="config" className="px-2.5 text-xs">
            {t("editor.configMode")}
          </TabsTrigger>
        </TabsList>
        <span className="text-xs text-muted-foreground">
          {editorMode === "code" ? "Mermaid" : "JSON"}
        </span>
      </div>
      <TabsContent
        value="code"
        forceMount
        className="mt-0 min-h-0 data-[state=inactive]:hidden"
      >
        <Suspense fallback={<PanelLoading label={t("editor.loading")} />}>
          <CodeEditor className="h-full min-h-0" />
        </Suspense>
      </TabsContent>
      <TabsContent
        value="config"
        forceMount
        className="mt-0 min-h-0 data-[state=inactive]:hidden"
      >
        <Suspense fallback={<PanelLoading label={t("editor.loading")} />}>
          <ConfigEditor className="h-full min-h-0" />
        </Suspense>
      </TabsContent>
    </Tabs>
  );
}

function PreviewPanel({ t }: { t(key: string): string }) {
  return (
    <div className="h-full min-h-0 flex flex-col">
      <div className="flex h-11 shrink-0 items-center justify-between border-b bg-muted/20 px-3 sm:px-4">
        <span className="text-xs font-medium text-muted-foreground">
          {t("preview.title")}
        </span>
        <span className="hidden text-xs text-muted-foreground sm:inline">
          {t("preview.wheelZoom")}
        </span>
      </div>
      <Suspense fallback={<PanelLoading label={t("preview.loading")} />}>
        <Preview className="min-h-0 flex-1 bg-[linear-gradient(to_right,var(--preview-grid)_1px,transparent_1px),linear-gradient(to_bottom,var(--preview-grid)_1px,transparent_1px)] bg-[size:20px_20px]" />
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

function WorkspaceTabs({
  value,
  onValueChange,
  editorLabel,
  previewLabel,
}: {
  value: WorkspacePane;
  onValueChange(value: WorkspacePane): void;
  editorLabel: string;
  previewLabel: string;
}) {
  const editorRef = useRef<HTMLButtonElement>(null);
  const previewRef = useRef<HTMLButtonElement>(null);
  const tabs = [
    { value: "editor" as const, label: editorLabel, ref: editorRef },
    { value: "preview" as const, label: previewLabel, ref: previewRef },
  ];

  const handleKeyDown = (
    event: KeyboardEvent<HTMLButtonElement>,
    index: number
  ) => {
    let nextIndex: number | null = null;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % tabs.length;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + tabs.length) % tabs.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = tabs.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    tabs[nextIndex]?.ref.current?.focus();
  };

  return (
    <div
      role="tablist"
      aria-label={`${editorLabel} / ${previewLabel}`}
      aria-orientation="horizontal"
      className="flex h-11 shrink-0 items-center gap-1 border-b bg-muted/20 p-1.5"
    >
      {tabs.map((tab, index) => (
        <button
          key={tab.value}
          ref={tab.ref}
          id={`workspace-${tab.value}-tab`}
          type="button"
          role="tab"
          tabIndex={value === tab.value ? 0 : -1}
          aria-selected={value === tab.value}
          aria-controls={`workspace-${tab.value}-panel`}
          onClick={() => onValueChange(tab.value)}
          onKeyDown={(event) => handleKeyDown(event, index)}
          className="flex-1 rounded-md px-3 py-1.5 text-sm font-medium text-muted-foreground transition-colors hover:bg-background/60 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring data-[active=true]:bg-background data-[active=true]:text-foreground data-[active=true]:shadow-sm"
          data-active={value === tab.value}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}

function subscribeNarrowLayout(onChange: () => void): () => void {
  const mediaQuery = window.matchMedia("(max-width: 767px)");
  mediaQuery.addEventListener("change", onChange);
  return () => mediaQuery.removeEventListener("change", onChange);
}

function getNarrowLayoutSnapshot(): boolean {
  return window.matchMedia("(max-width: 767px)").matches;
}

function useNarrowLayout(): boolean {
  return useSyncExternalStore(
    subscribeNarrowLayout,
    getNarrowLayoutSnapshot,
    () => false
  );
}
