import { FileCode, Folder } from "lucide-react";
import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";

export function ProjectFileBrowser({ files, selected, onSelect }: {
  readonly files: Readonly<Record<string, string>>;
  readonly selected: string;
  onSelect(filename: string): void;
}) {
  const { t } = useTranslation();
  const names = Object.keys(files).sort();
  const folders = [...new Set(names.filter((name) => name.includes("/")).map((name) => name.slice(0, name.indexOf("/"))))];
  const file = (filename: string, label = filename) => <li key={filename}>
    <button type="button" aria-label={filename} aria-current={selected === filename ? "true" : undefined}
      onClick={() => onSelect(filename)} title={filename}
      className={cn("flex w-full min-w-0 items-center gap-2 rounded px-2 py-1.5 text-left font-mono text-[11px] hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        selected === filename && "bg-primary/10 font-medium text-primary")}>
      <FileCode className="size-3.5 shrink-0" aria-hidden="true" /><span className="truncate">{label}</span>
    </button>
  </li>;
  return <nav aria-label={t("projectUse.projectFiles")} className="max-h-40 min-w-0 overflow-auto border-b bg-muted/15 p-2 sm:max-h-80 sm:border-b-0 sm:border-r" data-testid="project-file-browser">
    <p className="mb-1 px-2 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">{t("projectUse.projectFiles")}</p>
    <ul className="space-y-0.5">
      {folders.map((folder) => <li key={folder}>
        <span className="flex items-center gap-2 px-2 py-1.5 font-mono text-[11px] text-muted-foreground"><Folder className="size-3.5 shrink-0" aria-hidden="true" />{folder}/</span>
        <ul className="ml-3 border-l pl-1">{names.filter((name) => name.startsWith(`${folder}/`)).map((name) => file(name, name.slice(folder.length + 1)))}</ul>
      </li>)}
      {names.filter((name) => !name.includes("/")).map((name) => file(name))}
    </ul>
  </nav>;
}
