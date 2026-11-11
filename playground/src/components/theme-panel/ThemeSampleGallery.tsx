import { ArrowUpRight, Check, Code, Maximize2 } from "lucide-react";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { ThemeAppearance } from "@mermanjs/web";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { encodeShareHash } from "@/src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "@/src/lib/workspace-snapshot";
import { explorationAppearance, type ExplorationTheme } from "@/src/themes/catalog";
import { samplesForTheme, THEME_SAMPLES, type ThemeSampleId } from "@/src/themes/samples";
import { ThemeExamplePreview } from "./ThemeExamplePreview";
import { ThemeSampleNote } from "./ThemeSampleNote";
import { themeDesignTreatment } from "@/src/lib/theme-library";

export function ThemeSampleGallery({ theme, appearance, selectedSampleId, active, onInspect, onUse }: {
  readonly theme: ExplorationTheme;
  readonly appearance: ThemeAppearance;
  readonly selectedSampleId: ThemeSampleId;
  readonly active: boolean;
  onInspect(sampleId: ThemeSampleId, trigger: HTMLButtonElement): void;
  onUse(sampleId: ThemeSampleId): void;
}) {
  const { t } = useTranslation();
  const samples = useMemo(() => samplesForTheme(theme).map((sample) => ({
    ...sample, workspaceUrl: sampleWorkspaceUrl(theme, sample.id, appearance),
  })), [theme, appearance]);
  const colors = explorationAppearance(theme, appearance).colors;

  return <section className="min-w-0 space-y-4" aria-label={t("themeLibrary.sampleGallery")} data-testid="theme-sample-gallery">
    <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-2">
      <div className="flex items-center gap-2.5">
        <h3 className="text-lg font-semibold tracking-tight">{t("themeShowcase.acrossDiagrams")}</h3>
        <span className="rounded-full border px-2 py-0.5 text-[10px] tabular-nums text-muted-foreground">{t("themeLibrary.familyCount", { count: samples.length })}</span>
      </div>
      <p className="text-xs leading-relaxed text-muted-foreground">{t("themeLibrary.galleryHint")}</p>
    </div>
    <div className="grid min-w-0 grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
      {samples.map((sample) => {
        const family = t(`themeGallery.samples.${sample.id}`);
        const selected = selectedSampleId === sample.id;
        const treatment = themeDesignTreatment(theme, sample.id);
        return <article key={sample.id} data-theme-sample={sample.id} data-theme-sample-selected={selected} data-theme-coverage={treatment}
          className={cn("min-w-0 overflow-hidden rounded-xl border bg-card", selected && "border-primary/60 ring-1 ring-primary/20")}>
          <button type="button" className="group block w-full min-w-0 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
            aria-label={t("themeLibrary.inspectSample", { family })} aria-haspopup="dialog" data-theme-library-focus={`gallery-${sample.id}`}
            onClick={(event) => onInspect(sample.id, event.currentTarget)}>
            <div className="flex min-h-12 items-center justify-between gap-2 px-4 py-3">
              <div className="min-w-0">
                <h4 className="text-sm font-medium">{family}</h4>
                {treatment === "base_only" && <p className="text-[10px] text-muted-foreground">{t("themeLibrary.coverage.base_only")}</p>}
              </div>
              <div className="flex shrink-0 items-center gap-2 text-muted-foreground">
                {selected && <span className="inline-flex items-center gap-1 text-[10px] text-primary"><Check className="size-3" aria-hidden="true" />{t("themeLibrary.selectedExample")}</span>}
                <Maximize2 className="size-3.5 group-hover:text-foreground" aria-hidden="true" />
              </div>
            </div>
            <div className="flex h-64 min-w-0 items-center justify-center overflow-hidden p-4 sm:h-72 xl:h-64" style={{ backgroundColor: colors[0] }}>
              <ThemeExamplePreview theme={theme} appearance={appearance} sampleId={sample.id} active={active} decorative />
            </div>
          </button>
          <div className="flex min-w-0 flex-wrap items-center justify-between gap-2 border-t px-3 py-2">
            <a href={sample.workspaceUrl} target="_blank" rel="noopener noreferrer"
              aria-label={t("themeShowcase.openSampleNamed", { sample: family, name: theme.name })}
              className="inline-flex min-h-8 items-center gap-1 rounded-sm px-1 text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
              {t("themeShowcase.openSample")}<ArrowUpRight className="size-3" aria-hidden="true" />
            </a>
            <Button type="button" variant="ghost" size="sm" className="h-8 text-xs" data-theme-library-focus={`gallery-use-${sample.id}`} onClick={() => onUse(sample.id)}>
              <Code className="size-3.5" aria-hidden="true" />{t("themeLibrary.useExample")}
            </Button>
          </div>
          {theme.sampleNotes?.some((note) => note.familyId === sample.id && note.appearance === appearance) && <div className="px-3 pb-3">
            <ThemeSampleNote theme={theme} familyId={sample.id} appearance={appearance} />
          </div>}
        </article>;
      })}
    </div>
  </section>;
}

export function sampleWorkspaceUrl(theme: ExplorationTheme, sampleId: ThemeSampleId, appearance: ThemeAppearance): string {
  const sample = THEME_SAMPLES.find((entry) => entry.id === sampleId) ?? THEME_SAMPLES[0];
  return `${import.meta.env.BASE_URL}${encodeShareHash({ ...DEFAULT_WORKSPACE_SNAPSHOT,
    code: sample.source, mermaidConfig: sample.configJson, diagramTheme: "auto", diagramFont: "inherit",
    ...(theme.appearances ? { themeAppearance: appearance } : {}),
    themePresetId: null, themeRecipeJson: theme.recipeJson, themeRecipeLabel: { name: theme.name, source: "exploration" },
  })}`;
}
