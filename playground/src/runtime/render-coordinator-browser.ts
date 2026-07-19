import { assertSafeSvgForDom } from "@mermanjs/web/svg-safety";

import { compareMermaidRealmController } from "./mermaid-realm.ts";
import { createRenderCoordinator } from "./render-coordinator.ts";
import { PLAYGROUND_RENDER_VIEWPORT } from "./render-viewport.ts";

export const renderCoordinator = createRenderCoordinator({
  compare: compareMermaidRealmController,
  compareViewport: PLAYGROUND_RENDER_VIEWPORT,
  validateSvg: assertSafeSvgForDom,
});

export const renderCoordinatorStore = renderCoordinator.store;
export const disposeRenderCoordinator = () => renderCoordinator.dispose();
export const markRenderCoordinatorPresented = (
  requestId: number,
  engine: "merman" | "mermaid",
  at: number
) => renderCoordinator.markPresented(requestId, engine, at);
export const pauseRenderCoordinator = () => renderCoordinator.pause();
export const refreshRenderCoordinator = () => renderCoordinator.refresh();
export const resumeRenderCoordinator = () => renderCoordinator.resume();
export const setCompareEnabled = (enabled: boolean) =>
  renderCoordinator.setCompareEnabled(enabled);
export const setDiagnosticsEnabled = (enabled: boolean) =>
  renderCoordinator.setDiagnosticsEnabled(enabled);
export const setRenderCoordinatorInput = renderCoordinator.setInput;
export const suspendRenderCoordinator = () => renderCoordinator.suspend();
