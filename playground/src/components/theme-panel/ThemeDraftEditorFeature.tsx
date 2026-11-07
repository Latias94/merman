import Editor, { type OnMount } from "@monaco-editor/react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { activateLocalMonacoJson } from "@/src/editor/monaco-json";
import { WORKBENCH_EDITOR_THEMES } from "@/src/editor/workbench-editor-theme";
import { useAppStore } from "@/src/store";

activateLocalMonacoJson();

export default function ThemeDraftEditorFeature({ value, onChange, scope }: {
  readonly value: string;
  onChange(value: string): void;
  readonly scope: "shared" | "light" | "dark";
}) {
  const { t } = useTranslation();
  const resolvedTheme = useAppStore((state) => state.resolvedTheme);
  const editor = useRef<Parameters<OnMount>[0] | null>(null);
  useEffect(() => {
    const model = editor.current?.getModel();
    const field = scope === "shared" ? "design" : scope;
    const matches = model?.findMatches(`"${field}"\\s*:`, false, true, true, null, false);
    if (matches?.[0]) {
      editor.current?.revealLineNearTop(matches[0].range.startLineNumber);
      editor.current?.setPosition({ lineNumber: matches[0].range.startLineNumber, column: 1 });
    }
  }, [scope]);
  return <div data-testid="theme-json-draft" className="h-80 min-w-0 overflow-hidden rounded-md border">
    <Editor height="100%" language="json" value={value} onChange={(next) => onChange(next ?? "")}
      onMount={(instance) => { editor.current = instance; }}
      theme={WORKBENCH_EDITOR_THEMES[resolvedTheme].name}
      options={{ ariaLabel: t("themeDraft.editor"), automaticLayout: true, minimap: { enabled: false },
        fontSize: 12, wordWrap: "on", scrollBeyondLastLine: false, tabSize: 2,
        padding: { top: 8, bottom: 8 }, scrollbar: { alwaysConsumeMouseWheel: false } }} />
  </div>;
}
