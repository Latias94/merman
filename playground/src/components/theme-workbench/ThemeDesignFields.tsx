import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import type {
  ThemeDesignTokenV1,
  ThemeDesignV1,
  ThemeDesignValueDomain,
  ThemeDesignReferenceUse,
} from "@mermanjs/web";
import { Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  copyThemeReferenceValue,
  isThemeReference,
  themeDesignPointer,
  themeTokenFitsDomain,
} from "@/src/lib/theme-document";
import { NumberField, ListField } from "./ThemeEditorFields";
import { newThemePaint } from "./ThemePaintEditor";

export const DESIGN_SELECT_CLASS =
  "h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";
export type DesignType = ThemeDesignTokenV1["type"];
export type DesignPath = readonly (string | number)[];
export interface DesignFieldContext {
  design: ThemeDesignV1;
  references: readonly ThemeDesignReferenceUse[];
}

export function designDefault(type: DesignType): unknown {
  switch (type) {
    case "color":
      return "#2563eb";
    case "number":
    case "length":
      return 1;
    case "string":
      return "normal";
    case "insets":
      return 8;
    case "line-height":
      return 1.4;
    case "font-stack":
      return ["Inter", "Arial", "sans-serif"];
    case "palette":
      return ["#2563eb", "#ec4899", "#f59e0b"];
    case "paint":
      return "#ffffff";
    case "stroke":
      return { paint: "#64748b", width: 1 };
    case "text-style":
      return { font_size_px: 16, font_weight: 400 };
    case "effect":
      return {
        kind: "shadow-stack",
        composition: "independent",
        shadows: [newShadow()],
      };
  }
}

function newShadow() {
  return {
    offset_x: 4,
    offset_y: 4,
    std_deviation: 0,
    color: "#000000",
    opacity: 0.25,
  };
}
function asObject(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {};
}

/** These controls edit authored values only. Rust remains the materialization authority. */
export function DesignValueField({
  label,
  type,
  value,
  path,
  context,
  domain: suppliedDomain,
  optional = false,
  clearable = false,
  excludeToken,
  onChange,
}: {
  label: string;
  type: DesignType;
  value: unknown;
  path: DesignPath;
  context: DesignFieldContext;
  domain?: ThemeDesignValueDomain;
  optional?: boolean;
  clearable?: boolean;
  excludeToken?: string;
  onChange(value: unknown): void;
}) {
  const { t } = useTranslation();
  const pointer = themeDesignPointer(path);
  const domain =
    suppliedDomain ??
    context.references.find((use) => use.path === pointer)?.domain;
  const tokens = Object.entries(context.design.tokens ?? {}).filter(
    ([id]) =>
      id !== excludeToken &&
      themeTokenFitsDomain(context.design, id, type, domain),
  );
  const reference = isThemeReference(value) ? value.ref : undefined;
  const [copyError, setCopyError] = useState<string | null>(null);
  return (
    <div
      className="min-w-0 space-y-2"
      role="group"
      aria-label={label}
      data-design-value={pointer}
    >
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <span className="min-w-0 flex-1 text-xs font-medium">{label}</span>
        <select
          className={`${DESIGN_SELECT_CLASS} max-w-48`}
          aria-label={`${label}: ${t("themeDesigner.binding")}`}
          value={
            reference === undefined
              ? value === undefined
                ? "omit"
                : value === null
                  ? "clear"
                  : "local"
              : `ref:${reference}`
          }
          onChange={(event) => {
            setCopyError(null);
            const selected = event.target.value;
            try {
              onChange(
                selected === "omit"
                  ? undefined
                  : selected === "clear"
                    ? null
                    : selected === "local"
                      ? reference
                        ? copyThemeReferenceValue(context.design, reference)
                        : designDefault(type)
                      : { ref: selected.slice(4) },
              );
            } catch (cause) {
              setCopyError(
                cause instanceof Error ? cause.message : String(cause),
              );
            }
          }}
        >
          {optional && <option value="omit">{t("themeWorkbench.omit")}</option>}
          {clearable && (
            <option value="clear">{t("themeWorkbench.clear")}</option>
          )}
          <option value="local">{t("themeDesigner.localValue")}</option>
          {tokens.map(([id, token]) => (
            <option key={id} value={`ref:${id}`}>
              {token.label ?? id}
            </option>
          ))}
          {reference && !tokens.some(([id]) => id === reference) && (
            <option value={`ref:${reference}`}>{reference}</option>
          )}
        </select>
      </div>
      {reference !== undefined ? (
        <div className="space-y-1.5 rounded-md border bg-muted/20 p-2">
          <p className="text-[11px] text-muted-foreground">
            {t("themeDesigner.boundHint", {
              name: context.design.tokens?.[reference]?.label ?? reference,
            })}
          </p>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => {
              try {
                onChange(copyThemeReferenceValue(context.design, reference));
                setCopyError(null);
              } catch (cause) {
                setCopyError(
                  cause instanceof Error ? cause.message : String(cause),
                );
              }
            }}
          >
            {t("themeDesigner.localCopy")}
          </Button>
          <p className="text-[11px] text-muted-foreground">
            {t("themeDesigner.localCopyHint")}
          </p>
          {copyError && (
            <p role="alert" className="text-xs text-destructive">
              {copyError}
            </p>
          )}
        </div>
      ) : (
        value != null && (
          <DesignLiteralEditor
            label={label}
            type={type}
            value={value}
            path={path}
            context={context}
            domain={domain}
            onChange={onChange}
          />
        )
      )}
    </div>
  );
}

function DesignLiteralEditor({
  label,
  type,
  value,
  path,
  context,
  domain,
  onChange,
}: {
  label: string;
  type: DesignType;
  value: unknown;
  path: DesignPath;
  context: DesignFieldContext;
  domain?: ThemeDesignValueDomain;
  onChange(value: unknown): void;
}) {
  const { t } = useTranslation();
  if (type === "color")
    return (
      <DesignColorField
        label={label}
        value={String(value)}
        onChange={onChange}
      />
    );
  if (type === "number")
    return (
      <div className="space-y-2">
        <NumberField
          label={label}
          value={typeof value === "number" ? value : undefined}
          required
          min={domain?.min}
          max={domain?.max}
          onChange={(number) => {
            if (number != null) onChange(number);
          }}
        />
        {domain?.min === 0 && domain.max === 1 && typeof value === "number" && (
          <input
            type="range"
            aria-label={`${label}: ${t("themeDesigner.adjustValue")}`}
            min={0}
            max={1}
            step={0.01}
            value={value}
            className="w-full accent-primary"
            onChange={(event) => onChange(Number(event.target.value))}
          />
        )}
      </div>
    );
  if (type === "string")
    return domain?.choices ? (
      <select
        aria-label={label}
        className={DESIGN_SELECT_CLASS}
        value={String(value)}
        onChange={(event) => onChange(event.target.value)}
      >
        {!domain.choices.includes(String(value)) && (
          <option value={String(value)}>{String(value)}</option>
        )}
        {domain.choices.map((choice) => (
          <option key={choice} value={choice}>
            {choice}
          </option>
        ))}
      </select>
    ) : (
      <Input
        key={String(value)}
        aria-label={label}
        defaultValue={String(value)}
        className="h-8 text-xs"
        onBlur={(event) => {
          if (event.target.value !== value) onChange(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") event.currentTarget.blur();
        }}
      />
    );
  if (type === "length" || type === "line-height")
    return (
      <DesignMeasureField
        label={label}
        type={type}
        value={value}
        domain={domain}
        onChange={onChange}
      />
    );
  if (type === "font-stack")
    return (
      <ListField
        label={label}
        value={Array.isArray(value) ? (value as string[]) : []}
        onChange={(next) => onChange(next ?? [])}
      />
    );
  if (type === "paint")
    return (
      <DesignPaintEditor
        value={value}
        path={path}
        context={context}
        onChange={onChange}
      />
    );
  if (type === "effect")
    return (
      <DesignEffectEditor
        value={value}
        path={path}
        context={context}
        onChange={onChange}
      />
    );
  if (type === "stroke" || type === "text-style")
    return (
      <DesignObjectFields
        kind={type}
        value={value}
        path={path}
        context={context}
        onChange={onChange}
      />
    );
  if (type === "insets") {
    if (typeof value === "number")
      return (
        <div className="space-y-2">
          <NumberField
            label={label}
            value={value}
            required
            min={0}
            onChange={(next) => {
              if (next != null) onChange(next);
            }}
          />
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() =>
              onChange({ top: value, right: value, bottom: value, left: value })
            }
          >
            {t("themeDesigner.individualSides")}
          </Button>
        </div>
      );
    return (
      <div className="grid grid-cols-2 gap-2">
        {["top", "right", "bottom", "left"].map((side) => (
          <NumberField
            key={side}
            label={t(`themeDesigner.field.${side}`)}
            value={Number(asObject(value)[side])}
            required
            min={0}
            onChange={(next) => {
              if (next != null) onChange({ ...asObject(value), [side]: next });
            }}
          />
        ))}
      </div>
    );
  }
  if (type === "palette") {
    const colors = Array.isArray(value) ? value : [];
    return (
      <div className="space-y-2">
        {colors.map((color, index) => (
          <div key={index} className="rounded border p-2">
            <DesignValueField
              label={t("themeWorkbench.stopColor", { index: index + 1 })}
              type="color"
              value={color}
              path={[...path, index]}
              context={context}
              onChange={(next) =>
                onChange(colors.map((item, i) => (i === index ? next : item)))
              }
            />
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={colors.length <= 1}
              onClick={() => onChange(colors.filter((_, i) => i !== index))}
            >
              {t("themeDesigner.remove")}
            </Button>
          </div>
        ))}
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => onChange([...colors, "#2563eb"])}
        >
          <Plus className="size-3.5" />
          {t("themeDesigner.addColor")}
        </Button>
      </div>
    );
  }
  return null;
}

function DesignColorField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange(value: string): void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex min-w-0 gap-2">
      <input
        type="color"
        aria-label={`${label}: ${t("themeWorkbench.colorPicker")}`}
        className="size-8 shrink-0 cursor-pointer rounded border bg-background p-0.5"
        value={
          /^#[\da-f]{6}([\da-f]{2})?$/i.test(value)
            ? value.slice(0, 7)
            : "#ffffff"
        }
        onChange={(event) =>
          onChange(
            event.target.value +
              (/^#[\da-f]{8}$/i.test(value) ? value.slice(7) : ""),
          )
        }
      />
      <Input
        key={value}
        aria-label={label}
        defaultValue={value}
        className="h-8 min-w-0 font-mono text-xs"
        onBlur={(event) => {
          if (event.target.value !== value) onChange(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") event.currentTarget.blur();
        }}
      />
    </div>
  );
}

function DesignMeasureField({
  label,
  type,
  value,
  domain,
  onChange,
}: {
  label: string;
  type: "length" | "line-height";
  value: unknown;
  domain?: ThemeDesignValueDomain;
  onChange(value: unknown): void;
}) {
  const { t } = useTranslation();
  const object = asObject(value);
  const unit =
    typeof value === "number" ? "number" : (Object.keys(object)[0] ?? "px");
  const number =
    value === "normal"
      ? 1.4
      : typeof value === "number"
        ? value
        : Number(object[unit]);
  const units =
    domain?.units ??
    (type === "line-height" ? ["number", "px", "normal"] : ["px", "percent"]);
  return (
    <div className="flex min-w-0 items-end gap-2">
      <div className="min-w-0 flex-1">
        <NumberField
          label={label}
          value={number}
          required
          min={domain?.min}
          max={domain?.max}
          onChange={(next) => {
            if (next != null)
              onChange(
                unit === "number" || value === "normal"
                  ? next
                  : { [unit]: next },
              );
          }}
        />
      </div>
      <select
        aria-label={`${label}: ${t("themeWorkbench.unit")}`}
        value={
          value === "normal"
            ? "normal"
            : unit === "number" && type === "length"
              ? "px"
              : unit
        }
        className={`${DESIGN_SELECT_CLASS} max-w-24`}
        onChange={(event) =>
          onChange(
            event.target.value === "normal"
              ? "normal"
              : event.target.value === "number"
                ? number
                : { [event.target.value]: number },
          )
        }
      >
        {units.map((name) => (
          <option key={name} value={name}>
            {name === "percent" ? "%" : name}
          </option>
        ))}
      </select>
    </div>
  );
}

type PropertyDefinition = {
  key: string;
  type: DesignType;
  choices?: string[];
  min?: number;
  max?: number;
  clearable?: boolean;
};
const PROPERTIES: Record<
  "style" | "stroke" | "text-style",
  PropertyDefinition[]
> = {
  style: [
    { key: "fill", type: "paint", clearable: true },
    { key: "stroke", type: "stroke" },
    { key: "typography", type: "text-style" },
    { key: "effect", type: "effect", clearable: true },
    { key: "opacity", type: "number", min: 0, max: 1, clearable: true },
    { key: "fill_opacity", type: "number", min: 0, max: 1, clearable: true },
    { key: "radius", type: "length", min: 0, clearable: true },
    { key: "padding", type: "insets", min: 0, clearable: true },
  ],
  stroke: [
    { key: "paint", type: "paint", clearable: true },
    { key: "width", type: "length", min: 0, clearable: true },
    {
      key: "linecap",
      type: "string",
      choices: ["butt", "round", "square"],
      clearable: true,
    },
    {
      key: "linejoin",
      type: "string",
      choices: ["miter", "round", "bevel"],
      clearable: true,
    },
    { key: "opacity", type: "number", min: 0, max: 1, clearable: true },
  ],
  "text-style": [
    { key: "font_stack", type: "font-stack", clearable: true },
    { key: "font_size_px", type: "length", min: 0, clearable: true },
    { key: "font_weight", type: "number", min: 1, max: 1000, clearable: true },
    {
      key: "font_style",
      type: "string",
      choices: ["normal", "italic", "oblique"],
      clearable: true,
    },
    { key: "line_height", type: "line-height", clearable: true },
    { key: "letter_spacing_px", type: "length", clearable: true },
    { key: "word_spacing_px", type: "length", clearable: true },
    {
      key: "transform",
      type: "string",
      choices: ["none", "uppercase", "lowercase", "capitalize"],
      clearable: true,
    },
    {
      key: "decoration",
      type: "string",
      choices: ["none", "underline", "overline", "line-through"],
      clearable: true,
    },
    {
      key: "text_align",
      type: "string",
      choices: ["start", "center", "end"],
      clearable: true,
    },
    {
      key: "white_space",
      type: "string",
      choices: ["normal", "pre", "no-wrap", "pre-wrap", "pre-line"],
      clearable: true,
    },
    {
      key: "wrap",
      type: "string",
      choices: ["normal", "break-word", "anywhere"],
      clearable: true,
    },
  ],
};

export function DesignObjectFields({
  kind,
  value,
  path,
  context,
  onChange,
}: {
  kind: "style" | "stroke" | "text-style";
  value: unknown;
  path: DesignPath;
  context: DesignFieldContext;
  onChange(value: unknown): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const object = asObject(value);
  const properties = PROPERTIES[kind];
  const missing = properties.filter(({ key }) => !(key in object));
  function edit(key: string, next: unknown) {
    const copy = { ...object };
    if (next === undefined) delete copy[key];
    else copy[key] = next;
    onChange(copy);
  }
  return (
    <div className="min-w-0 space-y-3 rounded-md border p-2">
      {properties
        .filter(({ key }) => key in object)
        .map((property) => (
          <DesignValueField
            key={property.key}
            label={t(`themeDesigner.field.${property.key}`)}
            type={property.type}
            value={object[property.key]}
            path={[...path, property.key]}
            context={context}
            optional
            clearable={property.clearable}
            domain={{
              value_type: property.type,
              choices: property.choices,
              min: property.min,
              max: property.max,
              ...(property.type === "length" ? { units: ["px"] } : {}),
            }}
            onChange={(next) => edit(property.key, next)}
          />
        ))}
      {kind === "stroke" && "dasharray" in object && (
        <div className="space-y-2 rounded border p-2">
          <h4 className="text-xs font-medium">
            {t("themeDesigner.dashPattern")}
          </h4>
          {(Array.isArray(object.dasharray) ? object.dasharray : []).map(
            (dash: unknown, index: number) => (
              <div key={index} className="space-y-1">
                <DesignValueField
                  label={t("themeDesigner.dashLength", { index: index + 1 })}
                  type="number"
                  value={dash}
                  path={[...path, "dasharray", index]}
                  context={context}
                  domain={{ value_type: "number", min: 0 }}
                  onChange={(next) =>
                    edit(
                      "dasharray",
                      (object.dasharray as unknown[]).map((item, i) =>
                        i === index ? next : item,
                      ),
                    )
                  }
                />
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() =>
                    edit(
                      "dasharray",
                      (object.dasharray as unknown[]).filter(
                        (_, i) => i !== index,
                      ),
                    )
                  }
                >
                  {t("themeDesigner.remove")}
                </Button>
              </div>
            ),
          )}
          <div className="flex flex-wrap gap-1.5">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() =>
                edit("dasharray", [
                  ...(Array.isArray(object.dasharray) ? object.dasharray : []),
                  4,
                ])
              }
            >
              {t("themeDesigner.addDash")}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => edit("dasharray", null)}
            >
              {t("themeWorkbench.clear")}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => edit("dasharray", undefined)}
            >
              {t("themeWorkbench.omit")}
            </Button>
          </div>
        </div>
      )}
      {(missing.length > 0 ||
        (kind === "stroke" && !("dasharray" in object))) && (
        <select
          id={id}
          aria-label={t("themeDesigner.addProperty")}
          className={DESIGN_SELECT_CLASS}
          value=""
          onChange={(event) => {
            if (event.target.value === "dasharray") {
              edit("dasharray", [4, 2]);
              return;
            }
            const property = missing.find(
              ({ key }) => key === event.target.value,
            );
            if (property)
              edit(
                property.key,
                property.choices?.[0] ?? designDefault(property.type),
              );
          }}
        >
          <option value="">{t("themeDesigner.addProperty")}</option>
          {kind === "stroke" && !("dasharray" in object) && (
            <option value="dasharray">{t("themeDesigner.dashPattern")}</option>
          )}
          {missing.map(({ key }) => (
            <option key={key} value={key}>
              {t(`themeDesigner.field.${key}`)}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}

function DesignPaintEditor({
  value,
  path,
  context,
  onChange,
}: {
  value: unknown;
  path: DesignPath;
  context: DesignFieldContext;
  onChange(value: unknown): void;
}) {
  const { t } = useTranslation();
  const object = asObject(value);
  const mode =
    typeof value === "string"
      ? value === "transparent"
        ? "transparent"
        : "color"
      : object.kind === "solid"
        ? "color"
        : String(object.kind);
  const child = (
    key: string,
    type: DesignType,
    label: string,
    extra?: Partial<ThemeDesignValueDomain>,
  ) => (
    <DesignValueField
      label={label}
      type={type}
      value={object[key]}
      path={[...path, key]}
      context={context}
      domain={
        extra
          ? {
              value_type: type,
              ...extra,
              ...(key === "cell_width" || key === "cell_height"
                ? { units: ["px"] }
                : {}),
            }
          : undefined
      }
      onChange={(next) => onChange({ ...object, [key]: next })}
    />
  );
  return (
    <div className="min-w-0 space-y-3 rounded-md border bg-muted/10 p-2">
      <select
        aria-label={t("themeDesigner.paintType")}
        className={DESIGN_SELECT_CLASS}
        value={mode}
        onChange={(event) =>
          onChange(
            newThemePaint(
              event.target.value as
                | "color"
                | "transparent"
                | "linear-gradient"
                | "radial-gradient",
            ),
          )
        }
      >
        <option value="color">{t("themeWorkbench.color")}</option>
        <option value="transparent">{t("themeWorkbench.transparent")}</option>
        <option value="linear-gradient">
          {t("themeWorkbench.paint.linear-gradient")}
        </option>
        <option value="radial-gradient">
          {t("themeWorkbench.paint.radial-gradient")}
        </option>
        {mode === "pattern" && (
          <option value="pattern">{t("themeWorkbench.paint.pattern")}</option>
        )}
      </select>
      {mode === "color" &&
        (typeof value === "string" ? (
          <DesignValueField
            label={t("themeWorkbench.color")}
            type="color"
            value={value}
            path={[...path, "color"]}
            context={context}
            onChange={(color) =>
              onChange(
                typeof color === "string" ? color : { kind: "solid", color },
              )
            }
          />
        ) : (
          child("color", "color", t("themeWorkbench.color"))
        ))}
      {mode === "linear-gradient" &&
        child("angle_degrees", "number", t("themeWorkbench.gradientAngle"))}
      {mode === "radial-gradient" && (
        <>
          {child("center_x", "length", t("themeWorkbench.gradientCenterX"))}
          {child("center_y", "length", t("themeWorkbench.gradientCenterY"))}
          {child("radius", "length", t("themeWorkbench.gradientRadius"), {
            min: 0,
          })}
        </>
      )}
      {(mode === "linear-gradient" || mode === "radial-gradient") && (
        <>
          {(Array.isArray(object.stops) ? object.stops : []).map(
            (stop: unknown, index: number) => {
              const entry = asObject(stop);
              const stops = object.stops as unknown[];
              const offset = (item: unknown, fallback: number) => {
                const value = asObject(item).offset;
                const resolved = isThemeReference(value)
                  ? copyThemeReferenceValue(context.design, value.ref)
                  : value;
                return typeof resolved === "number" ? resolved : fallback;
              };
              const minOffset = index > 0 ? offset(stops[index - 1], 0) : 0;
              const maxOffset =
                index + 1 < stops.length ? offset(stops[index + 1], 1) : 1;
              const edit = (key: string, next: unknown) =>
                onChange({
                  ...object,
                  stops: stops.map((item, i) =>
                    i === index ? { ...entry, [key]: next } : item,
                  ),
                });
              return (
                <div key={index} className="space-y-2 rounded border p-2">
                  <DesignValueField
                    label={t("themeWorkbench.stopColor", { index: index + 1 })}
                    type="color"
                    value={entry.color}
                    path={[...path, "stops", index, "color"]}
                    context={context}
                    onChange={(next) => edit("color", next)}
                  />
                  <DesignValueField
                    label={t("themeDesigner.stopOffset", { index: index + 1 })}
                    type="number"
                    value={entry.offset}
                    path={[...path, "stops", index, "offset"]}
                    context={context}
                    domain={{
                      value_type: "number",
                      min: minOffset,
                      max: maxOffset,
                    }}
                    onChange={(next) => edit("offset", next)}
                  />
                  <DesignValueField
                    label={t("themeWorkbench.opacity")}
                    type="number"
                    value={entry.opacity}
                    path={[...path, "stops", index, "opacity"]}
                    context={context}
                    domain={{ value_type: "number", min: 0, max: 1 }}
                    optional
                    onChange={(next) => {
                      const changed = { ...entry };
                      if (next === undefined) delete changed.opacity;
                      else changed.opacity = next;
                      onChange({
                        ...object,
                        stops: stops.map((item, i) =>
                          i === index ? changed : item,
                        ),
                      });
                    }}
                  />
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    disabled={stops.length <= 2}
                    onClick={() =>
                      onChange({
                        ...object,
                        stops: stops.filter((_, i) => i !== index),
                      })
                    }
                  >
                    <Trash2 className="size-3.5" />
                    {t("themeWorkbench.removeStop", { index: index + 1 })}
                  </Button>
                </div>
              );
            },
          )}
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => {
              const stops = [...(object.stops as unknown[])];
              const before = asObject(stops[stops.length - 2]).offset;
              const after = asObject(stops[stops.length - 1]).offset;
              if (typeof before === "number" && typeof after === "number")
                stops.splice(stops.length - 1, 0, {
                  offset: (before + after) / 2,
                  color: "#2563eb",
                });
              else stops.push({ offset: 1, color: "#2563eb" });
              onChange({ ...object, stops });
            }}
          >
            <Plus className="size-3.5" />
            {t("themeWorkbench.addStop")}
          </Button>
          <DesignRepetition paint={object} onChange={onChange} />
        </>
      )}
      {mode === "pattern" && (
        <>
          <p className="text-[11px] text-muted-foreground">
            {t("themeWorkbench.patternNotRendered")}
          </p>
          {child("cell_width", "length", t("themeWorkbench.patternWidth"), {
            min: 0,
          })}
          {child("cell_height", "length", t("themeWorkbench.patternHeight"), {
            min: 0,
          })}
          {child("foreground", "color", t("themeWorkbench.patternForeground"))}
          {object.background !== undefined &&
            child("background", "color", t("themeWorkbench.patternBackground"))}
        </>
      )}
    </div>
  );
}

function DesignRepetition({
  paint,
  onChange,
}: {
  paint: Record<string, unknown>;
  onChange(value: unknown): void;
}) {
  const { t } = useTranslation();
  const repetition = asObject(paint.repetition);
  return (
    <details className="space-y-2 border-t pt-2">
      <summary className="cursor-pointer text-xs">
        {t("themeWorkbench.gradientRepetition")}
      </summary>
      <select
        aria-label={t("themeWorkbench.gradientRepetition")}
        value={String(repetition.kind ?? "none")}
        className={DESIGN_SELECT_CLASS}
        onChange={(event) => {
          const next = { ...paint };
          if (event.target.value === "none") delete next.repetition;
          else
            next.repetition =
              event.target.value === "tiled"
                ? { kind: "tiled", width_px: 64, height_px: 64 }
                : {
                    kind: "repeating",
                    ...(paint.kind === "linear-gradient"
                      ? { period_px: 32 }
                      : {}),
                  };
          onChange(next);
        }}
      >
        <option value="none">{t("themeWorkbench.noRepetition")}</option>
        <option value="repeating">{t("themeWorkbench.repeating")}</option>
        <option value="tiled">{t("themeWorkbench.tiled")}</option>
      </select>
      {(repetition.kind === "tiled"
        ? ["width_px", "height_px"]
        : repetition.kind === "repeating" && paint.kind === "linear-gradient"
          ? ["period_px"]
          : []
      ).map((key) => (
        <NumberField
          key={key}
          label={t(
            key === "width_px"
              ? "themeWorkbench.tileWidth"
              : key === "height_px"
                ? "themeWorkbench.tileHeight"
                : "themeWorkbench.gradientPeriod",
          )}
          value={Number(repetition[key])}
          required
          min={1}
          max={repetition.kind === "tiled" ? 4096 : 65536}
          onChange={(next) => {
            if (next != null)
              onChange({
                ...paint,
                repetition: { ...repetition, [key]: next },
              });
          }}
        />
      ))}
    </details>
  );
}

function DesignEffectEditor({
  value,
  path,
  context,
  onChange,
}: {
  value: unknown;
  path: DesignPath;
  context: DesignFieldContext;
  onChange(value: unknown): void;
}) {
  const { t } = useTranslation();
  const object = asObject(value);
  if (object.kind !== "shadow-stack")
    return (
      <div className="space-y-2 rounded border p-2">
        <p className="text-[11px] text-muted-foreground">
          {t("themeDesigner.effectGraphHint")}
        </p>
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => onChange(designDefault("effect"))}
        >
          {t("themeDesigner.replaceShadow")}
        </Button>
      </div>
    );
  const shadows = Array.isArray(object.shadows) ? object.shadows : [];
  return (
    <div className="min-w-0 space-y-3 rounded-md border p-2">
      <label className="block space-y-1 text-xs">
        <span>{t("themeDesigner.shadowComposition")}</span>
        <select
          aria-label={t("themeDesigner.shadowComposition")}
          className={DESIGN_SELECT_CLASS}
          value={String(object.composition)}
          onChange={(event) =>
            onChange({ ...object, composition: event.target.value })
          }
        >
          <option value="independent">
            {t("themeDesigner.independentShadows")}
          </option>
          <option value="cascade">{t("themeDesigner.cascadeShadows")}</option>
        </select>
      </label>
      <p className="text-[11px] text-muted-foreground">
        {t(
          object.composition === "cascade"
            ? "themeDesigner.cascadeHint"
            : "themeDesigner.independentHint",
        )}
      </p>
      {shadows.map((shadow: unknown, index: number) => {
        const entry = asObject(shadow);
        return (
          <div
            key={index}
            className="space-y-2 rounded border p-2"
            data-theme-shadow-editor
          >
            <h4 className="text-xs font-medium">
              {t("themeDesigner.shadow", { index: index + 1 })}
            </h4>
            {(
              [
                ["offset_x", "length"],
                ["offset_y", "length"],
                ["std_deviation", "length"],
                ["color", "color"],
                ["opacity", "number"],
              ] as const
            ).map(([key, type]) => (
              <DesignValueField
                key={key}
                label={t(`themeDesigner.field.${key}`)}
                type={type}
                value={entry[key]}
                path={[...path, "shadows", index, key]}
                context={context}
                optional={key === "opacity"}
                domain={
                  key === "opacity"
                    ? { value_type: type, min: 0, max: 1 }
                    : type === "length"
                      ? {
                          value_type: type,
                          ...(key === "std_deviation" ? { min: 0 } : {}),
                          units: ["px"],
                        }
                      : undefined
                }
                onChange={(next) => {
                  const changed = { ...entry };
                  if (next === undefined) delete changed[key];
                  else changed[key] = next;
                  onChange({
                    ...object,
                    shadows: shadows.map((item, i) =>
                      i === index ? changed : item,
                    ),
                  });
                }}
              />
            ))}
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={shadows.length <= 1}
              onClick={() =>
                onChange({
                  ...object,
                  shadows: shadows.filter((_, i) => i !== index),
                })
              }
            >
              <Trash2 className="size-3.5" />
              {t("themeDesigner.removeShadow")}
            </Button>
          </div>
        );
      })}
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={() =>
          onChange({ ...object, shadows: [...shadows, newShadow()] })
        }
      >
        <Plus className="size-3.5" />
        {t("themeDesigner.addShadow")}
      </Button>
    </div>
  );
}
