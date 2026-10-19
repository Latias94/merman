import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { ThemeGradientStop, ThemeLength, ThemePaint } from "@mermanjs/web";
import { Plus, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { NumberField } from "./ThemeEditorFields";

const SELECT_CLASS = "h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";
type GradientPaint = Extract<ThemePaint, { kind: "linear-gradient" | "radial-gradient" }>;
type PaintMode = "omit" | "clear" | "color" | "transparent" | "linear-gradient" | "radial-gradient" | "pattern";

export function newThemePaint(mode: Exclude<PaintMode, "omit" | "clear" | "pattern">): ThemePaint {
  if (mode === "color") return "#ffffff";
  if (mode === "transparent") return "transparent";
  const stops = [{ offset: 0, color: "#ffffff" }, { offset: 1, color: "#2563eb" }];
  return mode === "linear-gradient"
    ? { kind: mode, angle_degrees: 135, stops }
    : { kind: mode, center_x: { percent: 50 }, center_y: { percent: 50 }, radius: { percent: 70 }, stops };
}

export function PaintField({ label, value, clearable = false, required = false, allowAdvanced = true, omitLabel, onChange }: {
  label: string;
  value: ThemePaint | null | undefined;
  clearable?: boolean;
  required?: boolean;
  allowAdvanced?: boolean;
  omitLabel?: string;
  onChange(value: ThemePaint | null | undefined): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const mode: PaintMode = value === undefined ? "omit" : value === null ? "clear"
    : typeof value === "string" ? value === "transparent" ? "transparent" : "color"
    : value.kind === "solid" ? "color" : value.kind;
  const color = typeof value === "string" ? value : value?.kind === "solid" ? value.color : "#ffffff";
  return <div role="group" aria-label={label} className="min-w-0 space-y-2">
    <label htmlFor={id} className="block text-xs font-medium capitalize">{label}</label>
    <select id={id} aria-label={`${label}: ${t("themeWorkbench.valueMode")}`} value={mode} className={SELECT_CLASS} onChange={(event) => {
      const next = event.target.value as PaintMode;
      if (next === "pattern") return;
      onChange(next === "omit" ? undefined : next === "clear" ? null : newThemePaint(next));
    }}>
      {!required && <option value="omit">{omitLabel ?? t("themeWorkbench.omit")}</option>}
      {clearable && <option value="clear">{t("themeWorkbench.clear")}</option>}
      <option value="color">{t("themeWorkbench.color")}</option>
      <option value="transparent">{t("themeWorkbench.transparent")}</option>
      {allowAdvanced && <>
        <option value="linear-gradient">{t("themeWorkbench.paint.linear-gradient")}</option>
        <option value="radial-gradient">{t("themeWorkbench.paint.radial-gradient")}</option>
      </>}
      {mode === "pattern" && <option value="pattern">{t("themeWorkbench.paint.pattern")}</option>}
    </select>
    {mode === "color" && <ColorField label={label} value={color} onChange={(next) => onChange(typeof value === "object" && value?.kind === "solid" ? { ...value, color: next } : next)} />}
    {value && typeof value === "object" && (value.kind === "linear-gradient" || value.kind === "radial-gradient") && <div className="space-y-3 rounded-md border bg-muted/10 p-2">
      {value.kind === "linear-gradient"
        ? <NumberField label={t("themeWorkbench.gradientAngle")} value={value.angle_degrees} required onChange={(number) => { if (number != null) onChange({ ...value, angle_degrees: number }); }} />
        : <div className="grid min-w-0 gap-2">
          <LengthField label={t("themeWorkbench.gradientCenterX")} value={value.center_x} onChange={(next) => onChange({ ...value, center_x: next })} />
          <LengthField label={t("themeWorkbench.gradientCenterY")} value={value.center_y} onChange={(next) => onChange({ ...value, center_y: next })} />
          <LengthField label={t("themeWorkbench.gradientRadius")} value={value.radius} min={0.1} onChange={(next) => onChange({ ...value, radius: next })} />
        </div>}
      <GradientStops stops={value.stops} onChange={(stops) => onChange({ ...value, stops })} />
      <GradientRepetition paint={value} onChange={onChange} />
    </div>}
    {value && typeof value === "object" && value.kind === "pattern" && <div className="space-y-3 rounded-md border bg-muted/10 p-2">
      <p role="status" className="text-[11px] leading-relaxed text-muted-foreground">{t("themeWorkbench.patternNotRendered")}</p>
      <label className="block space-y-1.5 text-xs"><span>{t("themeWorkbench.patternKind")}</span><select aria-label={t("themeWorkbench.patternKind")} className={SELECT_CLASS} value={value.pattern} onChange={(event) => onChange({ ...value, pattern: event.target.value as typeof value.pattern })}>
        {(["grid", "dots", "stripes"] as const).map((kind) => <option key={kind} value={kind}>{t(`themeWorkbench.pattern.${kind}`)}</option>)}
      </select></label>
      <div className="grid min-w-0 grid-cols-2 gap-2">
        <NumberField label={t("themeWorkbench.patternWidth")} value={value.cell_width} required min={0.1} onChange={(number) => { if (number != null) onChange({ ...value, cell_width: number }); }} />
        <NumberField label={t("themeWorkbench.patternHeight")} value={value.cell_height} required min={0.1} onChange={(number) => { if (number != null) onChange({ ...value, cell_height: number }); }} />
      </div>
      <ColorField label={t("themeWorkbench.patternForeground")} value={value.foreground} onChange={(foreground) => onChange({ ...value, foreground })} />
      <ColorField label={t("themeWorkbench.patternBackground")} value={value.background ?? "transparent"} onChange={(background) => onChange({ ...value, background })} />
      {value.background !== undefined && <Button type="button" variant="ghost" size="sm" onClick={() => { const next = { ...value }; delete next.background; onChange(next); }}>{t("themeWorkbench.omitPatternBackground")}</Button>}
      <NumberField label={t("themeWorkbench.gradientAngle")} value={value.angle_degrees} onChange={(number) => { const next = { ...value }; if (number == null) delete next.angle_degrees; else next.angle_degrees = number; onChange(next); }} />
    </div>}
  </div>;
}

function ColorField({ label, value, onChange }: { label: string; value: string; onChange(value: string): void }) {
  const { t } = useTranslation();
  return <div className="min-w-0 space-y-1">
    <span className="text-[11px] text-muted-foreground">{label}</span>
    <div className="flex min-w-0 gap-2">
      <input type="color" aria-label={`${label}: ${t("themeWorkbench.colorPicker")}`} className="size-8 shrink-0 cursor-pointer rounded border bg-background p-0.5" value={/^#[\da-f]{6}([\da-f]{2})?$/i.test(value) ? value.slice(0, 7) : "#ffffff"} onChange={(event) => onChange(event.target.value + (/^#[\da-f]{8}$/i.test(value) ? value.slice(7) : ""))} />
      <Input key={value} aria-label={label} defaultValue={value} className="h-8 min-w-0 px-2 font-mono text-xs" onBlur={(event) => { if (event.target.value !== value) onChange(event.target.value); }} onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} />
    </div>
  </div>;
}

function GradientStops({ stops, onChange }: { stops: ThemeGradientStop[]; onChange(stops: ThemeGradientStop[]): void }) {
  const { t } = useTranslation();
  return <div className="space-y-2">
    <h4 className="text-xs font-medium">{t("themeWorkbench.gradientStops")}</h4>
    {stops.map((stop, index) => <div key={index} className="space-y-2 rounded-md border p-2">
      <ColorField label={t("themeWorkbench.stopColor", { index: index + 1 })} value={stop.color} onChange={(color) => onChange(stops.map((entry, i) => i === index ? { ...entry, color } : entry))} />
      <div className="flex min-w-0 items-end gap-2">
        <div className="min-w-0 flex-1"><NumberField label={t("themeWorkbench.stopPosition", { index: index + 1 })} value={stop.offset * 100} required min={(stops[index - 1]?.offset ?? 0) * 100} max={(stops[index + 1]?.offset ?? 1) * 100} onChange={(position) => { if (position != null) onChange(stops.map((entry, i) => i === index ? { ...entry, offset: position / 100 } : entry)); }} /></div>
        <Button type="button" variant="ghost" size="icon-sm" disabled={stops.length <= 2} aria-label={t("themeWorkbench.removeStop", { index: index + 1 })} onClick={() => onChange(stops.filter((_, i) => i !== index))}><Trash2 className="size-3.5" /></Button>
      </div>
    </div>)}
    <Button type="button" variant="outline" size="sm" onClick={() => {
      let gapIndex = 0;
      for (let index = 1; index < stops.length - 1; index++) {
        if (stops[index + 1].offset - stops[index].offset > stops[gapIndex + 1].offset - stops[gapIndex].offset) gapIndex = index;
      }
      const next = [...stops];
      next.splice(gapIndex + 1, 0, { offset: (stops[gapIndex].offset + stops[gapIndex + 1].offset) / 2, color: stops[gapIndex].color });
      onChange(next);
    }}><Plus className="size-3.5" />{t("themeWorkbench.addStop")}</Button>
  </div>;
}

function LengthField({ label, value, min, onChange }: { label: string; value: ThemeLength; min?: number; onChange(value: ThemeLength): void }) {
  const { t } = useTranslation();
  const percent = typeof value === "object" && "percent" in value;
  const number = typeof value === "number" ? value : "px" in value ? value.px : value.percent;
  return <div className="flex min-w-0 items-end gap-2">
    <div className="min-w-0 flex-1"><NumberField label={label} value={number} required min={min} onChange={(next) => { if (next != null) onChange(percent ? { percent: next } : typeof value === "number" ? next : { px: next }); }} /></div>
    <select aria-label={`${label}: ${t("themeWorkbench.unit")}`} value={percent ? "percent" : "px"} className={`${SELECT_CLASS} max-w-20`} onChange={(event) => onChange(event.target.value === "percent" ? { percent: number } : { px: number })}><option value="percent">%</option><option value="px">px</option></select>
  </div>;
}

function GradientRepetition({ paint, onChange }: { paint: GradientPaint; onChange(paint: ThemePaint): void }) {
  const { t } = useTranslation();
  const repetition = paint.repetition;
  return <details className="space-y-2 border-t pt-2">
    <summary className="cursor-pointer text-xs text-muted-foreground">{t("themeWorkbench.gradientRepetition")}</summary>
    <select aria-label={t("themeWorkbench.gradientRepetition")} className={SELECT_CLASS} value={repetition?.kind ?? "none"} onChange={(event) => {
      if (event.target.value === "none") { const next = { ...paint }; delete next.repetition; onChange(next); }
      else if (event.target.value === "tiled") onChange({ ...paint, repetition: { kind: "tiled", width_px: 64, height_px: 64 } });
      else if (paint.kind === "linear-gradient") onChange({ ...paint, repetition: { kind: "repeating", period_px: 32 } });
      else onChange({ ...paint, repetition: { kind: "repeating" } });
    }}>
      <option value="none">{t("themeWorkbench.noRepetition")}</option><option value="repeating">{t("themeWorkbench.repeating")}</option><option value="tiled">{t("themeWorkbench.tiled")}</option>
    </select>
    {paint.kind === "linear-gradient" && paint.repetition?.kind === "repeating" && <NumberField label={t("themeWorkbench.gradientPeriod")} value={paint.repetition.period_px} required min={1} onChange={(number) => { if (number != null) onChange({ ...paint, repetition: { kind: "repeating", period_px: number } }); }} />}
    {repetition?.kind === "tiled" && <div className="grid min-w-0 grid-cols-2 gap-2">
      <NumberField label={t("themeWorkbench.tileWidth")} value={repetition.width_px} required min={1} onChange={(number) => { if (number != null) onChange({ ...paint, repetition: { ...repetition, width_px: number } }); }} />
      <NumberField label={t("themeWorkbench.tileHeight")} value={repetition.height_px} required min={1} onChange={(number) => { if (number != null) onChange({ ...paint, repetition: { ...repetition, height_px: number } }); }} />
    </div>}
  </details>;
}
