import { useEffect } from "react";
import { useShallow } from "zustand/react/shallow";

import { selectWorkspaceSnapshot, useAppStore } from "../store";
import { setRenderCoordinatorInput } from "./render-coordinator-browser.ts";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "./use-merman-runtime.ts";
import { captureRenderViewport } from "./render-viewport.ts";

const PLAYGROUND_RENDER_VIEWPORT = captureRenderViewport();

export function RenderCoordinatorBridge() {
  const workspace = useAppStore(useShallow(selectWorkspaceSnapshot));
  const facade = useMermanRuntime(selectMermanFacade);

  useEffect(() => {
    setRenderCoordinatorInput({
      facade,
      renderViewport: PLAYGROUND_RENDER_VIEWPORT,
      workspace,
    });
  }, [facade, workspace]);

  return null;
}
