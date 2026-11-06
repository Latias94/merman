import { useEffect, useRef } from "react";
import Editor, { type EditorProps } from "@monaco-editor/react";
import type { IDisposable } from "monaco-editor";

import { ensureLocalMonacoConfigured } from "@/src/editor/monaco";
import {
  attachReadOnlyMermaidSyntax,
  ensureReadOnlyMermaidLanguage,
  READONLY_MERMAID_LANGUAGE_ID,
} from "@/src/editor/readonly-mermaid";
import { startMermaidSyntaxWorker } from "@/src/editor/syntax-worker-browser";

ensureLocalMonacoConfigured();

export function LocalReadOnlyMermaidEditor({ beforeMount, onMount, options, ...props }: EditorProps) {
  const binding = useRef<IDisposable | null>(null);
  useEffect(() => () => {
    binding.current?.dispose();
    binding.current = null;
  }, []);

  return (
    <Editor
      {...props}
      language={READONLY_MERMAID_LANGUAGE_ID}
      options={{ ...options, "semanticHighlighting.enabled": true }}
      beforeMount={(monaco) => {
        ensureReadOnlyMermaidLanguage(monaco);
        beforeMount?.(monaco);
      }}
      onMount={(preview, monaco) => {
        binding.current?.dispose();
        binding.current = attachReadOnlyMermaidSyntax(monaco, preview, startMermaidSyntaxWorker);
        onMount?.(preview, monaco);
      }}
    />
  );
}
