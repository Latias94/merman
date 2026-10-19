import { useId } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export function NumberField({ label, value, clearable = false, required = false, min, max, onChange }: {
  label: string;
  value: number | null | undefined;
  clearable?: boolean;
  required?: boolean;
  min?: number;
  max?: number;
  onChange(value: number | null | undefined): void;
}) {
  const { t } = useTranslation();
  const id = useId();

  return (
    <div className="space-y-1.5">
      <label htmlFor={id} className="block text-xs font-medium">{label}</label>
      <div className="flex min-w-0 gap-2">
        <Input key={String(value)} id={id} type="number" required={required} min={min} max={max} step="any" placeholder={value === null ? t("themeWorkbench.clear", { defaultValue: "Clear" }) : t("themeWorkbench.omit", { defaultValue: "Omit" })} defaultValue={value ?? ""} className="h-8 text-xs" onBlur={(event) => {
          if (!event.target.validity.valid) return;
          const next = event.target.value === "" ? undefined : Number(event.target.value);
          if (next !== value) onChange(next);
        }} onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} />
        {clearable && <Button type="button" size="sm" variant="outline" onClick={() => onChange(null)}>{t("themeWorkbench.clear", { defaultValue: "Clear" })}</Button>}
      </div>
    </div>
  );
}

export function ListField({ label, value, onChange }: {
  label: string;
  value: readonly string[] | undefined;
  onChange(value: string[] | undefined): void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const text = value?.join("\n") ?? "";

  return (
    <div className="min-w-0 space-y-1.5">
      <label htmlFor={id} className="block text-xs font-medium">{label}</label>
      <textarea key={text} id={id} defaultValue={text} spellCheck={false} rows={3} placeholder={t("themeWorkbench.omit", { defaultValue: "Omit" })} className="block w-full min-w-0 resize-y rounded-md border bg-background p-2 font-mono text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" onBlur={(event) => {
        if (event.target.value === text) return;
        const lines = event.target.value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
        onChange(lines.length ? lines : undefined);
      }} />
    </div>
  );
}
