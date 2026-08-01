import { createBenchmarkController, type BenchmarkLifecycleTarget } from "./controller.ts";
import { createBrowserBenchmarkRealmSession } from "./realm/controller.ts";
import { createRealmToken } from "../runtime/realm/channel-protocol.ts";
import { pauseRenderCoordinator } from "../runtime/render-coordinator-browser.ts";

const documentTarget = lifecycleTarget(document);
const windowTarget = lifecycleTarget(window);

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
  documentTarget,
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
  getVisibilityState: () => document.visibilityState,
  now: () => performance.now(),
  pauseCoordinator: pauseRenderCoordinator,
  setTimer: (callback, timeoutMs) => setTimeout(callback, timeoutMs),
  windowTarget,
});

if (import.meta.hot) {
  import.meta.hot.dispose(() => benchmarkController.dispose());
}

function lifecycleTarget(target: EventTarget): BenchmarkLifecycleTarget {
  return {
    addEventListener(type, listener) {
      target.addEventListener(type, listener as never);
    },
    removeEventListener(type, listener) {
      target.removeEventListener(type, listener as never);
    },
  };
}
