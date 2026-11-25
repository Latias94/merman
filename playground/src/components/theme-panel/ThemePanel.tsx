import { lazy, useEffect, useId, useMemo, useRef, useState } from "react";
import { Palette, Undo2, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";
import { DIAGRAM_FONT_VALUES, isDiagramFont } from "@/src/lib/diagram-font";
import { resolveThemeSelection } from "@/src/lib/theme-appearance";
import { projectError } from "@/src/runtime/error-projection";
import { currentWorkspaceRender } from "@/src/runtime/render-coordinator";
import { selectMermanFacade, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { useRenderCoordinator } from "@/src/runtime/use-render-coordinator";
import { selectWorkspaceSnapshot, useAppStore } from "@/src/store";
import { parseThemeDocument } from "@/src/themes/v2";
import { ThemeAppearanceControl } from "./ThemeAppearanceControl";
import { ThemeNotices } from "./ThemeNotices";
import { ThemeWorkshop } from "./ThemeWorkshop";

const ThemeGallery = lazy(() => import("./ThemeGallery"));
const TABS = ["customize", "gallery", "json"] as const;

type ThemeTab = (typeof TABS)[number];

export function ThemePanel({ className, onClose }: { className?: string; onClose(): void }) {
  const { t } = useTranslation();
  const headingId = useId();
  const heading = useRef<HTMLHeadingElement>(null);
  const facade = useMermanRuntime(selectMermanFacade);
  const workspace = useAppStore(useShallow(selectWorkspaceSnapshot));
  const hostAppearance = useAppStore((state) => state.resolvedTheme);
  const canUndo = useAppStore((state) => state.appearanceHistory.length > 0);
  const [view, setView] = useState<ThemeTab>("customize");
  const { catalog, error } = useMemo(() => {
    try { return { catalog: facade?.themeCatalog() ?? null, error: null }; }
    catch (cause) { return { catalog: null, error: projectError(cause).summary }; }
  }, [facade]);
  const selection = resolveThemeSelection(workspace, hostAppearance, catalog);
  const render = useRenderCoordinator((state) => currentWorkspaceRender(state, workspace, hostAppearance));
  const outcome = render?.merman.status === "success" ? render.merman.metadata?.theme_outcome : undefined;
  const name = themeName();

  useEffect(() => { heading.current?.focus(); }, []);

  function themeName() {
    if (workspace.themeDocumentJson !== null) {
      try {
        const name = parseThemeDocument(workspace.themeDocumentJson).name;
        if (typeof name === "string") return name;
      } catch { /* Older shared links can still supply a display name. */ }
      return workspace.themeDocumentLabel?.name ?? t("themeWorkshop.custom");
    }
    const preset = workspace.themePresetId;
    if (preset) return catalog?.presets.find(({ id }) => id === preset.trim())?.name ?? preset;
    return t(`themes.${workspace.diagramTheme}`, { defaultValue: workspace.diagramTheme });
  }

  function close() {
    useAppStore.getState().closeThemePanel();
    onClose();
  }

  return <aside id="theme-panel" role="region" aria-labelledby={headingId} data-theme-panel
    className={cn("flex min-h-0 min-w-0 flex-col overflow-hidden border-t bg-background lg:border-l lg:border-t-0", className)}
    onKeyDown={(event) => {
      if (event.key === "Escape" && !event.defaultPrevented) { event.preventDefault(); event.stopPropagation(); close(); }
    }}>
    <header className="flex shrink-0 items-start justify-between gap-2 border-b px-3 py-2">
      <div className="min-w-0 space-y-1">
        <h2 ref={heading} id={headingId} tabIndex={-1} className="flex items-center gap-2 text-sm font-semibold outline-none">
          <Palette className="size-4 shrink-0" aria-hidden="true" />{t("appearance.title")}
        </h2>
        <p className="break-words text-xs [overflow-wrap:anywhere]" data-testid="theme-current-name">{name}</p>
      </div>
      <div className="flex shrink-0 gap-1">
        <Button type="button" variant="ghost" size="icon-sm" disabled={!canUndo} aria-label={t("appearance.undo")} title={t("appearance.undo")}
          onClick={() => useAppStore.getState().undoAppearance()}><Undo2 className="size-4" aria-hidden="true" /></Button>
        <Button type="button" variant="ghost" size="icon-sm" aria-label={t("appearance.close")} onClick={close}><X className="size-4" aria-hidden="true" /></Button>
      </div>
    </header>
    <div className="min-h-0 flex-1 space-y-3 overflow-y-auto overscroll-contain p-3">
      {selection.supported_appearances.length > 0 && <ThemeAppearanceControl selection={selection}
        onChange={(appearance) => useAppStore.getState().setThemeAppearance(appearance)} />}
      {outcome && <div data-theme-mode={outcome.mode} className="space-y-1 text-xs text-muted-foreground">
        <p>{t(`themeWorkshop.modes.${outcome.mode}`)}</p>
        <ThemeNotices notices={view === "gallery" ? outcome.notices : outcome.notices.filter((notice) => typeof notice === "string")} />
      </div>}
      {error ? <p role="alert" className="text-xs text-destructive">{error}</p> : !catalog || !facade ?
        <p role="status" className="text-xs text-muted-foreground">{t("themeWorkshop.loading")}</p> :
        <Tabs value={view} onValueChange={(value) => setView(value as ThemeTab)}>
          <TabsList className="w-full justify-start" aria-label={t("appearance.title")}>
            {TABS.map((tab) => <TabsTrigger key={tab} value={tab}>{t(`themeWorkshop.${tab}`)}</TabsTrigger>)}
          </TabsList>
          <TabsContent value="gallery">
            <LazyFeatureBoundary feature={t("themeWorkshop.gallery")} presentation={{ kind: "panel" }}>
              <ThemeGallery catalog={catalog} workspace={workspace} onSelect={(choice) => useAppStore.getState().selectTheme(choice)} />
            </LazyFeatureBoundary>
          </TabsContent>
          <ThemeWorkshop catalog={catalog} facade={facade} workspace={workspace} hostAppearance={hostAppearance}
            view={view} showJson={() => setView("json")} />
        </Tabs>}
      <label className="grid gap-1.5 border-t pt-3 text-xs">
        {t("themePanel.diagramFont")}
        <select className="h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs"
          value={workspace.diagramFont} onChange={(event) => {
            if (isDiagramFont(event.target.value)) useAppStore.getState().setDiagramFont(event.target.value);
          }}>
          {DIAGRAM_FONT_VALUES.map((font) => <option key={font} value={font}>{t(`diagramFonts.${font}`)}</option>)}
        </select>
      </label>
    </div>
  </aside>;
}
