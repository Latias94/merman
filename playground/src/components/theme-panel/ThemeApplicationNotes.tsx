import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ThemePresetApplicationNote } from "@mermanjs/web";

import { Button } from "@/components/ui/button";
import { mergeRecommendedMermaidConfig, parseMermaidConfigJson } from "@/src/lib/mermaid-config";
import type { WorkspaceSnapshot } from "@/src/lib/workspace-snapshot";
import { projectError } from "@/src/runtime/error-projection";
import { useAppStore } from "@/src/store";

function containsRecommendation(configJson: string, recommended: Record<string, unknown>): boolean {
  try {
    const current = parseMermaidConfigJson(configJson);
    const merged = parseMermaidConfigJson(mergeRecommendedMermaidConfig(configJson, recommended));
    return JSON.stringify(current) === JSON.stringify(merged);
  } catch { return false; }
}

export function ThemeApplicationNotes({ notes, workspace, onApply }: {
  notes: readonly ThemePresetApplicationNote[];
  workspace: Readonly<WorkspaceSnapshot>;
  onApply(): void;
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);

  function applyConfig(config: Record<string, unknown>) {
    try {
      useAppStore.getState().applyThemeConfig(config);
      setError(null);
      onApply();
    } catch (cause) {
      setError(t("appearance.recommendedConfigError", { error: projectError(cause).summary }));
    }
  }

  return <section className="space-y-3 rounded-md border p-3 text-xs" aria-label={t("themePanel.applicationNotes")}>
    <h3 className="font-semibold">{t("themePanel.applicationNotes")}</h3>
    {notes.map((note) => {
      const config = note.recommended_config;
      const configured = config !== null && containsRecommendation(workspace.mermaidConfig, config);
      return <div key={note.id} className="space-y-2" data-theme-application-note={note.id}>
        <p className="leading-relaxed text-muted-foreground">{t(`themeApplicationNotes.${note.id}`, { defaultValue: note.message })}</p>
        {config !== null && <>
          {configured
            ? <p role="status" className="text-[11px] font-medium" data-testid="theme-recommended-state">{t("appearance.recommendationInConfig")}</p>
            : <Button type="button" variant="outline" size="sm" className="h-auto max-w-full whitespace-normal py-1.5" onClick={() => applyConfig(config)}>
              {t("appearance.applyRecommendedConfig")}
            </Button>}
          <details className="text-[11px] text-muted-foreground">
            <summary className="cursor-pointer">{t("themePanel.showRecommendedConfig")}</summary>
            <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-words rounded bg-muted/30 p-2">{JSON.stringify(config, null, 2)}</pre>
            <p className="mt-2 leading-relaxed">{t("appearance.recommendedConfigHint")}</p>
          </details>
        </>}
      </div>;
    })}
    {error && <p role="alert" className="break-words leading-relaxed text-destructive">{error}</p>}
  </section>;
}
