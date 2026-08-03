import type { RealmViewport } from "./realm/channel-protocol.ts";
import type { MermanLayoutEnvironment } from "./merman-core.ts";

export const PLAYGROUND_RENDER_VIEWPORT: RealmViewport = Object.freeze({
  width: 800,
  height: 600,
});

export function capturePlaygroundLayoutEnvironment(): MermanLayoutEnvironment {
  const availableWidth = window.screen.availWidth;
  const container = {
    containerWidth: PLAYGROUND_RENDER_VIEWPORT.width,
    containerHeight: PLAYGROUND_RENDER_VIEWPORT.height,
  };
  return Object.freeze(
    Number.isFinite(availableWidth) && availableWidth > 0
      ? { ...container, screenAvailableWidth: availableWidth }
      : container
  );
}
