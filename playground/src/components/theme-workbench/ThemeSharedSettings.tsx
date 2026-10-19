import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2 } from "lucide-react";
import type { ThemeDesignV1 } from "@mermanjs/web";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { renameThemeToken, setThemeDesignPath, themeTokenDomain } from "@/src/lib/theme-document";
import { searchThemeSettings, type ThemeBindingUse, type ThemeDesignerModel } from "@/src/lib/theme-designer-model";
import { DESIGN_SELECT_CLASS, designDefault, DesignValueField, type DesignFieldContext, type DesignType } from "./ThemeDesignFields";
import { ThemeBindingUses } from "./ThemeBindingUses";

const RESOURCE_TYPES: DesignType[] = ["color", "number", "string", "length", "insets", "line-height", "font-stack", "palette", "paint", "stroke", "text-style", "effect"];

export function ThemeSharedSettings({ context, model, selectedId, query, group, onQuery, onGroup, onSelect, onOpenUse, onChange }: {
  context: DesignFieldContext;
  model: ThemeDesignerModel;
  selectedId?: string;
  query: string;
  /** Undefined shows all groups; null shows authored settings without a group. */
  group: string | null | undefined;
  onQuery(query: string): void;
  onGroup(group: string | null | undefined): void;
  onSelect(id: string): void;
  onOpenUse(use: ThemeBindingUse): void;
  onChange(design: ThemeDesignV1): void;
}) {
  const { t } = useTranslation();
  const { design, references } = context;
  const [newLabel, setNewLabel] = useState("");
  const [newType, setNewType] = useState<DesignType>("color");
  const [resourceError, setResourceError] = useState<string | null>(null);
  const selected = model.settings.find((setting) => setting.id === selectedId);
  const knownGroup = group === undefined || model.groups.some((entry) => entry.id === group);
  const activeGroup = knownGroup ? group : undefined;
  const matches = searchThemeSettings(model, query).filter((setting) => activeGroup === undefined || (setting.group ?? null) === activeGroup);
  const groupValue = group === undefined ? "all" : group === null ? "none" : `group:${group}`;
  return <section className="min-w-0 space-y-4" aria-label={t("themeDesigner.sharedValues")}>
    <div>
      <h3 className="text-sm font-semibold">{t("themeDesigner.sharedValues")}</h3>
      <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground">{t("themeDesigner.sharedHint")}</p>
    </div>
    <div className="space-y-2">
      <Input type="search" aria-label={t("themeDesigner.searchSettings")} placeholder={t("themeDesigner.searchSettings")} value={query} className="h-8 text-xs" onChange={(event) => onQuery(event.target.value)} />
      <select aria-label={t("themeDesigner.settingGroup")} className={DESIGN_SELECT_CLASS} value={knownGroup ? groupValue : "all"} onChange={(event) => onGroup(event.target.value === "all" ? undefined : event.target.value === "none" ? null : event.target.value.slice(6))}>
        <option value="all">{t("themeDesigner.allGroups")}</option>
        {model.groups.map((entry) => <option key={entry.id === null ? "none" : `group:${entry.id}`} value={entry.id === null ? "none" : `group:${entry.id}`}>{entry.label || t(entry.id === null ? "themeDesigner.ungrouped" : "themeDesigner.unnamedGroup")}</option>)}
      </select>
      <div className="flex max-h-56 min-w-0 flex-wrap gap-1.5 overflow-y-auto rounded-md border p-2" aria-label={t("themeDesigner.settingList")}>
        {matches.map((setting) => <Button key={setting.id} type="button" size="sm" variant={setting.id === selectedId ? "secondary" : "ghost"} className="h-auto min-h-8 max-w-full whitespace-normal break-words text-left text-xs" aria-label={setting.label} aria-pressed={setting.id === selectedId} onClick={() => onSelect(setting.id)}>{setting.label}</Button>)}
        {!matches.length && <p className="p-1 text-xs text-muted-foreground">{t(model.settings.length ? "themeDesigner.noSearchResults" : "themeDesigner.noSharedValues")}</p>}
      </div>
      {(query || group !== undefined) && <Button type="button" variant="ghost" size="sm" onClick={() => { onQuery(""); onGroup(undefined); }}>{t("themeDesigner.showAllSettings")}</Button>}
    </div>
    {selected ? <section key={selected.id} className="min-w-0 space-y-3 rounded-md border p-3" data-theme-resource={selected.id} aria-label={selected.label}>
      <div className="flex flex-wrap items-baseline gap-2">
        <h4 className="min-w-0 flex-1 break-words text-sm font-semibold">{selected.label}</h4>
        <span className="text-[10px] text-muted-foreground">{t(`themeDesigner.type.${selected.type}`)}</span>
      </div>
      {selected.description && <p className="text-[11px] leading-relaxed text-muted-foreground">{selected.description}</p>}
      <DesignValueField label={selected.label} type={selected.type} value={selected.token.value} path={["tokens", selected.id, "value"]} context={context} excludeToken={selected.id} domain={themeTokenDomain(selected.id, selected.type, references)} onChange={(next) => onChange(setThemeDesignPath(design, ["tokens", selected.id, "value"], next))} />
      <ThemeBindingUses setting={selected} model={model} onOpen={onOpenUse} />
      <details className="space-y-2 border-t pt-2">
        <summary className="cursor-pointer text-[11px] text-muted-foreground">{t("themeDesigner.settingDetails")}</summary>
        <label className="block space-y-1 text-xs"><span>{t("themeDesigner.metadata.id")}</span><Input key={selected.id} aria-label={t("themeDesigner.metadata.id")} defaultValue={selected.id} className="h-8 text-xs" onBlur={(event) => {
          const next = event.target.value.trim();
          if (next === selected.id) return;
          try { const renamed = renameThemeToken(design, selected.id, next); onChange(renamed); onSelect(next); setResourceError(null); }
          catch { setResourceError(t("themeDesigner.renameError")); }
        }} onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} /></label>
        {resourceError && <p role="alert" className="text-xs text-destructive">{resourceError}</p>}
        {(["label", "group", "description"] as const).map((property) => <label key={property} className="block space-y-1 text-xs"><span>{t(`themeDesigner.metadata.${property}`)}</span><Input key={selected.token[property] ?? ""} aria-label={t(`themeDesigner.metadata.${property}`)} defaultValue={selected.token[property] ?? ""} className="h-8 text-xs" onBlur={(event) => { const next = event.target.value.trim() || undefined; if (next !== selected.token[property]) onChange(setThemeDesignPath(design, ["tokens", selected.id, property], next)); }} onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} /></label>)}
      </details>
      <Button type="button" variant="ghost" size="sm" disabled={!selected.uses || selected.uses.direct.length + selected.uses.dependencies.length > 0} title={!selected.uses ? t("themeDesigner.inspectionUnavailable") : selected.uses.direct.length + selected.uses.dependencies.length > 0 ? t("themeDesigner.usedValueHint") : undefined} onClick={() => onChange(setThemeDesignPath(design, ["tokens", selected.id], undefined))}><Trash2 className="size-3.5" />{t("themeDesigner.removeValue")}</Button>
    </section> : selectedId ? <p role="status" className="text-xs text-muted-foreground">{t("themeDesigner.settingUnavailable")}</p> : <p className="text-xs text-muted-foreground">{t("themeDesigner.selectSetting")}</p>}
    <details className="rounded-md border p-3">
      <summary className="cursor-pointer text-xs font-medium">{t("themeDesigner.addSharedValue")}</summary>
      <div className="mt-3 space-y-2">
        <Input aria-label={t("themeDesigner.valueName")} placeholder={t("themeDesigner.valueNamePlaceholder")} className="h-8 text-xs" value={newLabel} onChange={(event) => setNewLabel(event.target.value)} />
        <select aria-label={t("themeDesigner.valueType")} className={DESIGN_SELECT_CLASS} value={newType} onChange={(event) => setNewType(event.target.value as DesignType)}>{RESOURCE_TYPES.map((type) => <option key={type} value={type}>{t(`themeDesigner.type.${type}`)}</option>)}</select>
        <Button type="button" size="sm" variant="outline" disabled={!newLabel.trim()} onClick={() => {
          let index = 1; while (Object.hasOwn(design.tokens ?? {}, `custom-${index}`)) index++;
          const id = `custom-${index}`;
          onChange(setThemeDesignPath(design, ["tokens", id], { label: newLabel.trim(), type: newType, value: designDefault(newType), ...(group !== undefined && group !== null ? { group } : {}) }));
          onQuery(""); onSelect(id); setNewLabel("");
        }}><Plus className="size-3.5" />{t("themeDesigner.createValue")}</Button>
      </div>
    </details>
  </section>;
}
