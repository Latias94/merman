import { useId, useMemo, useState } from "react";
import { Copy, Download, RefreshCw, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import { strToU8, zipSync } from "fflate";

import { CodePreview } from "@/src/components/CodePreview";
import { ProjectFileBrowser } from "@/src/components/ProjectFileBrowser";
import { Button } from "@/components/ui/button";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { downloadBlob } from "@/src/lib/export";
import {
  captureProjectHandoff, generateProjectHandoff, projectHandoffMatchesWorkspace, ProjectHandoffError,
  type ProjectHandoffInput, type ProjectUseScope, type ProjectUseTarget,
} from "@/src/lib/project-handoff";
import { selectWorkspaceSnapshot, useAppStore } from "@/src/store";

const TARGETS = ["rust", "cli", "web"] as const;
const TARGET_LABELS = { rust: "Rust", cli: "CLI", web: "Web" };

export default function ProjectUseDialog({ input, onClose, onRefresh, restoreFocus }: {
  input: ProjectHandoffInput;
  onClose(): void;
  onRefresh(): void;
  restoreFocus(): void;
}) {
  const { t } = useTranslation();
  const scopeId = useId();
  const [snapshot] = useState(() => captureProjectHandoff(input));
  const workspace = useAppStore(useShallow(selectWorkspaceSnapshot));
  const changed = !projectHandoffMatchesWorkspace(snapshot, workspace);
  const [target, setTarget] = useState<ProjectUseTarget>("rust");
  const [scope, setScope] = useState<ProjectUseScope>("diagram");
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [actionStatus, setActionStatus] = useState<"copied" | "downloaded" | "failed" | null>(null);
  const result = useMemo(() => {
    try { return { bundle: generateProjectHandoff(snapshot, { target, scope }), error: null }; }
    catch (error) {
      return { bundle: null, error: error instanceof ProjectHandoffError ? error : new ProjectHandoffError("invalid-configuration") };
    }
  }, [snapshot, target, scope]);
  const { bundle, error } = result;
  const filename = bundle && selectedFile && Object.hasOwn(bundle.files, selectedFile) ? selectedFile : bundle?.codeFilename;
  const content = bundle && filename ? bundle.files[filename] : "";
  const label = snapshot.workspace.themeRecipeLabel;
  const themeName = label?.name ?? snapshot.workspace.themePresetId ?? t(snapshot.workspace.themeRecipeJson !== null ? "projectUse.unnamedRecipe" : "projectUse.noMermanTheme");

  function resetActions() { setActionStatus(null); setSelectedFile(null); }
  async function copy() {
    if (!bundle) return;
    try { await navigator.clipboard.writeText(content); setActionStatus("copied"); }
    catch { setActionStatus("failed"); }
  }
  function download() {
    if (!bundle) return;
    try {
      const entries = Object.fromEntries(Object.entries(bundle.files).map(([name, text]) => [name, strToU8(text)]));
      const archive = zipSync(entries, { level: 6 });
      downloadBlob(new Blob([new Uint8Array(archive).buffer], { type: "application/zip" }), `merman-${scope}-${target}.zip`);
      setActionStatus("downloaded");
    } catch { setActionStatus("failed"); }
  }

  return <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
    <DialogContent showCloseButton={false} data-testid="project-use-dialog"
      className="flex max-h-[calc(100dvh-2rem)] min-h-0 w-full min-w-0 flex-col gap-0 overflow-hidden p-0 sm:max-w-4xl"
      onCloseAutoFocus={(event) => { event.preventDefault(); restoreFocus(); }}>
      <DialogHeader className="relative shrink-0 border-b px-5 py-4 pr-14 text-left">
        <DialogTitle>{t("projectUse.title")}</DialogTitle>
        <DialogDescription>{t("projectUse.description")}</DialogDescription>
        <DialogClose asChild><Button type="button" variant="ghost" size="icon-sm" className="absolute right-3 top-3" aria-label={t("projectUse.close")}><X className="size-4" /></Button></DialogClose>
      </DialogHeader>
      <div className="min-h-0 space-y-4 overflow-y-auto overscroll-contain p-4 sm:p-5">
        <div className="flex flex-wrap items-start justify-between gap-2 rounded-md border bg-muted/20 p-3">
          <div className="min-w-0 space-y-1 text-xs">
            <p className="break-words font-medium" data-testid="project-use-snapshot">
              {snapshot.publicationId === null ? t("projectUse.unrenderedSnapshot") : t("projectUse.renderSnapshot", { publication: snapshot.publicationId })}
            </p>
            <p className="break-words text-muted-foreground">{t("projectUse.capturedAt", { time: new Date(snapshot.capturedAt).toLocaleString() })}</p>
            <p role="status" data-testid="project-use-freshness" className="max-w-xl break-words text-muted-foreground">
              {t(changed ? "projectUse.workspaceChanged" : "projectUse.snapshotFixed")}
            </p>
          </div>
          <Button type="button" size="sm" variant="outline" onClick={onRefresh}><RefreshCw className="size-3.5" aria-hidden="true" />{t("projectUse.refresh")}</Button>
        </div>

        <fieldset className="space-y-2">
          <legend className="text-sm font-medium">{t("projectUse.scope")}</legend>
          <div className="grid gap-2 sm:grid-cols-2">
            {(["theme", "diagram"] as const).map((value) => <label key={value} className="flex cursor-pointer items-start gap-2 rounded-md border p-3 text-sm has-checked:border-primary has-checked:bg-primary/5">
              <input type="radio" name={scopeId} value={value} checked={scope === value} className="mt-0.5 accent-primary"
                onChange={() => { setScope(value); resetActions(); }} />
              <span className="min-w-0"><span className="block font-medium">{t(`projectUse.scope_${value}`)}</span>
                <span className="mt-1 block text-xs text-muted-foreground">{t(`projectUse.scopeDescription_${value}`)}</span></span>
            </label>)}
          </div>
        </fieldset>

        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 break-words text-xs" data-testid="project-use-inputs">
          <dt className="text-muted-foreground">{t("projectUse.theme")}</dt><dd>{themeName} · {t(label ? `projectUse.origin_${label.source}` : snapshot.workspace.themePresetId ? "projectUse.origin_builtin" : snapshot.workspace.themeRecipeJson !== null ? "projectUse.origin_recipe" : "projectUse.origin_none")}</dd>
          <dt className="text-muted-foreground">{t("projectUse.baseTheme")}</dt><dd>{snapshot.workspace.diagramTheme}</dd>
          {scope === "diagram" && <>
            <dt className="text-muted-foreground">{t("projectUse.environment")}</dt>
            <dd>{snapshot.workspace.diagramFont} · {snapshot.workspace.textMeasurementMode} · {snapshot.workspace.svgPipeline}
              {snapshot.operation && ` · ${snapshot.operation.layoutEnvironment.containerWidth} × ${snapshot.operation.layoutEnvironment.containerHeight}`}</dd>
          </>}
          <dt className="text-muted-foreground">{t("projectUse.versions")}</dt><dd>{snapshot.versions ? `Merman ${snapshot.versions.merman} · Mermaid ${snapshot.versions.mermaid}` : t("projectUse.unknownVersion")}</dd>
          {snapshot.buildIdentity && <>
            <dt className="text-muted-foreground">{t("projectUse.checkout")}</dt>
            <dd>{snapshot.buildIdentity.commit ?? t("projectUse.unknownVersion")} · {snapshot.buildIdentity.ref ?? snapshot.buildIdentity.channel} · {t(snapshot.buildIdentity.dirty ? "projectUse.dirtyCheckout" : "projectUse.cleanCheckout")}</dd>
          </>}
        </dl>

        <Tabs value={target} onValueChange={(value) => { setTarget(value as ProjectUseTarget); resetActions(); }} className="min-w-0 gap-3">
          <TabsList aria-label={t("projectUse.target")} className="w-full sm:w-fit">
            {TARGETS.map((value) => <TabsTrigger key={value} value={value}>{TARGET_LABELS[value]}</TabsTrigger>)}
          </TabsList>
          <TabsContent value={target} className="min-w-0 space-y-3">
            {error && <div role="alert" className="space-y-1 rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm" data-testid="project-use-generation-error">
              <p>{t(`projectUse.error_${error.code}`, { id: snapshot.workspace.themePresetId })}</p>
              {error.code === "invalid-configuration" && error.message !== error.code && <p className="break-words text-xs text-muted-foreground">{error.message}</p>}
            </div>}
            {bundle && <>
              <p className="text-xs text-muted-foreground">{t(`projectUse.setup_${target}`)}</p>
              <div className="grid min-w-0 grid-cols-1 overflow-hidden rounded-md border sm:grid-cols-[11rem_minmax(0,1fr)]">
                <ProjectFileBrowser files={bundle.files} selected={filename!} onSelect={(name) => { setSelectedFile(name); setActionStatus(null); }} />
                <div className="min-w-0">
                <div className="flex flex-wrap items-center justify-between gap-2 border-b bg-muted/30 px-3 py-2">
                  <span className="min-w-0 break-all font-mono text-[11px]">{filename}</span>
                  <Button type="button" variant="ghost" size="sm" onClick={() => { void copy(); }} aria-label={t("projectUse.copyFile", { file: filename })}><Copy className="size-3.5" aria-hidden="true" />{t("projectUse.copy")}</Button>
                </div>
                <CodePreview value={content} filename={filename!} label={t("projectUse.generatedCode")} testId="project-use-code" />
                </div>
              </div>
              <p className="text-xs text-muted-foreground" data-testid="project-use-config-provenance">{t("projectUse.configProvenance")}</p>
              <ul className="space-y-1 text-xs text-muted-foreground" data-testid="project-use-host-notices">
                {bundle.notices.map((notice) => <li key={notice}>{t(`projectUse.notice_${notice}`)}</li>)}
              </ul>
            </>}
          </TabsContent>
        </Tabs>
      </div>
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t px-4 py-3 sm:px-5">
        <p role={actionStatus === "failed" ? "alert" : "status"} aria-live="polite" className="min-w-0 text-xs text-muted-foreground">
          {actionStatus ? t(`projectUse.action_${actionStatus}`) : bundle ? t("projectUse.fileCount", { count: Object.keys(bundle.files).length }) : t("projectUse.chooseScope")}
        </p>
        <Button type="button" size="sm" disabled={!bundle} onClick={download}><Download className="size-4" aria-hidden="true" />{t("projectUse.downloadAll")}</Button>
      </div>
    </DialogContent>
  </Dialog>;
}
