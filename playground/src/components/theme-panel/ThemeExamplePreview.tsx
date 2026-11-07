import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ThemeAppearance } from "@mermanjs/web";
import { configuredMermanOperationInput } from "@/src/runtime/merman-operation-input";
import { projectError } from "@/src/runtime/error-projection";
import { selectMermanFacade, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { THEME_SAMPLES, type ThemeSampleId } from "@/src/themes/samples";
import type { ExplorationTheme } from "@/src/themes/catalog";

export function ThemeExamplePreview({ theme, appearance, sampleId, decorative = false }: {
  readonly theme: ExplorationTheme;
  readonly appearance: ThemeAppearance;
  readonly sampleId: ThemeSampleId;
  readonly decorative?: boolean;
}) {
  const { t } = useTranslation();
  const facade = useMermanRuntime(selectMermanFacade);
  const [preview, setPreview] = useState<{ url: string; key: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const key = `${theme.id}:${appearance}:${sampleId}`;
  useEffect(() => {
    if (!theme.appearances || !facade) return;
    let disposed = false;
    let objectUrl: string | null = null;
    const timer = window.setTimeout(() => {
      try {
        const sample = THEME_SAMPLES.find((entry) => entry.id === sampleId)!;
        const input = configuredMermanOperationInput(sample.source, "auto", sample.configJson, {
          diagramFont: "inherit", themeRecipeJson: theme.recipeJson, themeAppearance: appearance,
          textMeasurementMode: "browser", layoutEnvironment: { containerWidth: 800, containerHeight: 600 },
        });
        const rendered = facade.render(input);
        if (rendered.status !== "success") throw new Error(rendered.error.summary);
        objectUrl = URL.createObjectURL(new Blob([rendered.artifact.svg], { type: "image/svg+xml" }));
        if (!disposed) { setPreview({ url: objectUrl, key }); setError(null); }
      } catch (cause) { if (!disposed) setError(projectError(cause).summary); }
    }, 0);
    return () => { disposed = true; window.clearTimeout(timer); if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [theme, appearance, sampleId, facade, key]);
  const url = theme.appearances ? preview?.key === key ? preview.url : null : theme.previews?.[sampleId];
  if (url) return <img src={url} alt={decorative ? "" : t("appearance.exampleAlt", { name: theme.name, sample: t(`themeGallery.samples.${sampleId}`) })}
    loading="lazy" className="h-full w-full min-w-0 object-contain" data-theme-preview-appearance={appearance} />;
  return <p role={error ? "alert" : "status"} className="p-2 text-xs text-muted-foreground" data-theme-preview-pending>
    {error ?? t("appearance.catalogPending")}
  </p>;
}
