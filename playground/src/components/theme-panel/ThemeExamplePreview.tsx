import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ThemeAppearance } from "@mermanjs/web";
import { configuredMermanOperationInput } from "@/src/runtime/merman-operation-input";
import { projectError } from "@/src/runtime/error-projection";
import { selectMermanFacade, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { THEME_SAMPLES, type ThemeSampleId } from "@/src/themes/samples";
import type { ExplorationTheme } from "@/src/themes/catalog";
import type { WorkspaceSnapshot } from "@/src/lib/workspace-snapshot";
import type { ConfiguredMermanOperationInput } from "@/src/runtime/merman-operation-input";
import type { NavigableInlineSvg } from "@/src/runtime/render-artifact";
import { SvgViewport, type SvgViewportController } from "@/src/components/SvgViewport";

export function ThemeExamplePreview({ theme, appearance, sampleId, decorative = false, workspace, active = true, viewport }: {
  readonly theme: ExplorationTheme;
  readonly appearance: ThemeAppearance;
  readonly sampleId: ThemeSampleId;
  readonly decorative?: boolean;
  readonly workspace?: Readonly<WorkspaceSnapshot>;
  readonly active?: boolean;
  readonly viewport?: {
    readonly controller: SvgViewportController;
    readonly interactionMode: "canvas" | "embedded";
  };
}) {
  const { t } = useTranslation();
  const facade = useMermanRuntime(selectMermanFacade);
  const container = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const [preview, setPreview] = useState<{ input: ConfiguredMermanOperationInput; artifact: NavigableInlineSvg | null; url: string | null; error: string | null } | null>(null);
  const sample = THEME_SAMPLES.find((entry) => entry.id === sampleId);
  const interactive = viewport !== undefined;
  const staticUrl = !interactive && !workspace
    ? (theme.appearances ? theme.appearances.find((entry) => entry.appearance === appearance)?.previews : theme.previews)?.[sampleId]
    : null;
  const input = useMemo(() => {
    if (staticUrl || !active || !visible || (!workspace && !sample)) return null;
    return configuredMermanOperationInput(workspace?.code ?? sample!.source, "auto", workspace?.mermaidConfig ?? sample!.configJson, {
      diagramFont: workspace?.diagramFont ?? "inherit", themeRecipeJson: theme.recipeJson, themeAppearance: theme.appearances ? appearance : null,
      svgPipeline: workspace?.svgPipeline, textMeasurementMode: workspace?.textMeasurementMode ?? "browser",
      layoutEnvironment: { containerWidth: 1000, containerHeight: 700 },
    });
  }, [active, appearance, sample, staticUrl, theme.appearances, theme.recipeJson, visible, workspace]);

  useEffect(() => {
    if (staticUrl || !active || !container.current) return;
    const observer = new IntersectionObserver((entries) => {
      const intersects = entries.some((entry) => entry.isIntersecting);
      setVisible((current) => interactive ? current || intersects : intersects);
    });
    observer.observe(container.current);
    return () => { observer.disconnect(); setVisible(false); };
  }, [active, interactive, staticUrl]);

  useEffect(() => {
    if (staticUrl || !facade || !active || !visible || !input) return;
    let disposed = false;
    let objectUrl: string | null = null;
    const timer = window.setTimeout(() => {
      try {
        const rendered = facade.render(input);
        if (rendered.status !== "success") throw new Error(rendered.error.summary);
        if (!interactive) objectUrl = URL.createObjectURL(new Blob([rendered.artifact.svg], { type: "image/svg+xml" }));
        if (!disposed) setPreview({ input, artifact: rendered.artifact, url: objectUrl, error: null });
      } catch (cause) { if (!disposed) setPreview({ input, artifact: null, url: null, error: projectError(cause).summary }); }
    }, 0);
    return () => {
      disposed = true;
      window.clearTimeout(timer);
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      setPreview((current) => current?.input === input ? null : current);
    };
  }, [active, facade, input, interactive, staticUrl, visible]);
  const current = preview?.input === input ? preview : null;
  const url = staticUrl ?? current?.url;
  const pending = <p role={decorative ? undefined : current?.error ? "alert" : "status"} className="p-2 text-xs text-muted-foreground" data-theme-preview-pending>
        {current?.error ? t("themeLibrary.previewFailed") : t("appearance.catalogPending")}
        {!decorative && current?.error && <span className="mt-2 block break-words">{current.error}</span>}
      </p>;
  return <div ref={container} className="flex h-full w-full min-w-0 items-center justify-center" data-theme-preview-target={workspace ? "current" : sampleId} data-theme-preview-appearance={appearance}>
    {viewport ? <SvgViewport artifact={current?.artifact ?? null} canvasTone={appearance} presentationKey={null}
      controller={viewport.controller} interactionMode={viewport.interactionMode} navigationEnabled={false} showSvgBounds={false} presentationMode="viewbox"
      empty={pending} renderMountError={() => <p role="alert" className="p-3 text-xs text-destructive">{t("themeLibrary.previewFailed")}</p>} />
      : url ? <img src={url} alt={decorative ? "" : workspace ? t("themeLibrary.currentPreviewAlt", { name: theme.name })
        : t("appearance.exampleAlt", { name: theme.name, sample: t(`themeGallery.samples.${sampleId}`) })}
        loading="lazy" className="h-full w-full min-w-0 object-contain" data-theme-preview-appearance={appearance} /> : pending}
  </div>;
}
