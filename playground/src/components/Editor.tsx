import Editor from "@monaco-editor/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { editor, IDisposable } from "monaco-editor";
import { localMonaco } from "@/src/editor/monaco";
import { startMermanLanguageWorker } from "@/src/editor/worker-browser";
import {
  MERMAID_DOCUMENT_URI,
  MERMAID_LANGUAGE_ID,
  registerMermaidLanguage,
  type MermaidLanguageRegistration,
} from "@/src/lib/mermaid-language";
import { useAppStore } from "@/src/store";

interface CodeEditorProps {
  className?: string;
}

type LanguageState =
  | { readonly status: "loading" }
  | {
      readonly status: "ready";
      readonly registration: MermaidLanguageRegistration;
    }
  | { readonly status: "error"; readonly error: Error };

export function CodeEditor({ className }: CodeEditorProps) {
  const { t } = useTranslation();
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const layoutBindingRef = useRef<IDisposable | null>(null);
  const modelBindingRef = useRef<IDisposable | null>(null);
  const code = useAppStore((state) => state.code);
  const setCode = useAppStore((state) => state.setCode);
  const resolvedTheme = useAppStore((state) => state.resolvedTheme);
  const [language, setLanguage] = useState<LanguageState>({
    status: "loading",
  });

  useEffect(() => {
    let active = true;
    let registration: MermaidLanguageRegistration | null = null;

    void startMermanLanguageWorker()
      .then(({ client, legend }) => {
        if (!active) {
          client.dispose();
          return;
        }
        registration = registerMermaidLanguage(localMonaco, client, legend);
        setLanguage({ status: "ready", registration });
      })
      .catch((error: unknown) => {
        if (!active) return;
        const failure = error instanceof Error ? error : new Error(String(error));
        console.error("Merman editor language worker failed to start", failure);
        setLanguage({ status: "error", error: failure });
      });

    return () => {
      active = false;
      modelBindingRef.current?.dispose();
      modelBindingRef.current = null;
      layoutBindingRef.current?.dispose();
      layoutBindingRef.current = null;
      registration?.dispose();
    };
  }, []);

  const handleEditorDidMount = useCallback(
    (
      instance: editor.IStandaloneCodeEditor,
      monaco: typeof import("monaco-editor")
    ) => {
      if (language.status !== "ready") return;
      editorRef.current = instance;
      layoutBindingRef.current?.dispose();
      layoutBindingRef.current = observeEditorLayout(instance);
      instance.updateOptions({
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

      const model = instance.getModel();
      if (!model || model.uri.toString() !== MERMAID_DOCUMENT_URI) {
        const failure = new Error("Mermaid editor did not create its managed document model.");
        layoutBindingRef.current?.dispose();
        layoutBindingRef.current = null;
        editorRef.current = null;
        console.error(failure);
        setLanguage({ status: "error", error: failure });
        return;
      }

      void language.registration
        .bindModel(model)
        .then((binding) => {
          modelBindingRef.current?.dispose();
          modelBindingRef.current = binding;
          monaco.editor.setTheme(resolvedTheme === "dark" ? "vs-dark" : "light");
          instance.layout();
          instance.focus();
        })
        .catch((error: unknown) => {
          const failure =
            error instanceof Error ? error : new Error(String(error));
          layoutBindingRef.current?.dispose();
          layoutBindingRef.current = null;
          editorRef.current = null;
          language.registration.dispose();
          console.error("Merman editor model failed to open", failure);
          setLanguage({ status: "error", error: failure });
        });
    },
    [language, resolvedTheme]
  );

  useEffect(() => {
    if (language.status === "ready") {
      localMonaco.editor.setTheme(
        resolvedTheme === "dark" ? "vs-dark" : "light"
      );
      editorRef.current?.layout();
    }
  }, [language.status, resolvedTheme]);

  const handleEditorChange = useCallback(
    (value: string | undefined) => setCode(value ?? ""),
    [setCode]
  );

  if (language.status === "error") {
    return (
      <div
        className={`${className ?? ""} flex items-center justify-center p-4 text-sm text-destructive`}
        role="alert"
      >
        {t("editor.languageUnavailable", {
          defaultValue: "Editor analysis is unavailable: {{message}}",
          message: language.error.message,
        })}
      </div>
    );
  }

  if (language.status === "loading") {
    return (
      <div
        className={`${className ?? ""} flex items-center justify-center text-muted-foreground`}
        role="status"
      >
        {t("editor.loading")}
      </div>
    );
  }

  return (
    <div className={className}>
      <Editor
        height="100%"
        language={MERMAID_LANGUAGE_ID}
        path={MERMAID_DOCUMENT_URI}
        theme={resolvedTheme === "dark" ? "vs-dark" : "light"}
        value={code}
        onChange={handleEditorChange}
        onMount={handleEditorDidMount}
        loading={
          <div className="flex h-full items-center justify-center text-muted-foreground">
            {t("editor.loading")}
          </div>
        }
        options={{ automaticLayout: false }}
      />
    </div>
  );
}

function observeEditorLayout(
  instance: editor.IStandaloneCodeEditor
): IDisposable {
  let frame = 0;
  const layout = () => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => instance.layout());
  };
  const node = instance.getDomNode();
  const observer = node ? new ResizeObserver(layout) : null;
  if (node) observer?.observe(node);
  window.addEventListener("resize", layout);
  window.visualViewport?.addEventListener("resize", layout);
  layout();

  return {
    dispose() {
      cancelAnimationFrame(frame);
      observer?.disconnect();
      window.removeEventListener("resize", layout);
      window.visualViewport?.removeEventListener("resize", layout);
    },
  };
}
