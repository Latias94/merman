import { useId } from "react";
import { ArrowRight, Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ThemeAppearance } from "@mermanjs/web";

import { cn } from "@/lib/utils";
import { themeDesignTreatment, themeSampleFor, themeSampleIds } from "@/src/lib/theme-library";
import { explorationAppearance, type ExplorationTheme } from "@/src/themes/catalog";
import type { ThemeSampleId } from "@/src/themes/families";
import { ThemeExamplePreview } from "./ThemeExamplePreview";

export function ThemeDesignCard({ theme, selected, sampleId, appearance, active, onExplore }: {
  readonly theme: ExplorationTheme;
  readonly selected: boolean;
  readonly sampleId: ThemeSampleId | null;
  readonly appearance: ThemeAppearance;
  readonly active: boolean;
  onExplore(): void;
}) {
  const { t } = useTranslation();
  const currentId = useId();
  const colors = explorationAppearance(theme, appearance).colors;
  const family = sampleId && t(`themeGallery.samples.${sampleId}`);
  const primarySample = themeSampleFor(theme, sampleId);
  const samples = themeSampleIds(theme);
  const widePrimary = ["sequence", "xychart", "mindmap", "gitGraph", "gantt", "kanban", "timeline", "sankey", "treeView"].includes(primarySample);
  const companions: readonly ThemeSampleId[] = widePrimary ? ["flowchart", "pie", "er"] : ["sequence", "pie", "flowchart"];
  const cardSamples = [primarySample, ...[...new Set([...companions, ...samples])]
    .filter((id) => id !== primarySample && samples.includes(id)).slice(0, 2)];

  return <article className={cn("min-w-0 overflow-hidden rounded-xl border bg-card transition-[border-color,box-shadow] duration-150 hover:border-foreground/25 hover:shadow-sm", selected && "border-primary")}
    data-theme-design-card={theme.id} data-theme-design-family={sampleId ?? "collage"}>
    <button type="button" onClick={onExplore} data-explore-theme={theme.id} aria-label={t("themeShowcase.exploreNamed", { name: theme.name })}
      aria-describedby={selected ? currentId : undefined}
      className="block w-full min-w-0 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring">
      <div className="space-y-2 px-4 pb-4 pt-4">
        <div className="flex items-center justify-between gap-3">
          <h3 className="text-xl font-semibold tracking-tight">{theme.name}</h3>
          {selected ? <><Check className="size-4 text-primary" aria-hidden="true" /><span id={currentId} className="sr-only">{t("themeShowcase.current")}</span></>
            : <span className="shrink-0 text-[10px] text-muted-foreground">{t(theme.appearances ? "appearanceMode.paired" : `appearanceMode.${theme.appearance}`)}</span>}
        </div>
        <p className="min-h-8 text-xs leading-relaxed text-muted-foreground">{t(`themeShowcase.mechanisms.${theme.id}`)}</p>
      </div>
      <div className={cn("grid h-72 gap-px overflow-hidden bg-border", cardSamples.length === 1 ? "grid-cols-1 grid-rows-1" : widePrimary
        ? "grid-cols-2 grid-rows-[minmax(0,1.5fr)_minmax(0,1fr)]"
        : "grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)] grid-rows-2")} aria-hidden="true">
        {cardSamples.map((sample, index) => <div key={sample} data-theme-card-sample={sample} data-theme-card-primary={index === 0}
          className={cn("relative flex min-h-0 min-w-0 items-center justify-center p-3 pt-8", cardSamples.length > 1 && index === 0 && (widePrimary ? "col-span-2" : "row-span-2"))}
          style={{ backgroundColor: colors[0] }}>
          <span className="absolute left-2 top-2 max-w-[calc(100%-1rem)] truncate rounded bg-background/90 px-1.5 py-0.5 text-[10px] leading-tight text-foreground">{t(`themeGallery.samples.${sample}`)}</span>
          <ThemeExamplePreview theme={theme} appearance={appearance} sampleId={sample} active={active} decorative />
        </div>)}
      </div>
      <div className="space-y-2 border-t px-4 py-3">
        <div className="flex min-w-0 items-center justify-between gap-3">
          <span className="text-xs text-muted-foreground" data-theme-coverage={sampleId ? themeDesignTreatment(theme, sampleId) : undefined}>
            {sampleId ? themeDesignTreatment(theme, sampleId) === "dedicated" ? t("themeLibrary.designedFor", { family })
              : `${t("themeLibrary.coverage.base_only")} · ${family}` : t("themeLibrary.familyCount", { count: samples.length })}
          </span>
          <span className="inline-flex shrink-0 items-center gap-1 text-xs font-medium">{t("themeShowcase.explore")}<ArrowRight className="size-3.5" aria-hidden="true" /></span>
        </div>
        <p className="text-[11px] text-muted-foreground">{theme.styles.map((style) => t(`themeLibrary.styles.${style}`)).join(" · ")}</p>
      </div>
    </button>
  </article>;
}
