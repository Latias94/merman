import { lazy, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ThemeRecipeInspectionV1 } from "@mermanjs/web";
import { Button } from "@/components/ui/button";
import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";
import { SHARE_THEME_RECIPE_BYTES } from "@/src/lib/share";
import { exceedsUtf8ByteBudget } from "@/src/lib/utf8";
import { compatibleAppearancePreference, inspectThemeRecipe, type ThemeAppearancePreference } from "@/src/lib/theme-appearance";
import { sameThemeEditOrigin } from "@/src/lib/theme-draft";
import type { ThemeRecipeLabel, WorkspaceSnapshot } from "@/src/lib/workspace-snapshot";
import type { MermanDomainFacade } from "@/src/runtime/merman-core";
import { projectError } from "@/src/runtime/error-projection";
import { selectWorkspaceSnapshot, useAppStore } from "@/src/store";

const ThemeDraftEditor = lazy(() => import("./ThemeDraftEditorFeature"));

export function ThemeDraftWorkbench({ raw, origin, label, facade, appearancePreference, onClose }: {
  readonly raw: string;
  readonly origin: Readonly<WorkspaceSnapshot>;
  readonly label: ThemeRecipeLabel;
  readonly facade: MermanDomainFacade;
  readonly appearancePreference: ThemeAppearancePreference;
  onClose(): void;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(raw);
  const [scope, setScope] = useState<"shared" | "light" | "dark">("shared");
  const [validation, setValidation] = useState<{ raw: string; inspection: ThemeRecipeInspectionV1 } | null>(null);
  const [replacement, setReplacement] = useState<ThemeAppearancePreference | "">("");
  const [error, setError] = useState<string | null>(null);
  const valid = validation?.raw === draft ? validation : null;
  const originalPreference = appearancePreference;
  const compatible = valid && compatibleAppearancePreference(originalPreference, valid.inspection);
  const preference = compatible ?? replacement;
  const canApply = Boolean(valid && preference);

  function validate() {
    const text = draft;
    setError(null);
    setValidation(null);
    try {
      if (exceedsUtf8ByteBudget(text, SHARE_THEME_RECIPE_BYTES)) {
        throw new Error(t("customTheme.tooLarge", { limit: SHARE_THEME_RECIPE_BYTES / 1024 }));
      }
      const inspection = inspectThemeRecipe(facade, text);
      if (!sameThemeEditOrigin(origin, selectWorkspaceSnapshot(useAppStore.getState()))) return;
      setValidation({ raw: text, inspection });
      setReplacement("");
    } catch (cause) {
      setError(projectError(cause).summary);
    }
  }

  function apply() {
    if (!valid || !preference || !sameThemeEditOrigin(origin, selectWorkspaceSnapshot(useAppStore.getState()))) return;
    try {
      inspectThemeRecipe(facade, valid.raw);
      useAppStore.getState().selectTheme({
        kind: "recipe", recipeJson: valid.raw, label, appearance: preference, compose: true,
      });
      onClose();
    } catch (cause) { setError(projectError(cause).summary); }
  }

  return <section className="min-w-0 space-y-3 rounded-md border p-3" data-testid="theme-draft-workbench">
    <h3 className="text-xs font-semibold">{t("themeDraft.title")}</h3>
    <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themeDraft.hint")}</p>
    <div className="flex flex-wrap gap-1" aria-label={t("themeDraft.navigate")}>
      {(["shared", "light", "dark"] as const).map((entry) => <Button key={entry} type="button" size="sm"
        variant={scope === entry ? "secondary" : "ghost"} aria-pressed={scope === entry} onClick={() => setScope(entry)}>
        {t(`themeDraft.${entry}`)}
      </Button>)}
    </div>
    <LazyFeatureBoundary feature={t("themeDraft.editor")} presentation={{ kind: "panel" }}>
      <ThemeDraftEditor value={draft} scope={scope} onChange={(value) => {
        setDraft(value); setValidation(null); setError(null); setReplacement("");
      }} />
    </LazyFeatureBoundary>
    {valid && <p role="status" className="text-xs text-muted-foreground">{t("themeDraft.valid")}</p>}
    {valid && !compatible && <label className="block space-y-2 text-xs">
      <span>{t("themeDraft.chooseAppearance")}</span>
      <select value={replacement} onChange={(event) => setReplacement(event.target.value as ThemeAppearancePreference)}
        className="h-8 w-full rounded-md border bg-background px-2">
        <option value="" disabled>{t("themeDraft.chooseAppearance")}</option>
        {valid.inspection.supported_appearances.length > 1 && <option value="follow">{t("appearanceMode.follow")}</option>}
        {valid.inspection.supported_appearances.map((entry) => <option key={entry} value={entry}>{t(`appearanceMode.${entry}`)}</option>)}
        {valid.inspection.supported_appearances.length === 0 && <option value="default">{t("appearanceMode.unclassified")}</option>}
      </select>
    </label>}
    {error && <p role="alert" className="whitespace-pre-wrap break-words text-xs text-destructive">{error}</p>}
    <div className="flex flex-wrap gap-2">
      <Button type="button" variant="outline" size="sm" onClick={validate}>{t("themeDraft.validate")}</Button>
      <Button type="button" size="sm" disabled={!canApply} onClick={apply}>{t("themeDraft.apply")}</Button>
      <Button type="button" variant="ghost" size="sm" onClick={onClose}>{t("themeDraft.cancel")}</Button>
    </div>
  </section>;
}
