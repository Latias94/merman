import {
  copyASCIIToClipboard,
  copySVGToClipboard,
  exportASCII,
} from "../lib/export.ts";
import { mermanRuntimeStore } from "./merman.ts";
import { selectWorkspaceSnapshot, useAppStore } from "../store/index.ts";
import { renderCoordinatorStore } from "./render-coordinator-browser.ts";
import {
  createArtifactActionOwner,
  createExportTargetOwner,
} from "./artifact-actions.ts";

export const executeArtifactAction = createArtifactActionOwner({
  getWorkspace: () => selectWorkspaceSnapshot(useAppStore.getState()),
  getHostAppearance: () => useAppStore.getState().resolvedTheme,
  getRenderState: renderCoordinatorStore.getState,
  getRuntimeState: mermanRuntimeStore.getState,
  io: {
    copyAscii: copyASCIIToClipboard,
    copySvg: copySVGToClipboard,
    downloadAscii: exportASCII,
  },
});

export const exportTargetOwner = createExportTargetOwner({
  getWorkspace: () => selectWorkspaceSnapshot(useAppStore.getState()),
  getHostAppearance: () => useAppStore.getState().resolvedTheme,
  getRenderState: renderCoordinatorStore.getState,
  getRuntimeState: mermanRuntimeStore.getState,
});
