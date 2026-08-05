import {
  copyASCIIToClipboard,
  copySVGToClipboard,
  exportASCII,
  exportPNG,
  exportSVG,
} from "../lib/export.ts";
import { mermanRuntimeStore } from "./merman.ts";
import { renderCoordinatorStore } from "./render-coordinator-browser.ts";
import { createArtifactActionOwner } from "./artifact-actions.ts";

export const executeArtifactAction = createArtifactActionOwner({
  getRenderState: renderCoordinatorStore.getState,
  getRuntimeState: mermanRuntimeStore.getState,
  io: {
    copyAscii: copyASCIIToClipboard,
    copySvg: copySVGToClipboard,
    downloadAscii: exportASCII,
    downloadPng: exportPNG,
    downloadSvg: exportSVG,
  },
});
