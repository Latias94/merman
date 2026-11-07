import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { ResolvedThemeSelection, ThemeAppearancePreference } from "@/src/lib/theme-appearance";

export function ThemeAppearanceControl({ selection, onChange }: {
  readonly selection: ResolvedThemeSelection;
  onChange(preference: ThemeAppearancePreference): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const { supported_appearances: supported, preference, appearance } = selection;
  return <section className="space-y-2" data-testid="theme-appearance-control" data-effective-appearance={selection.error ? "invalid" : appearance ?? "unclassified"}>
    <label htmlFor={id} className="block text-xs font-semibold">{t("appearanceMode.title")}</label>
    {supported.length > 1 ? <select id={id} className="h-8 w-full min-w-0 rounded-md border bg-background px-2 text-xs"
      value={preference} onChange={(event) => onChange(event.target.value as ThemeAppearancePreference)}>
      <option value="default">{t("appearanceMode.documentDefault")}</option>
      <option value="follow">{t("appearanceMode.follow")}</option>
      {supported.map((mode) => <option key={mode} value={mode}>{t(`appearanceMode.${mode}`)}</option>)}
    </select> : <p id={id} className="text-xs text-muted-foreground">
      {supported.length === 1 ? t("appearanceMode.only", { appearance: t(`appearanceMode.${supported[0]}`) }) : t("appearanceMode.unclassified")}
    </p>}
    {selection.error && <p role="alert" className="text-xs text-destructive" data-testid="theme-appearance-error">{selection.error}</p>}
    {appearance && <p role="status" className="text-[11px] leading-relaxed text-muted-foreground" data-testid="theme-effective-appearance">
      {t(preference === "follow" ? "appearanceMode.following" : "appearanceMode.selected", { appearance: t(`appearanceMode.${appearance}`) })}
    </p>}
  </section>;
}
