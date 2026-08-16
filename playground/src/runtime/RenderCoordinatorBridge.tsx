import { useEffect } from "react";
import { useShallow } from "zustand/react/shallow";

import { selectWorkspaceSnapshot, useAppStore } from "../store";
import { setRenderCoordinatorInput } from "./render-coordinator-browser.ts";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "./use-merman-runtime.ts";
import { captureRenderViewport } from "./render-viewport.ts";

export function RenderCoordinatorBridge() {
  const workspace = useAppStore(useShallow(selectWorkspaceSnapshot));
  const liveHostRenderViewport = useAppStore(
    (state) => state.liveHostRenderViewport,
  );
  const sharedRenderEnvironmentLock = useAppStore(
    (state) => state.sharedRenderEnvironmentLock,
  );
  const facade = useMermanRuntime(selectMermanFacade);

  useEffect(() => {
    setRenderCoordinatorInput({
      facade,
      renderViewport: captureRenderViewport(
        workspace.renderViewportMode,
        liveHostRenderViewport,
        window.screen.availWidth,
        sharedRenderEnvironmentLock,
      ),
      workspace,
    });
  }, [facade, liveHostRenderViewport, sharedRenderEnvironmentLock, workspace]);

  return null;
}
