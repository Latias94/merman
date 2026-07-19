import path from "node:path";
import { fileURLToPath } from "node:url";

export const wasmBuildModuleRoot = path.dirname(fileURLToPath(import.meta.url));
export const webPackageRoot = path.resolve(wasmBuildModuleRoot, "..", "..");
export const repositoryRoot = path.resolve(webPackageRoot, "..", "..");
export const defaultWasmOutputRoot = path.join(webPackageRoot, "pkg");
