import { assertSafeSvgForDom } from "@mermanjs/web/svg-safety";

import { compareMermaidRealmController } from "./mermaid-realm.ts";
import { createRenderCoordinator } from "./render-coordinator.ts";

export const renderCoordinator = createRenderCoordinator({
  compare: compareMermaidRealmController,
  validateSvg: assertSafeSvgForDom,
});

export const renderCoordinatorStore = renderCoordinator.store;
export const disposeRenderCoordinator = () => renderCoordinator.dispose();
export const markRenderCoordinatorPresented = (
  requestId: number,
  engine: "merman" | "mermaid",
  at: number
) => renderCoordinator.markPresented(requestId, engine, at);
export const refreshRenderCoordinator = () => renderCoordinator.refresh();
export const resumeRenderCoordinator = () => renderCoordinator.resume();
export const setCompareEnabled = (enabled: boolean) =>
  renderCoordinator.setCompareEnabled(enabled);
export const setCompareViewport = (
  viewport: { width: number; height: number } | null
) => renderCoordinator.setCompareViewport(viewport);
export const setDiagnosticsEnabled = (enabled: boolean) =>
  renderCoordinator.setDiagnosticsEnabled(enabled);
export const setRenderCoordinatorInput = renderCoordinator.setInput;
export const suspendRenderCoordinator = () => renderCoordinator.suspend();
