import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import type { ThemeDesignerModel } from "@/src/lib/theme-designer-model";

export function ThemeDesignOverview({ model, onOpenGroup, onOpenCanvas, onOpenTypography, onOpenObjects }: {
  model: ThemeDesignerModel;
  onOpenGroup(group: string | null): void;
  onOpenCanvas(): void;
  onOpenTypography(): void;
  onOpenObjects(): void;
}) {
  const { t } = useTranslation();
  return <div className="min-w-0 space-y-4">
    <div>
      <h3 className="text-sm font-semibold">{t("themeDesigner.overview")}</h3>
      <p className="mt-1 text-xs leading-relaxed text-muted-foreground">{t("themeDesigner.introduction")}</p>
    </div>
    <section className="space-y-2" aria-label={t("themeDesigner.settingGroups")}>
      <h4 className="text-xs font-medium">{t("themeDesigner.sharedValues")}</h4>
      <div className="grid min-w-0 gap-2 sm:grid-cols-2">
        {model.groups.map((group) => <Button key={group.id === null ? "absent" : `group:${group.id}`} type="button" variant="outline" className="h-auto min-w-0 items-start whitespace-normal p-3 text-left" onClick={() => onOpenGroup(group.id)}>
          <span className="min-w-0 flex-1 space-y-1">
            <span className="block break-words text-xs font-semibold">{group.label || t(group.id === null ? "themeDesigner.ungrouped" : "themeDesigner.unnamedGroup")}</span>
            <span className="block text-[11px] font-normal text-muted-foreground">{t("themeDesigner.settingCount", { count: group.settings.length })}</span>
          </span>
        </Button>)}
      </div>
      {!model.groups.length && <p className="text-xs text-muted-foreground">{t("themeDesigner.noSharedValues")}</p>}
    </section>
    <div className="grid min-w-0 gap-2 sm:grid-cols-2">
      <Button type="button" variant="outline" className="h-auto min-w-0 whitespace-normal py-3" onClick={onOpenCanvas}>{t("themeDesigner.canvasBackground")}</Button>
      <Button type="button" variant="outline" className="h-auto min-w-0 whitespace-normal py-3" onClick={onOpenTypography}>{t("themeWorkbench.typography")}</Button>
    </div>
    <p className="text-[11px] leading-relaxed text-muted-foreground">{t("themeDesigner.canvasOwnership")}</p>
    <Button type="button" variant="secondary" size="sm" onClick={onOpenObjects}>{t("themeDesigner.semanticStyles")}</Button>
  </div>;
}
