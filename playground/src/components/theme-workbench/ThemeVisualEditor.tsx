import { useId, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ThemeCatalog, ThemeRecipeV1, ThemeRuleEntry } from "@mermanjs/web";

import { ArrowDown, ArrowUp, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  appendThemeRule,
  getThemeStyleEntries,
  removeThemeCanvasBackground,
  restoreThemeCanvas,
  setCompleteTypography,
  setThemeCanvasBase,
  setThemeRuleFacet,
  moveThemeRule,
  removeThemeRule,
  setThemeRuleSelector,
  type ThemeRuleFacetValues,
} from "@/src/lib/theme-document";
import type { MermanDomainFacade } from "@/src/runtime/merman-core";
import { NumberField, ListField } from "./ThemeEditorFields";
import { PaintField } from "./ThemePaintEditor";
import { ThemeCanvasLayers } from "./ThemeCanvasLayers";
import { ThemeDesignEditor } from "./ThemeDesignEditor";

const FALLBACK_RULE_TARGETS = [
  "node", "node-label", "edge", "edge-label", "cluster", "cluster-label",
  "actor", "actor-label", "message", "message-label", "note", "note-label", "chart-series",
] as const;
const RULE_FACETS = [
  "fill", "opacity", "fill-opacity", "stroke-paint", "stroke-width", "stroke-dasharray",
  "stroke-line-cap", "stroke-line-join", "stroke-opacity", "radius", "padding", "font-stack",
  "font-size", "font-weight", "font-style", "line-height", "letter-spacing", "word-spacing",
  "text-transform", "text-decoration", "text-align", "white-space", "wrap", "effect",
] as const;
const SELECT_CLASS = "h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

function ruleTargets(catalog: ThemeCatalog | null): string[] {
  const targets = catalog?.known_semantic_target_ids ?? [...FALLBACK_RULE_TARGETS];
  return targets.filter((target) => target !== "canvas").sort();
}

export function ThemeVisualEditor({
  document,
  disabled,
  facade,
  catalog,
  familyId,
  onChange,
}: {
  document: ThemeRecipeV1;
  disabled: boolean;
  facade: MermanDomainFacade | null;
  catalog: ThemeCatalog | null;
  familyId?: string;
  onChange(document: ThemeRecipeV1): void;
}) {
  const { t } = useTranslation();

  if (document.kind === "design") return <ThemeDesignEditor document={document} disabled={disabled} facade={facade} catalog={catalog} familyId={familyId} onChange={onChange} />;

  return (
    <fieldset disabled={disabled} className="min-w-0 space-y-5 disabled:opacity-60">
      <p className="text-xs leading-relaxed text-muted-foreground">{t("themeWorkbench.visualHint", { defaultValue: "Edits preserve other fields. Advanced paints, resources, effects and typography remain available in JSON." })}</p>
      <ThemeCanvasEditor document={document} onChange={onChange} />
      {document.kind === "complete_spec" && (
        <section className="space-y-3">
          <h3 className="text-xs font-semibold">{t("themeWorkbench.typography", { defaultValue: "Typography" })}</h3>
          <ListField
            label={t("themeWorkbench.fontStack", { defaultValue: "Font stack, one family per line" })}
            value={document.complete_spec.typography?.default?.font_stack}
            onChange={(value) => onChange(setCompleteTypography(document, "font_stack", value))}
          />
          <NumberField label={t("themeWorkbench.fontSize", { defaultValue: "Font size (px)" })} value={document.complete_spec.typography?.default?.font_size_px} min={1} onChange={(value) => onChange(setCompleteTypography(document, "font_size_px", value ?? undefined))} />
          <NumberField label={t("themeWorkbench.fontWeight", { defaultValue: "Font weight" })} value={document.complete_spec.typography?.default?.font_weight} min={1} max={1000} onChange={(value) => onChange(setCompleteTypography(document, "font_weight", value ?? undefined))} />
        </section>
      )}
      <RuleEditor document={document} facade={facade} catalog={catalog} familyId={familyId} onChange={onChange} />
      <p className="border-t pt-3 text-[11px] leading-relaxed text-muted-foreground">{t("themeWorkbench.precedence", { defaultValue: "Later rules can replace earlier facets on the same target. Family filters are explicit scopes. Mermaid configuration and source styles can also affect the result; this editor does not infer per-field provenance." })}</p>
    </fieldset>
  );
}

function canvasIdentity(document: ThemeRecipeV1): string {
  return JSON.stringify(document.kind === "design" ? document.design.canvas ?? null : document.complete_spec.canvas ?? null);
}

function ThemeCanvasEditor({ document, onChange }: {
  document: ThemeRecipeV1;
  onChange(document: ThemeRecipeV1): void;
}) {
  const { t } = useTranslation();
  const [removed, setRemoved] = useState<{ before: ThemeRecipeV1; after: string } | null>(null);
  const base = document.kind === "complete_spec" ? document.complete_spec.canvas?.base : undefined;
  const layers = document.kind === "complete_spec" ? document.complete_spec.canvas?.layers ?? [] : [];
  const defaultLabel = t("themeWorkbench.canvasDefault");
  const canUndo = removed !== null && removed.after === canvasIdentity(document);

  return (
    <section className="space-y-3 rounded-md border p-3" aria-label={t("themeWorkbench.canvas")}>
      <h3 className="text-xs font-semibold">{t("themeWorkbench.canvas")}</h3>
      <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themeWorkbench.canvasExportHint")}</p>
      <PaintField label={t("themeWorkbench.canvasBase")} value={base} omitLabel={defaultLabel} onChange={(value) => {
        setRemoved(null);
        onChange(setThemeCanvasBase(document, value ?? undefined));
      }} />
      <div className="space-y-1 text-[11px] text-muted-foreground">
        <p>{t("themeWorkbench.canvasLayers", { count: layers.length })}</p>
        <p>{t("themeWorkbench.canvasLayersHint")}</p>
      </div>
      {document.kind === "complete_spec" && <ThemeCanvasLayers document={document} onChange={onChange} />}
      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="outline" size="sm" disabled={canUndo} onClick={() => {
          const next = removeThemeCanvasBackground(document);
          setRemoved({ before: document, after: canvasIdentity(next) });
          onChange(next);
        }}>{t("themeWorkbench.removeCanvas")}</Button>
        {canUndo && <Button type="button" variant="ghost" size="sm" onClick={() => {
          onChange(restoreThemeCanvas(document, removed.before));
          setRemoved(null);
        }}>{t("themeWorkbench.undoCanvas")}</Button>}
      </div>
      <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themeWorkbench.canvasHint")}</p>
    </section>
  );
}

function RuleEditor({ document, facade, catalog, familyId, onChange }: {
  document: ThemeRecipeV1;
  facade: MermanDomainFacade | null;
  catalog: ThemeCatalog | null;
  familyId?: string;
  onChange(document: ThemeRecipeV1): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const entries = getThemeStyleEntries(document);
  const rules = entries.flatMap((entry, index) => entry.kind === "rule" ? [{ rule: entry, index }] : []);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const active = rules.find((entry) => entry.index === selectedIndex) ?? rules[0];
  const [newTarget, setNewTarget] = useState("node");
  const [newFamily, setNewFamily] = useState("current");
  const [initialPaint, setInitialPaint] = useState("#ffffff");
  const families = useMemo(() => {
    try {
      return [...new Set(facade?.diagramFamilyCapabilities().map((family) => family.family_id) ?? [])].sort();
    } catch {
      return [];
    }
  }, [facade]);
  const targets = useMemo(() => ruleTargets(catalog), [catalog]);
  const selectedTarget = targets.includes(newTarget) ? newTarget : targets[0] ?? "";
  const canEdit = active && targets.includes(active.rule.target);

  function edit<Facet extends keyof ThemeRuleFacetValues>(facet: Facet, value: ThemeRuleFacetValues[Facet]) {
    if (active) onChange(setThemeRuleFacet(document, active.index, facet, value));
  }

  return (
    <section className="min-w-0 space-y-3 border-t pt-4">
      <h3 className="text-xs font-semibold">{t("themeWorkbench.rules", { defaultValue: "Scoped rules" })}</h3>
      <label htmlFor={`${id}-rule`} className="block text-xs text-muted-foreground">{t("themeWorkbench.ruleOrder", { defaultValue: "Authored order, later rules first in precedence" })}</label>
      <select id={`${id}-rule`} className={SELECT_CLASS} value={active?.index ?? ""} onChange={(event) => setSelectedIndex(Number(event.target.value))} disabled={!rules.length}>
        {!rules.length && <option value="">{t("themeWorkbench.noRules", { defaultValue: "No explicit rules" })}</option>}
        {rules.map(({ rule, index }) => <option key={index} value={index}>{index + 1}. {rule.family ?? "*"} / {rule.target}{rule.variant ? ` / ${rule.variant}` : ""}{rule.ordinal ? ` / ${JSON.stringify(rule.ordinal)}` : ""}</option>)}
      </select>
      {active && <>
        <p className="break-words text-[11px] text-muted-foreground">{t("themeWorkbench.ruleScope", { defaultValue: "Scope: {{family}} · {{target}}", family: active.rule.family ?? t("themeWorkbench.allFamilies", { defaultValue: "All families" }), target: active.rule.target })}{active.rule.variant ? ` · ${active.rule.variant}` : ""}{active.rule.ordinal ? ` · ${JSON.stringify(active.rule.ordinal)}` : ""}</p>
        {canEdit ? <div className="space-y-3 rounded-md border p-3">
          <div className="grid min-w-0 gap-2 sm:grid-cols-2">
            <div className="min-w-0 space-y-1.5">
              <label htmlFor={`${id}-active-target`} className="block text-xs font-medium">{t("themeWorkbench.ruleTarget", { defaultValue: "Rule semantic target" })}</label>
              <select id={`${id}-active-target`} className={SELECT_CLASS} value={active.rule.target} onChange={(event) => onChange(setThemeRuleSelector(document, active.index, { target: event.target.value }))}>
                {targets.map((target) => <option key={target} value={target}>{target}</option>)}
                {!targets.includes(active.rule.target) && <option value={active.rule.target}>{active.rule.target}</option>}
              </select>
            </div>
            <div className="min-w-0 space-y-1.5">
              <label htmlFor={`${id}-active-family`} className="block text-xs font-medium">{t("themeWorkbench.ruleFamily")}</label>
              <select id={`${id}-active-family`} value={active.rule.family ?? ""} className={SELECT_CLASS} onChange={(event) => onChange(setThemeRuleSelector(document, active.index, { family: event.target.value }))}>
                <option value="">{t("themeWorkbench.allFamilies")}</option>
                {families.map((family) => <option key={family} value={family}>{t(`diagramTypes.${family}`, { defaultValue: family })}</option>)}
                {active.rule.family && !families.includes(active.rule.family) && <option value={active.rule.family}>{active.rule.family}</option>}
              </select>
            </div>
          </div>
          <label className="block space-y-1.5 text-xs font-medium"><span>{t("themeWorkbench.variant")}</span><select value={active.rule.variant ?? ""} className={SELECT_CLASS} onChange={(event) => onChange(setThemeRuleSelector(document, active.index, { variant: event.target.value }))}>
            <option value="">{t("themeWorkbench.allVariants")}</option>
            {catalog?.known_variant_ids.map((variant) => <option key={variant} value={variant}>{variant}</option>)}
            {active.rule.variant && !catalog?.known_variant_ids.includes(active.rule.variant) && <option value={active.rule.variant}>{active.rule.variant}</option>}
          </select></label>
          <PaintField label={t("themeWorkbench.fill", { defaultValue: "Fill" })} value={active.rule.style.fill} clearable onChange={(value) => edit("fill", value)} />
          <PaintField label={t("themeWorkbench.strokePaint", { defaultValue: "Border / line paint" })} value={active.rule.style.stroke?.paint} clearable onChange={(value) => edit("stroke.paint", value)} />
          <NumberField label={t("themeWorkbench.strokeWidth", { defaultValue: "Border / line width" })} value={active.rule.style.stroke?.width} clearable min={0} onChange={(value) => edit("stroke.width", value)} />
          <NumberField label={t("themeWorkbench.opacity", { defaultValue: "Opacity" })} value={active.rule.style.opacity} clearable min={0} max={1} onChange={(value) => edit("opacity", value)} />
          <NumberField label={t("themeWorkbench.radius", { defaultValue: "Corner radius" })} value={active.rule.style.radius} clearable min={0} onChange={(value) => edit("radius", value)} />
          <NumberField label={t("themeWorkbench.fontSize", { defaultValue: "Font size (px)" })} value={active.rule.style.typography?.font_size_px} clearable min={1} onChange={(value) => edit("typography.font_size_px", value)} />
          <NumberField label={t("themeWorkbench.fontWeight", { defaultValue: "Font weight" })} value={active.rule.style.typography?.font_weight} clearable min={1} max={1000} onChange={(value) => edit("typography.font_weight", value)} />
          <div className="flex flex-wrap gap-1.5 border-t pt-2">
            <Button type="button" variant="outline" size="sm" disabled={active.index === 0} onClick={() => { onChange(moveThemeRule(document, active.index, "up")); setSelectedIndex(active.index - 1); }}><ArrowUp className="size-3.5" />{t("themeWorkbench.moveUp", { defaultValue: "Move up" })}</Button>
            <Button type="button" variant="outline" size="sm" disabled={active.index >= entries.length - 1} onClick={() => { onChange(moveThemeRule(document, active.index, "down")); setSelectedIndex(active.index + 1); }}><ArrowDown className="size-3.5" />{t("themeWorkbench.moveDown", { defaultValue: "Move down" })}</Button>
            <Button type="button" variant="destructive" size="sm" onClick={() => { onChange(removeThemeRule(document, active.index)); setSelectedIndex(null); }}><Trash2 className="size-3.5" />{t("themeWorkbench.deleteRule", { defaultValue: "Delete rule" })}</Button>
          </div>
          <RuleSupport facade={facade} rule={active.rule} familyId={familyId} />
        </div> : <p className="text-xs text-muted-foreground">{t("themeWorkbench.advancedRule", { defaultValue: "Edit this target in JSON. Its authored rule is preserved." })}</p>}
      </>}
      <details className="rounded-md border p-3">
        <summary className="cursor-pointer text-xs font-medium">{t("themeWorkbench.appendRule", { defaultValue: "Append an override" })}</summary>
        <div className="mt-3 space-y-2">
          <label htmlFor={`${id}-target`} className="block text-xs">{t("themeWorkbench.target", { defaultValue: "Semantic target" })}</label>
          <select id={`${id}-target`} className={SELECT_CLASS} value={selectedTarget} onChange={(event) => setNewTarget(event.target.value)}>{targets.map((target) => <option key={target} value={target}>{target}</option>)}</select>
          <label htmlFor={`${id}-family`} className="block text-xs">{t("themeWorkbench.family", { defaultValue: "Family scope" })}</label>
          <select id={`${id}-family`} className={SELECT_CLASS} value={newFamily} onChange={(event) => setNewFamily(event.target.value)}>
            <option value="current" disabled={!familyId}>{t("themeWorkbench.currentFamily", { defaultValue: "Current family" })}{familyId ? ` (${familyId})` : ""}</option>
            <option value="all">{t("themeWorkbench.allFamilies", { defaultValue: "All families" })}</option>
            {families.map((family) => <option key={family} value={`family:${family}`}>{t(`diagramTypes.${family}`, { defaultValue: family })}</option>)}
          </select>
          <label htmlFor={`${id}-initial-paint`} className="block text-xs">{t("themeWorkbench.initialPaint", { defaultValue: "Initial border / line paint" })}</label>
          <Input id={`${id}-initial-paint`} value={initialPaint} className="h-8 font-mono text-xs" onChange={(event) => setInitialPaint(event.target.value)} />
          <Button type="button" variant="outline" size="sm" disabled={!targets.length || (newFamily === "current" && !familyId)} onClick={() => {
            const family = newFamily === "all" ? undefined : newFamily === "current" ? familyId : newFamily.slice("family:".length);
            onChange(appendThemeRule(document, { target: selectedTarget, ...(family ? { family } : {}) }, { stroke: { paint: initialPaint } }));
            setSelectedIndex(entries.length);
          }}>{t("themeWorkbench.append", { defaultValue: "Append rule" })}</Button>
        </div>
      </details>
      {entries.some((entry) => entry.kind !== "rule") && <p className="text-[11px] text-muted-foreground">{t("themeWorkbench.palettePreserved", { defaultValue: "Ordinal palettes remain intact and can be edited in JSON." })}</p>}
    </section>
  );
}

function RuleSupport({ facade, rule, familyId }: {
  facade: MermanDomainFacade | null;
  rule: ThemeRuleEntry;
  familyId?: string;
}) {
  const { t } = useTranslation();
  const family = rule.family ?? familyId;
  const [open, setOpen] = useState(false);
  const [facet, setFacet] = useState<(typeof RULE_FACETS)[number]>(RULE_FACETS[0]);
  const [output, setOutput] = useState<"standalone-svg" | "browser-svg">("standalone-svg");
  const support = useMemo(() => {
    if (!open || !facade || !family) return null;
    try {
      return facade.describeThemeSupport({ schema_version: 1, family, output, subject: { kind: "rule", target: rule.target, facet } });
    } catch {
      return null;
    }
  }, [facade, family, open, output, facet, rule.target]);

  return (
    <details className="border-t pt-2 text-[11px] text-muted-foreground" onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary className="cursor-pointer">{t("themeWorkbench.support", { defaultValue: "Capability discovery" })}{family ? ` · ${family} / ${output}` : ""}</summary>
      <p className="mt-2 leading-relaxed">{t("themeWorkbench.supportHint", { defaultValue: "Static route information only. It is separate from the current render outcome and output portability. Unknown results stay unverified." })}</p>
      {family ? <div className="mt-2 space-y-2">
        <div className="grid min-w-0 gap-2 sm:grid-cols-2">
          <label className="min-w-0 space-y-1"><span className="block">{t("themeWorkbench.facet", { defaultValue: "Facet" })}</span><select className={SELECT_CLASS} value={facet} onChange={(event) => setFacet(event.target.value as (typeof RULE_FACETS)[number])}>{RULE_FACETS.map((item) => <option key={item} value={item}>{item}</option>)}</select></label>
          <label className="min-w-0 space-y-1"><span className="block">{t("themeWorkbench.output", { defaultValue: "Output" })}</span><select className={SELECT_CLASS} value={output} onChange={(event) => setOutput(event.target.value as typeof output)}><option value="standalone-svg">standalone-svg</option><option value="browser-svg">browser-svg</option></select></label>
        </div>
        {support ? <p className="break-words">{facet}: <code>{support.state}</code>{support.reason_ids.length ? ` (${support.reason_ids.join(", ")})` : ""}</p> : <p>{t("themeWorkbench.unknownFamily", { defaultValue: "A diagram family is needed to query support." })}</p>}
      </div> : <p className="mt-2">{t("themeWorkbench.unknownFamily", { defaultValue: "A diagram family is needed to query support." })}</p>}
    </details>
  );
}
