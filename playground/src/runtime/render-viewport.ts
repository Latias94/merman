import type { MermanLayoutEnvironment } from "./merman-core.ts";
import type { RealmViewport } from "./realm/channel-protocol.ts";

export interface CapturedRenderViewport {
  readonly layoutEnvironment: Readonly<MermanLayoutEnvironment>;
  readonly viewport: Readonly<RealmViewport>;
}

export const CANONICAL_RENDER_VIEWPORT: Readonly<RealmViewport> = Object.freeze({
  width: 800,
  height: 600,
});

export const CANONICAL_SCREEN_AVAILABLE_WIDTH =
  CANONICAL_RENDER_VIEWPORT.width;

const CANONICAL_PLAYGROUND_LAYOUT_ENVIRONMENT: Readonly<MermanLayoutEnvironment> =
  Object.freeze({
    containerWidth: CANONICAL_RENDER_VIEWPORT.width,
    containerHeight: CANONICAL_RENDER_VIEWPORT.height,
    screenAvailableWidth: CANONICAL_SCREEN_AVAILABLE_WIDTH,
  });

const CANONICAL_CAPTURED_RENDER_VIEWPORT: Readonly<CapturedRenderViewport> =
  Object.freeze({
    viewport: CANONICAL_RENDER_VIEWPORT,
    layoutEnvironment: CANONICAL_PLAYGROUND_LAYOUT_ENVIRONMENT,
  });

export function captureRenderViewport(): Readonly<CapturedRenderViewport> {
  return CANONICAL_CAPTURED_RENDER_VIEWPORT;
}
