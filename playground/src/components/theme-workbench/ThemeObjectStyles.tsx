import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowDown, ArrowUp, Trash2 } from "lucide-react";
import type { ThemeCatalog, ThemeDesignV1, ThemeDesignRuleV1 } from "@mermanjs/web";
import { Button } from "@/components/ui/button";
import { setThemeDesignPath } from "@/src/lib/theme-document";
import type { MermanDomainFacade } from "@/src/runtime/merman-core";
import { DESIGN_SELECT_CLASS, DesignObjectFields, type DesignFieldContext } from "./ThemeDesignFields";

export type ThemeObjectSelection = { kind: "style"; target: string } | { kind: "rule"; index: number };

export function ThemeObjectStyles({
  context,
  facade,
  catalog,
  familyId,
  selection,
  onSelect,
  onChange,
}: {
  context: DesignFieldContext;
  facade: MermanDomainFacade | null;
  catalog: ThemeCatalog | null;
  familyId?: string;
  selection: ThemeObjectSelection;
  onSelect(selection: ThemeObjectSelection): void;
  onChange(design: ThemeDesignV1): void;
}) {
  const { t } = useTranslation();
  const { design } = context;
  const targets = (
    catalog?.known_semantic_target_ids ?? [
      "node",
      "node-label",
      "edge",
      "edge-label",
      "cluster",
      "cluster-label",
    ]
  ).filter((target) => target !== "canvas");
  const target = selection.kind === "style" ? selection.target : "node";
  const [newTarget, setNewTarget] = useState("node");
  const [newFamily, setNewFamily] = useState("current");
  const rules = design.rules ?? [];
  const index = selection.kind === "rule" ? selection.index : 0;
  const rule = rules[index];
  const families = useMemo(() => {
    try {
      return [
        ...new Set(
          facade
            ?.diagramFamilyCapabilities()
            .map((family) => family.family_id) ?? [],
        ),
      ].sort();
    } catch {
      return [];
    }
  }, [facade]);
  function edit(path: (string | number)[], next: unknown) {
    onChange(setThemeDesignPath(design, path, next));
  }
  function selectRule(index: number) { onSelect({ kind: "rule", index }); }
  function selector(key: keyof ThemeDesignRuleV1, value: string) {
    edit(["rules", index, key], value || undefined);
  }
  return (
    <section className="min-w-0 space-y-3 border-t pt-4">
      <h3 className="text-xs font-semibold">
        {t("themeDesigner.semanticStyles")}
      </h3>
      <p className="text-[11px] text-muted-foreground">
        {t("themeDesigner.semanticHint")}
      </p>
      <div className="flex flex-wrap gap-1.5" role="group" aria-label={t("themeDesigner.styleScope")}>
        <Button type="button" size="sm" variant={selection.kind === "style" ? "secondary" : "outline"} aria-pressed={selection.kind === "style"} onClick={() => onSelect({ kind: "style", target: "node" })}>{t("themeDesigner.semanticDefaults")}</Button>
        <Button type="button" size="sm" variant={selection.kind === "rule" ? "secondary" : "outline"} aria-pressed={selection.kind === "rule"} onClick={() => selectRule(0)}>{t("themeWorkbench.rules")}</Button>
      </div>
      {selection.kind === "style" ? <>
      <label className="block space-y-1 text-xs">
        <span>{t("themeDesigner.object")}</span>
        <select
          aria-label={t("themeDesigner.object")}
          className={DESIGN_SELECT_CLASS}
          value={target}
          onChange={(event) => onSelect({ kind: "style", target: event.target.value })}
        >
          {[...new Set([...targets, ...Object.keys(design.styles ?? {})])].map(
            (item) => (
              <option key={item} value={item}>
                {t(`themeDesigner.target.${item}`, { defaultValue: item })}
              </option>
            ),
          )}
        </select>
      </label>
      <DesignObjectFields
        kind="style"
        value={design.styles?.[target] ?? {}}
        path={["styles", target]}
        context={context}
        onChange={(next) => edit(["styles", target], next)}
      />
      </> : <>
      <h3 className="pt-2 text-xs font-semibold">
        {t("themeWorkbench.rules")}
      </h3>
      {rules.length > 0 && (
        <select
          aria-label={t("themeWorkbench.ruleOrder")}
          className={DESIGN_SELECT_CLASS}
          value={index}
          onChange={(event) => selectRule(Number(event.target.value))}
        >
          {rules.map((entry, i) => (
            <option key={i} value={i}>
              {i + 1}. {entry.family ?? "*"} / {entry.target}
              {entry.variant ? ` / ${entry.variant}` : ""}
            </option>
          ))}
        </select>
      )}
      {!rules.length && <p className="text-xs text-muted-foreground">{t("themeWorkbench.noRules")}</p>}
      {rules.length > 0 && !rule && <p role="status" className="text-xs text-muted-foreground">{t("themeDesigner.ruleUnavailable")}</p>}
      {rule && (
        <div className="space-y-3 rounded-md border p-2">
          <label className="block space-y-1 text-xs">
            <span>{t("themeWorkbench.ruleTarget")}</span>
            <select
              aria-label={t("themeWorkbench.ruleTarget")}
              className={DESIGN_SELECT_CLASS}
              value={rule.target}
              onChange={(event) => selector("target", event.target.value)}
            >
              {[...new Set([...targets, rule.target])].map((item) => (
                <option key={item} value={item}>
                  {t(`themeDesigner.target.${item}`, { defaultValue: item })}
                </option>
              ))}
            </select>
          </label>
          <label className="block space-y-1 text-xs">
            <span>{t("themeWorkbench.ruleFamily")}</span>
            <select
              aria-label={t("themeWorkbench.ruleFamily")}
              className={DESIGN_SELECT_CLASS}
              value={rule.family ?? ""}
              onChange={(event) => selector("family", event.target.value)}
            >
              <option value="">{t("themeWorkbench.allFamilies")}</option>
              {[
                ...new Set([
                  ...families,
                  ...(rule.family ? [rule.family] : []),
                ]),
              ].map((item) => (
                <option key={item} value={item}>
                  {t(`themeDesigner.target.${item}`, { defaultValue: item })}
                </option>
              ))}
            </select>
          </label>
          <label className="block space-y-1 text-xs">
            <span>{t("themeWorkbench.variant")}</span>
            <select
              aria-label={t("themeWorkbench.variant")}
              className={DESIGN_SELECT_CLASS}
              value={rule.variant ?? ""}
              onChange={(event) => selector("variant", event.target.value)}
            >
              <option value="">{t("themeWorkbench.allVariants")}</option>
              {[
                ...new Set([
                  ...(catalog?.known_variant_ids ?? []),
                  ...(rule.variant ? [rule.variant] : []),
                ]),
              ].map((item) => (
                <option key={item} value={item}>
                  {t(`themeDesigner.target.${item}`, { defaultValue: item })}
                </option>
              ))}
            </select>
          </label>
          {rule.ordinal && (
            <p className="text-[11px] text-muted-foreground">
              {t("themeDesigner.ordinalPreserved", {
                selector: JSON.stringify(rule.ordinal),
              })}
            </p>
          )}
          <DesignObjectFields
            kind="style"
            value={rule.style}
            path={["rules", index, "style"]}
            context={context}
            onChange={(next) => edit(["rules", index, "style"], next)}
          />
          <div className="flex flex-wrap gap-1.5">
            {([-1, 1] as const).map((direction) => (
              <Button
                key={direction}
                type="button"
                size="sm"
                variant="outline"
                disabled={
                  index + direction < 0 || index + direction >= rules.length
                }
                onClick={() => {
                  const next = [...rules];
                  [next[index], next[index + direction]] = [
                    next[index + direction],
                    next[index],
                  ];
                  selectRule(index + direction);
                  edit(["rules"], next);
                }}
              >
                {direction === -1 ? (
                  <ArrowUp className="size-3.5" />
                ) : (
                  <ArrowDown className="size-3.5" />
                )}
                {t(
                  direction === -1
                    ? "themeWorkbench.moveUp"
                    : "themeWorkbench.moveDown",
                )}
              </Button>
            ))}
            <Button
              type="button"
              variant="destructive"
              size="sm"
              onClick={() =>
                edit(
                  ["rules"],
                  rules.filter((_, i) => i !== index),
                )
              }
            >
              <Trash2 className="size-3.5" />
              {t("themeWorkbench.deleteRule")}
            </Button>
          </div>
        </div>
      )}
      <details className="rounded-md border p-3">
        <summary className="cursor-pointer text-xs font-medium">
          {t("themeWorkbench.appendRule")}
        </summary>
        <div className="mt-3 space-y-2">
          <label className="block space-y-1 text-xs">
            <span>{t("themeWorkbench.target")}</span>
            <select
              aria-label={t("themeWorkbench.target")}
              className={DESIGN_SELECT_CLASS}
              value={newTarget}
              onChange={(event) => setNewTarget(event.target.value)}
            >
              {targets.map((item) => (
                <option key={item} value={item}>
                  {t(`themeDesigner.target.${item}`, { defaultValue: item })}
                </option>
              ))}
            </select>
          </label>
          <label className="block space-y-1 text-xs">
            <span>{t("themeWorkbench.family")}</span>
            <select
              aria-label={t("themeWorkbench.family")}
              className={DESIGN_SELECT_CLASS}
              value={newFamily}
              onChange={(event) => setNewFamily(event.target.value)}
            >
              <option value="current" disabled={!familyId}>
                {t("themeWorkbench.currentFamily")}
              </option>
              <option value="all">{t("themeWorkbench.allFamilies")}</option>
              {families.map((family) => (
                <option key={family} value={`family:${family}`}>
                  {family}
                </option>
              ))}
            </select>
          </label>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={newFamily === "current" && !familyId}
            onClick={() => {
              const family =
                newFamily === "all"
                  ? undefined
                  : newFamily === "current"
                    ? familyId
                    : newFamily.slice(7);
              edit(
                ["rules"],
                [
                  ...rules,
                  {
                    target: newTarget,
                    ...(family ? { family } : {}),
                    style: { stroke: { paint: "#64748b" } },
                  },
                ],
              );
              selectRule(rules.length);
            }}
          >
            {t("themeWorkbench.append")}
          </Button>
        </div>
      </details>
      </>}
    </section>
  );
}
