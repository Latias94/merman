import type { ResolvedThemeV2, ThemeAppearance, ThemeAppearanceLayer, ThemeDocumentV2, ThemeFamilyOverride, ThemePaint, ThemeResolution, ThemeRoles } from "@mermanjs/web";
import { THEME_APPEARANCES } from "./v2.ts";

export function withAppearance(
  document: ThemeDocumentV2,
  appearance: ThemeAppearance,
  patch: ThemeAppearanceLayer,
): ThemeDocumentV2 {
  return { ...document, appearances: {
    ...document.appearances,
    [appearance]: { ...document.appearances?.[appearance], ...patch },
  } };
}

/** Inline a single preset using the compiler's layer order. */
export function expandThemeDocument(document: ThemeDocumentV2, base: ThemeDocumentV2): ThemeDocumentV2 {
  const { extends: _extends, ...standalone } = document;
  const result: ThemeDocumentV2 = {
    ...standalone,
    $schema: document.$schema ?? base.$schema,
    default_appearance: document.default_appearance ?? base.default_appearance,
    typography: merge(base.typography, document.typography),
    form: base.form || document.form ? {
      ...base.form, ...document.form, chart: merge(base.form?.chart, document.form?.chart),
    } : undefined,
    appearances: {},
    overrides: [...(base.overrides ?? []), ...(document.overrides ?? [])],
  };
  return THEME_APPEARANCES.reduce((expanded, appearance) => {
    const inherited = base.appearances?.[appearance];
    const own = document.appearances?.[appearance];
    if (!inherited && !own) return expanded;
    const seed = { ...inherited?.seed, ...base.seed, ...own?.seed };
    // A standalone appearance must declare the required seeds in its own layer.
    for (const key of ["canvas", "ink", "accent"] as const) {
      if (seed[key] === undefined && document.seed?.[key] !== undefined) seed[key] = document.seed[key];
    }
    return withAppearance(expanded, appearance, {
      seed,
      roles: merge(inherited?.roles, base.roles, own?.roles),
      canvas: merge(inherited?.canvas, own?.canvas),
      data: merge(inherited?.data, own?.data),
      elevation: own?.elevation ?? (document.form?.elevation === undefined ? inherited?.elevation : undefined),
    });
  }, result);
}

function merge<T extends object>(...patches: (T | undefined)[]): T | undefined {
  return patches.some((patch) => patch !== undefined) ? Object.assign({}, ...patches) as T : undefined;
}

/** Commit compiler-proposed adjustments, or return null if they exceed the override limit. */
export function applyContrastFix(
  document: ThemeDocumentV2,
  before: ResolvedThemeV2,
  after: ResolvedThemeV2,
  appearances: readonly ThemeAppearance[],
  base?: ThemeDocumentV2,
): ThemeDocumentV2 | null {
  const appearance = after.appearance;
  const overrides = fixFamilyOverrides(document.overrides ?? [], base?.overrides ?? [], before, after,
    appearances.length === 1 && appearances[0] === appearance);
  if (!overrides) return null;

  const changes = changedTextRoles(before.roles, after.roles);
  const sharedRoles = { ...document.roles };
  let next = document;
  for (const [role] of TEXT_PAIRS) {
    if (changes[role] === undefined) continue;
    if (!Object.hasOwn(sharedRoles, role)) continue;
    for (const other of appearances) {
      if (other === appearance) continue;
      next = withAppearance(next, other, { roles: { ...next.appearances?.[other]?.roles, [role]: sharedRoles[role] } });
    }
    delete sharedRoles[role];
  }
  const layer = next.appearances?.[appearance];
  const data = {
    ...layer?.data,
    ...(before.data.ink.some((color, i) => after.data.ink[i] !== color) ? { ink: after.data.ink } : {}),
    ...(before.data.fill_ink.some((color, i) => after.data.fill_ink[i] !== color) ? { fill_ink: "auto" } : {}),
  };
  return withAppearance({ ...next, roles: Object.keys(sharedRoles).length ? sharedRoles : undefined,
    overrides: overrides.length ? overrides : undefined }, appearance, {
    roles: merge(layer?.roles, Object.keys(changes).length ? changes : undefined),
    data: Object.keys(data).length ? data : undefined,
  });
}

type TextRolePatch = Partial<Record<(typeof TEXT_PAIRS)[number][0], string>>;

// Theme v2 schema limits for the override array and each rule's family list.
const MAX_OVERRIDES = 32;
const MAX_FAMILIES_PER_OVERRIDE = 34;

function fixFamilyOverrides(
  authored: readonly ThemeFamilyOverride[],
  inherited: readonly ThemeFamilyOverride[],
  before: ResolvedThemeV2,
  after: ResolvedThemeV2,
  onlyAppearance: boolean,
): ThemeFamilyOverride[] | null {
  const { appearance } = after;
  const overrides = [...authored];
  const applies = (rule: ThemeFamilyOverride) => rule.appearance == null || rule.appearance === appearance;
  const active = authored.map((rule, index) => ({ rule, index })).filter(({ rule }) => applies(rule));
  const writable = active.filter(({ rule }) => onlyAppearance || rule.appearance === appearance).reverse();
  const inheritedRules = inherited.filter(applies);
  const families = Object.entries(after.family_overrides ?? {});
  const pending = new Map<string, TextRolePatch>();

  for (const [role] of TEXT_PAIRS) {
    // Inherited rules precede every authored rule and are read-only.
    const lastWriter = new Map<string, number>();
    for (const rule of inheritedRules) {
      if (rule.roles?.[role] != null) for (const id of rule.families) lastWriter.set(id, -1);
    }
    for (const { rule, index } of active) {
      if (rule.roles?.[role] != null) for (const id of rule.families) lastWriter.set(id, index);
    }

    const corrections = new Map<string, string>();
    for (const [id, family] of families) {
      const original = before.family_overrides?.[id];
      if (!original) continue;
      const current = lastWriter.has(id) ? original.roles[role] : after.roles[role];
      if (current !== family.roles[role]) corrections.set(id, family.roles[role]);
    }
    if (!corrections.size) continue;

    // Prefer editing the winning declaration before adding a role to another rule.
    for (const hasRole of [true, false]) {
      for (const { rule, index } of writable) {
        if ((rule.roles?.[role] != null) !== hasRole) continue;
        const winners = rule.families.filter((id) => (lastWriter.get(id) ?? -1) <= index);
        const color = corrections.get(winners[0]);
        if (color === undefined || !winners.every((id) => corrections.get(id) === color)) continue;
        overrides[index] = { ...overrides[index], roles: { ...overrides[index].roles, [role]: color } };
        for (const id of winners) {
          corrections.delete(id);
          lastWriter.set(id, index);
        }
      }
    }

    for (const [id, color] of corrections) {
      const roles = pending.get(id) ?? {};
      roles[role] = color;
      pending.set(id, roles);
    }
  }

  const groups = new Map<string, { families: string[]; roles: TextRolePatch }>();
  for (const [id, roles] of pending) {
    const key = JSON.stringify(roles);
    const group = groups.get(key);
    if (group) group.families.push(id);
    else groups.set(key, { families: [id], roles });
  }
  for (const { families, roles } of groups.values()) {
    for (let start = 0; start < families.length; start += MAX_FAMILIES_PER_OVERRIDE) {
      if (overrides.length === MAX_OVERRIDES) return null;
      overrides.push({ families: families.slice(start, start + MAX_FAMILIES_PER_OVERRIDE), appearance, roles });
    }
  }
  return overrides;
}

function changedTextRoles(before: ThemeRoles, after: ThemeRoles): Partial<ThemeRoles> {
  return Object.fromEntries(TEXT_PAIRS.map(([key]) => key).filter((key) => before[key] !== after[key]).map((key) => [key, after[key]]));
}

const TEXT_PAIRS = [
  ["text.primary", "canvas", "surface.node", "surface.container"],
  ["text.muted", "canvas", "surface.node"],
  ["text.title", "canvas", "surface.container"],
  ["text.callout", "surface.callout"],
  ["text.on_emphasis", "surface.emphasis"],
] as const;

/** The compiler reports AA failures. Reserve AAA for checked opaque colors. */
export function contrastGrade(resolution: Extract<ThemeResolution, { ok: true }>): "warning" | "AA" | "AAA" {
  if (resolution.notices.some((notice) => notice.kind === "low_contrast")) return "warning";
  const { resolved } = resolution;
  const aaa = [resolved, ...Object.values(resolved.family_overrides ?? {})].every(({ roles, data, form }) =>
    TEXT_PAIRS.every(([ink, ...surfaces]) => surfaces.every((surface) => contrast(rgb(roles[ink]), rgb(roles[surface])) >= 7)) &&
    data.fill.every((fill, index) => {
      const canvas = rgb(roles.canvas);
      const color = rgb(fill);
      const opacity = form.chart.pie_opacity;
      const background = canvas && color?.map((value, channel) => value * opacity + canvas[channel] * (1 - opacity));
      return contrast(rgb(data.fill_ink[index]), background) >= 7;
    }),
  );
  return aaa ? "AAA" : "AA";
}

function rgb(paint: ThemePaint | undefined): number[] | undefined {
  return typeof paint === "string" && /^#[\da-f]{6}$/i.test(paint)
    ? [1, 3, 5].map((index) => Number.parseInt(paint.slice(index, index + 2), 16) / 255)
    : undefined;
}

function contrast(foreground: number[] | undefined, background: number[] | undefined): number {
  if (!foreground || !background) return 0;
  const weights = [0.2126, 0.7152, 0.0722];
  const luminance = (color: number[]) => color.reduce((sum, value, index) =>
    sum + weights[index] * (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4), 0);
  const a = luminance(foreground);
  const b = luminance(background);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}
