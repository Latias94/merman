import { Check, CircleOff } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ThemePresetCatalogEntry } from "@mermanjs/web";
import { cn } from "@/lib/utils";
import type { ThemeCandidate } from "@/src/lib/theme-edit-session";
import { themeFamilyTreatment } from "@/src/lib/theme-discovery";

export function ThemePresetCards({ presets, candidate, familyId, pending, onSelect }: {
  presets: readonly ThemePresetCatalogEntry[];
  candidate: ThemeCandidate;
  familyId: string | null;
  pending: boolean;
  onSelect(candidate: Exclude<ThemeCandidate, { kind: "custom" }>): void;
}) {
  const { t } = useTranslation();
  const cardClass = "relative flex min-w-0 flex-col gap-2 rounded-lg border p-3 text-left text-xs outline-none transition-colors hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50";
  return <div className="grid grid-cols-2 gap-2" aria-label={t("themePanel.presets")}>
    <button type="button" aria-pressed={candidate.kind === "none"}
      className={cn(cardClass, candidate.kind === "none" && "border-primary bg-primary/5")} onClick={() => onSelect({ kind: "none" })}>
      <div className="flex h-7 items-center text-muted-foreground"><CircleOff className="size-5" aria-hidden="true" /></div>
      <span className="font-medium">{t("themePanel.none")}</span>
      <span className="text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.noneDescription")}</span>
      {candidate.kind === "none" && <Check className="absolute right-2 top-2 size-3.5 text-primary" aria-hidden="true" />}
    </button>
    {presets.map((preset) => {
      const selected = candidate.kind === "preset" && candidate.presetId === preset.id;
      const colors = preset.preview_colors;
      const treatment = pending ? null : themeFamilyTreatment(preset, familyId);
      return <button key={preset.id} type="button" aria-pressed={selected} disabled={!preset.available}
        data-theme-preset={preset.id} data-theme-treatment={treatment ?? "waiting"}
        title={!preset.available ? preset.availability_reason_ids.join(", ") : undefined}
        className={cn(cardClass, selected && "border-primary bg-primary/5")}
        onClick={() => onSelect({ kind: "preset", presetId: preset.id })}>
        <div className="flex h-7 overflow-hidden rounded border" aria-hidden="true" data-theme-swatches>{colors.map((color, index) => <span key={index} className="flex-1" style={{ backgroundColor: color }} />)}</div>
        <span className="break-words font-medium">{t(`themePresets.${preset.id}`, { defaultValue: preset.display_name })}</span>
        <span className="text-[11px] leading-relaxed text-muted-foreground">{treatment ? t(`themePanel.treatments.${treatment}`) : t("themePanel.treatmentWaiting")}</span>
        {!preset.available && <span className="text-[11px] text-destructive">{t("themePanel.unavailable")}</span>}
        {selected && <Check className="absolute right-2 top-2 size-3.5 text-primary" aria-hidden="true" />}
      </button>;
    })}
    <p className="col-span-2 text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.swatchHint")}</p>
  </div>;
}
