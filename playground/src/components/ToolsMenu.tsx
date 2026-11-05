import { lazy, useCallback, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Braces, Columns2, Gauge, Settings2, Wrench } from "lucide-react";
import { useShallow } from "zustand/react/shallow";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { LazyFeatureBoundary } from "./LazyFeatureBoundary";
import { pauseRenderCoordinator } from "@/src/runtime/render-coordinator-browser";
import { useAppStore } from "@/src/store";

const BenchWorkbench = lazy(() => import("./BenchWorkbench").then((module) => ({
  default: module.BenchWorkbench,
})));

export function ToolsMenu() {
  const { t } = useTranslation();
  const { previewMode, textMeasurementMode, setTextMeasurementMode } = useAppStore(useShallow((state) => ({
    previewMode: state.previewMode,
    textMeasurementMode: state.textMeasurementMode,
    setTextMeasurementMode: state.setTextMeasurementMode,
  })));
  const [activated, setActivated] = useState(false);
  const [benchOpen, setBenchOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const restoreFocus = useCallback(() => triggerRef.current?.focus(), []);

  function openTool(mode: "compare" | "diagnostics") {
    const store = useAppStore.getState();
    store.setPreviewMode(mode);
    store.setWorkspacePane("preview");
  }

  return <>
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger asChild>
          <DropdownMenuTrigger asChild>
            <Button ref={triggerRef} variant="ghost" size="sm"
              className="w-8 px-0 xl:w-auto xl:px-2.5" aria-label={t("workbench.tools.title")}>
              <Wrench className="size-4" />
              <span className="hidden xl:inline">{t("workbench.tools.title")}</span>
            </Button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent>{t("workbench.tools.title")}</TooltipContent>
      </Tooltip>
      <DropdownMenuContent align="end" className="w-60 max-w-[calc(100vw-1rem)]"
        onCloseAutoFocus={(event) => {
          if (benchOpen) event.preventDefault();
        }}>
        <DropdownMenuLabel>{t("workbench.tools.title")}</DropdownMenuLabel>
        <DropdownMenuItem onSelect={() => openTool("compare")}
          aria-current={previewMode === "compare" ? "page" : undefined}>
          <Columns2 className="size-4" />{t("preview.compareMode")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => openTool("diagnostics")}
          aria-current={previewMode === "diagnostics" ? "page" : undefined}>
          <Braces className="size-4" />{t("preview.diagnosticsMode")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => {
          setActivated(true);
          setBenchOpen(true);
        }}>
          <Gauge className="size-4" />{t("workbench.tools.benchmark")}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuSub>
          <DropdownMenuSubTrigger><Settings2 className="size-4" />{t("workbench.tools.advancedRendering")}</DropdownMenuSubTrigger>
          <DropdownMenuSubContent className="max-w-[calc(100vw-1rem)]">
            <DropdownMenuLabel>{t("toolbar.textMeasurement")}</DropdownMenuLabel>
            <DropdownMenuRadioGroup value={textMeasurementMode} onValueChange={(value) => {
              if (value === "browser" || value === "headless") setTextMeasurementMode(value);
            }}>
              {(["browser", "headless"] as const).map((mode) => <DropdownMenuRadioItem key={mode} value={mode}>
                {t(`textMeasurement.${mode}`)}
              </DropdownMenuRadioItem>)}
            </DropdownMenuRadioGroup>
          </DropdownMenuSubContent>
        </DropdownMenuSub>
      </DropdownMenuContent>
    </DropdownMenu>

    {activated && <LazyFeatureBoundary feature={t("workbench.tools.benchmark")} presentation={{
      kind: "dialog", open: benchOpen, onOpenChange: setBenchOpen, restoreFocus,
    }}>
      <BenchWorkbench open={benchOpen} onOpenChange={setBenchOpen}
        pauseCoordinator={pauseRenderCoordinator} restoreFocus={restoreFocus} />
    </LazyFeatureBoundary>}
  </>;
}
