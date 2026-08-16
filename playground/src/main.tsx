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
import { configureLocalMonaco } from "./editor/monaco";
import { installUIThemeLifecycle, useAppStore } from "./store";
import { hydrateStartupShareLocation } from "./lib/share-view";

hydrateStartupShareLocation(window.location, (hydration) => {
  useAppStore.getState().applyStartupShareHydration(hydration);
});

const monacoOwner = configureLocalMonaco();
const removeThemeLifecycle = installUIThemeLifecycle();
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
    removeThemeLifecycle();
    disposeRenderCoordinator();
    disposeMermanRuntime();
    monacoOwner.dispose();
    root.unmount();
  });
}
