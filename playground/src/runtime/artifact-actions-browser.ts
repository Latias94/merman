import {
  copyASCIIToClipboard,
  copySVGToClipboard,
  exportASCII,
} from "../lib/export.ts";
import { mermanRuntimeStore } from "./merman.ts";
import { selectThemeDraftChanged, selectWorkspaceSnapshot, useAppStore } from "../store/index.ts";
import { renderCoordinatorStore } from "./render-coordinator-browser.ts";
import {
  createArtifactActionOwner,
  createExportTargetOwner,
} from "./artifact-actions.ts";

export const executeArtifactAction = createArtifactActionOwner({
  getAppliedWorkspace: () => selectWorkspaceSnapshot(useAppStore.getState()),
  hasUnappliedThemeDraft: () => selectThemeDraftChanged(useAppStore.getState()),
  getRenderState: renderCoordinatorStore.getState,
  getRuntimeState: mermanRuntimeStore.getState,
  io: {
    copyAscii: copyASCIIToClipboard,
    copySvg: copySVGToClipboard,
    downloadAscii: exportASCII,
  },
});

export const exportTargetOwner = createExportTargetOwner({
  getAppliedWorkspace: () => selectWorkspaceSnapshot(useAppStore.getState()),
  hasUnappliedThemeDraft: () => selectThemeDraftChanged(useAppStore.getState()),
  getRenderState: renderCoordinatorStore.getState,
  getRuntimeState: mermanRuntimeStore.getState,
});
