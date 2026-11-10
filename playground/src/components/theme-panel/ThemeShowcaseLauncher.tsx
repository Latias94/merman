import { lazy, useEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { PanelsTopLeft } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";
import { useThemeLibraryStore } from "@/src/lib/theme-library-state";
import {
  currentDiagramDetection, currentWorkspaceDiagramDetection, detectedThemeFamily, detectWorkspaceDiagram,
  type WorkspaceDiagramDetection,
} from "@/src/lib/theme-discovery";
import { selectMermanFacade, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { useRenderCoordinator } from "@/src/runtime/use-render-coordinator";
import { selectWorkspaceSnapshot, useAppStore } from "@/src/store";

const ThemeShowcase = lazy(() => import("./ThemeShowcase"));

export function ThemeShowcaseLauncher({ panel = false, themeId }: {
  readonly panel?: boolean;
  readonly themeId?: string;
}) {
  const { t } = useTranslation();
  const trigger = useRef<HTMLButtonElement>(null);
  const filters = useThemeLibraryStore((state) => state.filters);
  const changeFilters = useThemeLibraryStore((state) => state.changeFilters);
  const facade = useMermanRuntime(selectMermanFacade);
  const workspace = useAppStore(useShallow(selectWorkspaceSnapshot));
  const hostAppearance = useAppStore((state) => state.resolvedTheme);
  const coordinator = useRenderCoordinator((state) => state);
  const families = useMemo(() => facade?.diagramFamilyCapabilities() ?? [], [facade]);
  const [workspaceDetection, setWorkspaceDetection] = useState<WorkspaceDiagramDetection | null>(null);
  const coordinatedDetection = currentDiagramDetection(coordinator, workspace, hostAppearance);
  const detection = coordinatedDetection ?? currentWorkspaceDiagramDetection(workspaceDetection, facade, workspace, hostAppearance);
  const currentFamilyId = detectedThemeFamily(detection, families);
  const [linkedThemeId] = useState(() => {
    if (panel) return undefined;
    const values = new URLSearchParams(window.location.search).getAll("theme-design");
    return values.length === 1 && /^[a-z][a-z0-9-]{0,63}$/.test(values[0]) ? values[0] : undefined;
  });
  const initialThemeId = themeId ?? linkedThemeId;
  const [activated, setActivated] = useState(linkedThemeId !== undefined);
  const [open, setOpen] = useState(linkedThemeId !== undefined);
  useEffect(() => {
    if (!open || !facade || coordinatedDetection) return;
    setWorkspaceDetection(detectWorkspaceDiagram(facade, workspace, hostAppearance));
  }, [coordinatedDetection, facade, hostAppearance, open, workspace]);
  useEffect(() => {
    if (open) useThemeLibraryStore.getState().initialize(currentFamilyId);
  }, [currentFamilyId, open]);
  const restoreFocus = () => {
    if (trigger.current?.isConnected) trigger.current.focus();
    else document.querySelector<HTMLButtonElement>("[data-theme-showcase-launcher]")?.focus();
  };
  const button = <Button ref={trigger} type="button" variant={panel ? "outline" : "ghost"} size="sm"
    data-theme-showcase-launcher aria-label={t("themeShowcase.title")} aria-haspopup="dialog"
    className={panel ? "h-auto w-full justify-start whitespace-normal py-2 text-left" : "h-8 w-8 px-0 xl:w-auto xl:px-2.5"}
    onClick={() => { useThemeLibraryStore.getState().initialize(currentFamilyId); setActivated(true); setOpen(true); }}>
    <PanelsTopLeft className="size-4 shrink-0" aria-hidden="true" />
    <span className={panel ? undefined : "hidden xl:inline"}>{t("themeShowcase.title")}</span>
  </Button>;

  return <>
    {panel ? button : <Tooltip><TooltipTrigger asChild>{button}</TooltipTrigger><TooltipContent>{t("themeShowcase.title")}</TooltipContent></Tooltip>}
    {activated && <LazyFeatureBoundary feature={t("themeShowcase.title")}
      presentation={{ kind: "dialog", open, onOpenChange: setOpen, restoreFocus }}>
      <ThemeShowcase key={initialThemeId ?? "collection"} open={open} onOpenChange={setOpen} restoreFocus={restoreFocus} initialThemeId={initialThemeId}
        filters={filters} onFiltersChange={changeFilters} currentFamilyId={currentFamilyId} />
    </LazyFeatureBoundary>}
  </>;
}
