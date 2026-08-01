import { loader } from "@monaco-editor/react";
import * as monacoApi from "monaco-editor/esm/vs/editor/editor.api.js";
import "monaco-editor/esm/vs/language/json/monaco.contribution.js";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import JsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";

export const localMonaco = monacoApi as typeof import("monaco-editor");

interface MonacoEnvironmentOwner {
  readonly monaco: typeof localMonaco;
  dispose(): void;
}

interface MonacoEnvironment {
  getWorker(moduleId: string, label: string): Worker;
}

export function configureLocalMonaco(): MonacoEnvironmentOwner {
  const target = globalThis as typeof globalThis & {
    MonacoEnvironment?: MonacoEnvironment;
  };
  const previous = target.MonacoEnvironment;
  target.MonacoEnvironment = {
    getWorker(_moduleId, label) {
      return label === "json"
        ? new JsonWorker({ name: "monaco-json" })
        : new EditorWorker({ name: "monaco-editor" });
    },
  };
  loader.config({ monaco: localMonaco });

  return {
    monaco: localMonaco,
    dispose() {
      if (previous) {
        target.MonacoEnvironment = previous;
      } else {
        delete target.MonacoEnvironment;
      }
    },
  };
}
