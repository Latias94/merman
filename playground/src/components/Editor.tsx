import Editor from "@monaco-editor/react";
import { useCallback, useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { editor } from "monaco-editor";
import { useMerman } from "@/src/hooks/useMerman";
import {
  clearMermaidMarkers,
  getMermaidHoverDocs,
  MERMAID_LANGUAGE_ID,
  registerMermaidLanguage,
  setMermaidHoverDocs,
  updateMermaidMarkers,
} from "@/src/lib/mermaid-language";
import { useAppStore } from "@/src/store";

interface CodeEditorProps {
  className?: string;
}

export function CodeEditor({ className }: CodeEditorProps) {
  const { t } = useTranslation();
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof import("monaco-editor") | null>(null);
  const { code, setCode, uiTheme } = useAppStore();
  const { ready, validate } = useMerman();
  const hoverDocs = useMemo(
    () => getMermaidHoverDocs((key) => t(key)),
    [t]
  );

  const handleEditorDidMount = useCallback(
    (editor: editor.IStandaloneCodeEditor, monaco: typeof import("monaco-editor")) => {
      editorRef.current = editor;
      monacoRef.current = monaco;
      registerMermaidLanguage(monaco);

      editor.updateOptions({
        minimap: { enabled: false },
        lineNumbers: "on",
        fontSize: 14,
        fontFamily: '"JetBrains Mono", "Fira Code", monospace',
        fontLigatures: true,
        wordWrap: "on",
        scrollBeyondLastLine: false,
        padding: { top: 16, bottom: 16 },
        renderLineHighlight: "line",
        cursorBlinking: "smooth",
        smoothScrolling: true,
        tabSize: 2,
      });

      editor.focus();
    },
    []
  );

  useEffect(() => {
    setMermaidHoverDocs(hoverDocs);
  }, [hoverDocs]);

  useEffect(() => {
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    const model = editor?.getModel();
    if (!editor || !monaco || !model) return;

    clearMermaidMarkers(monaco, model);
    if (!ready || !code.trim()) return;

    const timeout = window.setTimeout(() => {
      updateMermaidMarkers(monaco, model, validate(code));
    }, 300);

    return () => window.clearTimeout(timeout);
  }, [code, ready, validate]);

  useEffect(() => {
    return () => {
      const editor = editorRef.current;
      const monaco = monacoRef.current;
      const model = editor?.getModel();
      if (monaco && model) {
        clearMermaidMarkers(monaco, model);
      }
    };
  }, []);

  const handleEditorChange = useCallback(
    (value: string | undefined) => {
      setCode(value || "");
    },
    [setCode]
  );

  const editorTheme =
    uiTheme === "dark" ||
    (uiTheme === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches)
      ? "vs-dark"
      : "light";

  return (
    <div className={className}>
      <Editor
        height="100%"
        language={MERMAID_LANGUAGE_ID}
        theme={editorTheme}
        value={code}
        onChange={handleEditorChange}
        onMount={handleEditorDidMount}
        loading={
          <div className="flex h-full items-center justify-center text-muted-foreground">
            {t("editor.loading")}
          </div>
        }
        options={{
          automaticLayout: true,
        }}
      />
    </div>
  );
}
