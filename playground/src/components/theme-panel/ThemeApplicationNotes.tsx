import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ThemePresetApplicationNote } from "@mermanjs/web";

import { Button } from "@/components/ui/button";
import { mergeRecommendedMermaidConfig, parseMermaidConfigJson } from "@/src/lib/mermaid-config";
import type { MermaidAppearanceOverride } from "@/src/lib/mermaid-appearance";
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

export function ThemeApplicationNotes({ notes, workspace, declaredLook, onPreview }: {
  notes: readonly ThemePresetApplicationNote[];
  workspace: Readonly<WorkspaceSnapshot>;
  declaredLook?: MermaidAppearanceOverride;
  onPreview(): void;
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);

  function previewConfig(config: Record<string, unknown>) {
    try {
      useAppStore.getState().previewThemeConfig(config);
      setError(null);
      onPreview();
    } catch (cause) {
      setError(t("themePanel.recommendedConfigError", { error: projectError(cause).summary }));
    }
  }

  return <section className="space-y-3 rounded-md border p-3 text-xs" aria-label={t("themePanel.applicationNotes")}>
    <h3 className="font-semibold">{t("themePanel.applicationNotes")}</h3>
    {notes.map((note) => {
      const config = note.recommended_config;
      const classicAction = note.id === "flowchart-classic-shadows" || note.id === "flowchart-classic-glow";
      const classicDeclared = classicAction && declaredLook?.value === "classic";
      const configured = config !== null && (containsRecommendation(workspace.mermaidConfig, config) || (classicDeclared && declaredLook.origin === "config"));
      const inSource = classicDeclared && declaredLook.origin === "source";
      return <div key={note.id} className="space-y-2" data-theme-application-note={note.id}>
        <p className="leading-relaxed text-muted-foreground">{t(`themeApplicationNotes.${note.id}`, { defaultValue: note.message })}</p>
        {config !== null && <>
          {configured || inSource
            ? <p role="status" className="text-[11px] font-medium" data-testid="theme-recommended-state">{t(inSource ? "themePanel.recommendationInSource" : "themePanel.recommendationInConfig")}</p>
            : <Button type="button" variant="outline" size="sm" className="h-auto max-w-full whitespace-normal py-1.5" onClick={() => previewConfig(config)}>
              {t(`themeApplicationActions.${note.id}`, { defaultValue: t("themePanel.previewRecommendedConfig") })}
            </Button>}
          {classicAction && declaredLook?.origin === "source" && !inSource && <p className="text-[11px] leading-relaxed text-muted-foreground" data-testid="theme-recommendation-source-override">
            {t("themePanel.recommendationSourceOverride", { value: declaredLook.value })}
          </p>}
          <details className="text-[11px] text-muted-foreground">
            <summary className="cursor-pointer">{t("themePanel.showRecommendedConfig")}</summary>
            <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-words rounded bg-muted/30 p-2">{JSON.stringify(config, null, 2)}</pre>
            <p className="mt-2 leading-relaxed">{t("themePanel.recommendedConfigHint")}</p>
          </details>
        </>}
      </div>;
    })}
    {error && <p role="alert" className="break-words leading-relaxed text-destructive">{error}</p>}
  </section>;
}
