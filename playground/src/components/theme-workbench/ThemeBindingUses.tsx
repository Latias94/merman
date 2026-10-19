import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import type { ThemeBindingUse, ThemeDesignerModel, ThemeDesignerSetting } from "@/src/lib/theme-designer-model";

export function ThemeBindingUses({ setting, model, onOpen }: {
  setting: ThemeDesignerSetting;
  model: ThemeDesignerModel;
  onOpen(use: ThemeBindingUse): void;
}) {
  const { t } = useTranslation();
  function describe(use: ThemeBindingUse) {
    const destination = use.destination;
    const parts = destination.segments ?? [];
    const target = (id: string) => t(`themeDesigner.target.${id}`, { defaultValue: id });
    let owner: string;
    let tail: readonly string[];
    switch (destination.kind) {
      case "token":
        owner = model.settings.find((item) => item.id === destination.tokenId)?.label ?? destination.tokenId;
        tail = parts.slice(3);
        break;
      case "style": owner = target(destination.target); tail = parts.slice(2); break;
      case "rule": owner = `${t("themeDesigner.ruleNumber", { index: destination.index + 1 })} · ${target(destination.target)}`; tail = parts.slice(3); break;
      case "canvas": owner = t("themeDesigner.canvasBackground"); tail = parts.slice(1); break;
      case "typography": owner = t("themeDesigner.defaultTypography"); tail = parts.slice(2); break;
      default: owner = t("themeDesigner.jsonLocation"); tail = parts;
    }
    return [owner, ...tail.map((part) => /^\d+$/.test(part) ? String(Number(part) + 1) : part === "color" && !parts.includes("shadows") ? t("themeWorkbench.color") : t(`themeDesigner.field.${part}`, { defaultValue: part }))].join(" / ");
  }
  return <section className="min-w-0 space-y-3 border-t pt-3" aria-label={t("themeDesigner.declaredBindings")}>
    <div>
      <h4 className="text-xs font-semibold">{t("themeDesigner.declaredBindings")}</h4>
      <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground">{t("themeDesigner.declaredBindingsHint")}</p>
    </div>
    {!setting.uses ? <p role="status" className="text-xs text-muted-foreground">{t("themeDesigner.inspectionUnavailable")}</p> : <>
      {(["direct", "dependencies", "downstream"] as const).map((kind) => setting.uses![kind].length > 0 && <section key={kind} className="space-y-1.5" aria-label={t(`themeDesigner.bindingKind.${kind}`)}>
        <h5 className="text-[11px] font-medium">{t(`themeDesigner.bindingKind.${kind}`)} ({setting.uses![kind].length})</h5>
        {setting.uses![kind].map((use) => <div key={use.path} className="min-w-0 rounded-md border p-2">
          <Button type="button" variant="ghost" size="sm" className="h-auto w-full min-w-0 justify-start whitespace-normal px-1 text-left text-xs" aria-label={t("themeDesigner.editLocation", { path: use.path })} onClick={() => onOpen(use)}>{describe(use)}</Button>
          <code className="block break-all text-[10px] text-muted-foreground">{use.path}</code>
          {use.via.length > 0 && <p className="mt-1 break-words text-[10px] text-muted-foreground">{t("themeDesigner.viaSettings", { names: use.via.map((id) => model.settings.find((item) => item.id === id)?.label ?? id).join(" → ") })}</p>}
          {use.destination.kind === "json" && <p className="mt-1 text-[10px] text-muted-foreground">{t("themeDesigner.opensJson")}</p>}
        </div>)}
      </section>)}
      {!setting.uses.direct.length && !setting.uses.dependencies.length && !setting.uses.downstream.length && <p className="text-xs text-muted-foreground">{t("themeDesigner.noDeclaredBindings")}</p>}
    </>}
  </section>;
}
