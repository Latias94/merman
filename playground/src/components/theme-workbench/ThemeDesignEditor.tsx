import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowDown, ArrowUp, Copy, Plus, Trash2 } from "lucide-react";
import type {
  ThemeCatalog,
  ThemeDesignV1,
  ThemeDesignTokenV1,
  ThemeRecipeV1,
  ThemeDesignRuleV1,
} from "@mermanjs/web";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  copyThemeReferenceValue,
  removeThemeCanvasBackground,
  restoreThemeCanvas,
  setThemeDesignPath,
  themeTokenDomain,
  renameThemeToken,
} from "@/src/lib/theme-document";
import type { MermanDomainFacade } from "@/src/runtime/merman-core";
import { projectError } from "@/src/runtime/error-projection";
import { stripeLayer } from "./ThemeCanvasLayers";
import { newThemePaint } from "./ThemePaintEditor";
import {
  DESIGN_SELECT_CLASS,
  designDefault,
  DesignObjectFields,
  DesignValueField,
  type DesignFieldContext,
  type DesignType,
} from "./ThemeDesignFields";

type DesignRecipe = Extract<ThemeRecipeV1, { kind: "design" }>;
const RESOURCE_TYPES: DesignType[] = [
  "color",
  "number",
  "string",
  "length",
  "insets",
  "line-height",
  "font-stack",
  "palette",
  "paint",
  "stroke",
  "text-style",
  "effect",
];

export function ThemeDesignEditor({
  document,
  disabled,
  facade,
  catalog,
  familyId,
  onChange,
}: {
  document: DesignRecipe;
  disabled: boolean;
  facade: MermanDomainFacade | null;
  catalog: ThemeCatalog | null;
  familyId?: string;
  onChange(document: ThemeRecipeV1): void;
}) {
  const { t } = useTranslation();
  const design = document.design;
  const materialized = useMemo(() => {
    try {
      return { value: facade?.materializeTheme(design), error: null };
    } catch (cause) {
      return { value: undefined, error: projectError(cause).summary };
    }
  }, [design, facade]);
  const context: DesignFieldContext = {
    design,
    references: materialized.value?.references ?? [],
  };
  function change(next: ThemeDesignV1) {
    onChange({ ...document, design: next });
  }
  return (
    <fieldset
      disabled={disabled}
      className="min-w-0 space-y-5 disabled:opacity-60"
    >
      <p className="text-xs leading-relaxed text-muted-foreground">
        {t("themeDesigner.introduction")}
      </p>
      {materialized.error && (
        <p role="alert" className="text-xs text-destructive">
          {materialized.error}
        </p>
      )}
      <SharedValues context={context} onChange={change} />
      <DesignCanvas document={document} context={context} onChange={onChange} />
      <section className="space-y-3 border-t pt-4">
        <h3 className="text-xs font-semibold">
          {t("themeWorkbench.typography")}
        </h3>
        <DesignValueField
          label={t("themeDesigner.defaultTypography")}
          type="text-style"
          value={design.typography?.default}
          path={["typography", "default"]}
          context={context}
          optional
          onChange={(next) =>
            change(setThemeDesignPath(design, ["typography", "default"], next))
          }
        />
      </section>
      <DesignSemanticStyles
        context={context}
        facade={facade}
        catalog={catalog}
        familyId={familyId}
        onChange={change}
      />
      <p className="border-t pt-3 text-[11px] leading-relaxed text-muted-foreground">
        {t("themeWorkbench.precedence")}
      </p>
    </fieldset>
  );
}

function SharedValues({
  context,
  onChange,
}: {
  context: DesignFieldContext;
  onChange(design: ThemeDesignV1): void;
}) {
  const { t } = useTranslation();
  const { design, references } = context;
  const [newLabel, setNewLabel] = useState("");
  const [resourceError, setResourceError] = useState<string | null>(null);
  const [newType, setNewType] = useState<DesignType>("color");
  const groups = new Map<string, [string, ThemeDesignTokenV1][]>();
  for (const [id, token] of Object.entries(design.tokens ?? {})) {
    const group =
      token.group ??
      (token.type === "color" ||
      token.type === "palette" ||
      token.type === "paint"
        ? t("themeDesigner.colors")
        : token.type === "text-style" ||
            token.type === "font-stack" ||
            token.type === "line-height"
          ? t("themeWorkbench.typography")
          : token.type === "effect"
            ? t("themeDesigner.effects")
            : t("themeDesigner.dimensions"));
    groups.set(group, [...(groups.get(group) ?? []), [id, token]]);
  }
  return (
    <section
      className="min-w-0 space-y-3"
      aria-label={t("themeDesigner.sharedValues")}
    >
      <div>
        <h3 className="text-sm font-semibold">
          {t("themeDesigner.sharedValues")}
        </h3>
        <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground">
          {t("themeDesigner.sharedHint")}
        </p>
      </div>
      {[...groups].map(([group, tokens]) => (
        <section key={group} className="min-w-0 space-y-2">
          <h4 className="text-xs font-semibold text-muted-foreground">
            {group}
          </h4>
          {tokens.map(([id, token]) => {
            const count = references.filter((use) => use.token === id).length;
            let swatch: string | undefined;
            if (token.type === "color") {
              try {
                const literal = copyThemeReferenceValue(design, id);
                if (typeof literal === "string") swatch = literal;
              } catch {
                /* Invalid references are reported by Rust validation. */
              }
            }
            return (
              <details
                key={id}
                className="min-w-0 rounded-md border"
                data-theme-resource={id}
              >
                <summary className="flex cursor-pointer items-center gap-2 px-3 py-2 text-xs font-medium">
                  {swatch && (
                    <span
                      aria-hidden="true"
                      className="size-4 shrink-0 rounded border"
                      style={{ backgroundColor: swatch }}
                    />
                  )}
                  <span className="min-w-0 flex-1 break-words">
                    {token.label ?? id}
                  </span>
                  <span className="shrink-0 text-[10px] font-normal text-muted-foreground">
                    {t(`themeDesigner.type.${token.type}`)}
                  </span>
                </summary>
                <div className="min-w-0 space-y-3 border-t p-3">
                  {token.description && (
                    <p className="text-[11px] leading-relaxed text-muted-foreground">
                      {token.description}
                    </p>
                  )}
                  <DesignValueField
                    label={token.label ?? id}
                    type={token.type}
                    value={token.value}
                    path={["tokens", id, "value"]}
                    context={context}
                    excludeToken={id}
                    domain={themeTokenDomain(id, token.type, references)}
                    onChange={(next) =>
                      onChange(
                        setThemeDesignPath(
                          design,
                          ["tokens", id, "value"],
                          next,
                        ),
                      )
                    }
                  />
                  <p className="text-[11px] text-muted-foreground">
                    {t("themeDesigner.usage", { count })}
                  </p>
                  <details className="space-y-2 border-t pt-2">
                    <summary className="cursor-pointer text-[11px] text-muted-foreground">
                      {t("themeDesigner.settingDetails")}
                    </summary>
                    <label className="block space-y-1 text-xs">
                      <span>{t("themeDesigner.metadata.id")}</span>
                      <Input
                        key={id}
                        aria-label={t("themeDesigner.metadata.id")}
                        defaultValue={id}
                        className="h-8 text-xs"
                        onBlur={(event) => {
                          const next = event.target.value.trim();
                          try {
                            onChange(renameThemeToken(design, id, next));
                            setResourceError(null);
                          } catch {
                            setResourceError(t("themeDesigner.renameError"));
                          }
                        }}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") event.currentTarget.blur();
                        }}
                      />
                    </label>
                    {resourceError && (
                      <p role="alert" className="text-xs text-destructive">
                        {resourceError}
                      </p>
                    )}
                    {(["label", "group", "description"] as const).map(
                      (property) => (
                        <label
                          key={property}
                          className="block space-y-1 text-xs"
                        >
                          <span>{t(`themeDesigner.metadata.${property}`)}</span>
                          <Input
                            key={token[property] ?? ""}
                            aria-label={t(`themeDesigner.metadata.${property}`)}
                            defaultValue={token[property] ?? ""}
                            className="h-8 text-xs"
                            onBlur={(event) => {
                              const next =
                                event.target.value.trim() || undefined;
                              if (next !== token[property])
                                onChange(
                                  setThemeDesignPath(
                                    design,
                                    ["tokens", id, property],
                                    next,
                                  ),
                                );
                            }}
                            onKeyDown={(event) => {
                              if (event.key === "Enter")
                                event.currentTarget.blur();
                            }}
                          />
                        </label>
                      ),
                    )}
                  </details>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    disabled={count > 0}
                    title={
                      count > 0 ? t("themeDesigner.usedValueHint") : undefined
                    }
                    onClick={() =>
                      onChange(
                        setThemeDesignPath(design, ["tokens", id], undefined),
                      )
                    }
                  >
                    <Trash2 className="size-3.5" />
                    {t("themeDesigner.removeValue")}
                  </Button>
                </div>
              </details>
            );
          })}
        </section>
      ))}
      {!groups.size && (
        <p className="text-xs text-muted-foreground">
          {t("themeDesigner.noSharedValues")}
        </p>
      )}
      <details className="rounded-md border p-3">
        <summary className="cursor-pointer text-xs font-medium">
          {t("themeDesigner.addSharedValue")}
        </summary>
        <div className="mt-3 space-y-2">
          <Input
            aria-label={t("themeDesigner.valueName")}
            placeholder={t("themeDesigner.valueNamePlaceholder")}
            className="h-8 text-xs"
            value={newLabel}
            onChange={(event) => setNewLabel(event.target.value)}
          />
          <select
            aria-label={t("themeDesigner.valueType")}
            className={DESIGN_SELECT_CLASS}
            value={newType}
            onChange={(event) => setNewType(event.target.value as DesignType)}
          >
            {RESOURCE_TYPES.map((type) => (
              <option key={type} value={type}>
                {t(`themeDesigner.type.${type}`)}
              </option>
            ))}
          </select>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={!newLabel.trim()}
            onClick={() => {
              let index = 1;
              while (design.tokens?.[`custom-${index}`]) index++;
              onChange(
                setThemeDesignPath(design, ["tokens", `custom-${index}`], {
                  label: newLabel.trim(),
                  type: newType,
                  value: designDefault(newType),
                }),
              );
              setNewLabel("");
            }}
          >
            <Plus className="size-3.5" />
            {t("themeDesigner.createValue")}
          </Button>
        </div>
      </details>
    </section>
  );
}

function DesignCanvas({
  document,
  context,
  onChange,
}: {
  document: DesignRecipe;
  context: DesignFieldContext;
  onChange(document: ThemeRecipeV1): void;
}) {
  const { t } = useTranslation();
  const design = document.design;
  const layers = design.canvas?.layers ?? [];
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [newKind, setNewKind] = useState<
    "color" | "linear-gradient" | "radial-gradient" | "grid" | "stripes"
  >("linear-gradient");
  const [removed, setRemoved] = useState<{
    before: DesignRecipe;
    after: string;
  } | null>(null);
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
    setSelectedIndex(index + direction);
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
              onClick={() => setSelectedIndex(i)}
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
              setSelectedIndex(layers.length);
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
                  setSelectedIndex(index + 1);
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
            <label className="block space-y-1 text-xs">
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
            setRemoved({
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
              setRemoved(null);
            }}
          >
            {t("themeWorkbench.undoCanvas")}
          </Button>
        )}
      </div>
    </section>
  );
}

function DesignSemanticStyles({
  context,
  facade,
  catalog,
  familyId,
  onChange,
}: {
  context: DesignFieldContext;
  facade: MermanDomainFacade | null;
  catalog: ThemeCatalog | null;
  familyId?: string;
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
  const [target, setTarget] = useState("node");
  const [selectedRule, setSelectedRule] = useState(0);
  const [newTarget, setNewTarget] = useState("node");
  const [newFamily, setNewFamily] = useState("current");
  const rules = design.rules ?? [];
  const index = Math.min(selectedRule, Math.max(0, rules.length - 1));
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
      <label className="block space-y-1 text-xs">
        <span>{t("themeDesigner.object")}</span>
        <select
          aria-label={t("themeDesigner.object")}
          className={DESIGN_SELECT_CLASS}
          value={target}
          onChange={(event) => setTarget(event.target.value)}
        >
          {[...new Set([...targets, ...Object.keys(design.styles ?? {})])].map(
            (item) => (
              <option key={item} value={item}>
                {item}
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
      <h3 className="pt-2 text-xs font-semibold">
        {t("themeWorkbench.rules")}
      </h3>
      {rules.length > 0 && (
        <select
          aria-label={t("themeWorkbench.ruleOrder")}
          className={DESIGN_SELECT_CLASS}
          value={index}
          onChange={(event) => setSelectedRule(Number(event.target.value))}
        >
          {rules.map((entry, i) => (
            <option key={i} value={i}>
              {i + 1}. {entry.family ?? "*"} / {entry.target}
              {entry.variant ? ` / ${entry.variant}` : ""}
            </option>
          ))}
        </select>
      )}
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
                  {item}
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
                  {item}
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
                  {item}
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
                  setSelectedRule(index + direction);
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
                  {item}
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
              setSelectedRule(rules.length);
            }}
          >
            {t("themeWorkbench.append")}
          </Button>
        </div>
      </details>
    </section>
  );
}
