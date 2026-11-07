import { lazy, useRef, useState } from "react";
import { CodeXml } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";
import type { ProjectHandoffInput } from "@/src/lib/project-handoff";
import { mermanRuntimeStore } from "@/src/runtime/merman";
import { MERMAID_JS_VERSION } from "@/src/runtime/mermaid-requirements";
import { renderCoordinatorStore } from "@/src/runtime/render-coordinator-browser";
import { selectWorkspaceSnapshot, useAppStore } from "@/src/store";
import { inspectThemeRecipe } from "@/src/lib/theme-appearance";

const ProjectUseDialog = lazy(() => import("./ProjectUseDialog"));

/** Standalone toolbar entry; all consumer generation stays behind the lazy dialog. */
export function ProjectUseLauncher({ compact }: { compact: boolean }) {
  const { t } = useTranslation();
  const trigger = useRef<HTMLButtonElement>(null);
  const [input, setInput] = useState<ProjectHandoffInput | null>(null);

  function capture() {
    const runtime = mermanRuntimeStore.getState();
    const facade = runtime.status === "ready" ? runtime.facade : null;
    let presets: ProjectHandoffInput["presets"];
    let themeCatalog: ProjectHandoffInput["themeCatalog"];
    try { themeCatalog = facade?.themeCatalog(); presets = themeCatalog?.presets; }
    catch { /* A completed render can still establish preset availability. */ }
    const state = useAppStore.getState();
    let themeRecipeInspection: ProjectHandoffInput["themeRecipeInspection"];
    if (facade && state.themeRecipeJson) {
      try { themeRecipeInspection = inspectThemeRecipe(facade, state.themeRecipeJson); } catch { /* Validation reports its error in the dialog. */ }
    }
    setInput({
      workspace: selectWorkspaceSnapshot(state),
      hostAppearance: state.resolvedTheme,
      themeCatalog,
      themeRecipeInspection,
      renderState: renderCoordinatorStore.getState(),
      presets,
      validateThemeRecipe: facade ? (raw) => facade.validateThemeRecipe(raw) : undefined,
      versions: facade ? { merman: facade.packageVersion, mermaid: MERMAID_JS_VERSION } : undefined,
      buildIdentity: typeof __PLAYGROUND_BUILD__ === "undefined" ? undefined : __PLAYGROUND_BUILD__,
      capturedAt: new Date().toISOString(),
    });
  }

  const restoreFocus = () => { trigger.current?.focus(); };
  return <>
    <Button ref={trigger} type="button" variant="outline" size={compact ? "icon-sm" : "sm"}
      aria-label={t("projectUse.title")} title={t("projectUse.title")} data-testid="project-use-launcher" onClick={capture}>
      <CodeXml className="size-4" aria-hidden="true" />
      {!compact && <span>{t("projectUse.title")}</span>}
    </Button>
    {input && <LazyFeatureBoundary key={input.capturedAt} feature={t("projectUse.title")}
      presentation={{ kind: "dialog", open: true, onOpenChange: (open) => { if (!open) setInput(null); }, restoreFocus }}>
      <ProjectUseDialog input={input} onClose={() => setInput(null)} onRefresh={capture} restoreFocus={restoreFocus} />
    </LazyFeatureBoundary>}
  </>;
}
