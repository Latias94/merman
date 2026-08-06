import "monaco-editor/esm/vs/language/json/monaco.contribution.js";
import JsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";

import { registerLocalMonacoJsonWorker } from "./monaco";

let registration: ReturnType<typeof registerLocalMonacoJsonWorker> | null = null;

export function activateLocalMonacoJson(): void {
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
