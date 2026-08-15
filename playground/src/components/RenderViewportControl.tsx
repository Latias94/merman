import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import { resolveRenderViewport } from "@/src/runtime/render-viewport";
import { useAppStore } from "@/src/store";

export function RenderViewportControl() {
  const { t } = useTranslation();
  const mode = useAppStore((state) => state.renderViewportMode);
  const hostViewport = useAppStore((state) => state.hostRenderViewport);
  const setMode = useAppStore((state) => state.setRenderViewportMode);
  const resolved = resolveRenderViewport(mode, hostViewport);
  const status =
    resolved.status === "host-measuring"
      ? t("preview.viewportMeasuring", {
          width: resolved.viewport.width,
          height: resolved.viewport.height,
        })
      : t("preview.viewportSize", {
          width: resolved.viewport.width,
          height: resolved.viewport.height,
        });

  return (
    <div className="ml-auto flex min-w-0 items-center gap-2">
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
        className="min-w-0 whitespace-nowrap text-[11px] tabular-nums text-muted-foreground"
      >
        {resolved.status === "host-measuring"
          ? t("preview.viewportMeasuringShort")
          : status}
      </span>
    </div>
  );
}
