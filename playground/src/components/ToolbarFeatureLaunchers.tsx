import { lazy, useCallback, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { BookOpen } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";

const ExampleGallery = lazy(() =>
  import("@/src/components/ExampleGallery").then((module) => ({
    default: module.ExampleGallery,
  })),
);

export function ToolbarFeatureLaunchers() {
  return <ExamplesLauncher />;
}

function ExamplesLauncher() {
  const { t } = useTranslation();
  const [activated, setActivated] = useState(false);
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
            onClick={() => {
              setActivated(true);
              setOpen(true);
            }}
            aria-label={t("toolbar.examples")}
            className="h-8 w-8 px-0 xl:w-auto xl:px-2.5"
          >
            <BookOpen className="size-4" />
            <span className="hidden xl:inline">{t("toolbar.examples")}</span>
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("toolbar.examples")}</TooltipContent>
      </Tooltip>

      {activated && (
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
