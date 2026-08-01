import { useStore } from "zustand";

import { mermanRuntimeStore } from "./merman.ts";
import type {
  MermanDomainFacade,
  MermanLoadStage,
  MermanRuntimeFailure,
  MermanRuntimeState,
} from "./merman-core.ts";

export function useMermanRuntime<T>(
  selector: (state: MermanRuntimeState) => T
): T {
  return useStore(mermanRuntimeStore, selector);
}

export const selectMermanStatus = (state: MermanRuntimeState) => state.status;
export const selectMermanFacade = (
  state: MermanRuntimeState
): MermanDomainFacade | null =>
  state.status === "ready" ? state.facade : null;
export const selectMermanFailure = (
  state: MermanRuntimeState
): MermanRuntimeFailure | null =>
  state.status === "error" ? state.error : null;
export const selectMermanLoadStage = (
  state: MermanRuntimeState
): MermanLoadStage | null =>
  state.status === "loading" ? state.stage : null;
