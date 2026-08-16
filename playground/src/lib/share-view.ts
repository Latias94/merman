import {
  decodeShareHash,
  encodeShareHash,
  type ShareCommandEnvironment,
  type ShareLocation,
} from "./share.ts";
import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";
import {
  validateRealmViewport,
  type RealmViewport,
} from "../runtime/realm/channel-protocol.ts";

const SHARE_VIEW_VERSION = "1";
const VIEW_PARAMETER_KEYS = [
  "rv",
  "workspacePane",
  "editorMode",
  "previewMode",
  "hostWidth",
  "hostHeight",
  "screenAvailableWidth",
  "renderViewportMode",
] as const;
const LOCK_PARAMETER_KEYS = [
  "hostWidth",
  "hostHeight",
  "screenAvailableWidth",
] as const;

export const SHARE_VIEW_LIMITS = Object.freeze({
  // CSS-pixel screen width is a separate C4 input, not the 4096px Host viewport.
  screenAvailableWidth: 16_384,
});

export interface LockedRenderEnvironment extends RealmViewport {
  readonly screenAvailableWidth: number;
}

export type ShareWorkspacePane = "editor" | "preview";
export type ShareEditorMode = "code" | "config";
export type SharePreviewMode = "svg" | "ascii" | "compare" | "diagnostics";

export interface ShareViewDescriptor {
  readonly workspacePane: ShareWorkspacePane;
  readonly editorMode: ShareEditorMode;
  readonly previewMode: SharePreviewMode;
  readonly lockedEnvironment: Readonly<LockedRenderEnvironment> | null;
}

export const SHARE_VIEW_DEFAULTS: Readonly<ShareViewDescriptor> = Object.freeze({
  workspacePane: "editor",
  editorMode: "code",
  previewMode: "svg",
  lockedEnvironment: null,
});

export interface ShareViewWarning {
  readonly code: "share-view-not-restored";
  readonly message: string;
}

export const SHARE_VIEW_NOT_RESTORED_WARNING: Readonly<ShareViewWarning> =
  Object.freeze({
    code: "share-view-not-restored",
    message:
      "The issue reproduction context could not be restored. Local view defaults and live Host sizing are being used.",
  });

export type ShareViewDecodeResult =
  | Readonly<{ status: "absent"; view: null; warning: null }>
  | Readonly<{
      status: "valid";
      view: Readonly<ShareViewDescriptor>;
      warning: null;
    }>
  | Readonly<{
      status: "invalid";
      view: null;
      warning: Readonly<ShareViewWarning>;
    }>;

export interface StartupShareHydration {
  readonly workspace: Readonly<WorkspaceSnapshot>;
  readonly view: Readonly<ShareViewDescriptor>;
  readonly warning: Readonly<ShareViewWarning> | null;
}

export type StartupShareHydrationResult =
  | Readonly<{ status: "ignored"; warning: null }>
  | Readonly<{
      status: "applied";
      warning: Readonly<ShareViewWarning> | null;
    }>;

export function decodeShareView(
  input: string | URLSearchParams | Pick<Location, "search">
): ShareViewDecodeResult {
  const params = toSearchParams(input);
  const hasViewState = VIEW_PARAMETER_KEYS.some((key) => params.has(key));
  if (!hasViewState) {
    return Object.freeze({ status: "absent", view: null, warning: null });
  }

  if (
    params.has("renderViewportMode") ||
    !hasSingleValue(params, "rv") ||
    params.get("rv") !== SHARE_VIEW_VERSION ||
    !hasSingleValue(params, "workspacePane") ||
    !hasSingleValue(params, "editorMode") ||
    !hasSingleValue(params, "previewMode")
  ) {
    return invalidShareView();
  }

  const workspacePane = params.get("workspacePane");
  const editorMode = params.get("editorMode");
  const previewMode = params.get("previewMode");
  if (
    !isWorkspacePane(workspacePane) ||
    !isEditorMode(editorMode) ||
    !isPreviewMode(previewMode)
  ) {
    return invalidShareView();
  }

  const presentLockKeys = LOCK_PARAMETER_KEYS.filter((key) => params.has(key));
  let lockedEnvironment: Readonly<LockedRenderEnvironment> | null = null;
  if (presentLockKeys.length > 0) {
    if (
      presentLockKeys.length !== LOCK_PARAMETER_KEYS.length ||
      LOCK_PARAMETER_KEYS.some((key) => !hasSingleValue(params, key))
    ) {
      return invalidShareView();
    }
    lockedEnvironment = decodeLockedEnvironment(params);
    if (!lockedEnvironment) return invalidShareView();
  }

  return Object.freeze({
    status: "valid",
    view: Object.freeze({
      workspacePane,
      editorMode,
      previewMode,
      lockedEnvironment,
    }),
    warning: null,
  });
}

export function encodeShareView(view: ShareViewDescriptor): string {
  const normalized = normalizeShareView(view);
  const params = new URLSearchParams();
  params.set("rv", SHARE_VIEW_VERSION);
  params.set("workspacePane", normalized.workspacePane);
  params.set("editorMode", normalized.editorMode);
  params.set("previewMode", normalized.previewMode);
  if (normalized.lockedEnvironment) {
    params.set("hostWidth", String(normalized.lockedEnvironment.width));
    params.set("hostHeight", String(normalized.lockedEnvironment.height));
    params.set(
      "screenAvailableWidth",
      String(normalized.lockedEnvironment.screenAvailableWidth)
    );
  }
  return params.toString();
}

export function createIssueShareUrl(
  workspace: WorkspaceSnapshot,
  view: ShareViewDescriptor,
  location: ShareLocation = window.location
): string {
  const normalizedView = normalizeShareView(view);
  if (
    normalizedView.lockedEnvironment &&
    workspace.renderViewportMode !== "host"
  ) {
    throw new RangeError("A locked environment requires a Host workspace.");
  }

  const hash = encodeShareHash(workspace);
  return `${location.origin}${location.pathname}?${encodeShareView(normalizedView)}${hash}`;
}

export async function copyIssueShareUrl(
  workspace: WorkspaceSnapshot,
  view: ShareViewDescriptor,
  environment: ShareCommandEnvironment = browserShareEnvironment()
): Promise<void> {
  await environment.writeClipboardText(
    createIssueShareUrl(workspace, view, environment)
  );
}

export function hydrateStartupShareLocation(
  location: Pick<Location, "hash" | "search">,
  apply: (hydration: StartupShareHydration) => void
): StartupShareHydrationResult {
  const workspace = decodeShareHash(location.hash);
  if (!workspace) {
    return Object.freeze({ status: "ignored", warning: null });
  }

  let viewResult = decodeShareView(location.search);
  if (
    viewResult.status === "valid" &&
    viewResult.view.lockedEnvironment &&
    workspace.renderViewportMode !== "host"
  ) {
    viewResult = invalidShareView();
  }

  const warning =
    viewResult.status === "invalid" ? viewResult.warning : null;
  const view =
    viewResult.status === "valid" ? viewResult.view : SHARE_VIEW_DEFAULTS;
  apply(Object.freeze({ workspace, view, warning }));
  return Object.freeze({ status: "applied", warning });
}

function normalizeShareView(
  view: ShareViewDescriptor
): Readonly<ShareViewDescriptor> {
  if (
    !isWorkspacePane(view.workspacePane) ||
    !isEditorMode(view.editorMode) ||
    !isPreviewMode(view.previewMode)
  ) {
    throw new RangeError("View descriptor is invalid.");
  }

  return Object.freeze({
    workspacePane: view.workspacePane,
    editorMode: view.editorMode,
    previewMode: view.previewMode,
    lockedEnvironment: view.lockedEnvironment
      ? validateLockedEnvironment(view.lockedEnvironment)
      : null,
  });
}

function decodeLockedEnvironment(
  params: URLSearchParams
): Readonly<LockedRenderEnvironment> | null {
  const width = parsePositiveInteger(params.get("hostWidth"));
  const height = parsePositiveInteger(params.get("hostHeight"));
  const screenAvailableWidth = parsePositiveInteger(
    params.get("screenAvailableWidth")
  );
  if (
    width === null ||
    height === null ||
    screenAvailableWidth === null
  ) {
    return null;
  }
  try {
    return validateLockedEnvironment({ width, height, screenAvailableWidth });
  } catch {
    return null;
  }
}

function validateLockedEnvironment(
  value: LockedRenderEnvironment
): Readonly<LockedRenderEnvironment> {
  if (
    !Number.isSafeInteger(value.width) ||
    !Number.isSafeInteger(value.height)
  ) {
    throw new RangeError("Shared Host dimensions are invalid.");
  }
  const viewport = validateRealmViewport({
    width: value.width,
    height: value.height,
  });
  if (
    !Number.isSafeInteger(value.screenAvailableWidth) ||
    value.screenAvailableWidth <= 0 ||
    value.screenAvailableWidth > SHARE_VIEW_LIMITS.screenAvailableWidth
  ) {
    throw new RangeError("Shared screen width is invalid.");
  }
  return Object.freeze({
    ...viewport,
    screenAvailableWidth: value.screenAvailableWidth,
  });
}

function parsePositiveInteger(value: string | null): number | null {
  if (value === null || !/^[1-9][0-9]*$/u.test(value)) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function hasSingleValue(params: URLSearchParams, key: string): boolean {
  return params.getAll(key).length === 1;
}

function toSearchParams(
  input: string | URLSearchParams | Pick<Location, "search">
): URLSearchParams {
  if (input instanceof URLSearchParams) return new URLSearchParams(input);
  return new URLSearchParams(typeof input === "string" ? input : input.search);
}

function invalidShareView(): ShareViewDecodeResult {
  return Object.freeze({
    status: "invalid",
    view: null,
    warning: SHARE_VIEW_NOT_RESTORED_WARNING,
  });
}

function isWorkspacePane(value: unknown): value is ShareWorkspacePane {
  return value === "editor" || value === "preview";
}

function isEditorMode(value: unknown): value is ShareEditorMode {
  return value === "code" || value === "config";
}

function isPreviewMode(value: unknown): value is SharePreviewMode {
  return (
    value === "svg" ||
    value === "ascii" ||
    value === "compare" ||
    value === "diagnostics"
  );
}

function browserShareEnvironment(): ShareCommandEnvironment {
  return {
    origin: window.location.origin,
    pathname: window.location.pathname,
    writeClipboardText: (value) => navigator.clipboard.writeText(value),
  };
}
