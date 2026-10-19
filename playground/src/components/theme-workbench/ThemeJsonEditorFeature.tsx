import Editor from "@monaco-editor/react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { editor as MonacoEditor } from "monaco-editor";
import { getWorker } from "monaco-editor/languages/features/json/register";

import { activateLocalMonacoJson } from "@/src/editor/monaco-json";
import { WORKBENCH_EDITOR_THEMES } from "@/src/editor/workbench-editor-theme";
import { findThemeJsonValue, type ThemeJsonRevealRequest } from "@/src/lib/theme-json-location";
import { useAppStore } from "@/src/store";

activateLocalMonacoJson();

export function ThemeJsonEditor({
  sessionId,
  value,
  reveal,
  onChange,
}: {
  sessionId: number;
  value: string;
  reveal?: ThemeJsonRevealRequest;
  onChange(value: string): void;
}) {
  const { t } = useTranslation();
  const resolvedTheme = useAppStore((state) => state.resolvedTheme);
  const [editor, setEditor] = useState<MonacoEditor.IStandaloneCodeEditor | null>(null);
  const [unavailable, setUnavailable] = useState<ThemeJsonRevealRequest | null>(null);

  useEffect(() => {
    if (!editor || !reveal || reveal.sessionId !== sessionId || reveal.rawJson !== value) return;
    const instance = editor;
    const request = reveal;
    const model = instance.getModel();
    if (!model || model.getValue() !== value) return;
    const documentModel = model;
    const version = model.getVersionId();
    let current = true;
    const isCurrent = () => current && !documentModel.isDisposed() && instance.getModel() === documentModel && documentModel.getVersionId() === version;
    async function showLocation() {
      try {
        const factory = await getWorker();
        const worker = await factory(documentModel.uri);
        const document = await worker.parseJSONDocument(documentModel.uri.toString());
        if (!isCurrent()) return;
        const node = findThemeJsonValue(document?.root, request.pointer);
        if (!node) {
          setUnavailable(request);
          return;
        }
        const start = documentModel.getPositionAt(node.offset);
        const end = documentModel.getPositionAt(node.offset + node.length);
        const range = { startLineNumber: start.lineNumber, startColumn: start.column, endLineNumber: end.lineNumber, endColumn: end.column };
        instance.setSelection(range);
        instance.revealRangeInCenter(range);
        instance.focus();
      } catch {
        if (isCurrent()) setUnavailable(request);
      }
    }
    void showLocation();
    return () => { current = false; };
  }, [editor, reveal, sessionId, value]);

  return (
    <div className="flex h-full min-h-0 flex-col" data-theme-json-pointer={reveal?.pointer}>
      {reveal && <p role="status" className="shrink-0 break-all border-b px-3 py-2 text-[11px] text-muted-foreground">
        {unavailable === reveal
          ? t("themeDesigner.locationUnavailable", { defaultValue: "This location is no longer available. The document has not been changed." })
          : reveal.pointer}
      </p>}
      <div className="min-h-0 flex-1">
    <Editor
      height="100%"
      language="json"
      path={`file:///merman/theme-recipe-${sessionId}.json`}
      theme={WORKBENCH_EDITOR_THEMES[resolvedTheme].name}
      value={value}
      onMount={(mounted) => setEditor(mounted)}
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
      </div>
    </div>
  );
}
