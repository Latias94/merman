import { Check, Palette } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import type { DiagramThemeChoice } from "@/src/lib/diagram-appearance";
import type { ThemeFamilyTreatment } from "@/src/lib/theme-discovery";

export interface ThemeCatalogChoice {
  readonly key: string;
  readonly name: string;
  readonly source: "mermaid" | "builtin" | "exploration" | "imported";
  readonly choice: DiagramThemeChoice;
  readonly description?: string;
  readonly colors?: readonly string[];
  readonly available: boolean;
  readonly loading?: boolean;
  readonly unavailableReason?: string;
  readonly maturity?: string;
  readonly treatment?: ThemeFamilyTreatment | "waiting";
}

export function ThemeChoiceCards({ entries, selectedKey, onSelect }: {
  readonly entries: readonly ThemeCatalogChoice[];
  readonly selectedKey: string;
  onSelect(choice: DiagramThemeChoice): void;
}) {
  const { t } = useTranslation();
  return <div className="grid grid-cols-2 gap-2" data-testid="theme-catalog-choices">
    {entries.map((entry) => {
      const selected = entry.key === selectedKey;
      return <button key={entry.key} type="button" aria-pressed={selected} disabled={!entry.available}
        data-theme-choice={entry.key} data-theme-source={entry.source}
        data-theme-preset={entry.choice.kind === "preset" ? entry.choice.presetId : undefined}
        data-theme-exploration={entry.key.startsWith("exploration:") ? entry.key.slice(12) : undefined}
        data-theme-treatment={entry.treatment} title={entry.unavailableReason}
        className={cn("relative flex min-w-0 flex-col gap-1.5 rounded-md border p-2.5 text-left text-xs transition-colors hover:bg-accent/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-60",
          selected && "border-primary bg-primary/5")}
        onClick={() => onSelect(entry.choice)}>
        {entry.colors?.length ? <span className="mb-1 flex h-5 w-full overflow-hidden rounded border" aria-hidden="true" data-theme-swatches>
          {entry.colors.map((color, index) => <span key={index} className="min-w-0 flex-1" style={{ backgroundColor: color }} />)}
        </span> : <Palette className="mb-1 size-5 text-muted-foreground" aria-hidden="true" />}
        <span className="break-words pr-4 font-semibold [overflow-wrap:anywhere]" data-theme-choice-name>{entry.name}</span>
        <span className="flex flex-wrap gap-x-1.5 gap-y-0.5 text-[10px] text-muted-foreground">
          <span data-theme-source-label>{t(`appearance.sources.${entry.source}`)}</span>
          {entry.maturity && <span data-theme-maturity={entry.maturity}>{t(`themePanel.maturity.${entry.maturity}`, { defaultValue: entry.maturity })}</span>}
        </span>
        {entry.description && <span className="text-[11px] leading-relaxed text-muted-foreground">{entry.description}</span>}
        {entry.treatment && <span className="text-[11px] leading-relaxed text-muted-foreground">
          {t(entry.treatment === "waiting" ? "themePanel.treatmentWaiting" : `themePanel.treatments.${entry.treatment}`)}
        </span>}
        {!entry.available && <span className="text-[11px] text-muted-foreground">{t(entry.loading ? "appearance.catalogPending" : "appearance.unavailableChoice")}</span>}
        {selected && <Check className="absolute right-2 top-2 size-3.5 text-primary" aria-hidden="true" />}
      </button>;
    })}
  </div>;
}
