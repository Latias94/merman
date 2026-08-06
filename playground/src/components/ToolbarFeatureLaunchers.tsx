import { lazy, useCallback, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { BookOpen, Gauge } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";
import { pauseRenderCoordinator } from "@/src/runtime/render-coordinator-browser";

const BenchWorkbench = lazy(() =>
  import("@/src/components/BenchWorkbench").then((module) => ({
    default: module.BenchWorkbench,
  })),
);
const ExampleGallery = lazy(() =>
  import("@/src/components/ExampleGallery").then((module) => ({
    default: module.ExampleGallery,
  })),
);

export function ToolbarFeatureLaunchers() {
  return (
    <>
      <ExamplesLauncher />
      <BenchLauncher />
    </>
  );
}

function BenchLauncher() {
  const { t } = useTranslation();
  const [activated, setActivated] = useState(false);
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const restoreFocus = useCallback(() => triggerRef.current?.focus(), []);

  const openBench = () => {
    setActivated(true);
    setOpen(true);
  };

  return (
    <>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            ref={triggerRef}
            variant={open ? "secondary" : "ghost"}
            size="sm"
            aria-label={t("toolbar.bench")}
            className="size-10 px-0 lg:h-8 lg:w-auto lg:px-2.5"
            onClick={openBench}
          >
            <Gauge className="size-4" />
            <span className="hidden lg:inline">{t("toolbar.bench")}</span>
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("toolbar.bench")}</TooltipContent>
      </Tooltip>

      {activated && (
        <LazyFeatureBoundary
          feature={t("toolbar.bench")}
          presentation={{
            kind: "dialog",
            open,
            onOpenChange: setOpen,
            restoreFocus,
          }}
        >
          <BenchWorkbench
            open={open}
            onOpenChange={setOpen}
            pauseCoordinator={pauseRenderCoordinator}
            restoreFocus={restoreFocus}
          />
        </LazyFeatureBoundary>
      )}
    </>
  );
}

function ExamplesLauncher() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const restoreFocus = useCallback(() => triggerRef.current?.focus(), []);

  return (
    <>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            ref={triggerRef}
            variant={open ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setOpen(true)}
            aria-label={t("toolbar.examples")}
            className="size-10 px-0 sm:h-8 sm:w-auto sm:px-2.5"
          >
            <BookOpen className="size-4" />
            <span className="hidden sm:inline">{t("toolbar.examples")}</span>
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("toolbar.examples")}</TooltipContent>
      </Tooltip>

      {open && (
        <LazyFeatureBoundary
          feature={t("toolbar.examples")}
          presentation={{
            kind: "dialog",
            open,
            onOpenChange: setOpen,
            restoreFocus,
          }}
        >
          <ExampleGallery
            open={open}
            onOpenChange={setOpen}
            restoreFocus={restoreFocus}
          />
        </LazyFeatureBoundary>
      )}
    </>
  );
}
