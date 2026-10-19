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
import { useDefaultLayout } from "react-resizable-panels";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Toolbar } from "./components/Toolbar";
import { StatusBar } from "./components/StatusBar";
import { Preview } from "./components/Preview";
import { ExportWorkbench } from "./components/ExportDialog";
import { LazyFeatureBoundary } from "./components/LazyFeatureBoundary";
import { useAppStore, type WorkspacePane } from "./store";
import { RenderCoordinatorBridge } from "@/src/runtime/RenderCoordinatorBridge";
import { playgroundStartupBoundary } from "@/src/runtime/startup-boundary";
import { ThemeWorkbench } from "./components/theme-workbench/ThemeWorkbench";

const CodeEditor = lazy(() =>
  import("./components/EditorFeature").then((module) => ({
    default: module.CodeEditor,
  })),
);

const ConfigEditor = lazy(() =>
  import("./components/ConfigEditorFeature").then((module) => ({
    default: module.ConfigEditor,
  }))
);

const themeLayoutStorage = {
  getItem(key: string): string | null {
    try {
      const stored = localStorage.getItem(key);
      const parsed: unknown = stored ? JSON.parse(stored) : null;
      return parsed && typeof parsed === "object" && !Array.isArray(parsed) &&
        Object.values(parsed).every((size) => typeof size === "number" && Number.isFinite(size))
        ? stored : null;
    } catch {
      return null;
    }
  },
  setItem(key: string, value: string): void {
    try {
      localStorage.setItem(key, value);
    } catch {
      // Layout changes still work without storage.
    }
  },
};
export default function App() {
  const { t, i18n } = useTranslation();
  const {
    editorMode,
    setEditorMode,
    shareViewWarning,
    workspacePane,
    setWorkspacePane,
    themeEditSession,
  } = useAppStore(
    useShallow((state) => ({
      editorMode: state.editorMode,
      setEditorMode: state.setEditorMode,
      shareViewWarning: state.shareViewWarning,
      setWorkspacePane: state.setWorkspacePane,
      workspacePane: state.workspacePane,
      themeEditSession: state.themeEditSession,
    }))
  );
  const isNarrowLayout = useNarrowLayout();
  const isWideLayout = useWideLayout();
  const themeLayout = useDefaultLayout({
    id: isWideLayout ? "merman-theme-horizontal" : "merman-theme-vertical",
    onlySaveAfterUserInteractions: true,
    storage: themeLayoutStorage,
    panelIds: themeEditSession ? ["diagram-workspace", "theme-workbench-panel"] : ["diagram-workspace"],
  });

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

          <main className="relative flex min-h-0 flex-1 overflow-hidden">
            <ResizablePanelGroup
              direction={isWideLayout ? "horizontal" : "vertical"}
              className="min-h-0 flex-1"
              defaultLayout={themeLayout.defaultLayout}
              onLayoutChanged={themeLayout.onLayoutChanged}
            >
              <ResizablePanel id="diagram-workspace" defaultSize={isWideLayout ? "68%" : "55%"} minSize={isWideLayout ? 360 : "25%"}>
                <div className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden">
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
                        activateEditorLanguageImmediately={
                          isNarrowLayout && workspacePane === "editor"
                        }
                        editorMode={editorMode}
                        mountCodeEditorImmediately={
                          !isNarrowLayout || workspacePane === "editor"
                        }
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
                      <Preview
                        active={!isNarrowLayout || workspacePane === "preview"}
                        className="h-full min-h-0"
                      />
                    </ResizablePanel>
                  </ResizablePanelGroup>
                </div>
              </ResizablePanel>
              {themeEditSession && <>
                <ResizableHandle withHandle aria-label={t("themeWorkbench.resize")} />
                <ResizablePanel id="theme-workbench-panel" defaultSize={isWideLayout ? "32%" : "45%"} minSize={isWideLayout ? 320 : "30%"} maxSize={isWideLayout ? "60%" : "75%"}>
                  <ThemeWorkbench
                    className="h-full min-h-0"
                    onClose={() => requestAnimationFrame(() => {
                      const launchers = document.querySelectorAll<HTMLButtonElement>("[data-theme-launcher]");
                      Array.from(launchers).find((launcher) => launcher.offsetParent !== null)?.focus({ preventScroll: true });
                    })}
                  />
                </ResizablePanel>
              </>}
            </ResizablePanelGroup>
          </main>

          <StatusBar />
        </div>
      </ExportWorkbench>
    </TooltipProvider>
  );
}

function EditorPanel({
  activateEditorLanguageImmediately,
  editorMode,
  mountCodeEditorImmediately,
  setEditorMode,
  t,
}: {
  activateEditorLanguageImmediately: boolean;
  editorMode: "code" | "config";
  mountCodeEditorImmediately: boolean;
  setEditorMode(mode: "code" | "config"): void;
  t(key: string): string;
}) {
  return (
    <Tabs
      value={editorMode}
      onValueChange={(value) => {
        const mode = value as "code" | "config";
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
        <StartupCodeEditor
          activateLanguageImmediately={activateEditorLanguageImmediately}
          className="h-full min-h-0"
          mountImmediately={mountCodeEditorImmediately}
        />
      </TabsContent>
      <TabsContent
        value="config"
        className="mt-0 min-h-0 data-[state=inactive]:hidden"
      >
        <LazyFeatureBoundary
          feature={t("editor.configMode")}
          presentation={{ kind: "panel" }}
        >
          <ConfigEditor className="h-full min-h-0" />
        </LazyFeatureBoundary>
      </TabsContent>
    </Tabs>
  );
}

function StartupCodeEditor({
  activateLanguageImmediately,
  className,
  mountImmediately,
}: {
  readonly activateLanguageImmediately: boolean;
  readonly className?: string;
  readonly mountImmediately: boolean;
}) {
  const { t } = useTranslation();
  const [mounted, setMounted] = useState(
    () => mountImmediately || playgroundStartupBoundary.reason() !== null,
  );

  useEffect(() => {
    let active = true;
    void playgroundStartupBoundary.wait().then(() => {
      if (active) setMounted(true);
    });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!mountImmediately) return;
    setMounted(true);
  }, [mountImmediately]);

  useEffect(() => {
    if (!activateLanguageImmediately) return;
    playgroundStartupBoundary.activate("editor-visible");
  }, [activateLanguageImmediately]);

  const activateLanguageFromEditorIntent = () => {
    playgroundStartupBoundary.activate("editor-intent");
  };

  const mountFromEditorIntent = () => {
    activateLanguageFromEditorIntent();
    setMounted(true);
  };

  if (!mounted) {
    return (
      <button
        type="button"
        data-testid="editor-activation"
        aria-label={t("editor.loading")}
        className={`${className ?? ""} flex w-full items-center justify-center bg-card text-sm text-muted-foreground`}
        onClick={mountFromEditorIntent}
        onFocus={mountFromEditorIntent}
        onPointerDown={mountFromEditorIntent}
      >
        {t("editor.loading")}
      </button>
    );
  }

  return (
    <div
      className={`${className ?? ""} flex flex-col`}
      data-testid="editor-startup-surface"
      onFocusCapture={activateLanguageFromEditorIntent}
      onPointerDownCapture={activateLanguageFromEditorIntent}
    >
      <LazyFeatureBoundary
        feature={t("layout.editor")}
        presentation={{ kind: "panel" }}
      >
        <CodeEditor
          className="h-full min-h-0"
          waitForLanguageActivation={playgroundStartupBoundary.wait}
        />
      </LazyFeatureBoundary>
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

function subscribeWideLayout(onChange: () => void): () => void {
  const mediaQuery = window.matchMedia("(min-width: 1024px)");
  mediaQuery.addEventListener("change", onChange);
  return () => mediaQuery.removeEventListener("change", onChange);
}

function useWideLayout(): boolean {
  return useSyncExternalStore(
    subscribeWideLayout,
    () => window.matchMedia("(min-width: 1024px)").matches,
    () => false,
  );
}
