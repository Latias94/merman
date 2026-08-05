import {
  createBenchmarkController,
  type BenchmarkController,
} from "./controller.ts";
import { createBrowserBenchmarkRealmSession } from "./realm/controller.ts";
import {
  createBrowserBenchmarkDocumentLifecycle,
  type BenchmarkDocumentLifecycle,
} from "./document-lifecycle.ts";
import { createRealmToken } from "../runtime/realm/channel-protocol.ts";

export interface BrowserBenchmarkRuntime {
  readonly controller: BenchmarkController;
  readonly lifecycle: BenchmarkDocumentLifecycle;
}

export function createBrowserBenchmarkRuntime(
  pauseCoordinator: () => Promise<() => void> = async () => () => {},
): BrowserBenchmarkRuntime {
  const lifecycle = createBrowserBenchmarkDocumentLifecycle();
  const controller = createBenchmarkController({
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
    lifecycle,
    now: () => performance.now(),
    pauseCoordinator,
    setTimer: (callback, timeoutMs) => setTimeout(callback, timeoutMs),
  });
  return Object.freeze({ controller, lifecycle });
}
