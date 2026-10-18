import Editor from "@monaco-editor/react";
import { useTranslation } from "react-i18next";

import { activateLocalMonacoJson } from "@/src/editor/monaco-json";
import { WORKBENCH_EDITOR_THEMES } from "@/src/editor/workbench-editor-theme";
import { useAppStore } from "@/src/store";

activateLocalMonacoJson();

export function ThemeJsonEditor({
  sessionId,
  value,
  onChange,
}: {
  sessionId: number;
  value: string;
  onChange(value: string): void;
}) {
  const { t } = useTranslation();
  const resolvedTheme = useAppStore((state) => state.resolvedTheme);

  return (
    <Editor
      height="100%"
      language="json"
      path={`file:///merman/theme-recipe-${sessionId}.json`}
      theme={WORKBENCH_EDITOR_THEMES[resolvedTheme].name}
      value={value}
      onChange={(next) => onChange(next ?? "")}
      loading={<p role="status" className="p-3 text-xs text-muted-foreground">{t("editor.loading")}</p>}
      options={{
        ariaLabel: t("customTheme.recipeLabel"),
        automaticLayout: true,
        minimap: { enabled: false },
        lineNumbers: "on",
        fontSize: 12,
        wordWrap: "on",
        scrollBeyondLastLine: false,
        padding: { top: 12, bottom: 12 },
        tabSize: 2,
        folding: true,
        formatOnPaste: false,
      }}
    />
  );
}
