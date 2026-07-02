import type { PreviewBackground } from "./preview-model.js";

export const PREVIEW_DARK_BACKGROUND_COLOR = "#111827";

export function previewCliBackground(background: PreviewBackground): string {
  switch (background) {
    case "paper":
      return "white";
    case "dark":
      return PREVIEW_DARK_BACKGROUND_COLOR;
    case "transparent":
    default:
      return "transparent";
  }
}
