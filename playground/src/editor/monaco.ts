import { loader } from "@monaco-editor/react";
import type { IDisposable } from "monaco-editor";
import * as monacoApi from "monaco-editor/esm/vs/editor/editor.api.js";
import "monaco-editor/esm/vs/editor/editor.all.js";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import { registerWorkbenchEditorThemes } from "./workbench-editor-theme";

export const localMonaco = monacoApi as typeof import("monaco-editor");

interface MonacoEnvironmentOwner {
  dispose(): void;
}

interface MonacoEnvironment {
  getWorker(moduleId: string, label: string): Worker;
}

type MonacoWorkerFactory = () => Worker;

let jsonWorkerFactory: MonacoWorkerFactory | null = null;
let configuredOwner: MonacoEnvironmentOwner | null = null;

export function registerLocalMonacoJsonWorker(
  factory: MonacoWorkerFactory,
): IDisposable {
  if (jsonWorkerFactory && jsonWorkerFactory !== factory) {
    throw new Error("The Monaco JSON worker is already registered.");
  }

  jsonWorkerFactory = factory;
  let active = true;
  return {
    dispose() {
      if (!active) return;
      active = false;
      if (jsonWorkerFactory === factory) jsonWorkerFactory = null;
    },
  };
}

function configureLocalMonaco(): MonacoEnvironmentOwner {
  const target = globalThis as typeof globalThis & {
    MonacoEnvironment?: MonacoEnvironment;
  };
  const previous = target.MonacoEnvironment;
  target.MonacoEnvironment = {
    getWorker(_moduleId, label) {
      if (label === "json") {
        if (!jsonWorkerFactory) {
          throw new Error(
            "The Monaco JSON worker was requested before Config activation.",
          );
        }
        return jsonWorkerFactory();
      }
      return new EditorWorker({ name: "monaco-editor" });
    },
  };
  registerWorkbenchEditorThemes(localMonaco);
  loader.config({ monaco: localMonaco });

  return {
    dispose() {
      if (previous) {
        target.MonacoEnvironment = previous;
      } else {
        delete target.MonacoEnvironment;
      }
    },
  };
}

export function ensureLocalMonacoConfigured(): void {
  configuredOwner ??= configureLocalMonaco();
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    configuredOwner?.dispose();
    configuredOwner = null;
  });
}
