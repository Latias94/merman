import {
  initMerman as initRootMerman,
  type MermanInitOptions,
  type MermanInitInput,
  type MermanWasmModule,
} from "../index.js";
export * from "../index.js";

function surfaceLoader(): Promise<MermanWasmModule> {
  // @ts-ignore -- generated wasm-bindgen artifact exists after build:surfaces runs.
  return import("../../pkg/render/merman_wasm.js");
}

export function initMerman(init?: MermanInitInput) {
  if (typeof init === "function") {
    return initRootMerman(init);
  }
  const options: MermanInitOptions = init ?? {};
  return initRootMerman({
    loader: surfaceLoader,
    ...options,
  });
}
