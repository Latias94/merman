import { lazy, useCallback, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Braces, Columns2, Gauge, MoveHorizontal, Settings2, Wrench } from "lucide-react";
import { useShallow } from "zustand/react/shallow";

import { Button } from "@/components/ui/button";
import {
  Dialog, DialogClose, DialogContent, DialogDescription,
  DialogFooter, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
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
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { LazyFeatureBoundary } from "./LazyFeatureBoundary";
import { pauseRenderCoordinator } from "@/src/runtime/render-coordinator-browser";
import { CANONICAL_RENDER_VIEWPORT, isPositiveCssWidth } from "@/src/runtime/render-viewport";
import { selectVisibleRenderBatch, useRenderCoordinator } from "@/src/runtime/use-render-coordinator";
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
  const [fitWidthOpen, setFitWidthOpen] = useState(false);
  const previewWidth = useAppStore((state) => state.shoalPreviewWidth);
  const supportsFitWidth = useRenderCoordinator(
    (state) => selectVisibleRenderBatch(state)?.snapshot.supportsShoalFitWidth === true,
  );
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
          if (benchOpen || fitWidthOpen) event.preventDefault();
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
        {previewMode === "svg" && supportsFitWidth && (
          <DropdownMenuItem onSelect={() => setFitWidthOpen(true)}>
            <MoveHorizontal className="size-4" />
            <span className="min-w-0 break-words">{t("workbench.fitWidth.title")}</span>
            {previewWidth !== null && <span className="ml-auto truncate text-xs text-muted-foreground">
              {t("workbench.fitWidth.activeWidth", { width: previewWidth })}
            </span>}
          </DropdownMenuItem>
        )}
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
    {fitWidthOpen && <ShoalFitWidthDialog
      previewWidth={previewWidth}
      supported={supportsFitWidth}
      onClose={() => setFitWidthOpen(false)}
      restoreFocus={restoreFocus}
    />}
  </>;
}

function ShoalFitWidthDialog({ previewWidth, supported, onClose, restoreFocus }: {
  previewWidth: number | null;
  supported: boolean;
  onClose(): void;
  restoreFocus(): void;
}) {
  const { t } = useTranslation();
  const widthId = useId();
  const [enabled, setEnabled] = useState(previewWidth !== null);
  const [width, setWidth] = useState(String(previewWidth ?? CANONICAL_RENDER_VIEWPORT.width));
  const targetWidth = Number(width);
  const invalidWidth = enabled && !isPositiveCssWidth(targetWidth);
  const canApply = !enabled || (supported && !invalidWidth);

  return <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
    <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-sm"
      onCloseAutoFocus={(event) => { event.preventDefault(); restoreFocus(); }}>
      <DialogHeader>
        <DialogTitle>{t("workbench.fitWidth.title")}</DialogTitle>
        <DialogDescription>{t("workbench.fitWidth.description")}</DialogDescription>
      </DialogHeader>
      <form className="space-y-4" onSubmit={(event) => {
        event.preventDefault();
        if (!canApply) return;
        const store = useAppStore.getState();
        store.setShoalPreviewWidth(enabled ? targetWidth : null);
        store.setWorkspacePane("preview");
        onClose();
      }}>
        <label className="flex min-h-9 cursor-pointer items-center gap-2 text-sm font-medium">
          <input type="checkbox" className="size-4 shrink-0 accent-primary"
            checked={enabled} onChange={(event) => setEnabled(event.target.checked)} />
          {t("workbench.fitWidth.enable")}
        </label>
        <div className="space-y-1.5">
          <label htmlFor={widthId} className="block text-sm font-medium">{t("workbench.fitWidth.width")}</label>
          <Input id={widthId} type="number" inputMode="decimal" step="any" min={0}
            disabled={!enabled} aria-invalid={invalidWidth}
            aria-describedby={invalidWidth ? `${widthId}-error` : undefined}
            value={width} onChange={(event) => setWidth(event.target.value)} />
          {invalidWidth && <p id={`${widthId}-error`} className="text-xs text-destructive">
            {t("workbench.fitWidth.invalidWidth")}
          </p>}
        </div>
        <DialogFooter>
          <DialogClose asChild><Button type="button" variant="outline">{t("workbench.fitWidth.cancel")}</Button></DialogClose>
          <Button type="submit" disabled={!canApply}>{t("workbench.fitWidth.apply")}</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>;
}
