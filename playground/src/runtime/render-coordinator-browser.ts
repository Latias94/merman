import { compareMermaidRealmController } from "./mermaid-realm.ts";
import { createRenderCoordinator } from "./render-coordinator.ts";
import type { RenderPublicationId } from "./render-coordinator.ts";
import {
  capturePlaygroundLayoutEnvironment,
  PLAYGROUND_RENDER_VIEWPORT,
} from "./render-viewport.ts";

export const renderCoordinator = createRenderCoordinator({
  compare: compareMermaidRealmController,
  compareViewport: PLAYGROUND_RENDER_VIEWPORT,
  captureLayoutEnvironment: capturePlaygroundLayoutEnvironment,
});

export const renderCoordinatorStore = renderCoordinator.store;
export const disposeRenderCoordinator = () => renderCoordinator.dispose();
export const markRenderCoordinatorPresented = (
  publicationId: RenderPublicationId,
  engine: "merman" | "mermaid",
  at: number
) => renderCoordinator.markPresented(publicationId, engine, at);
export const pauseRenderCoordinator = () => renderCoordinator.pause();
export const refreshRenderCoordinator = () => renderCoordinator.refresh();
export const resumeRenderCoordinator = () => renderCoordinator.resume();
export const setRenderFeatures = renderCoordinator.setFeatures;
export const setRenderCoordinatorInput = renderCoordinator.setInput;
export const suspendRenderCoordinator = () => renderCoordinator.suspend();
