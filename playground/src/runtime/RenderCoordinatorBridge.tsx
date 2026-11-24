import { useEffect } from "react";
import { shallow } from "zustand/shallow";

import { selectWorkspaceSnapshot, useAppStore, type AppState } from "../store";
import { setRenderCoordinatorInput } from "./render-coordinator-browser.ts";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "./use-merman-runtime.ts";
import { captureRenderViewport } from "./render-viewport.ts";

const PLAYGROUND_RENDER_VIEWPORT = captureRenderViewport();

export function RenderCoordinatorBridge() {
  const facade = useMermanRuntime(selectMermanFacade);

  useEffect(() => {
    const sync = (state: AppState, workspace = selectWorkspaceSnapshot(state)) => setRenderCoordinatorInput({
      documentRevision: state.documentRevision,
      facade,
      renderViewport: PLAYGROUND_RENDER_VIEWPORT,
      workspace,
      shoalPreviewWidth: state.shoalPreviewWidth,
      hostAppearance: state.resolvedTheme,
    });
    sync(useAppStore.getState());
    return useAppStore.subscribe((state, previous) => {
      const workspace = selectWorkspaceSnapshot(state);
      if (state.documentRevision === previous.documentRevision &&
        state.shoalPreviewWidth === previous.shoalPreviewWidth &&
        state.resolvedTheme === previous.resolvedTheme &&
        shallow(workspace, selectWorkspaceSnapshot(previous))) return;
      sync(state, workspace);
    });
  }, [facade]);

  return null;
}
