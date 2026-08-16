import assert from "node:assert/strict";
import test from "node:test";

import {
  copyIssueShareUrl,
  createIssueShareUrl,
  decodeShareView,
  hydrateStartupShareLocation,
  SHARE_VIEW_DEFAULTS,
  SHARE_VIEW_LIMITS,
  type ShareViewDescriptor,
  type StartupShareHydration,
} from "./share-view.ts";
import { encodeShareHash, type ShareCommandEnvironment } from "./share.ts";
import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";

const HOST_WORKSPACE: WorkspaceSnapshot = {
  code: "flowchart TD\nA --> B",
  mermaidConfig: '{"look":"neo"}',
  diagramTheme: "forest",
  diagramFont: "arial",
  presentationProfileId: "future-profile",
  presentationThemePresetId: "future-theme",
  renderViewportMode: "host",
  svgPipeline: "readable",
  textMeasurementMode: "headless",
};

const LOCKED_VIEW: ShareViewDescriptor = {
  workspacePane: "preview",
  editorMode: "config",
  previewMode: "compare",
  lockedEnvironment: {
    width: 640,
    height: 480,
    screenAvailableWidth: 1512,
  },
};

test("distinguishes an absent view from a valid rv=1 descriptor", () => {
  assert.deepEqual(decodeShareView("?utm_source=issue"), {
    status: "absent",
    view: null,
    warning: null,
  });
  assert.deepEqual(
    decodeShareView(
      "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare"
    ),
    {
      status: "valid",
      view: { ...LOCKED_VIEW, lockedEnvironment: null },
      warning: null,
    }
  );
});

test("round-trips one complete locked Host view in a canonical issue URL", () => {
  const currentLocation = {
    origin: "https://example.test",
    pathname: "/merman/",
    search: "?utm_source=current&rv=99",
  };
  const url = createIssueShareUrl(HOST_WORKSPACE, LOCKED_VIEW, currentLocation);
  const parsed = new URL(url);

  assert.equal(parsed.origin, currentLocation.origin);
  assert.equal(parsed.pathname, currentLocation.pathname);
  assert.equal(parsed.searchParams.get("utm_source"), null);
  assert.equal(parsed.hash, encodeShareHash(HOST_WORKSPACE));
  assert.deepEqual(decodeShareView(parsed.search), {
    status: "valid",
    view: LOCKED_VIEW,
    warning: null,
  });
  assert.equal(parsed.searchParams.has("renderViewportMode"), false);
});

test("rejects an invalid or future view atomically with one warning signal", () => {
  const tooWide = SHARE_VIEW_LIMITS.screenAvailableWidth + 1;
  const invalidQueries = [
    "?rv=2&workspacePane=preview&editorMode=config&previewMode=compare",
    "?rv=1&workspacePane=preview&editorMode=config",
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=640",
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=-1&hostHeight=480&screenAvailableWidth=1512",
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=Infinity&hostHeight=480&screenAvailableWidth=1512",
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=4097&hostHeight=480&screenAvailableWidth=1512",
    `?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=640&hostHeight=480&screenAvailableWidth=${tooWide}`,
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&previewMode=svg",
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&renderViewportMode=host",
  ];

  for (const query of invalidQueries) {
    const result = decodeShareView(query);
    assert.equal(result.status, "invalid", query);
    assert.equal(result.view, null, query);
    assert.equal(result.warning?.code, "share-view-not-restored", query);
  }
});

test("does not permit a Host lock when the shared workspace is canonical", () => {
  assert.throws(
    () =>
      createIssueShareUrl(
        { ...HOST_WORKSPACE, renderViewportMode: "canonical" },
        LOCKED_VIEW,
        { origin: "https://example.test", pathname: "/merman/" }
      ),
    /Host workspace/u
  );
});

test("creates a canonical issue view without inventing a Host lock", () => {
  const view = { ...LOCKED_VIEW, lockedEnvironment: null };
  const url = new URL(
    createIssueShareUrl(
      { ...HOST_WORKSPACE, renderViewportMode: "canonical" },
      view,
      { origin: "https://example.test", pathname: "/merman/" }
    )
  );
  assert.deepEqual(decodeShareView(url.search), {
    status: "valid",
    view,
    warning: null,
  });
  assert.equal(url.searchParams.has("hostWidth"), false);
});

test("issue copying writes only the selected canonical URL", async () => {
  const copied: string[] = [];
  let historyUpdates = 0;
  const environment = {
    origin: "https://example.test",
    pathname: "/merman/",
    async writeClipboardText(value) {
      copied.push(value);
    },
    replaceUrl() {
      historyUpdates += 1;
    },
  } satisfies ShareCommandEnvironment & { replaceUrl(value: string): void };

  await copyIssueShareUrl(HOST_WORKSPACE, LOCKED_VIEW, environment);
  assert.deepEqual(copied, [
    createIssueShareUrl(HOST_WORKSPACE, LOCKED_VIEW, environment),
  ]);
  assert.equal(historyUpdates, 0);
});

test("hydrates workspace, view, and lock through one pre-mount apply call", () => {
  const issueUrl = new URL(
    createIssueShareUrl(HOST_WORKSPACE, LOCKED_VIEW, {
      origin: "https://example.test",
      pathname: "/merman/",
    })
  );
  const applied: StartupShareHydration[] = [];

  const result = hydrateStartupShareLocation(issueUrl, (hydration) => {
    applied.push(hydration);
  });

  assert.equal(result.status, "applied");
  assert.deepEqual(applied, [
    {
      workspace: HOST_WORKSPACE,
      view: LOCKED_VIEW,
      warning: null,
    },
  ]);
});

test("keeps a valid workspace but defaults the complete invalid view layer", () => {
  const applied: StartupShareHydration[] = [];
  const result = hydrateStartupShareLocation(
    {
      hash: encodeShareHash(HOST_WORKSPACE),
      search:
        "?rv=2&workspacePane=preview&editorMode=config&previewMode=compare",
    },
    (hydration) => applied.push(hydration)
  );

  assert.equal(result.status, "applied");
  assert.equal(result.warning?.code, "share-view-not-restored");
  assert.deepEqual(applied, [
    {
      workspace: HOST_WORKSPACE,
      view: SHARE_VIEW_DEFAULTS,
      warning: result.warning,
    },
  ]);
});

test("writes nothing when the workspace fragment is invalid", () => {
  let applyCalls = 0;
  const result = hydrateStartupShareLocation(
    {
      hash: "#s2:not-zlib",
      search: "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare",
    },
    () => {
      applyCalls += 1;
    }
  );

  assert.deepEqual(result, { status: "ignored", warning: null });
  assert.equal(applyCalls, 0);
});
