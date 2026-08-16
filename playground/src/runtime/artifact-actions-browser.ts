import {
  copyASCIIToClipboard,
  copySVGToClipboard,
  exportASCII,
} from "../lib/export.ts";
import { mermanRuntimeStore } from "./merman.ts";
import { renderCoordinatorStore } from "./render-coordinator-browser.ts";
import {
  createArtifactActionOwner,
  createExportTargetOwner,
} from "./artifact-actions.ts";

export const executeArtifactAction = createArtifactActionOwner({
  getRenderState: renderCoordinatorStore.getState,
  io: {
    copyAscii: copyASCIIToClipboard,
    copySvg: copySVGToClipboard,
    downloadAscii: exportASCII,
  },
});

export const exportTargetOwner = createExportTargetOwner({
  getRenderState: renderCoordinatorStore.getState,
  getRuntimeState: mermanRuntimeStore.getState,
});
