import { useId, useMemo, useState } from "react";
import { Copy, Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import { strToU8, zipSync } from "fflate";

import { CodePreview } from "@/src/components/CodePreview";
import { ProjectFileBrowser } from "@/src/components/ProjectFileBrowser";
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
import type { ThemeAppearance } from "@mermanjs/web";
import { inspectThemeRecipe } from "@/src/lib/theme-appearance";

type UseTab = "theme" | ProjectUseTarget;
const TABS: readonly UseTab[] = ["theme", "rust", "cli", "web"];

export function ThemeShowcaseUse({ theme, appearance, sampleId, onSampleChange }: {
  readonly theme: ExplorationTheme;
  readonly sampleId: ThemeSampleId;
  readonly appearance: ThemeAppearance;
  onSampleChange(id: ThemeSampleId): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const facade = useMermanRuntime(selectMermanFacade);
  const failure = useMermanRuntime(selectMermanFailure);
  const [tab, setTab] = useState<UseTab>("theme");
  const [rustAssets, setRustAssets] = useState<"inline" | "files">("inline");
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [showExample, setShowExample] = useState(false);
  const [capturedAt] = useState(() => new Date().toISOString());
  const [action, setAction] = useState<"copied" | "downloaded" | "failed" | null>(null);
  const sample = THEME_SAMPLES.find((entry) => entry.id === sampleId)!;
  const target = tab === "theme" ? null : tab;
  const result = useMemo(() => {
    if (!target || !facade) return { bundle: null, error: null };
    try {
      const snapshot = captureProjectHandoff({
        workspace: { ...DEFAULT_WORKSPACE_SNAPSHOT, diagramTheme: "auto", diagramFont: "inherit",
          ...(theme.appearances ? { themeAppearance: appearance } : {}),
          themePresetId: null, themeRecipeJson: theme.recipeJson, themeRecipeLabel: { name: theme.name, source: "exploration" } },
        renderState: { status: "empty" },
        validateThemeRecipe: (raw) => facade.validateThemeRecipe(raw),
        themeRecipeInspection: inspectThemeRecipe(facade, theme.recipeJson),
        hostAppearance: appearance,
        versions: { merman: facade.packageVersion, mermaid: MERMAID_JS_VERSION },
        buildIdentity: typeof __PLAYGROUND_BUILD__ === "undefined" ? undefined : __PLAYGROUND_BUILD__,
        capturedAt,
      });
      return { bundle: generateProjectHandoff(snapshot, { target, scope: "theme", rustAssets,
        themeExample: { source: sample.source, configJson: sample.configJson } }), error: null };
    } catch (error) { return { bundle: null, error: projectError(error).summary }; }
  }, [capturedAt, facade, rustAssets, sample, target, theme, appearance]);
  const bundle = result.bundle;
  const singleFile = target === "rust" && rustAssets === "inline";
  const filename = tab === "theme" ? "theme.json" : bundle && !singleFile && selectedFile && Object.hasOwn(bundle.files, selectedFile) ? selectedFile : bundle?.codeFilename;
  const content = tab === "theme" ? theme.recipeJson : bundle && filename ? bundle.files[filename] : "";

  async function copy(text: string) {
    try { await navigator.clipboard.writeText(text); setAction("copied"); }
    catch { setAction("failed"); }
  }

  function download() {
    try {
      if (bundle && singleFile) {
        downloadBlob(new Blob([bundle.code], { type: "text/plain;charset=utf-8" }), "main.rs");
      } else if (bundle && target) {
        const archive = zipSync(Object.fromEntries(Object.entries(bundle.files).map(([name, text]) => [name, strToU8(text)])), { level: 6 });
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
    {theme.appearances && <p className="text-xs text-muted-foreground" data-testid="theme-adoption-appearance">
      {t("appearanceMode.adoption", { appearance: t(`appearanceMode.${appearance}`) })}
    </p>}
    <dl className="grid gap-3 rounded-md border bg-background/60 p-3 text-xs sm:grid-cols-2" data-testid="theme-adoption-guide">
      <div className="space-y-1"><dt className="font-semibold">{t("themeShowcase.themePurposeTitle")}</dt>
        <dd className="leading-relaxed text-muted-foreground">{t("themeShowcase.themePurpose")}</dd></div>
      <div className="space-y-1"><dt className="font-semibold">{t("themeShowcase.optionsPurposeTitle")}</dt>
        <dd className="leading-relaxed text-muted-foreground">{t("themeShowcase.optionsPurpose")}</dd></div>
    </dl>
    <Tabs value={tab} onValueChange={(value) => { if (TABS.includes(value as UseTab)) setTab(value as UseTab); setSelectedFile(null); setAction(null); }} className="min-w-0 gap-3">
      <TabsList aria-label={t("themeShowcase.codeTarget")} className="h-auto max-w-full flex-wrap justify-start">
        {TABS.map((entry) => <TabsTrigger key={entry} value={entry} className="px-2.5 text-xs">{t(`themeShowcase.tabs.${entry}`)}</TabsTrigger>)}
      </TabsList>
      <TabsContent value={tab} className="min-w-0 space-y-3">
        {target ? <>
          <label className="flex flex-wrap items-center gap-2 text-xs" htmlFor={`${id}-sample`}>
            <span className="font-medium">{t("themeShowcase.integrationExample")}</span>
            <select id={`${id}-sample`} value={sampleId} className="h-8 min-w-0 rounded-md border bg-background px-2"
              onChange={(event) => { const next = THEME_SAMPLES.find((entry) => entry.id === event.target.value); if (next) onSampleChange(next.id); setSelectedFile(null); setAction(null); }}>
              {THEME_SAMPLES.map((entry) => <option key={entry.id} value={entry.id}>{t(`themeGallery.samples.${entry.id}`)}</option>)}
            </select>
          </label>
          {target === "rust" && <fieldset className="space-y-2 text-xs">
            <legend className="font-medium">{t("themeShowcase.rustFormat")}</legend>
            <div className="flex flex-wrap gap-4">
              {(["inline", "files"] as const).map((mode) => <label key={mode} className="flex cursor-pointer items-center gap-2">
                <input type="radio" name={`${id}-rust`} checked={rustAssets === mode} onChange={() => { setRustAssets(mode); setSelectedFile(null); setAction(null); }} />
                {t(`themeShowcase.rust_${mode}`)}
              </label>)}
            </div>
            <p className="leading-relaxed text-muted-foreground">{t(`themeShowcase.rustHint_${rustAssets}`)}</p>
          </fieldset>}
          <p className="text-xs leading-relaxed text-muted-foreground">{t(singleFile ? "themeShowcase.rustDependencies" : `projectUse.setup_${target}`)}</p>
        </> : <p className="text-xs leading-relaxed text-muted-foreground">{t("themeShowcase.themeFileHint")}</p>}
        {target && failure ? <div role="alert" className="space-y-2 rounded-md border border-destructive/40 p-3 text-xs">
          <p className="break-words">{t("themeShowcase.runtimeFailed")}</p>
          <p className="break-words text-muted-foreground">{failure.message}</p>
          <Button type="button" size="sm" variant="outline" onClick={() => {
            if (failure.recovery === "reload") window.location.reload();
            else void retryMermanRuntime().catch(() => undefined);
          }}>{t(failure.recovery === "reload" ? "wasm.reload" : "wasm.retry")}</Button>
        </div> : result.error ? <p role="alert" className="break-words text-sm text-destructive">{result.error}</p>
          : content ? <div className={bundle && !singleFile ? "grid min-w-0 grid-cols-1 overflow-hidden rounded-md border bg-background sm:grid-cols-[11rem_minmax(0,1fr)]" : "min-w-0 overflow-hidden rounded-md border bg-background"}>
            {bundle && !singleFile && <ProjectFileBrowser files={bundle.files} selected={filename!} onSelect={(name) => { setSelectedFile(name); setAction(null); }} />}
            <div className="min-w-0">
              <div className="flex flex-wrap items-center justify-between gap-2 border-b bg-muted/20 px-3 py-2">
                <span className="min-w-0 break-all font-mono text-[11px]">{filename}</span>
                <Button type="button" variant="ghost" size="sm" onClick={() => void copy(content)}><Copy className="size-3.5" aria-hidden="true" />{t("themeShowcase.copy")}</Button>
              </div>
              <CodePreview value={content} filename={filename!} label={t("themeShowcase.code")} testId="theme-showcase-code" />
            </div>
          </div> : <p role="status" className="text-xs text-muted-foreground">{t("appearance.catalogPending")}</p>}
        {target && !singleFile && <p className="text-xs leading-relaxed text-muted-foreground">{t("themeShowcase.companionFiles")}</p>}
      </TabsContent>
    </Tabs>
    <div className="flex flex-wrap items-center justify-between gap-3">
      <p role={action === "failed" ? "alert" : "status"} className="text-xs text-muted-foreground">{action ? t(`themeShowcase.action_${action}`) : t("themeShowcase.customRecipeHint")}</p>
      <Button type="button" variant="outline" size="sm" disabled={!content} onClick={download}><Download className="size-3.5" aria-hidden="true" />{t(singleFile ? "themeShowcase.downloadRust" : target ? "themeShowcase.downloadProject" : "themeShowcase.downloadFile")}</Button>
    </div>
    <details className="min-w-0 border-t pt-3 text-xs" onToggle={(event) => setShowExample(event.currentTarget.open)}>
      <summary className="cursor-pointer font-medium">{t("themeShowcase.sampleAndFonts")}</summary>
      {showExample && <div className="mt-3 min-w-0 space-y-3 text-muted-foreground">
        <p className="leading-relaxed">{t("themeShowcase.exampleOptionsHint")}</p>
        <div className="grid min-w-0 gap-3 lg:grid-cols-2">
          {([{ name: `${sampleId}.mmd`, text: sample.source, label: t("themeShowcase.exampleSource") },
            { name: `${sampleId}.config.json`, text: sample.configJson, label: t("themeShowcase.exampleOptions") }]).map((file) => <div key={file.name} className="min-w-0 overflow-hidden rounded-md border bg-background">
              <div className="flex items-center justify-between gap-2 border-b px-3 py-2"><span className="font-medium">{file.label}</span>
                <Button type="button" variant="ghost" size="sm" aria-label={t("projectUse.copyFile", { file: file.name })} onClick={() => void copy(file.text)}><Copy className="size-3.5" aria-hidden="true" />{t("themeShowcase.copy")}</Button></div>
              <CodePreview value={file.text} filename={file.name} label={file.label} />
            </div>)}
        </div>
        <p className="leading-relaxed">{t("themeShowcase.coverageHint")}</p>
        <p className="leading-relaxed">{t("projectUse.notice_fonts")}</p>
      </div>}
    </details>
  </section>;
}
