import assert from "node:assert/strict";
import test from "node:test";

import {
  copyIssueShareUrl,
  createIssueShareUrl,
  decodeShareView,
  hydrateStartupShareLocation,
  SHARE_VIEW_DEFAULTS,
  type ShareViewDescriptor,
  type StartupShareHydration,
} from "./share-view.ts";
import {
  encodeShareHash,
  type ShareCommandEnvironment,
} from "./share.ts";
import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";
import { REALM_BUDGETS } from "../runtime/realm/channel-protocol.ts";

const WORKSPACE: WorkspaceSnapshot = {
  code: "flowchart TD\nA --> B",
  mermaidConfig: '{"look":"neo"}',
  diagramTheme: "forest",
  diagramFont: "arial",
  presentationProfileId: "future-profile",
  presentationThemePresetId: "future-theme",
  svgPipeline: "readable",
  textMeasurementMode: "headless",
};

const VIEW: ShareViewDescriptor = {
  workspacePane: "preview",
  editorMode: "config",
  previewMode: "compare",
  showSvgBounds: true,
};

test("distinguishes an absent view from current and pre-Bounds rv=1 descriptors", () => {
  assert.deepEqual(decodeShareView("?utm_source=issue"), {
    status: "absent",
    view: null,
    warning: null,
  });
  assert.deepEqual(
    decodeShareView(
      "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare",
    ),
    {
      status: "valid",
      view: { ...VIEW, showSvgBounds: false },
      warning: null,
    },
  );
});

test("round-trips pane, editor, Preview mode, and SVG Bounds without Host keys", () => {
  const url = createIssueShareUrl(WORKSPACE, VIEW, {
    origin: "https://example.test",
    pathname: "/merman/",
  });
  const parsed = new URL(url);

  assert.equal(parsed.hash, encodeShareHash(WORKSPACE));
  assert.deepEqual(decodeShareView(parsed.search), {
    status: "valid",
    view: VIEW,
    warning: null,
  });
  for (const key of [
    "renderViewportMode",
    "hostWidth",
    "hostHeight",
    "screenAvailableWidth",
  ]) {
    assert.equal(parsed.searchParams.has(key), false, key);
  }
});

test("validates and ignores a complete legacy rv=1 Host lock without warning", () => {
  assert.deepEqual(
    decodeShareView(
      "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=640&hostHeight=480&screenAvailableWidth=1512",
    ),
    {
      status: "valid",
      view: { ...VIEW, showSvgBounds: false },
      warning: null,
    },
  );
});

test("rejects malformed or future view state atomically", () => {
  const tooWide = REALM_BUDGETS.maxScreenAvailableWidth + 1;
  const invalidQueries = [
    "?rv=2&workspacePane=preview&editorMode=config&previewMode=compare",
    "?rv=1&workspacePane=preview&editorMode=config",
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&showSvgBounds=maybe",
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=640",
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=-1&hostHeight=480&screenAvailableWidth=1512",
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=4097&hostHeight=480&screenAvailableWidth=1512",
    `?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=640&hostHeight=480&screenAvailableWidth=${tooWide}`,
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&previewMode=svg",
    "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&renderViewportMode=fluid",
  ];

  for (const query of invalidQueries) {
    const result = decodeShareView(query);
    assert.equal(result.status, "invalid", query);
    assert.equal(result.view, null, query);
    assert.equal(result.warning?.code, "share-view-not-restored", query);
  }
});

test("issue copying is a pure clipboard command", async () => {
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

  await copyIssueShareUrl(WORKSPACE, VIEW, environment);

  assert.deepEqual(copied, [createIssueShareUrl(WORKSPACE, VIEW, environment)]);
  assert.equal(historyUpdates, 0);
});

test("hydrates workspace and current view through one pre-mount apply call", () => {
  const issueUrl = new URL(
    createIssueShareUrl(WORKSPACE, VIEW, {
      origin: "https://example.test",
      pathname: "/merman/",
    }),
  );
  const applied: StartupShareHydration[] = [];

  const result = hydrateStartupShareLocation(issueUrl, (hydration) => {
    applied.push(hydration);
  });

  assert.equal(result.status, "applied");
  assert.deepEqual(applied, [
    {
      workspace: WORKSPACE,
      view: VIEW,
      warning: null,
    },
  ]);
});

test("hydrates a legacy Host issue link canonically without an avoidable warning", () => {
  const applied: StartupShareHydration[] = [];
  const result = hydrateStartupShareLocation(
    {
      hash: encodeShareHash(WORKSPACE),
      search:
        "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare&hostWidth=640&hostHeight=480&screenAvailableWidth=1512",
    },
    (hydration) => applied.push(hydration),
  );

  assert.deepEqual(result, { status: "applied", warning: null });
  assert.deepEqual(applied, [
    {
      workspace: WORKSPACE,
      view: { ...VIEW, showSvgBounds: false },
      warning: null,
    },
  ]);
});

test("keeps a valid workspace but defaults one invalid view layer", () => {
  const applied: StartupShareHydration[] = [];
  const result = hydrateStartupShareLocation(
    {
      hash: encodeShareHash(WORKSPACE),
      search:
        "?rv=2&workspacePane=preview&editorMode=config&previewMode=compare",
    },
    (hydration) => applied.push(hydration),
  );

  assert.equal(result.status, "applied");
  assert.equal(result.warning?.code, "share-view-not-restored");
  assert.deepEqual(applied, [
    {
      workspace: WORKSPACE,
      view: SHARE_VIEW_DEFAULTS,
      warning: result.warning,
    },
  ]);
});

test("keeps an absent issue view as the local default", () => {
  const applied: StartupShareHydration[] = [];
  const result = hydrateStartupShareLocation(
    {
      hash: encodeShareHash(WORKSPACE),
      search: "?utm_source=workspace",
    },
    (hydration) => applied.push(hydration),
  );

  assert.deepEqual(result, { status: "applied", warning: null });
  assert.deepEqual(applied, [
    {
      workspace: WORKSPACE,
      view: SHARE_VIEW_DEFAULTS,
      warning: null,
    },
  ]);
});

test("writes nothing when the workspace fragment is invalid", () => {
  let applyCalls = 0;
  const result = hydrateStartupShareLocation(
    {
      hash: "#s2:not-zlib",
      search:
        "?rv=1&workspacePane=preview&editorMode=config&previewMode=compare",
    },
    () => {
      applyCalls += 1;
    },
  );

  assert.deepEqual(result, { status: "ignored", warning: null });
  assert.equal(applyCalls, 0);
});
