import {
  validateRealmViewport,
  type RealmViewport,
} from "./realm/channel-protocol.ts";
import type { MermanLayoutEnvironment } from "./merman-core.ts";

export type RenderViewportMode = "canonical" | "host";
export type RenderViewportStatus = "canonical" | "host" | "host-measuring";

export interface ResolvedRenderViewport {
  readonly mode: RenderViewportMode;
  readonly status: RenderViewportStatus;
  readonly viewport: Readonly<RealmViewport>;
}

export interface CapturedRenderViewport extends ResolvedRenderViewport {
  readonly layoutEnvironment: Readonly<MermanLayoutEnvironment>;
}

export const CANONICAL_RENDER_VIEWPORT: Readonly<RealmViewport> = Object.freeze({
  width: 800,
  height: 600,
});

export function resolveRenderViewport(
  mode: RenderViewportMode,
  hostViewport: RealmViewport | null,
): ResolvedRenderViewport {
  if (mode === "canonical") {
    return Object.freeze({
      mode,
      status: "canonical",
      viewport: CANONICAL_RENDER_VIEWPORT,
    });
  }

  const viewport = normalizeHostViewport(hostViewport);
  return viewport
    ? Object.freeze({ mode, status: "host", viewport })
    : Object.freeze({
        mode,
        status: "host-measuring",
        viewport: CANONICAL_RENDER_VIEWPORT,
      });
}

export function capturePlaygroundLayoutEnvironment(
  viewport: Readonly<RealmViewport>,
  availableWidth: number = window.screen.availWidth,
): MermanLayoutEnvironment {
  const container = {
    containerWidth: viewport.width,
    containerHeight: viewport.height,
  };
  return Number.isFinite(availableWidth) && availableWidth > 0
    ? { ...container, screenAvailableWidth: availableWidth }
    : container;
}

export function captureRenderViewport(
  mode: RenderViewportMode,
  hostViewport: RealmViewport | null,
  availableWidth: number = window.screen.availWidth,
): CapturedRenderViewport {
  const resolved = resolveRenderViewport(mode, hostViewport);
  return Object.freeze({
    ...resolved,
    layoutEnvironment: Object.freeze(
      capturePlaygroundLayoutEnvironment(resolved.viewport, availableWidth),
    ),
  });
}

function normalizeHostViewport(
  viewport: RealmViewport | null,
): Readonly<RealmViewport> | null {
  if (!viewport) return null;
  try {
    return Object.freeze(
      validateRealmViewport({
        width: Math.round(viewport.width),
        height: Math.round(viewport.height),
      }),
    );
  } catch {
    return null;
  }
}
