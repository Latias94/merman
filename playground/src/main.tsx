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

void ensureMermanReady().catch(() => undefined);
const removeDocumentLifecycle = installMermanDocumentLifecycle({
  document,
  window,
});
const root = createRoot(document.getElementById("root")!);

root.render(
  <StrictMode>
    <App />
  </StrictMode>
);

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    removeDocumentLifecycle();
    disposeMermanRuntime();
    root.unmount();
  });
}
