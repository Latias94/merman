import "monaco-editor/languages/features/json/register";
import JsonWorker from "monaco-editor/languages/features/json/json.worker?worker";

import {
  ensureLocalMonacoConfigured,
  registerLocalMonacoJsonWorker,
} from "./monaco";

let registration: ReturnType<typeof registerLocalMonacoJsonWorker> | null = null;

export function activateLocalMonacoJson(): void {
  ensureLocalMonacoConfigured();
  registration ??= registerLocalMonacoJsonWorker(
    () => new JsonWorker({ name: "monaco-json" }),
  );
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    registration?.dispose();
    registration = null;
  });
}
