import { useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ThemeCatalog, ThemeDesignReferenceUse, ThemeDesignV1, ThemeRecipeV1 } from "@mermanjs/web";
import { Button } from "@/components/ui/button";
import { setThemeDesignPath, themeDesignPointer } from "@/src/lib/theme-document";
import { buildThemeDesignerModel, classifyThemeDesignDestination, type ThemeBindingUse } from "@/src/lib/theme-designer-model";
import type { MermanDomainFacade } from "@/src/runtime/merman-core";
import { projectError } from "@/src/runtime/error-projection";
import { DesignValueField, type DesignFieldContext } from "./ThemeDesignFields";
import { ThemeDesignOverview } from "./ThemeDesignOverview";
import { ThemeSharedSettings } from "./ThemeSharedSettings";
import { ThemeObjectStyles, type ThemeObjectSelection } from "./ThemeObjectStyles";
import { ThemeDesignCanvas, type RemovedThemeCanvas } from "./ThemeDesignCanvas";

type DesignRecipe = Extract<ThemeRecipeV1, { kind: "design" }>;
type View = "overview" | "shared" | "objects" | "canvas" | "typography";
const SECTIONS = ["overview", "shared", "objects"] as const;
const NO_REFERENCES: readonly ThemeDesignReferenceUse[] = [];

export function ThemeDesignEditor({ document, disabled, facade, catalog, familyId, initialTokenId, onOpenJson, onChange }: {
  document: DesignRecipe;
  disabled: boolean;
  facade: MermanDomainFacade | null;
  catalog: ThemeCatalog | null;
  familyId?: string;
  initialTokenId?: string;
  onOpenJson?: (designRelativePointer: string, returnTokenId?: string) => void;
  onChange(document: ThemeRecipeV1): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const root = useRef<HTMLFieldSetElement>(null);
  const design = document.design;
  const [view, setView] = useState<View>(initialTokenId ? "shared" : "overview");
  const [selectedId, setSelectedId] = useState<string | undefined>(initialTokenId);
  const [group, setGroup] = useState<string | null | undefined>(undefined);
  const [query, setQuery] = useState("");
  const [objectSelection, setObjectSelection] = useState<ThemeObjectSelection>({ kind: "style", target: "node" });
  const [layerIndex, setLayerIndex] = useState(0);
  const [removedCanvas, setRemovedCanvas] = useState<RemovedThemeCanvas | null>(null);
  const [originToken, setOriginToken] = useState<string | undefined>();
  const [locationUnavailable, setLocationUnavailable] = useState(false);
  const [focusRequest, setFocusRequest] = useState<{ sequence: number; path: string; design: ThemeDesignV1; returnToken?: string } | null>(() => initialTokenId ? { sequence: 1, path: themeDesignPointer(["tokens", initialTokenId, "value"]), design } : null);
  const sequence = useRef(initialTokenId ? 1 : 0);
  const processedFocus = useRef(0);
  const materialized = useMemo(() => {
    try { return { value: facade?.materializeTheme(design), error: null }; }
    catch (cause) { return { value: undefined, error: projectError(cause).summary }; }
  }, [design, facade]);
  const references = materialized.value ? materialized.value.references ?? NO_REFERENCES : undefined;
  const model = useMemo(() => buildThemeDesignerModel(design, references), [design, references]);
  const context: DesignFieldContext = { design, references: references ?? [] };
  const activeSection = view === "canvas" || view === "typography" ? "overview" : view;
  const sectionLabels = { overview: t("themeDesigner.overview"), shared: t("themeDesigner.sharedValues"), objects: t("themeDesigner.semanticStyles") };

  useLayoutEffect(() => {
    if (!focusRequest || processedFocus.current >= focusRequest.sequence) return;
    processedFocus.current = focusRequest.sequence;
    if (focusRequest.design !== design) return;
    const field = [...(root.current?.querySelectorAll<HTMLElement>("[data-design-value]") ?? [])].find((element) => element.dataset.designValue === focusRequest.path);
    if (!field) { onOpenJson?.(focusRequest.path, focusRequest.returnToken); return; }
    field.scrollIntoView({ block: "nearest" });
    (field.querySelector<HTMLElement>("select, input, textarea, button") ?? field).focus({ preventScroll: true });
  }, [design, focusRequest, onOpenJson]);

  function change(next: ThemeDesignV1) { processedFocus.current = sequence.current; onChange({ ...document, design: next }); }
  function selectSetting(token: string) { setSelectedId(token); setView("shared"); setLocationUnavailable(false); }
  function openUse(use: ThemeBindingUse) {
    const destination = classifyThemeDesignDestination(design, use.path);
    const returnToken = originToken ?? selectedId;
    if (destination.kind === "json") { if (onOpenJson) onOpenJson(use.path, returnToken); else setLocationUnavailable(true); return; }
    setOriginToken(returnToken); setLocationUnavailable(false);
    switch (destination.kind) {
      case "token": selectSetting(destination.tokenId); break;
      case "style": setObjectSelection({ kind: "style", target: destination.target }); setView("objects"); break;
      case "rule": setObjectSelection({ kind: "rule", index: destination.index }); setView("objects"); break;
      case "canvas": if (destination.index !== undefined) setLayerIndex(destination.index); setView("canvas"); break;
      case "typography": setView("typography"); break;
    }
    setFocusRequest({ sequence: ++sequence.current, path: use.path, design, returnToken });
  }
  function navigate(next: View) { processedFocus.current = sequence.current; setView(next); setOriginToken(undefined); setLocationUnavailable(false); }
  function returnToSetting() {
    if (!originToken) return;
    selectSetting(originToken); setOriginToken(undefined);
    setFocusRequest({ sequence: ++sequence.current, path: themeDesignPointer(["tokens", originToken, "value"]), design });
  }

  return <fieldset ref={root} disabled={disabled} className="min-w-0 space-y-4 disabled:opacity-60">
    <div className="grid min-w-0 grid-cols-3 gap-1 rounded-md bg-muted/40 p-1" role="tablist" aria-label={t("themeDesigner.sections")}>
      {SECTIONS.map((section, index) => <button key={section} id={`${id}-${section}`} type="button" role="tab" aria-selected={activeSection === section} aria-controls={`${id}-panel`} tabIndex={activeSection === section ? 0 : -1} className={`min-h-9 min-w-0 break-words rounded px-1.5 py-1 text-xs font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${activeSection === section ? "bg-background shadow-sm" : "text-muted-foreground hover:bg-background/60"}`} onClick={() => navigate(section)} onKeyDown={(event) => {
        let next: number;
        switch (event.key) {
          case "ArrowRight": next = (index + 1) % SECTIONS.length; break;
          case "ArrowLeft": next = (index + SECTIONS.length - 1) % SECTIONS.length; break;
          case "Home": next = 0; break;
          case "End": next = SECTIONS.length - 1; break;
          default: return;
        }
        event.preventDefault(); navigate(SECTIONS[next]);
        const tabs = event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>("[role=tab]"); tabs?.[next]?.focus();
      }}>{sectionLabels[section]}</button>)}
    </div>
    {materialized.error && <p role="alert" className="break-words text-xs text-destructive">{materialized.error}</p>}
    {originToken && <Button type="button" variant="outline" size="sm" className="max-w-full whitespace-normal text-xs" onClick={returnToSetting}>{t("themeDesigner.returnToSetting")}</Button>}
    {locationUnavailable && <p role="status" className="text-xs text-muted-foreground">{t("themeDesigner.locationUnavailable")}</p>}
    <div id={`${id}-panel`} role="tabpanel" aria-labelledby={`${id}-${activeSection}`} className="min-w-0">
      {view === "overview" && <ThemeDesignOverview model={model} onOpenGroup={(next) => { setGroup(next); setQuery(""); navigate("shared"); }} onOpenCanvas={() => navigate("canvas")} onOpenTypography={() => navigate("typography")} onOpenObjects={() => navigate("objects")} />}
      {view === "shared" && <ThemeSharedSettings context={context} model={model} selectedId={selectedId} query={query} group={group} onQuery={setQuery} onGroup={setGroup} onSelect={selectSetting} onOpenUse={openUse} onChange={change} />}
      {view === "objects" && <ThemeObjectStyles context={context} facade={facade} catalog={catalog} familyId={familyId} selection={objectSelection} onSelect={setObjectSelection} onChange={change} />}
      {(view === "canvas" || view === "typography") && <div className="mb-3"><Button type="button" variant="ghost" size="sm" onClick={() => navigate("overview")}>{t("themeDesigner.backToOverview")}</Button></div>}
      {view === "canvas" && <ThemeDesignCanvas document={document} context={context} selectedIndex={layerIndex} onSelectLayer={setLayerIndex} removed={removedCanvas} onRemoved={setRemovedCanvas} onChange={(next) => { processedFocus.current = sequence.current; onChange(next); }} />}
      {view === "typography" && <section className="space-y-3 rounded-md border p-3"><h3 className="text-sm font-semibold">{t("themeWorkbench.typography")}</h3><DesignValueField label={t("themeDesigner.defaultTypography")} type="text-style" value={design.typography?.default} path={["typography", "default"]} context={context} optional onChange={(next) => change(setThemeDesignPath(design, ["typography", "default"], next))} /></section>}
    </div>
    <p className="border-t pt-3 text-[11px] leading-relaxed text-muted-foreground">{t("themeWorkbench.precedence")}</p>
  </fieldset>;
}
