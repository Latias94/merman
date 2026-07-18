import {
  createMermanRuntime,
  installMermanDocumentLifecycle as installDocumentLifecycle,
  type MermanDocumentLifecycleCallbacks,
  type MermanDocumentLifecycleTarget,
} from "./merman-core.ts";
import { mermanBrowserDependencies } from "./merman-browser.ts";

const mermanRuntime = createMermanRuntime(mermanBrowserDependencies);
export const mermanRuntimeStore = mermanRuntime.store;
export const ensureMermanReady = () => mermanRuntime.ensureReady();
export const retryMermanRuntime = () => mermanRuntime.retry();
export const disposeMermanRuntime = () => mermanRuntime.dispose();

export function installMermanDocumentLifecycle(
  target: MermanDocumentLifecycleTarget,
  callbacks?: MermanDocumentLifecycleCallbacks
): () => void {
  return installDocumentLifecycle(mermanRuntime, target, callbacks);
}
