import { Check } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";
import type { DiagramThemeChoice } from "@/src/lib/diagram-appearance";
import type { WorkspaceSnapshot } from "@/src/lib/workspace-snapshot";
import type { ThemeCatalog } from "@mermanjs/web";

const CATEGORIES = ["reading", "editor", "art", "custom"] as const;

export default function ThemeGallery({ catalog, workspace, onSelect }: {
  readonly catalog: ThemeCatalog;
  readonly workspace: Pick<WorkspaceSnapshot, "themePresetId" | "themeDocumentJson" | "diagramTheme">;
  onSelect(choice: DiagramThemeChoice): void;
}) {
  const { t } = useTranslation();
  return <div className="space-y-4" data-testid="theme-gallery">
    {CATEGORIES.map((category) => {
      const presets = catalog.presets.filter((preset) => preset.category === category);
      if (!presets.length) return null;
      return <section key={category} aria-label={t(`themeWorkshop.categories.${category}`)} className="space-y-2">
        <h3 className="text-xs font-semibold">{t(`themeWorkshop.categories.${category}`)}</h3>
        <div className="grid gap-2 sm:grid-cols-[repeat(auto-fit,minmax(min(100%,9.5rem),1fr))]">
          {presets.map((preset) => {
            const selected = workspace.themePresetId?.trim() === preset.id;
            return <button key={preset.id} type="button" data-theme-preset={preset.id} aria-pressed={selected}
              className={cn("min-w-0 space-y-2 rounded-lg border bg-card p-2.5 text-left hover:border-primary/60 focus-visible:outline-2 focus-visible:outline-primary",
                selected && "border-primary bg-primary/5")}
              onClick={() => onSelect({ kind: "preset", presetId: preset.id })}>
              <span className="flex items-center justify-between gap-1 text-sm font-semibold">
                {preset.name}{selected && <Check className="size-3.5 shrink-0 text-primary" aria-hidden="true" />}
              </span>
              <span className="grid h-4 grid-cols-6 overflow-hidden rounded border" aria-hidden="true">
                {preset.swatches.map((color, index) => <span key={index} data-preset-swatch style={{ backgroundColor: color }} />)}
              </span>
              <span className="block text-xs leading-snug text-muted-foreground">{preset.description}</span>
              <span className="flex flex-wrap items-center gap-1 text-[10px]">
                <span className="rounded border px-1.5 py-0.5">{t(`themeWorkshop.categories.${category}`)}</span>
                {preset.appearances.map((appearance) => <span key={appearance} className="rounded bg-muted px-1.5 py-0.5 text-muted-foreground">
                  {t(`appearanceMode.${appearance}`)}
                </span>)}
              </span>
            </button>;
          })}
        </div>
      </section>;
    })}
    <details className="rounded-lg border p-2.5" data-testid="mermaid-classic-themes">
      <summary className="cursor-pointer text-xs font-semibold">{t("themeWorkshop.classic")}</summary>
      <div className="mt-2 flex flex-wrap gap-1.5">
        {(["auto", ...catalog.mermaid_themes] as const).map((theme) => <button key={theme} type="button" data-mermaid-theme={theme}
          aria-pressed={workspace.themePresetId === null && workspace.themeDocumentJson === null && workspace.diagramTheme === theme}
          className="rounded border px-2 py-1 text-xs hover:border-primary/60 aria-pressed:border-primary aria-pressed:bg-primary/5"
          onClick={() => onSelect({ kind: "mermaid", theme })}>
          {t(`themes.${theme}`, { defaultValue: theme })}
        </button>)}
      </div>
    </details>
  </div>;
}
