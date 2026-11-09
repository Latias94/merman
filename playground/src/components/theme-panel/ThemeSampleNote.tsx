import { useTranslation } from "react-i18next";
import type { ThemeAppearance } from "@mermanjs/web";
import type { ExplorationTheme } from "@/src/themes/catalog";

export function ThemeSampleNote({ theme, familyId, appearance }: {
  readonly theme: ExplorationTheme;
  readonly familyId: string | null;
  readonly appearance: ThemeAppearance;
}) {
  const { t } = useTranslation();
  const note = theme.sampleNotes?.find((entry) => entry.familyId === familyId && entry.appearance === appearance);
  return note ? <p role="note" className="rounded-md border bg-muted/30 px-3 py-2 text-xs leading-relaxed text-muted-foreground" data-theme-sample-note={familyId}>
    {t(note.messageKey)}
  </p> : null;
}
