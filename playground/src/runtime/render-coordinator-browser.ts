import { compareMermaidRealmController } from "./mermaid-realm.ts";
import { createRenderCoordinator } from "./render-coordinator.ts";
import type { RenderPublicationId } from "./render-coordinator.ts";
import { playgroundStartupBoundary } from "./startup-boundary.ts";

const INITIAL_PREVIEW_PRESENTED_MARK = "merman:initial-preview-presented";
let initialPreviewPresented = false;

export const renderCoordinator = createRenderCoordinator({
  compare: compareMermaidRealmController,
});

export const renderCoordinatorStore = renderCoordinator.store;
export const disposeRenderCoordinator = () => renderCoordinator.dispose();
export const markRenderCoordinatorPresented = (
  publicationId: RenderPublicationId,
  engine: "merman" | "mermaid",
  at: number
) => {
  if (engine === "merman" && !initialPreviewPresented) {
    initialPreviewPresented = true;
    performance.mark(INITIAL_PREVIEW_PRESENTED_MARK, { startTime: at });
    playgroundStartupBoundary.activate("preview-presented");
  }
  renderCoordinator.markPresented(publicationId, engine, at);
};
export const pauseRenderCoordinator = () => renderCoordinator.pause();
export const refreshRenderCoordinator = () => renderCoordinator.refresh();
export const resumeRenderCoordinator = () => renderCoordinator.resume();
export const setRenderFeatures = renderCoordinator.setFeatures;
export const setRenderCoordinatorInput = renderCoordinator.setInput;
export const suspendRenderCoordinator = () => renderCoordinator.suspend();
