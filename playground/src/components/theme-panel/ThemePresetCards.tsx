import { Check, CircleOff } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ThemePresetCatalogEntry } from "@mermanjs/web";
import { cn } from "@/lib/utils";
import type { ThemeCandidate } from "@/src/lib/theme-edit-session";

const PRESET_SWATCHES: Readonly<Record<string, readonly string[]>> = {
  "editor-light": ["#ffffff", "#f0f4f8", "#24292e", "#0969da"],
  "editor-dark": ["#1e1e1e", "#31343b", "#d4d4d4", "#569cd6"],
  "one-dark": ["#282c34", "#3e4451", "#abb2bf", "#61afef"],
  "gruvbox-light": ["#fbf1c7", "#ebdbb2", "#3c3836", "#b57614"],
  "gruvbox-dark": ["#282828", "#3c3836", "#ebdbb2", "#fabd2f"],
  "ayu-light": ["#fafafa", "#f3f4f5", "#5c6166", "#ff9940"],
  "ayu-dark": ["#0b0e14", "#131721", "#bfbdb6", "#e6b450"],
  brutalist: ["#f3f0e7", "#ffffff", "#171717", "#ff5b35"],
  spotless: ["#ffffff", "#f8fafc", "#0f172a", "#94a3b8"],
  cyberpunk: ["#080b18", "#14162d", "#00e5ff", "#ff3cac"],
};

export function ThemePresetCards({ presets, candidate, onSelect }: {
  presets: readonly ThemePresetCatalogEntry[];
  candidate: ThemeCandidate;
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
      const colors = PRESET_SWATCHES[preset.id] ?? (preset.appearance === "dark"
        ? ["#18181b", "#3f3f46", "#e4e4e7", "#a1a1aa"] : ["#fafafa", "#e4e4e7", "#27272a", "#71717a"]);
      return <button key={preset.id} type="button" aria-pressed={selected} disabled={!preset.available}
        title={!preset.available ? preset.availability_reason_ids.join(", ") : undefined}
        className={cn(cardClass, selected && "border-primary bg-primary/5")}
        onClick={() => onSelect({ kind: "preset", presetId: preset.id })}>
        <div className="flex h-7 overflow-hidden rounded border" aria-hidden="true">{colors.map((color, index) => <span key={index} className="flex-1" style={{ backgroundColor: color }} />)}</div>
        <span className="break-words font-medium">{t(`themePresets.${preset.id}`, { defaultValue: preset.display_name })}</span>
        {!preset.available && <span className="text-[11px] text-destructive">{t("themePanel.unavailable")}</span>}
        {selected && <Check className="absolute right-2 top-2 size-3.5 text-primary" aria-hidden="true" />}
      </button>;
    })}
    <p className="col-span-2 text-[11px] leading-relaxed text-muted-foreground">{t("themePanel.swatchHint")}</p>
  </div>;
}
