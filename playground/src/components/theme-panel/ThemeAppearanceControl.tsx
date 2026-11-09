import { useId } from "react";
import { useTranslation } from "react-i18next";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { ResolvedThemeSelection, ThemeAppearancePreference } from "@/src/lib/theme-appearance";

export function ThemeAppearanceControl({ selection, onChange }: {
  readonly selection: ResolvedThemeSelection;
  onChange(preference: ThemeAppearancePreference): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const { supported_appearances: supported, preference, appearance } = selection;
  return <section className="flex min-w-0 flex-col gap-2" data-testid="theme-appearance-control" data-effective-appearance={selection.error ? "invalid" : appearance ?? "unclassified"}>
    <label htmlFor={id} className="block text-xs font-semibold">{t("appearanceMode.title")}</label>
    {supported.length > 1 ? <Select value={preference} onValueChange={(value) => onChange(value as ThemeAppearancePreference)}>
      <SelectTrigger id={id} size="sm" aria-label={t("appearanceMode.title")} data-value={preference} className="w-full min-w-0"><SelectValue /></SelectTrigger>
      <SelectContent>
        <SelectGroup>
          <SelectItem value="default" data-value="default">{t("appearanceMode.documentDefault")}</SelectItem>
          <SelectItem value="follow" data-value="follow">{t("appearanceMode.follow")}</SelectItem>
          {supported.map((mode) => <SelectItem key={mode} value={mode} data-value={mode}>{t(`appearanceMode.${mode}`)}</SelectItem>)}
        </SelectGroup>
      </SelectContent>
    </Select> : <p id={id} className="text-xs text-muted-foreground">
      {supported.length === 1 ? t("appearanceMode.only", { appearance: t(`appearanceMode.${supported[0]}`) }) : t("appearanceMode.unclassified")}
    </p>}
    {selection.error && <p role="alert" className="text-xs text-destructive" data-testid="theme-appearance-error">{selection.error}</p>}
    {appearance && <p role="status" className="text-[11px] leading-relaxed text-muted-foreground" data-testid="theme-effective-appearance">
      {t(preference === "follow" ? "appearanceMode.following" : "appearanceMode.selected", { appearance: t(`appearanceMode.${appearance}`) })}
    </p>}
  </section>;
}
