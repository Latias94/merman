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
  const hostRenderViewport = useAppStore((state) => state.hostRenderViewport);
  const facade = useMermanRuntime(selectMermanFacade);

  useEffect(() => {
    setRenderCoordinatorInput({
      facade,
      renderViewport: captureRenderViewport(
        workspace.renderViewportMode,
        hostRenderViewport
      ),
      workspace,
    });
  }, [facade, hostRenderViewport, workspace]);

  return null;
}
