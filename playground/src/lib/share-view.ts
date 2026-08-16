import {
  browserShareEnvironment,
  decodeShareHash,
  encodeShareHash,
  type ShareCommandEnvironment,
  type ShareLocation,
} from "./share.ts";
import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";
import {
  validateRealmViewport,
  validateScreenAvailableWidth,
} from "../runtime/realm/channel-protocol.ts";
import {
  DEFAULT_SVG_PRESENTATION_MODE,
  isSvgPresentationMode,
  type SvgPresentationMode,
} from "./svg-presentation.ts";

const SHARE_VIEW_VERSION = "1";
// Decoder-only compatibility for Host-bearing rv=1 links created on the open branch.
const LEGACY_RENDER_VIEWPORT_KEY = "renderViewportMode";
const LEGACY_HOST_LOCK_KEYS = [
  "hostWidth",
  "hostHeight",
  "screenAvailableWidth",
] as const;
const VIEW_PARAMETER_KEYS = [
  "rv",
  "workspacePane",
  "editorMode",
  "previewMode",
  "showSvgBounds",
  "svgPresentationMode",
  LEGACY_RENDER_VIEWPORT_KEY,
  ...LEGACY_HOST_LOCK_KEYS,
] as const;

export type ShareWorkspacePane = "editor" | "preview";
export type ShareEditorMode = "code" | "config";
export type SharePreviewMode = "svg" | "ascii" | "compare" | "diagnostics";

export interface ShareViewDescriptor {
  readonly workspacePane: ShareWorkspacePane;
  readonly editorMode: ShareEditorMode;
  readonly previewMode: SharePreviewMode;
  readonly showSvgBounds: boolean;
  readonly svgPresentationMode: SvgPresentationMode;
}

export const SHARE_VIEW_DEFAULTS: Readonly<ShareViewDescriptor> = Object.freeze({
  workspacePane: "editor",
  editorMode: "code",
  previewMode: "svg",
  showSvgBounds: false,
  svgPresentationMode: DEFAULT_SVG_PRESENTATION_MODE,
});

export interface ShareViewWarning {
  readonly code: "share-view-not-restored";
  readonly message: string;
}

export const SHARE_VIEW_NOT_RESTORED_WARNING: Readonly<ShareViewWarning> =
  Object.freeze({
    code: "share-view-not-restored",
    message:
      "The issue reproduction context could not be restored. Local view defaults are being used.",
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
  input: string | URLSearchParams | Pick<Location, "search">,
): ShareViewDecodeResult {
  const params = toSearchParams(input);
  const hasViewState = VIEW_PARAMETER_KEYS.some((key) => params.has(key));
  if (!hasViewState) {
    return Object.freeze({ status: "absent", view: null, warning: null });
  }

  if (
    !hasSingleValue(params, "rv") ||
    params.get("rv") !== SHARE_VIEW_VERSION ||
    !hasSingleValue(params, "workspacePane") ||
    !hasSingleValue(params, "editorMode") ||
    !hasSingleValue(params, "previewMode") ||
    !hasValidLegacyHostState(params)
  ) {
    return invalidShareView();
  }

  const workspacePane = params.get("workspacePane");
  const editorMode = params.get("editorMode");
  const previewMode = params.get("previewMode");
  const showSvgBounds = decodeOptionalBoolean(params, "showSvgBounds", false);
  const svgPresentationMode = decodeOptionalSvgPresentationMode(params);
  if (
    !isWorkspacePane(workspacePane) ||
    !isEditorMode(editorMode) ||
    !isPreviewMode(previewMode) ||
    showSvgBounds === null ||
    svgPresentationMode === null
  ) {
    return invalidShareView();
  }

  return Object.freeze({
    status: "valid",
    view: Object.freeze({
      workspacePane,
      editorMode,
      previewMode,
      showSvgBounds,
      svgPresentationMode,
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
  params.set("showSvgBounds", String(normalized.showSvgBounds));
  params.set("svgPresentationMode", normalized.svgPresentationMode);
  return params.toString();
}

export function createIssueShareUrl(
  workspace: WorkspaceSnapshot,
  view: ShareViewDescriptor,
  location: ShareLocation = window.location,
): string {
  const hash = encodeShareHash(workspace);
  return `${location.origin}${location.pathname}?${encodeShareView(view)}${hash}`;
}

export async function copyIssueShareUrl(
  workspace: WorkspaceSnapshot,
  view: ShareViewDescriptor,
  environment: ShareCommandEnvironment = browserShareEnvironment(),
): Promise<void> {
  await environment.writeClipboardText(
    createIssueShareUrl(workspace, view, environment),
  );
}

export function hydrateStartupShareLocation(
  location: Pick<Location, "hash" | "search">,
  apply: (hydration: StartupShareHydration) => void,
): StartupShareHydrationResult {
  const workspace = decodeShareHash(location.hash);
  if (!workspace) {
    return Object.freeze({ status: "ignored", warning: null });
  }

  const viewResult = decodeShareView(location.search);
  const warning = viewResult.status === "invalid" ? viewResult.warning : null;
  const view =
    viewResult.status === "valid" ? viewResult.view : SHARE_VIEW_DEFAULTS;
  apply(Object.freeze({ workspace, view, warning }));
  return Object.freeze({ status: "applied", warning });
}

function normalizeShareView(
  view: ShareViewDescriptor,
): Readonly<ShareViewDescriptor> {
  if (
    !isWorkspacePane(view.workspacePane) ||
    !isEditorMode(view.editorMode) ||
    !isPreviewMode(view.previewMode) ||
    typeof view.showSvgBounds !== "boolean" ||
    !isSvgPresentationMode(view.svgPresentationMode)
  ) {
    throw new RangeError("View descriptor is invalid.");
  }

  return Object.freeze({
    workspacePane: view.workspacePane,
    editorMode: view.editorMode,
    previewMode: view.previewMode,
    showSvgBounds: view.showSvgBounds,
    svgPresentationMode: view.svgPresentationMode,
  });
}

function decodeOptionalSvgPresentationMode(
  params: URLSearchParams,
): SvgPresentationMode | null {
  if (!params.has("svgPresentationMode")) {
    return DEFAULT_SVG_PRESENTATION_MODE;
  }
  if (!hasSingleValue(params, "svgPresentationMode")) return null;
  const value = params.get("svgPresentationMode");
  return isSvgPresentationMode(value) ? value : null;
}

function hasValidLegacyHostState(params: URLSearchParams): boolean {
  if (params.has(LEGACY_RENDER_VIEWPORT_KEY)) {
    if (!hasSingleValue(params, LEGACY_RENDER_VIEWPORT_KEY)) return false;
    const mode = params.get(LEGACY_RENDER_VIEWPORT_KEY);
    if (mode !== "canonical" && mode !== "host") return false;
  }

  const presentLockKeys = LEGACY_HOST_LOCK_KEYS.filter((key) =>
    params.has(key),
  );
  if (presentLockKeys.length === 0) return true;
  if (
    presentLockKeys.length !== LEGACY_HOST_LOCK_KEYS.length ||
    LEGACY_HOST_LOCK_KEYS.some((key) => !hasSingleValue(params, key))
  ) {
    return false;
  }

  const width = parsePositiveInteger(params.get("hostWidth"));
  const height = parsePositiveInteger(params.get("hostHeight"));
  const screenAvailableWidth = parsePositiveInteger(
    params.get("screenAvailableWidth"),
  );
  if (width === null || height === null || screenAvailableWidth === null) {
    return false;
  }
  try {
    validateRealmViewport({ width, height });
    validateScreenAvailableWidth(screenAvailableWidth);
    return true;
  } catch {
    return false;
  }
}

function decodeOptionalBoolean(
  params: URLSearchParams,
  key: string,
  fallback: boolean,
): boolean | null {
  if (!params.has(key)) return fallback;
  if (!hasSingleValue(params, key)) return null;
  const value = params.get(key);
  if (value === "true") return true;
  if (value === "false") return false;
  return null;
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
  input: string | URLSearchParams | Pick<Location, "search">,
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
