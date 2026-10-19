import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowDown, ArrowUp, Copy, Plus, Trash2 } from "lucide-react";
import type { ThemeRecipeV1 } from "@mermanjs/web";
import { Button } from "@/components/ui/button";
import { removeThemeCanvasBackground, restoreThemeCanvas, setThemeDesignPath } from "@/src/lib/theme-document";
import { stripeLayer } from "./ThemeCanvasLayers";
import { newThemePaint } from "./ThemePaintEditor";
import { DESIGN_SELECT_CLASS, DesignValueField, type DesignFieldContext } from "./ThemeDesignFields";
type DesignRecipe = Extract<ThemeRecipeV1, { kind: "design" }>;
export interface RemovedThemeCanvas { before: DesignRecipe; after: string; }

export function ThemeDesignCanvas({
  document,
  context,
  selectedIndex,
  onSelectLayer,
  removed,
  onRemoved,
  onChange,
}: {
  document: DesignRecipe;
  context: DesignFieldContext;
  selectedIndex: number;
  onSelectLayer(index: number): void;
  removed: RemovedThemeCanvas | null;
  onRemoved(removed: RemovedThemeCanvas | null): void;
  onChange(document: ThemeRecipeV1): void;
}) {
  const { t } = useTranslation();
  const design = document.design;
  const layers = design.canvas?.layers ?? [];
  const [newKind, setNewKind] = useState<
    "color" | "linear-gradient" | "radial-gradient" | "grid" | "stripes"
  >("linear-gradient");
  const index = Math.min(selectedIndex, Math.max(0, layers.length - 1));
  const active = layers[index];
  function edit(path: (string | number)[], value: unknown) {
    onChange({ ...document, design: setThemeDesignPath(design, path, value) });
  }
  function replace(next: typeof layers) {
    edit(["canvas", "layers"], next.length ? next : undefined);
  }
  function move(direction: -1 | 1) {
    const next = [...layers];
    [next[index], next[index + direction]] = [
      next[index + direction],
      next[index],
    ];
    onSelectLayer(index + direction);
    replace(next);
  }
  const canRestore = removed?.after === JSON.stringify(design.canvas);
  return (
    <section
      className="min-w-0 space-y-3 rounded-md border p-3"
      aria-label={t("themeWorkbench.canvas")}
    >
      <h3 className="text-xs font-semibold">{t("themeWorkbench.canvas")}</h3>
      <p className="text-[11px] leading-relaxed text-muted-foreground">
        {t("themeWorkbench.canvasExportHint")}
      </p>
      <DesignValueField
        label={t("themeWorkbench.canvasBase")}
        type="paint"
        value={design.canvas?.base}
        path={["canvas", "base"]}
        context={context}
        optional
        onChange={(next) => edit(["canvas", "base"], next)}
      />
      <section
        className="space-y-3 border-t pt-3"
        aria-label={t("themeWorkbench.backgroundLayers")}
      >
        <h4 className="text-xs font-semibold">
          {t("themeWorkbench.backgroundLayers")}
        </h4>
        <p className="text-[11px] text-muted-foreground">
          {t("themeWorkbench.layerOrderHint")}
        </p>
        <div className="space-y-1">
          {layers.map((_, i) => (
            <Button
              key={i}
              type="button"
              variant={i === index ? "secondary" : "outline"}
              size="sm"
              className="mr-1"
              aria-pressed={i === index}
              onClick={() => onSelectLayer(i)}
            >
              {t("themeWorkbench.layerName", { index: i + 1 })}
            </Button>
          ))}
        </div>
        <div className="flex min-w-0 gap-2">
          <select
            aria-label={t("themeWorkbench.newLayerPaint")}
            className={DESIGN_SELECT_CLASS}
            value={newKind}
            onChange={(event) =>
              setNewKind(event.target.value as typeof newKind)
            }
          >
            <option value="color">{t("themeWorkbench.color")}</option>
            <option value="linear-gradient">
              {t("themeWorkbench.paint.linear-gradient")}
            </option>
            <option value="radial-gradient">
              {t("themeWorkbench.paint.radial-gradient")}
            </option>
            <option value="grid">{t("themeWorkbench.pattern.grid")}</option>
            <option value="stripes">
              {t("themeWorkbench.pattern.stripes")}
            </option>
          </select>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => {
              onSelectLayer(layers.length);
              replace([
                ...layers,
                ...(newKind === "grid"
                  ? [stripeLayer(0), stripeLayer(90)]
                  : newKind === "stripes"
                    ? [stripeLayer(45)]
                    : [{ paint: newThemePaint(newKind) }]),
              ]);
            }}
          >
            <Plus className="size-3.5" />
            {t("themeWorkbench.addLayer")}
          </Button>
        </div>
        {active && (
          <div
            className="space-y-3 rounded-md border p-2"
            data-theme-layer-editor
          >
            <div className="flex flex-wrap gap-1.5">
              <Button
                type="button"
                size="icon-sm"
                variant="outline"
                aria-label={t("themeWorkbench.moveLayerDown")}
                disabled={index === 0}
                onClick={() => move(-1)}
              >
                <ArrowUp className="size-3.5" />
              </Button>
              <Button
                type="button"
                size="icon-sm"
                variant="outline"
                aria-label={t("themeWorkbench.moveLayerUp")}
                disabled={index === layers.length - 1}
                onClick={() => move(1)}
              >
                <ArrowDown className="size-3.5" />
              </Button>
              <Button
                type="button"
                size="icon-sm"
                variant="outline"
                aria-label={t("themeWorkbench.duplicateLayer")}
                onClick={() => {
                  const next = [...layers];
                  next.splice(index + 1, 0, structuredClone(active));
                  onSelectLayer(index + 1);
                  replace(next);
                }}
              >
                <Copy className="size-3.5" />
              </Button>
              <Button
                type="button"
                size="icon-sm"
                variant="destructive"
                aria-label={t("themeWorkbench.removeLayer")}
                onClick={() => replace(layers.filter((_, i) => i !== index))}
              >
                <Trash2 className="size-3.5" />
              </Button>
            </div>
            <DesignValueField
              label={t("themeWorkbench.layerPaint")}
              type="paint"
              value={active.paint}
              path={["canvas", "layers", index, "paint"]}
              context={context}
              onChange={(next) =>
                edit(["canvas", "layers", index, "paint"], next)
              }
            />
            <DesignValueField
              label={t("themeWorkbench.layerOpacity")}
              type="number"
              value={active.opacity}
              path={["canvas", "layers", index, "opacity"]}
              context={context}
              optional
              domain={{ value_type: "number", min: 0, max: 1 }}
              onChange={(next) =>
                edit(["canvas", "layers", index, "opacity"], next)
              }
            />
            {(typeof active.opacity === "number" ||
              active.opacity === undefined) && (
              <input
                type="range"
                aria-label={t("themeWorkbench.layerOpacitySlider")}
                min={0}
                max={1}
                step={0.01}
                value={active.opacity ?? 1}
                className="w-full accent-primary"
                onChange={(event) =>
                  edit(
                    ["canvas", "layers", index, "opacity"],
                    Number(event.target.value),
                  )
                }
              />
            )}
            <label className="block space-y-1 text-xs" data-design-value={`/canvas/layers/${index}/blend_mode`} tabIndex={-1}>
              <span>{t("themeWorkbench.layerBlend")}</span>
              <select
                aria-label={t("themeWorkbench.layerBlend")}
                value={active.blend_mode ?? ""}
                className={DESIGN_SELECT_CLASS}
                onChange={(event) =>
                  edit(
                    ["canvas", "layers", index, "blend_mode"],
                    event.target.value || undefined,
                  )
                }
              >
                <option value="">{t("themeWorkbench.omit")}</option>
                {[
                  "normal",
                  "multiply",
                  "screen",
                  "overlay",
                  "darken",
                  "lighten",
                  "difference",
                ].map((mode) => (
                  <option key={mode} value={mode}>
                    {t(`themeWorkbench.blend.${mode}`)}
                  </option>
                ))}
              </select>
            </label>
            <DesignValueField
              label={t("themeWorkbench.layerOffsetX")}
              type="length"
              value={active.offset_x}
              path={["canvas", "layers", index, "offset_x"]}
              context={context}
              optional
              domain={{ value_type: "length", units: ["px"] }}
              onChange={(next) =>
                edit(["canvas", "layers", index, "offset_x"], next)
              }
            />
            <DesignValueField
              label={t("themeWorkbench.layerOffsetY")}
              type="length"
              value={active.offset_y}
              path={["canvas", "layers", index, "offset_y"]}
              context={context}
              optional
              domain={{ value_type: "length", units: ["px"] }}
              onChange={(next) =>
                edit(["canvas", "layers", index, "offset_y"], next)
              }
            />
          </div>
        )}
      </section>
      <DesignValueField
        label={t("themeDesigner.canvasBleed")}
        type="insets"
        value={design.canvas?.bleed}
        path={["canvas", "bleed"]}
        context={context}
        optional
        onChange={(next) => edit(["canvas", "bleed"], next)}
      />
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={canRestore}
          onClick={() => {
            const next = removeThemeCanvasBackground(document) as DesignRecipe;
            onRemoved({
              before: document,
              after: JSON.stringify(next.design.canvas),
            });
            onChange(next);
          }}
        >
          {t("themeWorkbench.removeCanvas")}
        </Button>
        {canRestore && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => {
              onChange(restoreThemeCanvas(document, removed.before));
              onRemoved(null);
            }}
          >
            {t("themeWorkbench.undoCanvas")}
          </Button>
        )}
      </div>
    </section>
  );
}
