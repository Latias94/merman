import { lazy, useRef, useState } from "react";
import { PanelsTopLeft } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";

const ThemeShowcase = lazy(() => import("./ThemeShowcase"));

export function ThemeShowcaseLauncher({ panel = false, themeId }: {
  readonly panel?: boolean;
  readonly themeId?: string;
}) {
  const { t } = useTranslation();
  const trigger = useRef<HTMLButtonElement>(null);
  const [linkedThemeId] = useState(() => {
    if (panel) return undefined;
    const values = new URLSearchParams(window.location.search).getAll("theme-design");
    return values.length === 1 && /^[a-z][a-z0-9-]{0,63}$/.test(values[0]) ? values[0] : undefined;
  });
  const initialThemeId = themeId ?? linkedThemeId;
  const [activated, setActivated] = useState(linkedThemeId !== undefined);
  const [open, setOpen] = useState(linkedThemeId !== undefined);
  const restoreFocus = () => {
    if (trigger.current?.isConnected) trigger.current.focus();
    else document.querySelector<HTMLButtonElement>("[data-theme-showcase-launcher]")?.focus();
  };
  const button = <Button ref={trigger} type="button" variant={panel ? "outline" : "ghost"} size="sm"
    data-theme-showcase-launcher aria-label={t("themeShowcase.title")} aria-haspopup="dialog"
    className={panel ? "h-auto w-full justify-start whitespace-normal py-2 text-left" : "h-8 w-8 px-0 xl:w-auto xl:px-2.5"}
    onClick={() => { setActivated(true); setOpen(true); }}>
    <PanelsTopLeft className="size-4 shrink-0" aria-hidden="true" />
    <span className={panel ? undefined : "hidden xl:inline"}>{t("themeShowcase.title")}</span>
  </Button>;

  return <>
    {panel ? button : <Tooltip><TooltipTrigger asChild>{button}</TooltipTrigger><TooltipContent>{t("themeShowcase.title")}</TooltipContent></Tooltip>}
    {activated && <LazyFeatureBoundary feature={t("themeShowcase.title")}
      presentation={{ kind: "dialog", open, onOpenChange: setOpen, restoreFocus }}>
      <ThemeShowcase key={initialThemeId ?? "collection"} open={open} onOpenChange={setOpen} restoreFocus={restoreFocus} initialThemeId={initialThemeId} />
    </LazyFeatureBoundary>}
  </>;
}
