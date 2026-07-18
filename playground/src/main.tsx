import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles/globals.css";
import "./i18n";
import {
  disposeMermanRuntime,
  ensureMermanReady,
  installMermanDocumentLifecycle,
} from "./runtime/merman";
import {
  disposeRenderCoordinator,
  resumeRenderCoordinator,
  suspendRenderCoordinator,
} from "./runtime/render-coordinator-browser";

void ensureMermanReady().catch(() => undefined);
const removeDocumentLifecycle = installMermanDocumentLifecycle(
  { document, window },
  {
    onDestroy: disposeRenderCoordinator,
    onResume: resumeRenderCoordinator,
    onSuspend: suspendRenderCoordinator,
  }
);
const root = createRoot(document.getElementById("root")!);

root.render(
  <StrictMode>
    <App />
  </StrictMode>
);

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    removeDocumentLifecycle();
    disposeRenderCoordinator();
    disposeMermanRuntime();
    root.unmount();
  });
}
