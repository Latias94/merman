import { lazy, useEffect, useMemo, useRef, useState } from "react";
import { Download, FileUp, WandSparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ThemeAppearance, ThemeCatalog, ThemeResolution, ThemeResolveOptions } from "@mermanjs/web";

import { Button } from "@/components/ui/button";
import { TabsContent } from "@/components/ui/tabs";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";
import { downloadBlob } from "@/src/lib/export";
import { resolveThemeSelection, type ThemeAppearancePreference } from "@/src/lib/theme-appearance";
import { exceedsUtf8ByteBudget } from "@/src/lib/utf8";
import type { ThemeDocumentLabel, WorkspaceSnapshot } from "@/src/lib/workspace-snapshot";
import { projectError } from "@/src/runtime/error-projection";
import type { MermanDomainFacade } from "@/src/runtime/merman-core";
import { useAppStore } from "@/src/store";
import { applyContrastFix, contrastGrade, expandThemeDocument } from "@/src/themes/authoring";
import { formatThemeDocument, parseThemeDocument, THEME_DOCUMENT_BYTE_LIMIT } from "@/src/themes/v2";
import { ThemeCustomize } from "./ThemeCustomize";
import { ThemeNotices } from "./ThemeNotices";

const ThemeJsonEditor = lazy(() => import("./ThemeJsonEditor"));

export function ThemeWorkshop({ catalog, facade, workspace, hostAppearance, view, showJson }: {
  readonly catalog: ThemeCatalog;
  readonly facade: MermanDomainFacade;
  readonly workspace: WorkspaceSnapshot;
  readonly hostAppearance: ThemeAppearance;
  readonly view: "customize" | "json" | "gallery";
  showJson(): void;
}) {
  const { t } = useTranslation();
  const fileInput = useRef<HTMLInputElement>(null);
  const importGeneration = useRef(0);
  const [pending, setPending] = useState<{ origin: string; value: string; label: ThemeDocumentLabel | null; preference: ThemeAppearancePreference } | null>(null);
  const [message, setMessage] = useState<{ error: boolean; text: string } | null>(null);
  const basePreset = catalog.presets.find(({ id }) => id === workspace.themePresetId?.trim())
    ?? catalog.presets.find(({ id }) => id === "studio") ?? catalog.presets[0];
  const activeRaw = workspace.themeDocumentJson ?? formatThemeDocument({ schema_version: 2, extends: basePreset?.id });
  const origin = `${workspace.diagramTheme}:${workspace.themePresetId}:${workspace.themeAppearance ?? "default"}:${activeRaw}`;
  if (pending && pending.origin !== origin) setPending(null);
  const draft = pending?.origin === origin ? pending : null;
  const raw = draft?.value ?? activeRaw;
  const preference = draft?.preference ?? (workspace.themePresetId !== null || workspace.themeDocumentJson !== null
    ? workspace.themeAppearance ?? "default" : "default");
  const { appearance, supported_appearances: appearances } = resolveThemeSelection({ ...workspace, themeDocumentJson: raw, themeAppearance: preference }, hostAppearance, catalog);
  const compileAppearance = preference === "default" ? undefined : appearance ?? undefined;
  const resolution = useMemo(() => resolve(facade, raw, { appearance: compileAppearance }), [facade, raw, compileAppearance]);
  const document = resolution.ok ? parseThemeDocument(raw) : null;
  const inherits = document?.extends;
  const exported = useMemo(() => {
    try { return { base: inherits ? facade.exportThemePreset(inherits) : undefined, error: null }; }
    catch (cause) { return { base: undefined, error: projectError(cause).summary }; }
  }, [facade, inherits]);
  const { base } = exported;
  const grade = resolution.ok ? contrastGrade(resolution) : null;

  useEffect(() => () => { importGeneration.current += 1; }, []);

  function apply(next: string, label = draft ? draft.label : workspace.themeDocumentLabel ?? null, nextPreference = preference) {
    setMessage(null);
    importGeneration.current += 1;
    const selection = resolveThemeSelection({ ...workspace, themeDocumentJson: next, themeAppearance: nextPreference }, hostAppearance, catalog);
    const appearance = nextPreference === "default" ? undefined : selection.appearance ?? undefined;
    let checked = resolve(facade, next, { appearance });
    if (!checked.ok && appearance && checked.path === "appearance") {
      checked = resolve(facade, next, {});
      if (checked.ok) nextPreference = "default";
    }
    if (!checked.ok) {
      setPending({ origin, value: next, label, preference: nextPreference });
      showJson();
      return;
    }
    setPending(null);
    useAppStore.getState().selectTheme({ kind: "document", documentJson: next, label, appearance: nextPreference });
  }

  async function importFile(file: File) {
    const generation = ++importGeneration.current;
    const initial = useAppStore.getState();
    setMessage(null);
    try {
      if (file.size > THEME_DOCUMENT_BYTE_LIMIT) throw new Error(t("themeWorkshop.tooLarge"));
      const next = await file.text();
      const current = useAppStore.getState();
      if (generation !== importGeneration.current || initial.appearanceHistory !== current.appearanceHistory ||
        initial.themeDocumentJson !== current.themeDocumentJson || initial.diagramTheme !== current.diagramTheme) return;
      const name = file.name.replace(/(?:\.theme)?\.json$/i, "").trim().slice(0, 100) || t("themeWorkshop.custom");
      apply(next, { name, source: "imported" }, "default");
    } catch (error) {
      if (generation === importGeneration.current) setMessage({ error: true, text: projectError(error).summary });
    }
  }

  function download() {
    downloadBlob(new Blob([raw], { type: "application/json;charset=utf-8" }), `${document?.id ?? "merman"}.theme.json`);
  }

  function fixContrast() {
    if (!document || !resolution.ok) return;
    const fixed = resolve(facade, raw, { appearance: resolution.resolved.appearance, auto_fix_contrast: true });
    if (!fixed.ok) {
      setMessage({ error: true, text: fixed.message });
      return;
    }
    if (!fixed.notices.some(({ kind }) => kind === "contrast_adjusted")) {
      setMessage({ error: false, text: t("themeWorkshop.noContrastFix") });
      return;
    }
    apply(formatThemeDocument(applyContrastFix(document, resolution.resolved, fixed.resolved, appearances)));
  }

  if (view === "gallery") return null;
  return <TabsContent value={view} className="space-y-3">
    <div className="flex flex-wrap gap-1.5">
      <Button type="button" variant="outline" size="sm" onClick={() => fileInput.current?.click()}>
        <FileUp aria-hidden="true" />{t("themeWorkshop.import")}
      </Button>
      <input ref={fileInput} type="file" accept=".json,application/json" className="hidden" aria-label={t("themeWorkshop.import")}
        onChange={(event) => { const file = event.target.files?.[0]; event.target.value = ""; if (file) void importFile(file); }} />
      <Button type="button" variant="outline" size="sm" disabled={!resolution.ok} onClick={download}>
        <Download aria-hidden="true" />{t("themeWorkshop.download")}
      </Button>
    </div>
    {resolution.ok && <div className="space-y-2 rounded-lg border bg-muted/20 p-2.5 text-xs">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span role="status" data-theme-contrast={grade} className={grade === "warning"
          ? "rounded-full bg-amber-500/15 px-2 py-1 font-medium text-amber-900 dark:text-amber-200"
          : "rounded-full bg-emerald-500/15 px-2 py-1 font-medium text-emerald-900 dark:text-emerald-200"}>
          {grade === "warning" ? t("themeWorkshop.contrastWarning") : t("themeWorkshop.contrastPass", { grade })}
        </span>
        <Button type="button" variant="ghost" size="sm" disabled={grade !== "warning"} onClick={fixContrast}>
          <WandSparkles aria-hidden="true" />{t("themeWorkshop.autoFix")}
        </Button>
      </div>
      <ThemeNotices notices={resolution.notices} />
    </div>}
    {message && <p className={message.error ? "text-xs text-destructive" : "text-xs text-muted-foreground"}
      role={message.error ? "alert" : "status"}>{message.text}</p>}
    {exported.error && <p role="alert" className="break-words text-xs text-destructive">{exported.error}</p>}
    {!resolution.ok && <p role="alert" data-testid="theme-json-error" className="break-words rounded-md border border-destructive/40 p-2 text-xs text-destructive">
      {resolution.line ? `${resolution.line}:${resolution.column ?? 1} · ` : ""}
      {resolution.path && resolution.path !== "." ? `${resolution.path} · ` : ""}{resolution.message}
    </p>}
    {draft && <Button type="button" variant="ghost" size="sm" onClick={() => setPending(null)}>{t("themeWorkshop.discard")}</Button>}
    {view === "customize" && document && resolution.ok && <ThemeCustomize catalog={catalog} document={document}
      base={base} resolved={resolution.resolved} onChange={(next) => apply(formatThemeDocument(next))} />}
    {view === "json" && <>
      <LazyFeatureBoundary feature={t("themeWorkshop.json")} presentation={{ kind: "panel" }}>
        <ThemeJsonEditor value={raw} onChange={apply} error={resolution.ok ? null : resolution} />
      </LazyFeatureBoundary>
      {document && base && <Button type="button" variant="outline" size="sm"
        onClick={() => apply(formatThemeDocument(expandThemeDocument(document, base)))}>{t("themeWorkshop.expand")}</Button>}
    </>}
  </TabsContent>;
}

function resolve(facade: MermanDomainFacade, raw: string, options: ThemeResolveOptions): ThemeResolution {
  if (exceedsUtf8ByteBudget(raw, THEME_DOCUMENT_BYTE_LIMIT)) {
    return { ok: false, line: null, column: null, path: null, message: `Theme documents are limited to ${THEME_DOCUMENT_BYTE_LIMIT / 1024} KiB.` };
  }
  try { return facade.resolveTheme(raw, options); }
  catch (cause) { return { ok: false, line: null, column: null, path: null, message: projectError(cause).summary }; }
}
