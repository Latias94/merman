import { useId, useMemo, useState } from "react";
import { Copy, Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import { strToU8, zipSync } from "fflate";

import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { downloadBlob } from "@/src/lib/export";
import { captureProjectHandoff, generateProjectHandoff, type ProjectUseTarget } from "@/src/lib/project-handoff";
import { projectError } from "@/src/runtime/error-projection";
import { retryMermanRuntime } from "@/src/runtime/merman";
import { MERMAID_JS_VERSION } from "@/src/runtime/mermaid-requirements";
import { selectMermanFacade, selectMermanFailure, useMermanRuntime } from "@/src/runtime/use-merman-runtime";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "@/src/store";
import type { ExplorationTheme } from "@/src/themes/catalog";
import { THEME_SAMPLES, type ThemeSampleId } from "@/src/themes/samples";

type UseTab = "theme" | "config" | ProjectUseTarget;
const TABS: readonly UseTab[] = ["theme", "config", "rust", "cli", "web"];

export function ThemeShowcaseUse({ theme, sampleId, onSampleChange }: {
  readonly theme: ExplorationTheme;
  readonly sampleId: ThemeSampleId;
  onSampleChange(id: ThemeSampleId): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const facade = useMermanRuntime(selectMermanFacade);
  const failure = useMermanRuntime(selectMermanFailure);
  const [tab, setTab] = useState<UseTab>("theme");
  const [capturedAt] = useState(() => new Date().toISOString());
  const [action, setAction] = useState<"copied" | "downloaded" | "failed" | null>(null);
  const sample = THEME_SAMPLES.find((entry) => entry.id === sampleId)!;
  const target = tab === "theme" || tab === "config" ? null : tab;
  const result = useMemo(() => {
    if (!target || !facade) return { bundle: null, error: null };
    try {
      const snapshot = captureProjectHandoff({
        workspace: { ...DEFAULT_WORKSPACE_SNAPSHOT, diagramTheme: "default", diagramFont: "inherit",
          themePresetId: null, themeRecipeJson: theme.recipeJson, themeRecipeLabel: { name: theme.name, source: "exploration" } },
        renderState: { status: "empty" },
        validateThemeRecipe: (raw) => facade.validateThemeRecipe(raw),
        versions: { merman: facade.packageVersion, mermaid: MERMAID_JS_VERSION },
        buildIdentity: typeof __PLAYGROUND_BUILD__ === "undefined" ? undefined : __PLAYGROUND_BUILD__,
        capturedAt,
      });
      return { bundle: generateProjectHandoff(snapshot, { target, scope: "theme", themeExample: { source: sample.source, configJson: sample.configJson } }), error: null };
    } catch (error) { return { bundle: null, error: projectError(error).summary }; }
  }, [capturedAt, facade, sample, target, theme]);
  const content = tab === "theme" ? theme.recipeJson : tab === "config" ? sample.configJson : result.bundle?.code ?? "";
  const filename = tab === "theme" ? `${theme.id}.theme.json` : tab === "config" ? `${sampleId}.config.json` : result.bundle?.codeFilename;

  async function copy() {
    try { await navigator.clipboard.writeText(content); setAction("copied"); }
    catch { setAction("failed"); }
  }

  function download() {
    try {
      if (result.bundle && target) {
        const archive = zipSync(Object.fromEntries(Object.entries(result.bundle.files).map(([name, text]) => [name, strToU8(text)])), { level: 6 });
        downloadBlob(new Blob([new Uint8Array(archive).buffer], { type: "application/zip" }), `${theme.id}-${sampleId}-${target}.zip`);
      } else if (filename) {
        downloadBlob(new Blob([content], { type: "application/json;charset=utf-8" }), filename);
      }
      setAction("downloaded");
    } catch { setAction("failed"); }
  }

  return <section aria-labelledby={`${id}-title`} className="min-w-0 space-y-3 rounded-lg border bg-muted/10 p-4 sm:p-5" data-testid="theme-showcase-use">
    <h3 id={`${id}-title`} className="text-lg font-semibold tracking-tight">{t("themeShowcase.useTitle")}</h3>
    <p className="max-w-3xl text-xs leading-relaxed text-muted-foreground">{t("themeShowcase.useHint")}</p>
    <label className="flex flex-wrap items-center gap-2 text-xs" htmlFor={`${id}-sample`}>
      <span className="font-medium">{t("themeGallery.sample")}</span>
      <select id={`${id}-sample`} value={sampleId} className="h-8 min-w-0 rounded-md border bg-background px-2"
        onChange={(event) => { const next = THEME_SAMPLES.find((entry) => entry.id === event.target.value); if (next) onSampleChange(next.id); setAction(null); }}>
        {THEME_SAMPLES.map((entry) => <option key={entry.id} value={entry.id}>{t(`themeGallery.samples.${entry.id}`)}</option>)}
      </select>
    </label>
    <Tabs value={tab} onValueChange={(value) => { if (TABS.includes(value as UseTab)) setTab(value as UseTab); setAction(null); }} className="min-w-0 gap-3">
      <TabsList aria-label={t("themeShowcase.codeTarget")} className="h-auto max-w-full flex-wrap justify-start">
        {TABS.map((entry) => <TabsTrigger key={entry} value={entry} className="px-2.5 text-xs">{t(`themeShowcase.tabs.${entry}`)}</TabsTrigger>)}
      </TabsList>
      <TabsContent value={tab} className="min-w-0 space-y-3">
        {target && <p className="text-xs leading-relaxed text-muted-foreground">{t(`projectUse.setup_${target}`)}</p>}
        {target && failure ? <div role="alert" className="space-y-2 rounded-md border border-destructive/40 p-3 text-xs">
          <p className="break-words">{t("themeShowcase.runtimeFailed")}</p>
          <p className="break-words text-muted-foreground">{failure.message}</p>
          <Button type="button" size="sm" variant="outline" onClick={() => {
            if (failure.recovery === "reload") window.location.reload();
            else void retryMermanRuntime().catch(() => undefined);
          }}>{t(failure.recovery === "reload" ? "wasm.reload" : "wasm.retry")}</Button>
        </div> : result.error ? <p role="alert" className="break-words text-sm text-destructive">{result.error}</p>
          : content ? <div className="min-w-0 overflow-hidden rounded-md border bg-background">
            <div className="flex flex-wrap items-center justify-between gap-2 border-b bg-muted/20 px-3 py-2">
              <span className="min-w-0 break-all font-mono text-[11px]">{filename}</span>
              <Button type="button" variant="ghost" size="sm" onClick={() => void copy()}><Copy className="size-3.5" aria-hidden="true" />{t("themeShowcase.copy")}</Button>
            </div>
            <pre tabIndex={0} aria-label={t("themeShowcase.code")} data-testid="theme-showcase-code" className="max-h-72 min-w-0 overflow-auto p-3 text-xs leading-relaxed"><code>{content}</code></pre>
          </div> : <p role="status" className="text-xs text-muted-foreground">{t("appearance.catalogPending")}</p>}
        {target && <p className="text-xs leading-relaxed text-muted-foreground">{t("themeShowcase.companionFiles")}</p>}
      </TabsContent>
    </Tabs>
    <div className="flex flex-wrap items-center justify-between gap-3">
      <p role={action === "failed" ? "alert" : "status"} className="text-xs text-muted-foreground">{action ? t(`projectUse.action_${action}`) : t("themeShowcase.customRecipeHint")}</p>
      <Button type="button" variant="outline" size="sm" disabled={!content} onClick={download}><Download className="size-3.5" aria-hidden="true" />{t(target ? "themeShowcase.downloadProject" : "themeShowcase.downloadFile")}</Button>
    </div>
    <details className="min-w-0 border-t pt-3 text-xs">
      <summary className="cursor-pointer font-medium">{t("themeShowcase.sampleAndFonts")}</summary>
      <div className="mt-3 min-w-0 space-y-3 text-muted-foreground">
        <pre className="overflow-auto rounded border bg-background p-3 text-xs"><code>{sample.source}</code></pre>
        <p className="leading-relaxed">{t("themeShowcase.coverageHint")}</p>
        <p className="leading-relaxed">{t("projectUse.notice_fonts")}</p>
      </div>
    </details>
  </section>;
}
