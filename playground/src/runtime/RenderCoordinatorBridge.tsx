import { useEffect } from "react";
import { useShallow } from "zustand/react/shallow";

import { selectWorkspaceSnapshot, useAppStore } from "../store";
import { setRenderCoordinatorInput } from "./render-coordinator-browser.ts";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "./use-merman-runtime.ts";

export function RenderCoordinatorBridge() {
  const workspace = useAppStore(useShallow(selectWorkspaceSnapshot));
  const facade = useMermanRuntime(selectMermanFacade);

  useEffect(() => {
    setRenderCoordinatorInput({ facade, workspace });
  }, [facade, workspace]);

  return null;
}
