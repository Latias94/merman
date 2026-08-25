import Editor from "@monaco-editor/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { editor, IDisposable } from "monaco-editor";
import { LoaderCircle, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { localMonaco } from "@/src/editor/monaco";
import { WORKBENCH_EDITOR_THEMES } from "@/src/editor/workbench-editor-theme";
import {
  ensureMermaidLanguageRegistered,
  MERMAID_DOCUMENT_URI,
  MERMAID_LANGUAGE_ID,
  type MermaidLanguageRequestRejection,
  type MermaidLanguageRegistration,
} from "@/src/lib/mermaid-language";
import { registerBrowserMermaidLanguage } from "@/src/lib/mermaid-language-browser";
import { useAppStore } from "@/src/store";

interface CodeEditorProps {
  readonly className?: string;
  readonly waitForLanguageActivation?: () => Promise<unknown>;
}

const waitForImmediateLanguageActivation = () => Promise.resolve();

type LanguageState =
  | { readonly status: "initializing" }
  | { readonly status: "ready" }
  | { readonly status: "degraded"; readonly error: Error }
  | { readonly status: "reconnecting" }
  | { readonly status: "unavailable"; readonly error: Error };

export function CodeEditor({
  className,
  waitForLanguageActivation = waitForImmediateLanguageActivation,
}: CodeEditorProps) {
  const { t } = useTranslation();
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const layoutBindingRef = useRef<IDisposable | null>(null);
  const modelBindingRef = useRef<IDisposable | null>(null);
  const registrationRef = useRef<MermaidLanguageRegistration | null>(null);
  const languageGenerationRef = useRef(0);
  const languageFailureRef = useRef<Error | null>(null);
  const code = useAppStore((state) => state.code);
  const setCode = useAppStore((state) => state.setCode);
  const resolvedTheme = useAppStore((state) => state.resolvedTheme);
  const editorTheme = WORKBENCH_EDITOR_THEMES[resolvedTheme].name;
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
        setLanguage(
          languageFailureRef.current
            ? { status: "degraded", error: languageFailureRef.current }
            : { status: "ready" },
        );
        editorRef.current?.layout();
      } catch (error) {
        markLanguageUnavailable(error, generation);
      }
    },
    [markLanguageUnavailable],
  );

  const markLanguageDegraded = useCallback(
    (error: unknown, generation: number) => {
      if (languageGenerationRef.current !== generation) return;
      const failure = error instanceof Error ? error : new Error(String(error));
      languageFailureRef.current = failure;
      setLanguage({ status: "degraded", error: failure });
    },
    [],
  );

  useEffect(() => {
    const generation = languageGenerationRef.current + 1;
    languageGenerationRef.current = generation;
    let active = true;
    let registration: MermaidLanguageRegistration | null = null;
    languageFailureRef.current = null;
    void (async () => {
      await waitForLanguageActivation();
      if (!active || languageGenerationRef.current !== generation) return;

      const nextRegistration = await registerBrowserMermaidLanguage(
        localMonaco,
        {
          onRequestRejected: (rejection) => {
            if (languageGenerationRef.current !== generation) return;
            setRequestRejection(rejection);
          },
          onSemanticUnavailable: (error) =>
            markLanguageDegraded(error, generation),
          onSyntaxUnavailable: (error) =>
            markLanguageDegraded(error, generation),
        },
      );

      if (!active || languageGenerationRef.current !== generation) {
        nextRegistration.dispose();
        return;
      }

      registration = nextRegistration;
      registrationRef.current = registration;
      const model = editorRef.current?.getModel();
      if (model) void bindLanguageService(registration, model, generation);
    })().catch((error: unknown) => {
      if (!active || languageGenerationRef.current !== generation) return;
      markLanguageUnavailable(error, generation);
    });

    return () => {
      active = false;
      if (languageGenerationRef.current === generation) {
        languageGenerationRef.current += 1;
      }
      if (registrationRef.current === registration) {
        disposeLanguageService();
      } else {
        registration?.dispose();
      }
    };
  }, [
    bindLanguageService,
    disposeLanguageService,
    languageAttempt,
    markLanguageDegraded,
    markLanguageUnavailable,
    waitForLanguageActivation,
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
    (instance: editor.IStandaloneCodeEditor) => {
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
        "semanticHighlighting.enabled": true,
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
    [bindLanguageService, markLanguageUnavailable, t],
  );

  const handleEditorChange = useCallback(
    (value: string | undefined) => {
      setRequestRejection(null);
      setCode(value ?? "");
    },
    [setCode],
  );

  const retryLanguageService = useCallback(() => {
    if (language.status !== "unavailable" && language.status !== "degraded") return;
    languageGenerationRef.current += 1;
    disposeLanguageService();
    setRequestRejection(null);
    languageFailureRef.current = null;
    setLanguage({ status: "reconnecting" });
    setLanguageAttempt((attempt) => attempt + 1);
  }, [disposeLanguageService, language.status]);

  return (
    <div className={`${className ?? ""} relative`}>
      <Editor
        height="100%"
        language={MERMAID_LANGUAGE_ID}
        path={MERMAID_DOCUMENT_URI}
        theme={editorTheme}
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
          "semanticHighlighting.enabled": true,
        }}
      />
      {(language.status === "ready" || language.status === "degraded") &&
        requestRejection && (
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
          role={
            language.status === "unavailable" || language.status === "degraded"
              ? "alert"
              : "status"
          }
          aria-live="polite"
          aria-atomic="true"
        >
          {language.status === "unavailable" || language.status === "degraded" ? (
            <>
              <span
                className="max-w-48 truncate text-destructive sm:max-w-80"
                title={language.error.message}
              >
                {t(
                  language.status === "degraded"
                    ? "editor.languageDegraded"
                    : "editor.languageUnavailable",
                  { message: language.error.message },
                )}
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
