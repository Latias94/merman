import Editor from "@monaco-editor/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { editor, IDisposable } from "monaco-editor";
import { LoaderCircle, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { localMonaco } from "@/src/editor/monaco";
import { startMermanLanguageWorker } from "@/src/editor/worker-browser";
import {
  ensureMermaidLanguageRegistered,
  MERMAID_DOCUMENT_URI,
  MERMAID_LANGUAGE_ID,
  registerMermaidLanguage,
  type MermaidLanguageRequestRejection,
  type MermaidLanguageRegistration,
} from "@/src/lib/mermaid-language";
import { useAppStore } from "@/src/store";

interface CodeEditorProps {
  className?: string;
}

type LanguageState =
  | { readonly status: "initializing" }
  | { readonly status: "ready" }
  | { readonly status: "reconnecting" }
  | { readonly status: "unavailable"; readonly error: Error };

export function CodeEditor({ className }: CodeEditorProps) {
  const { t } = useTranslation();
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const layoutBindingRef = useRef<IDisposable | null>(null);
  const modelBindingRef = useRef<IDisposable | null>(null);
  const registrationRef = useRef<MermaidLanguageRegistration | null>(null);
  const languageGenerationRef = useRef(0);
  const code = useAppStore((state) => state.code);
  const setCode = useAppStore((state) => state.setCode);
  const resolvedTheme = useAppStore((state) => state.resolvedTheme);
  const [language, setLanguage] = useState<LanguageState>({
    status: "initializing",
  });
  const [languageAttempt, setLanguageAttempt] = useState(0);
  const [requestRejection, setRequestRejection] =
    useState<MermaidLanguageRequestRejection | null>(null);

  const disposeLanguageService = useCallback(() => {
    modelBindingRef.current?.dispose();
    modelBindingRef.current = null;
    registrationRef.current?.dispose();
    registrationRef.current = null;
  }, []);

  const markLanguageUnavailable = useCallback(
    (error: unknown, generation: number) => {
      if (languageGenerationRef.current !== generation) return;
      languageGenerationRef.current += 1;
      const failure = error instanceof Error ? error : new Error(String(error));
      disposeLanguageService();
      console.error("Merman editor language service is unavailable", failure);
      setLanguage({ status: "unavailable", error: failure });
    },
    [disposeLanguageService],
  );

  const bindLanguageService = useCallback(
    async (
      registration: MermaidLanguageRegistration,
      model: editor.ITextModel,
      generation: number,
    ) => {
      try {
        const binding = await registration.bindModel(model);
        if (
          languageGenerationRef.current !== generation ||
          registrationRef.current !== registration ||
          model.isDisposed()
        ) {
          binding.dispose();
          return;
        }
        modelBindingRef.current?.dispose();
        modelBindingRef.current = binding;
        setLanguage({ status: "ready" });
        editorRef.current?.layout();
      } catch (error) {
        markLanguageUnavailable(error, generation);
      }
    },
    [markLanguageUnavailable],
  );

  useEffect(() => {
    const generation = languageGenerationRef.current + 1;
    languageGenerationRef.current = generation;
    let active = true;
    let registration: MermaidLanguageRegistration | null = null;
    let client: ReturnType<typeof startMermanLanguageWorker>["client"] | null =
      null;
    let failureSubscription: { dispose(): void } | null = null;

    try {
      const startup = startMermanLanguageWorker();
      client = startup.client;
      failureSubscription = startup.client.onDidFail((error) => {
        if (!active || languageGenerationRef.current !== generation) return;
        markLanguageUnavailable(error, generation);
      });
      void startup.ready
        .then((identity) => {
          if (!active || languageGenerationRef.current !== generation) {
            startup.client.dispose();
            return;
          }
          registration = registerMermaidLanguage(
            localMonaco,
            startup.client,
            identity,
            {
              onRequestRejected: (rejection) => {
                if (languageGenerationRef.current !== generation) return;
                setRequestRejection(rejection);
              },
              onUnavailable: (error) =>
                markLanguageUnavailable(error, generation),
            },
          );
          registrationRef.current = registration;
          const model = editorRef.current?.getModel();
          if (model) {
            void bindLanguageService(registration, model, generation);
          }
        })
        .catch((error: unknown) => {
          startup.client.dispose();
          if (!active || languageGenerationRef.current !== generation) return;
          markLanguageUnavailable(error, generation);
        });
    } catch (error) {
      markLanguageUnavailable(error, generation);
    }

    return () => {
      active = false;
      failureSubscription?.dispose();
      if (languageGenerationRef.current === generation) {
        languageGenerationRef.current += 1;
      }
      if (registrationRef.current === registration) {
        disposeLanguageService();
      } else {
        registration?.dispose();
      }
      client?.dispose();
    };
  }, [
    bindLanguageService,
    disposeLanguageService,
    languageAttempt,
    markLanguageUnavailable,
  ]);

  useEffect(
    () => () => {
      layoutBindingRef.current?.dispose();
      layoutBindingRef.current = null;
      editorRef.current = null;
    },
    [],
  );

  const handleEditorWillMount = useCallback(
    (monaco: typeof import("monaco-editor")) => {
      ensureMermaidLanguageRegistered(monaco);
    },
    [],
  );

  const handleEditorDidMount = useCallback(
    (
      instance: editor.IStandaloneCodeEditor,
      monaco: typeof import("monaco-editor"),
    ) => {
      editorRef.current = instance;
      layoutBindingRef.current?.dispose();
      layoutBindingRef.current = observeEditorLayout(instance);
      instance.updateOptions({
        ariaLabel: t("editor.ariaLabel"),
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
        const failure = new Error(
          "Mermaid editor did not create its managed document model.",
        );
        layoutBindingRef.current?.dispose();
        layoutBindingRef.current = null;
        editorRef.current = null;
        console.error(failure);
        markLanguageUnavailable(failure, languageGenerationRef.current);
        return;
      }

      monaco.editor.setTheme(resolvedTheme === "dark" ? "vs-dark" : "light");
      instance.layout();
      const registration = registrationRef.current;
      if (registration) {
        void bindLanguageService(
          registration,
          model,
          languageGenerationRef.current,
        );
      }
    },
    [bindLanguageService, markLanguageUnavailable, resolvedTheme, t],
  );

  useEffect(() => {
    localMonaco.editor.setTheme(resolvedTheme === "dark" ? "vs-dark" : "light");
    editorRef.current?.layout();
  }, [resolvedTheme]);

  const handleEditorChange = useCallback(
    (value: string | undefined) => {
      setRequestRejection(null);
      setCode(value ?? "");
    },
    [setCode],
  );

  const retryLanguageService = useCallback(() => {
    if (language.status !== "unavailable") return;
    languageGenerationRef.current += 1;
    disposeLanguageService();
    setRequestRejection(null);
    setLanguage({ status: "reconnecting" });
    setLanguageAttempt((attempt) => attempt + 1);
  }, [disposeLanguageService, language.status]);

  return (
    <div className={`${className ?? ""} relative`}>
      <Editor
        height="100%"
        language={MERMAID_LANGUAGE_ID}
        path={MERMAID_DOCUMENT_URI}
        theme={resolvedTheme === "dark" ? "vs-dark" : "light"}
        value={code}
        onChange={handleEditorChange}
        beforeMount={handleEditorWillMount}
        onMount={handleEditorDidMount}
        loading={
          <div className="flex h-full items-center justify-center text-muted-foreground">
            {t("editor.loading")}
          </div>
        }
        options={{
          ariaLabel: t("editor.ariaLabel"),
          automaticLayout: false,
        }}
      />
      {language.status === "ready" && requestRejection && (
        <div
          className="absolute bottom-2 left-2 right-2 z-10 max-h-24 overflow-auto break-words border border-destructive/50 bg-background/95 px-3 py-2 text-xs text-destructive shadow-sm"
          role="alert"
          aria-atomic="true"
          data-merman-editor-request-error="rename"
        >
          {requestRejection.message}
          {requestRejection.nativeCode
            ? ` (${requestRejection.nativeCode})`
            : ""}
        </div>
      )}
      {language.status !== "ready" && (
        <div
          className="absolute right-2 top-2 z-10 flex max-w-[calc(100%-1rem)] items-center gap-2 border bg-background/95 px-2 py-1 text-xs shadow-sm"
          role={language.status === "unavailable" ? "alert" : "status"}
          aria-live="polite"
          aria-atomic="true"
        >
          {language.status === "unavailable" ? (
            <>
              <span
                className="max-w-48 truncate text-destructive sm:max-w-80"
                title={language.error.message}
              >
                {t("editor.languageUnavailable", {
                  message: language.error.message,
                })}
              </span>
              <Button
                type="button"
                size="sm"
                variant="outline"
                className="h-7 shrink-0 gap-1 px-2 text-xs"
                onClick={retryLanguageService}
              >
                <RotateCcw className="size-3.5" aria-hidden="true" />
                {t("editor.retryLanguage")}
              </Button>
            </>
          ) : (
            <>
              <LoaderCircle
                className="size-3.5 animate-spin text-muted-foreground"
                aria-hidden="true"
              />
              <span className="text-muted-foreground">
                {t(
                  language.status === "initializing"
                    ? "editor.languageInitializing"
                    : "editor.languageReconnecting",
                )}
              </span>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function observeEditorLayout(
  instance: editor.IStandaloneCodeEditor,
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
