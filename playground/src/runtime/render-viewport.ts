import {
  validateScreenAvailableWidth,
  validateRealmViewport,
  type RealmViewport,
} from "./realm/channel-protocol.ts";
import type { MermanLayoutEnvironment } from "./merman-core.ts";

export type RenderViewportMode = "canonical" | "host";
export type RenderViewportStatus =
  | "canonical"
  | "host"
  | "host-locked"
  | "host-measuring";

export interface LockedRenderEnvironment extends RealmViewport {
  readonly screenAvailableWidth: number;
}

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
  sharedEnvironmentLock: LockedRenderEnvironment | null = null,
): ResolvedRenderViewport {
  if (mode === "canonical") {
    return Object.freeze({
      mode,
      status: "canonical",
      viewport: CANONICAL_RENDER_VIEWPORT,
    });
  }

  const lockedEnvironment = normalizeLockedRenderEnvironment(
    sharedEnvironmentLock,
  );
  if (lockedEnvironment) {
    return Object.freeze({
      mode,
      status: "host-locked",
      viewport: Object.freeze({
        width: lockedEnvironment.width,
        height: lockedEnvironment.height,
      }),
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
  try {
    return {
      ...container,
      screenAvailableWidth: validateScreenAvailableWidth(
        Math.round(availableWidth),
      ),
    };
  } catch {
    return container;
  }
}

export function captureRenderViewport(
  mode: RenderViewportMode,
  hostViewport: RealmViewport | null,
  availableWidth: number = window.screen.availWidth,
  sharedEnvironmentLock: LockedRenderEnvironment | null = null,
): CapturedRenderViewport {
  const lockedEnvironment = normalizeLockedRenderEnvironment(
    sharedEnvironmentLock,
  );
  const resolved = resolveRenderViewport(mode, hostViewport, lockedEnvironment);
  const effectiveAvailableWidth =
    resolved.status === "host-locked" && lockedEnvironment
      ? lockedEnvironment.screenAvailableWidth
      : availableWidth;
  return Object.freeze({
    ...resolved,
    layoutEnvironment: Object.freeze(
      capturePlaygroundLayoutEnvironment(
        resolved.viewport,
        effectiveAvailableWidth,
      ),
    ),
  });
}

export function validateLockedRenderEnvironment(
  value: LockedRenderEnvironment,
): Readonly<LockedRenderEnvironment> {
  if (!Number.isSafeInteger(value.width) || !Number.isSafeInteger(value.height)) {
    throw new RangeError("Shared Host dimensions are invalid.");
  }
  const viewport = validateRealmViewport({
    width: value.width,
    height: value.height,
  });
  return Object.freeze({
    ...viewport,
    screenAvailableWidth: validateScreenAvailableWidth(
      value.screenAvailableWidth,
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

function normalizeLockedRenderEnvironment(
  value: LockedRenderEnvironment | null,
): Readonly<LockedRenderEnvironment> | null {
  if (!value) return null;
  try {
    return validateLockedRenderEnvironment(value);
  } catch {
    return null;
  }
}
