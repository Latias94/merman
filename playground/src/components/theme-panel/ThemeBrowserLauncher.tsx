import { lazy, useCallback, useRef, useState } from "react";
import { PanelsTopLeft } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";

const ThemeBrowser = lazy(() => import("./ThemeBrowser"));

function linkedThemeDesign(): string | null {
  const ids = new URLSearchParams(window.location.search).getAll("theme-design");
  return ids.length === 1 && /^[a-z][a-z0-9-]{0,47}$/.test(ids[0]) ? ids[0] : null;
}

export function ThemeBrowserLauncher({ placement }: { placement: "toolbar" | "panel" }) {
  const { t } = useTranslation();
  const toolbar = placement === "toolbar";
  const [linkedPreset, setLinkedPreset] = useState(() => toolbar ? linkedThemeDesign() : null);
  const [open, setOpen] = useState(linkedPreset !== null);
  const trigger = useRef<HTMLButtonElement>(null);
  const restoreFocus = useCallback(() => trigger.current?.focus(), []);
  const label = t(toolbar ? "themeBrowser.title" : "themeBrowser.browse");
  const button = <Button ref={trigger} type="button" variant={toolbar ? "ghost" : "outline"} size="sm"
    className={toolbar ? "h-8 w-8 px-0 xl:w-auto xl:px-2.5" : "w-full justify-start"}
    aria-label={label} aria-haspopup="dialog" onClick={() => { setLinkedPreset(null); setOpen(true); }}>
    <PanelsTopLeft className="size-4" aria-hidden="true" />
    <span className={toolbar ? "hidden xl:inline" : undefined}>{label}</span>
  </Button>;

  return <>
    {toolbar ? <Tooltip><TooltipTrigger asChild>{button}</TooltipTrigger><TooltipContent>{label}</TooltipContent></Tooltip> : button}
    {open && <LazyFeatureBoundary feature={t("themeBrowser.title")} presentation={{ kind: "dialog", open, onOpenChange: setOpen, restoreFocus }}>
      <ThemeBrowser linkedPreset={linkedPreset} onOpenChange={setOpen} restoreFocus={restoreFocus} />
    </LazyFeatureBoundary>}
  </>;
}
