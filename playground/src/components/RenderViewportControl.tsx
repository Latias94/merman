import { useTranslation } from "react-i18next";
import { Radio } from "lucide-react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { removeShareViewFromCurrentUrl } from "@/src/lib/share-view";
import {
  resolveRenderViewport,
  type RenderViewportStatus,
} from "@/src/runtime/render-viewport";
import { useAppStore } from "@/src/store";

const VIEWPORT_STATUS_TRANSLATION_KEYS = {
  canonical: "preview.viewportCanonicalStatus",
  host: "preview.viewportLiveStatus",
  "host-locked": "preview.viewportLockedStatus",
  "host-measuring": "preview.viewportMeasuring",
} as const satisfies Record<RenderViewportStatus, string>;

export function RenderViewportControl() {
  const { t } = useTranslation();
  const mode = useAppStore((state) => state.renderViewportMode);
  const liveHostViewport = useAppStore(
    (state) => state.liveHostRenderViewport,
  );
  const sharedRenderEnvironmentLock = useAppStore(
    (state) => state.sharedRenderEnvironmentLock,
  );
  const setMode = useAppStore((state) => state.setRenderViewportMode);
  const clearSharedRenderEnvironmentLock = useAppStore(
    (state) => state.clearSharedRenderEnvironmentLock,
  );
  const resolved = resolveRenderViewport(
    mode,
    liveHostViewport,
    sharedRenderEnvironmentLock,
  );
  const dimensions = t("preview.viewportSize", {
    width: resolved.viewport.width,
    height: resolved.viewport.height,
  });
  const status = t(VIEWPORT_STATUS_TRANSLATION_KEYS[resolved.status], {
    width: resolved.viewport.width,
    height: resolved.viewport.height,
  });
  const canUseLiveHostSize = liveHostViewport !== null;
  const useLiveHostSize = () => {
    if (!canUseLiveHostSize) return;
    removeShareViewFromCurrentUrl();
    clearSharedRenderEnvironmentLock();
  };

  return (
    <div
      data-testid="render-viewport-control"
      data-viewport-status={resolved.status}
      data-viewport-width={resolved.viewport.width}
      data-viewport-height={resolved.viewport.height}
      className="ml-auto flex min-w-0 items-center gap-2"
    >
      <div
        role="group"
        aria-label={t("preview.viewportMode")}
        className="inline-flex shrink-0 rounded-md border bg-background/70 p-0.5"
      >
        {(["canonical", "host"] as const).map((value) => (
          <button
            key={value}
            type="button"
            aria-pressed={mode === value}
            onClick={() => setMode(value)}
            className={cn(
              "h-6 rounded px-2 text-[11px] font-medium text-muted-foreground transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              mode === value && "bg-muted text-foreground shadow-sm",
            )}
          >
            {t(`preview.viewport${value === "canonical" ? "Canonical" : "Host"}`)}
          </button>
        ))}
      </div>
      <span
        role="status"
        aria-live="polite"
        title={status}
        className="min-w-0 max-w-24 truncate whitespace-nowrap text-[11px] tabular-nums text-muted-foreground sm:max-w-none"
      >
        <span className="sm:hidden">
          {resolved.status === "host-measuring"
            ? t("preview.viewportMeasuringShort")
            : dimensions}
        </span>
        <span className="hidden sm:inline">{status}</span>
      </span>
      {resolved.status === "host-locked" && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="h-6 w-6 shrink-0 sm:w-auto sm:px-2"
              aria-label={t("share.useLiveHostSize")}
              disabled={!canUseLiveHostSize}
              onClick={useLiveHostSize}
            >
              <Radio className="size-3.5" />
              <span className="hidden sm:inline">
                {t("share.useLiveHostSizeShort")}
              </span>
            </Button>
          </TooltipTrigger>
          <TooltipContent>{t("share.useLiveHostSize")}</TooltipContent>
        </Tooltip>
      )}
    </div>
  );
}
