import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { withAppearance } from "@/src/themes/authoring";
import type { ResolvedThemeV2, ThemeCatalog, ThemeDocumentV2, ThemeTexture } from "@mermanjs/web";

const TEXTURES: Readonly<Record<string, readonly ThemeTexture[]>> = {
  none: [],
  grid: [{ kind: "grid", step_px: 20 }],
  dots: [{ kind: "dots", step_px: 12 }],
  ruled: [{ kind: "ruled", line_height_px: 24 }],
  fibers: [{ kind: "fibers" }],
};

export function ThemeCustomize({ catalog, document, base, resolved, onChange }: {
  readonly catalog: ThemeCatalog;
  readonly document: ThemeDocumentV2;
  readonly base?: ThemeDocumentV2;
  readonly resolved: ResolvedThemeV2;
  onChange(document: ThemeDocumentV2): void;
}) {
  const { t } = useTranslation();
  const { appearance, roles, typography, form, canvas } = resolved;
  const layer = document.appearances?.[appearance];
  const seed = { ...base?.appearances?.[appearance]?.seed, ...base?.seed, ...layer?.seed, ...document.seed };
  const roleOverrides = { ...base?.appearances?.[appearance]?.roles, ...base?.roles, ...layer?.roles, ...document.roles };
  const elevation = layer?.elevation ?? document.form?.elevation ?? base?.appearances?.[appearance]?.elevation ?? base?.form?.elevation ?? "flat";
  const texture = canvas.textures.length === 1 ? canvas.textures[0].kind : canvas.textures.length ? "custom" : "none";

  function changeColor(key: "accent" | "canvas" | "surface", value: string) {
    const role = key === "canvas" ? "canvas" : key === "surface" ? "surface.node" : null;
    if (role && Object.hasOwn(roleOverrides, role)) {
      if (Object.hasOwn(document.roles ?? {}, role)) onChange({ ...document, roles: { ...document.roles, [role]: value } });
      else onChange(withAppearance(document, appearance, { roles: { ...layer?.roles, [role]: value } }));
    } else if (Object.hasOwn(document.seed ?? {}, key)) {
      onChange({ ...document, seed: { ...document.seed, [key]: value } });
    } else {
      onChange(withAppearance(document, appearance, { seed: { ...layer?.seed, [key]: value } }));
    }
  }

  return <div className="grid gap-3" data-testid="theme-customize">
    <ControlGroup title={t("themeWorkshop.groups.base")}>
      <Choice label={t("themeWorkshop.base")} value={document.extends ?? ""}
        choices={[["", t("themeWorkshop.standalone")], ...catalog.presets.map((preset) => [preset.id, preset.name] as const)]}
        onChange={(value) => { if (value) onChange({ ...document, extends: value }); }} />
      <div className="grid grid-cols-3 gap-2">
        {([["accent", seed.accent ?? roles["status.info"]], ["canvas", roles.canvas], ["surface", roles["surface.node"]]] as const).map(([key, paint]) => {
          const color = typeof paint === "string" ? paint : paint.stops[0]?.color;
          const value = color && /^#[0-9a-f]{6}/i.test(color) ? color.slice(0, 7) : seed[key]?.slice(0, 7) ?? "#000000";
          return <label key={key} className="grid gap-1.5 text-xs">
            {t(`themeWorkshop.colors.${key}`)}
            <input type="color" className="h-8 w-full cursor-pointer rounded border bg-background p-0.5"
              value={value} onChange={(event) => changeColor(key, event.target.value)} />
          </label>;
        })}
      </div>
    </ControlGroup>
    <ControlGroup title={t("themeWorkshop.groups.typography")}>
      <Choice label={t("themeWorkshop.font")} value={typography.body_family}
        choices={(["sans", "serif", "mono"] as const).map((value) => [value, t(`themeWorkshop.fonts.${value}`)])}
        onChange={(value) => onChange({ ...document, typography: { ...document.typography, body_family: value, title_family: value } })} />
      <Slider label={t("themeWorkshop.fontSize")} value={typography.base_size_px} min={10} max={24}
        onChange={(value) => onChange({ ...document, typography: { ...document.typography, base_size_px: value } })} />
    </ControlGroup>
    <ControlGroup title={t("themeWorkshop.groups.form")}>
      {([
        ["density", form.density, ["compact", "balanced", "airy"]],
        ["edge_style", form.edge_style, ["curved", "orthogonal", "straight", "sketch"]],
        ["marker_style", form.marker_style, ["stealth", "filled", "open", "classic"]],
      ] as const).map(([key, value, options]) => <Choice key={key} label={t(`themeWorkshop.form.${key}`)} value={value}
        choices={options.map((entry) => [entry, t(`themeWorkshop.values.${entry}`)])}
        onChange={(next) => onChange({ ...document, form: { ...document.form, [key]: next } })} />)}
      <Slider label={t("themeWorkshop.radius")} value={form.radius} min={0} max={24}
        onChange={(value) => onChange({ ...document, form: { ...document.form, radius: value } })} />
      <Choice label={t("themeWorkshop.shadow")} value={typeof elevation === "string" ? elevation : "custom"}
        choices={(["flat", "soft", "crisp"] as const).map((value) => [value, t(`themeWorkshop.values.${value}`)])}
        onChange={(value) => onChange(withAppearance(document, appearance, { elevation: value }))} />
    </ControlGroup>
    <ControlGroup title={t("themeWorkshop.groups.texture")}>
      <Choice label={t("themeWorkshop.pattern")} value={texture}
        choices={Object.keys(TEXTURES).map((value) => [value, t(`themeWorkshop.textures.${value}`)])}
        onChange={(value) => onChange(withAppearance(document, appearance, { canvas: { ...layer?.canvas, textures: TEXTURES[value] } }))} />
    </ControlGroup>
  </div>;
}

function ControlGroup({ title, children }: { title: string; children: ReactNode }) {
  return <fieldset className="min-w-0 space-y-3 rounded-lg border bg-card/50 px-3 pb-3 pt-1">
    <legend className="px-1 text-xs font-semibold">{title}</legend>
    {children}
  </fieldset>;
}

function Choice<Value extends string>({ label, value, choices, onChange }: {
  label: string; value: string; choices: readonly (readonly [Value, string])[]; onChange(value: Value): void;
}) {
  const { t } = useTranslation();
  return <label className="grid gap-1.5 text-xs">
    {label}
    <select value={value} className="h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs"
      onChange={(event) => {
        const choice = choices.find(([key]) => key === event.target.value);
        if (choice) onChange(choice[0]);
      }}>
      {!choices.some(([key]) => key === value) && <option value={value} disabled>{t("themeWorkshop.customValue")}</option>}
      {choices.map(([key, name]) => <option key={key} value={key} disabled={key === ""}>{name}</option>)}
    </select>
  </label>;
}

function Slider({ label, value, min, max, onChange }: {
  label: string; value: number; min: number; max: number; onChange(value: number): void;
}) {
  return <label className="grid grid-cols-[1fr_auto] gap-1.5 text-xs">
    <span>{label}</span><span className="tabular-nums text-muted-foreground" aria-hidden="true">{value} px</span>
    <input type="range" min={min} max={max} step={1} value={value} aria-valuetext={`${value} px`}
      className="col-span-2 w-full accent-primary" onChange={(event) => onChange(event.target.valueAsNumber)} />
  </label>;
}
