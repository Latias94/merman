import { useStore } from "zustand";
import type { DiagramType, SvgRenderMetadata } from "@mermanjs/web";

import { renderCoordinatorStore } from "./render-coordinator-browser.ts";
import type {
  CompletedRenderBatch,
  EngineRenderFailure,
  RenderCoordinatorState,
} from "./render-coordinator.ts";
import { isCompletedRenderState } from "./render-coordinator.ts";
import {
  resolveMermaidCanvasTone,
  type MermaidCanvasTone,
} from "../lib/mermaid-canvas-tone.ts";

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

export function selectVisibleCanvasTone(
  state: RenderCoordinatorState,
): MermaidCanvasTone {
  const batch = selectVisibleRenderBatch(state);
  if (batch?.merman.status === "success") {
    const operation = batch.snapshot.operation;
    const appearance = operation.resolvedThemeAppearance;
    if (appearance && operation.unavailableThemePresetId === null &&
      (operation.themePresetId !== null || operation.themeRecipeJson !== null)) return appearance;
    const evidence = batch.merman.metadata?.theme_execution_evidence;
    return resolveMermaidCanvasTone(
      evidence?.version === 2 ? evidence.materialized_theme : null,
    );
  }
  return resolveMermaidCanvasTone(
    batch?.mermaid?.status === "success" ? batch.mermaid.materializedTheme : null,
  );
}

export function selectCurrentDiagramType(
  state: RenderCoordinatorState
): DiagramType | "unknown" {
  const batch = selectVisibleRenderBatch(state);
  if (!batch || batch.detection.status !== "available") {
    return "unknown";
  }
  return batch.detection.diagramType;
}

export function selectCurrentDetectionValidity(
  state: RenderCoordinatorState
): "valid" | "recoverable-invalid" | "unknown" {
  return selectVisibleRenderBatch(state)?.detection.validity ?? "unknown";
}

export function selectCurrentMermanRenderTime(
  state: RenderCoordinatorState
): number {
  return isCompletedRenderState(state) && state.merman.status === "success"
    ? state.merman.renderTimeMs
    : 0;
}

export function selectVisibleMermanRenderMetadata(
  state: RenderCoordinatorState
): SvgRenderMetadata | null {
  const batch = selectVisibleRenderBatch(state);
  return batch?.merman.status === "success" ? batch.merman.metadata : null;
}

export function selectRenderPending(state: RenderCoordinatorState): boolean {
  return state.status === "pending" || state.status === "updating";
}

export function selectCurrentMermanRenderFailure(
  state: RenderCoordinatorState
): EngineRenderFailure | null {
  return isCompletedRenderState(state) && state.merman.status === "failure"
    ? state.merman
    : null;
}

export function selectCurrentMermaidRenderFailure(
  state: RenderCoordinatorState
): EngineRenderFailure | null {
  return isCompletedRenderState(state) && state.mermaid?.status === "failure"
    ? state.mermaid
    : null;
}
