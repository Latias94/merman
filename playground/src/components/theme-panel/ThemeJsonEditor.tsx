import Editor, { type OnMount } from "@monaco-editor/react";
import { getWorker, jsonDefaults, type ASTNode } from "monaco-editor/languages/features/json/register";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { localMonaco } from "@/src/editor/monaco";
import { activateLocalMonacoJson } from "@/src/editor/monaco-json";
import { WORKBENCH_EDITOR_THEMES } from "@/src/editor/workbench-editor-theme";
import { useAppStore } from "@/src/store";
import schema from "../../../../schemas/theme-v2.schema.json";
import type { ThemeResolveError } from "@mermanjs/web";

activateLocalMonacoJson();

const THEME_MODEL_PATH = "file:///playground/diagram.theme.json";
const defaults = jsonDefaults.diagnosticsOptions;
jsonDefaults.setDiagnosticsOptions({
  ...defaults,
  schemaValidation: "error",
  schemas: [...(defaults.schemas ?? []), { uri: schema.$id, fileMatch: [THEME_MODEL_PATH], schema }],
});

export default function ThemeJsonEditor({ value, onChange, error }: {
  readonly value: string;
  onChange(value: string): void;
  readonly error: ThemeResolveError | null;
}) {
  const { t } = useTranslation();
  const resolvedTheme = useAppStore((state) => state.resolvedTheme);
  const [editor, setEditor] = useState<Parameters<OnMount>[0] | null>(null);
  useEffect(() => {
    const model = editor?.getModel();
    if (!model || !error) return;
    let current = true;
    const owner = "merman-theme-v2";
    const mark = (start: { lineNumber: number; column: number }, end: { lineNumber: number; column: number }) => {
      if (!current || model.isDisposed()) return;
      localMonaco.editor.setModelMarkers(model, owner, [{
        startLineNumber: start.lineNumber,
        startColumn: start.column,
        endLineNumber: end.lineNumber,
        endColumn: end.column,
        message: error.message,
        severity: localMonaco.MarkerSeverity.Error,
      }]);
    };
    if (error.line) {
      const lineNumber = Math.min(error.line, model.getLineCount());
      const column = jsonColumn(model.getLineContent(lineNumber), error.column ?? 1);
      mark({ lineNumber, column }, { lineNumber, column: Math.min(column + 1, model.getLineMaxColumn(lineNumber)) });
    } else if (error.path && error.path !== ".") {
      const path = error.path;
      void getWorker().then((worker) => worker(model.uri))
        .then((worker) => worker.parseJSONDocument(model.uri.toString())).then((document) => {
          const node = fieldAtPath(document?.root, path);
          if (current && !model.isDisposed() && node) {
            mark(model.getPositionAt(node.offset), model.getPositionAt(node.offset + node.length));
          }
        }).catch(() => { /* The panel shows the resolver message independently of Monaco. */ });
    }
    return () => {
      current = false;
      if (!model.isDisposed()) localMonaco.editor.setModelMarkers(model, owner, []);
    };
  }, [editor, error, value]);

  return <div data-testid="theme-json-editor" className="h-80 min-w-0 overflow-hidden rounded-md border">
    <Editor height="100%" language="json" path={THEME_MODEL_PATH} value={value} onChange={(next) => onChange(next ?? "")}
      onMount={setEditor} theme={WORKBENCH_EDITOR_THEMES[resolvedTheme].name}
      options={{ ariaLabel: t("themeWorkshop.jsonEditor"), automaticLayout: true, minimap: { enabled: false },
        fontSize: 12, wordWrap: "on", scrollBeyondLastLine: false, tabSize: 2,
        padding: { top: 8, bottom: 8 }, scrollbar: { alwaysConsumeMouseWheel: false } }} />
  </div>;
}

/** Serde reports UTF-8 byte columns, Monaco uses UTF-16 code units. */
function jsonColumn(line: string, byteColumn: number): number {
  let bytes = 1;
  let column = 1;
  const encoder = new TextEncoder();
  for (const character of line) {
    bytes += encoder.encode(character).length;
    if (bytes > byteColumn) break;
    column += character.length;
  }
  return column;
}

function fieldAtPath(node: ASTNode | undefined, path: string): ASTNode | undefined {
  if (!node || !path) return node;
  if (node.type === "array") {
    const index = /^\[(\d+)\]/.exec(path);
    if (index) return fieldAtPath(node.items[Number(index[1])], path.slice(index[0].length).replace(/^\./, "")) ?? node;
  }
  if (node.type === "object") {
    const property = node.properties.find(({ keyNode: { value } }) =>
      path === value || path.startsWith(`${value}.`) || path.startsWith(`${value}[`));
    if (property) {
      const remaining = path.slice(property.keyNode.value.length).replace(/^\./, "");
      return remaining ? fieldAtPath(property.valueNode, remaining) ?? property.keyNode : property.keyNode;
    }
  }
  return node;
}
