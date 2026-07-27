import {
  createMermanRuntimeState,
  withMermanRuntimeState,
} from "./runtime-state.js";
import type {
  MermanInitInput,
  MermanWasmLoader,
  MermanWasmModule,
  MermanWasmModuleBase,
} from "./public-types.js";

type WorkerGlobalScopeConstructor = new (...args: never[]) => object;
type ServerProcess = {
  release?: { name?: unknown };
  versions?: { node?: unknown };
};
type RuntimeFunction = (...args: any[]) => unknown;
type SurfaceImplementation = Record<string, RuntimeFunction>;

/// Reject server runtimes at the public browser-package boundary.
///
/// A main-window package may use `window` and `document`; an editor package may instead run in a
/// real browser Worker. Node and SSR runtimes match neither shape. This check intentionally runs
/// before a caller-supplied loader so a custom WASM source cannot turn a browser package into an
/// undocumented server transport.
export function assertBrowserRuntime(): void {
  const processLike = (
    globalThis as typeof globalThis & { process?: ServerProcess }
  ).process;
  const isNodeRuntime =
    processLike?.release?.name === "node" &&
    typeof processLike.versions?.node === "string";
  const isDenoRuntime = "Deno" in globalThis;
  const isBunRuntime = "Bun" in globalThis;
  if (isNodeRuntime || isDenoRuntime || isBunRuntime) {
    throw new Error(
      "Merman browser packages require a browser main-thread or Web Worker realm. Use a native or Node transport for SSR and server runtimes.",
    );
  }
  const isBrowserWindow = typeof window !== "undefined" && typeof document !== "undefined";
  const workerGlobalScope = (
    globalThis as typeof globalThis & { WorkerGlobalScope?: WorkerGlobalScopeConstructor }
  ).WorkerGlobalScope;
  const isBrowserWorker =
    typeof workerGlobalScope === "function" && globalThis instanceof workerGlobalScope;
  if (!isBrowserWindow && !isBrowserWorker) {
    throw new Error(
      "Merman browser packages require a browser main-thread or Web Worker realm. Use a native or Node transport for SSR and Node.js.",
    );
  }
}

export type SurfaceRuntime<
  Module extends MermanWasmModuleBase = MermanWasmModule,
  Implementation extends SurfaceImplementation = SurfaceImplementation,
> = {
  [Key in keyof Implementation]: Key extends "initMerman"
    ? (init?: MermanInitInput<Module>) => Promise<Module>
    : Key extends "getMerman"
      ? () => Module
      : Implementation[Key];
};

export function bindSurfaceRuntime<
  Module extends MermanWasmModuleBase,
  Implementation extends SurfaceImplementation,
>(
  surfaceLoader: MermanWasmLoader<Module>,
  implementation: Implementation,
): SurfaceRuntime<Module, Implementation> {
  const sharedLoader = surfaceLoader as unknown as MermanWasmLoader;
  const state = createMermanRuntimeState(sharedLoader);
  const runtime: Record<string, RuntimeFunction> = {};

  for (const [name, binding] of Object.entries(implementation)) {
    runtime[name] = (...args: unknown[]) =>
      withMermanRuntimeState(state, () => binding(...args));
  }

  return runtime as SurfaceRuntime<Module, Implementation>;
}
