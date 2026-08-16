import {
  lazy,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type KeyboardEvent,
} from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { AlertTriangle } from "lucide-react";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Toolbar } from "./components/Toolbar";
import { StatusBar } from "./components/StatusBar";
import { CodeEditor } from "./components/Editor";
import { Preview } from "./components/Preview";
import { RenderViewportControl } from "./components/RenderViewportControl";
import { ExportWorkbench } from "./components/ExportDialog";
import { LazyFeatureBoundary } from "./components/LazyFeatureBoundary";
import { useAppStore, type WorkspacePane } from "./store";
import { RenderCoordinatorBridge } from "@/src/runtime/RenderCoordinatorBridge";

const ConfigEditor = lazy(() =>
  import("./components/ConfigEditorFeature").then((module) => ({
    default: module.ConfigEditor,
  }))
);
export default function App() {
  const { t, i18n } = useTranslation();
  const {
    editorMode,
    setEditorMode,
    shareViewWarning,
    workspacePane,
    setWorkspacePane,
  } = useAppStore(
    useShallow((state) => ({
      editorMode: state.editorMode,
      setEditorMode: state.setEditorMode,
      shareViewWarning: state.shareViewWarning,
      setWorkspacePane: state.setWorkspacePane,
      workspacePane: state.workspacePane,
    }))
  );
  const isNarrowLayout = useNarrowLayout();

  useEffect(() => {
    const lang = i18n.language.startsWith("zh") ? "zh-CN" : "en";
    document.documentElement.lang = lang;
    document.title = t("app.title");
    document
      .querySelector('meta[name="description"]')
      ?.setAttribute("content", t("app.description"));
  }, [i18n.language, t]);

  return (
    <TooltipProvider delayDuration={300}>
      <ExportWorkbench>
        <RenderCoordinatorBridge />
        <div className="flex h-[100dvh] min-h-0 flex-col overflow-hidden bg-background pt-[env(safe-area-inset-top)] pr-[env(safe-area-inset-right)] pb-[env(safe-area-inset-bottom)] pl-[env(safe-area-inset-left)]">
          <Toolbar />

          {shareViewWarning && (
            <div
              role="alert"
              data-testid="share-view-warning"
              className="flex shrink-0 items-start gap-2 border-b border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-950 dark:text-amber-100 sm:px-4"
            >
              <AlertTriangle className="mt-0.5 size-3.5 shrink-0" />
              <span>{t("share.issueViewNotRestored")}</span>
            </div>
          )}

          <main className="relative min-h-0 flex-1 overflow-hidden">
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
                  defaultSize="45%"
                  minSize="25%"
                  maxSize="75%"
                  className="bg-card"
                  id={isNarrowLayout ? "workspace-editor-panel" : undefined}
                  role={isNarrowLayout ? "tabpanel" : undefined}
                  aria-labelledby={isNarrowLayout ? "workspace-editor-tab" : undefined}
                  hidden={isNarrowLayout && workspacePane !== "editor"}
                  onFocusCapture={() => setWorkspacePane("editor")}
                  onPointerDownCapture={() => setWorkspacePane("editor")}
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
                  defaultSize="55%"
                  minSize="25%"
                  id={isNarrowLayout ? "workspace-preview-panel" : undefined}
                  role={isNarrowLayout ? "tabpanel" : undefined}
                  aria-labelledby={isNarrowLayout ? "workspace-preview-tab" : undefined}
                  hidden={isNarrowLayout && workspacePane !== "preview"}
                  onFocusCapture={() => setWorkspacePane("preview")}
                  onPointerDownCapture={() => setWorkspacePane("preview")}
                >
                  <PreviewPanel t={t} />
                </ResizablePanel>
              </ResizablePanelGroup>
            </div>
          </main>

          <StatusBar />
        </div>
      </ExportWorkbench>
    </TooltipProvider>
  );
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
  const [hasActivatedConfig, setHasActivatedConfig] = useState(
    editorMode === "config",
  );
  const configActivated = hasActivatedConfig || editorMode === "config";

  return (
    <Tabs
      value={editorMode}
      onValueChange={(value) => {
        const mode = value as "code" | "config";
        if (mode === "config") setHasActivatedConfig(true);
        setEditorMode(mode);
      }}
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
        <CodeEditor className="h-full min-h-0" />
      </TabsContent>
      <TabsContent
        value="config"
        forceMount={configActivated ? true : undefined}
        className="mt-0 min-h-0 data-[state=inactive]:hidden"
      >
        {configActivated && (
          <LazyFeatureBoundary
            feature={t("editor.configMode")}
            presentation={{ kind: "panel" }}
          >
            <ConfigEditor className="h-full min-h-0" />
          </LazyFeatureBoundary>
        )}
      </TabsContent>
    </Tabs>
  );
}

function PreviewPanel({ t }: { t(key: string): string }) {
  return (
    <div className="h-full min-h-0 flex flex-col">
      <div className="flex h-11 shrink-0 items-center gap-2 border-b bg-muted/20 px-3 sm:px-4">
        <span className="text-xs font-medium text-muted-foreground">
          {t("preview.title")}
        </span>
        <RenderViewportControl />
      </div>
      <Preview className="min-h-0 flex-1 bg-[linear-gradient(to_right,var(--preview-grid)_1px,transparent_1px),linear-gradient(to_bottom,var(--preview-grid)_1px,transparent_1px)] bg-[size:20px_20px]" />
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
