import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ThemeCanvasLayer, ThemeRecipeV1 } from "@mermanjs/web";
import { ArrowDown, ArrowUp, Copy, Plus, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { setThemeCanvasLayers } from "@/src/lib/theme-document";
import { NumberField } from "./ThemeEditorFields";
import { newThemePaint, PaintField } from "./ThemePaintEditor";

const SELECT_CLASS = "h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

export function stripeLayer(angle_degrees: number): ThemeCanvasLayer {
  return { paint: {
    kind: "linear-gradient", angle_degrees,
    stops: [
      { offset: 0, color: "#64748b40" }, { offset: 1 / 24, color: "#64748b40" },
      { offset: 1 / 24, color: "transparent" }, { offset: 1, color: "transparent" },
    ],
    repetition: { kind: "tiled", width_px: 24, height_px: 24 },
  } };
}

export function ThemeCanvasLayers({ document, onChange }: { document: Extract<ThemeRecipeV1, { kind: "complete_spec" }>; onChange(document: ThemeRecipeV1): void }) {
  const { t } = useTranslation();
  const layers = document.complete_spec.canvas?.layers ?? [];
  const [selectedIndex, setSelectedIndex] = useState(0);
  const index = Math.min(selectedIndex, Math.max(0, layers.length - 1));
  const active = layers[index];
  const [newKind, setNewKind] = useState<"color" | "linear-gradient" | "radial-gradient" | "grid" | "stripes">("linear-gradient");

  function replace(next: ThemeCanvasLayer[]) {
    onChange(setThemeCanvasLayers(document, next));
  }

  function edit<Property extends keyof ThemeCanvasLayer>(property: Property, value: ThemeCanvasLayer[Property]) {
    const next = { ...active };
    if (value === undefined) delete next[property];
    else next[property] = value;
    replace(layers.map((layer, i) => i === index ? next : layer));
  }

  function move(direction: -1 | 1) {
    const next = [...layers];
    [next[index], next[index + direction]] = [next[index + direction], next[index]];
    setSelectedIndex(index + direction);
    replace(next);
  }

  return <section className="min-w-0 space-y-3 border-t pt-3" aria-label={t("themeWorkbench.backgroundLayers")}>
    <h4 className="text-xs font-semibold">{t("themeWorkbench.backgroundLayers")}</h4>
    <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themeWorkbench.layerOrderHint")}</p>
    {layers.length > 0 && <div className="space-y-1" role="group" aria-label={t("themeWorkbench.backgroundLayers")}>
      {layers.map((layer, i) => <button key={i} type="button" aria-pressed={i === index} className="flex w-full min-w-0 items-center gap-2 rounded-md border px-2 py-2 text-left text-xs hover:bg-accent/50 aria-pressed:border-primary aria-pressed:bg-primary/5" onClick={() => setSelectedIndex(i)}>
        <span className="min-w-0 flex-1 truncate">{t("themeWorkbench.layerName", { index: i + 1 })} · {typeof layer.paint === "string" ? layer.paint : t(`themeWorkbench.paint.${layer.paint.kind}`, { defaultValue: layer.paint.kind })}</span>
      </button>)}
    </div>}
    <div className="flex min-w-0 gap-2">
      <select aria-label={t("themeWorkbench.newLayerPaint")} className={SELECT_CLASS} value={newKind} onChange={(event) => setNewKind(event.target.value as typeof newKind)}>
        <option value="color">{t("themeWorkbench.color")}</option>
        <option value="linear-gradient">{t("themeWorkbench.paint.linear-gradient")}</option>
        <option value="radial-gradient">{t("themeWorkbench.paint.radial-gradient")}</option>
        <option value="grid">{t("themeWorkbench.pattern.grid")}</option>
        <option value="stripes">{t("themeWorkbench.pattern.stripes")}</option>
      </select>
      <Button type="button" variant="outline" size="sm" onClick={() => {
        const added = newKind === "grid" ? [stripeLayer(0), stripeLayer(90)] : newKind === "stripes" ? [stripeLayer(45)] : [{ paint: newThemePaint(newKind) }];
        setSelectedIndex(layers.length);
        replace([...layers, ...added]);
      }}><Plus className="size-3.5" />{t("themeWorkbench.addLayer")}</Button>
    </div>
    {(newKind === "grid" || newKind === "stripes") && <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themeWorkbench.gridLayersHint")}</p>}
    {active && <div className="space-y-3 rounded-md border p-2" data-theme-layer-editor>
      <div className="flex flex-wrap gap-1.5">
        <Button type="button" variant="outline" size="icon-sm" aria-label={t("themeWorkbench.moveLayerDown")} disabled={index === 0} onClick={() => move(-1)}><ArrowUp className="size-3.5" /></Button>
        <Button type="button" variant="outline" size="icon-sm" aria-label={t("themeWorkbench.moveLayerUp")} disabled={index === layers.length - 1} onClick={() => move(1)}><ArrowDown className="size-3.5" /></Button>
        <Button type="button" variant="outline" size="icon-sm" aria-label={t("themeWorkbench.duplicateLayer")} onClick={() => { const next = [...layers]; next.splice(index + 1, 0, structuredClone(active)); setSelectedIndex(index + 1); replace(next); }}><Copy className="size-3.5" /></Button>
        <Button type="button" variant="destructive" size="icon-sm" aria-label={t("themeWorkbench.removeLayer")} onClick={() => replace(layers.filter((_, i) => i !== index))}><Trash2 className="size-3.5" /></Button>
      </div>
      <PaintField label={t("themeWorkbench.layerPaint")} value={active.paint} required onChange={(paint) => { if (paint != null) edit("paint", paint); }} />
      <NumberField label={t("themeWorkbench.layerOpacity")} value={active.opacity} min={0} max={1} onChange={(value) => edit("opacity", value ?? undefined)} />
      <input type="range" aria-label={t("themeWorkbench.layerOpacitySlider")} min={0} max={1} step={0.01} value={active.opacity ?? 1} className="w-full accent-primary" onChange={(event) => edit("opacity", Number(event.target.value))} />
      <label className="block space-y-1.5 text-xs"><span>{t("themeWorkbench.layerBlend")}</span><select aria-label={t("themeWorkbench.layerBlend")} className={SELECT_CLASS} value={active.blend_mode ?? ""} onChange={(event) => edit("blend_mode", event.target.value === "" ? undefined : event.target.value as ThemeCanvasLayer["blend_mode"])}>
        <option value="">{t("themeWorkbench.omit")}</option>
        {(["normal", "multiply", "screen", "overlay", "darken", "lighten", "difference"] as const).map((mode) => <option key={mode} value={mode}>{t(`themeWorkbench.blend.${mode}`)}</option>)}
      </select></label>
      <div className="grid min-w-0 grid-cols-2 gap-2">
        <NumberField label={t("themeWorkbench.layerOffsetX")} value={active.offset_x} onChange={(value) => edit("offset_x", value ?? undefined)} />
        <NumberField label={t("themeWorkbench.layerOffsetY")} value={active.offset_y} onChange={(value) => edit("offset_y", value ?? undefined)} />
      </div>
    </div>}
  </section>;
}
