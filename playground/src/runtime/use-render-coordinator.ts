import { useStore } from "zustand";
import type { DiagramType } from "@mermanjs/web";

import { renderCoordinatorStore } from "./render-coordinator-browser.ts";
import type {
  CompletedRenderBatch,
  RenderCoordinatorState,
} from "./render-coordinator.ts";
import { isCompletedRenderState } from "./render-coordinator.ts";

export function useRenderCoordinator<T>(
  selector: (state: RenderCoordinatorState) => T
): T {
  return useStore(renderCoordinatorStore, selector);
}

export function selectCompletedRenderBatch(
  state: RenderCoordinatorState
): CompletedRenderBatch | null {
  return isCompletedRenderState(state) ? state : null;
}

export function selectVisibleRenderBatch(
  state: RenderCoordinatorState
): CompletedRenderBatch | null {
  if (isCompletedRenderState(state)) return state;
  return state.status === "updating" ? state.previous : null;
}

export function selectCurrentDiagramType(
  state: RenderCoordinatorState
): DiagramType | "unknown" {
  if (
    !isCompletedRenderState(state) ||
    state.detection.status !== "available"
  ) {
    return "unknown";
  }
  return state.detection.diagramType;
}

export function selectCurrentDetectionValidity(
  state: RenderCoordinatorState
): "valid" | "recoverable-invalid" | "unknown" {
  return isCompletedRenderState(state)
    ? state.detection.validity
    : "unknown";
}

export function selectCurrentMermanRenderTime(
  state: RenderCoordinatorState
): number {
  return isCompletedRenderState(state) && state.merman.status === "success"
    ? state.merman.renderTimeMs
    : 0;
}

export function selectRenderPending(state: RenderCoordinatorState): boolean {
  return state.status === "pending" || state.status === "updating";
}
