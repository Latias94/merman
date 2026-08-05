import { createBenchmarkController } from "./controller.ts";
import { createBrowserBenchmarkRealmSession } from "./realm/controller.ts";
import { createBrowserBenchmarkDocumentLifecycle } from "./document-lifecycle.ts";
import { createRealmToken } from "../runtime/realm/channel-protocol.ts";
import { pauseRenderCoordinator } from "../runtime/render-coordinator-browser.ts";

export const benchmarkDocumentLifecycle = createBrowserBenchmarkDocumentLifecycle();

export const benchmarkController = createBenchmarkController({
  clearTimer: (handle) =>
    clearTimeout(handle as ReturnType<typeof setTimeout>),
  createRealm: createBrowserBenchmarkRealmSession,
  createSeed() {
    const seed = new Uint32Array(1);
    crypto.getRandomValues(seed);
    return seed[0];
  },
  createToken: createRealmToken,
  dateNow: Date.now,
  getEnvironment() {
    return {
      userAgent: navigator.userAgent,
      language: navigator.language,
      platform: navigator.platform || "unknown",
      hardwareConcurrency:
        Number.isFinite(navigator.hardwareConcurrency) &&
        navigator.hardwareConcurrency > 0
          ? navigator.hardwareConcurrency
          : null,
      devicePixelRatio: window.devicePixelRatio,
      crossOriginIsolated: globalThis.crossOriginIsolated === true,
    };
  },
  lifecycle: benchmarkDocumentLifecycle,
  now: () => performance.now(),
  pauseCoordinator: pauseRenderCoordinator,
  setTimer: (callback, timeoutMs) => setTimeout(callback, timeoutMs),
});

if (import.meta.hot) {
  import.meta.hot.dispose(() => benchmarkController.dispose());
}
