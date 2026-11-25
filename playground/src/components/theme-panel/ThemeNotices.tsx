import { useTranslation } from "react-i18next";

import type { ThemeOutcome } from "@mermanjs/web";

export function ThemeNotices({ notices }: { notices: ThemeOutcome["notices"] }) {
  const { t } = useTranslation();
  const messages = Array.from(new Set(notices.map((notice) => typeof notice === "string" ? notice
    : notice.kind === "low_contrast" ? t("themeWorkshop.lowContrast", {
      foreground: notice.foreground, background: notice.background,
      ratio: notice.ratio.toFixed(2), minimum: notice.minimum,
    }) : t("themeWorkshop.adjusted", { foreground: notice.foreground, color: notice.color }))));
  if (!messages.length) return null;
  return <details className="text-xs text-muted-foreground">
    <summary className="cursor-pointer">{t("themeWorkshop.notices", { count: messages.length })}</summary>
    <ul className="mt-1 list-disc space-y-1 pl-4">
      {messages.map((message) => <li key={message} className="break-words">{message}</li>)}
    </ul>
  </details>;
}
