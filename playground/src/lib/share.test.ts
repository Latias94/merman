import assert from "node:assert/strict";
import test from "node:test";

import {
  copyShareUrl,
  decodeShareHash,
  encodeShareHash,
  migrateLegacyHostTheme,
  SHARE_LIMITS,
  type ShareCommandEnvironment,
} from "./share.ts";
import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  useAppStore,
  type WorkspaceSnapshot,
} from "../store/index.ts";

const COMPLETE_SNAPSHOT: WorkspaceSnapshot = {
  code: "flowchart TD\nA --> B",
  mermaidConfig: '{"look":"neo"}',
  diagramTheme: "forest",
  diagramFont: "arial",
  presentationProfileId: "future-profile",
  presentationThemePresetId: "future-theme",
  svgPipeline: "readable",
  textMeasurementMode: "headless",
};

test("round-trips one complete workspace snapshot", () => {
  const hash = encodeShareHash(COMPLETE_SNAPSHOT);
  assert.deepEqual(decodeShareHash(hash), COMPLETE_SNAPSHOT);

  const raw = JSON.parse(decodeURIComponent(atob(hash))) as Record<string, unknown>;
  assert.equal("hostThemePreset" in raw, false);
});

test("round-trips Unicode without changing byte-oriented validation", () => {
  const snapshot = {
    ...COMPLETE_SNAPSHOT,
    code: "flowchart TD\nA[中文] --> B[日本語]",
    mermaidConfig: '{"title":"café"}',
  };
  assert.deepEqual(decodeShareHash(encodeShareHash(snapshot)), snapshot);
});

test("migrates legacy host theme values into a complete defaulted snapshot", () => {
  const cases = [
    ["editor-light", "editor-light", null, "resvg-safe"],
    ["merman-modern", null, "merman-modern", "parity"],
    ["none", null, null, "parity"],
    ["mermaid", null, null, "parity"],
    ["future-theme", "future-theme", null, "parity"],
  ] as const;

  for (const [legacy, themePreset, profile, pipeline] of cases) {
    assert.deepEqual(decodeShareHash(legacyHash(legacy)), {
      ...DEFAULT_WORKSPACE_SNAPSHOT,
      code: "flowchart TD\nA",
      presentationProfileId: profile,
      presentationThemePresetId: themePreset,
      svgPipeline: pipeline,
    });
  }
});

test("keeps the legacy presentation migration callable as a pure contract", () => {
  assert.deepEqual(migrateLegacyHostTheme("editor-light"), {
    presentationProfileId: null,
    presentationThemePresetId: "editor-light",
    svgPipeline: "resvg-safe",
  });
});

test("prefers present current fields and defaults omitted optional fields", () => {
  assert.deepEqual(
    decodeShareHash(
      encodedPayload({
        code: "flowchart TD\nA",
        theme: "default",
        hostThemePreset: "editor-light",
        presentationProfileId: "future-profile",
      })
    ),
    {
      ...DEFAULT_WORKSPACE_SNAPSHOT,
      code: "flowchart TD\nA",
      presentationProfileId: "future-profile",
    }
  );
});

test("inherits caller defaults when every optional presentation field is absent", () => {
  const defaults: WorkspaceSnapshot = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    presentationProfileId: "default-profile",
    presentationThemePresetId: "default-theme",
    svgPipeline: "readable",
  };
  assert.deepEqual(
    decodeShareHash(
      encodedPayload({ code: "flowchart TD\nA", theme: "forest" }),
      defaults
    ),
    { ...defaults, code: "flowchart TD\nA", diagramTheme: "forest" }
  );
});

test("rejects an invalid required or present optional field as one payload", () => {
  const valid = {
    code: "flowchart TD\nA",
    theme: "default",
  };
  const invalidPayloads = [
    { ...valid, code: 42 },
    { ...valid, theme: "future-mermaid-theme" },
    { ...valid, config: false },
    { ...valid, svgPipeline: "future-pipeline" },
    { ...valid, textMeasurementMode: "approximate" },
    { ...valid, diagramFont: "comic-sans" },
    { ...valid, presentationProfileId: "" },
    { ...valid, presentationThemePresetId: false },
    { ...valid, hostThemePreset: "" },
  ];

  for (const payload of invalidPayloads) {
    assert.equal(decodeShareHash(encodedPayload(payload)), null);
  }
});

test("rejects source, config, and total payloads beyond shared byte budgets", () => {
  assert.equal(
    decodeShareHash(
      encodedPayload({
        code: "x".repeat(SHARE_LIMITS.sourceBytes + 1),
        theme: "default",
      })
    ),
    null
  );
  assert.equal(
    decodeShareHash(
      encodedPayload({
        code: "flowchart TD\nA",
        config: "x".repeat(SHARE_LIMITS.configBytes + 1),
        theme: "default",
      })
    ),
    null
  );
  assert.equal(
    decodeShareHash(
      encodedPayload({
        code: "flowchart TD\nA",
        config: "{}",
        theme: "default",
        ignored: "x".repeat(SHARE_LIMITS.jsonBytes),
      })
    ),
    null
  );
});

test("rejects oversized current and legacy presentation IDs", () => {
  const oversizedId = "x".repeat(SHARE_LIMITS.idBytes + 1);
  assert.equal(
    decodeShareHash(
      encodedPayload({
        code: "flowchart TD\nA",
        theme: "default",
        presentationProfileId: oversizedId,
      })
    ),
    null
  );
  assert.equal(decodeShareHash(legacyHash(oversizedId)), null);
});

test("refuses to serialize a workspace that cannot be decoded", () => {
  assert.throws(
    () =>
      encodeShareHash({
        ...COMPLETE_SNAPSHOT,
        code: "x".repeat(SHARE_LIMITS.sourceBytes + 1),
      }),
    /share URL contract/u
  );
});

test("copy is a pure supplied-snapshot command and updates history after clipboard", async () => {
  const events: string[] = [];
  let storeNotifications = 0;
  const unsubscribe = useAppStore.subscribe(() => {
    storeNotifications += 1;
  });
  const environment: ShareCommandEnvironment = {
    origin: "https://example.test",
    pathname: "/merman/",
    async writeClipboardText(value) {
      events.push(`clipboard:${value}`);
    },
    replaceUrl(value) {
      events.push(`history:${value}`);
    },
  };

  await copyShareUrl(COMPLETE_SNAPSHOT, environment);
  await copyShareUrl(COMPLETE_SNAPSHOT, environment);
  unsubscribe();

  assert.equal(events.length, 4);
  assert.match(events[0] ?? "", /^clipboard:https:\/\/example\.test\/merman\/#/u);
  assert.equal(events[1], events[0]?.replace("clipboard:", "history:"));
  assert.equal(events[2], events[0]);
  assert.equal(events[3], events[1]);
  assert.equal(storeNotifications, 0);
});

test("does not update history when clipboard permission fails", async () => {
  let historyUpdates = 0;
  await assert.rejects(
    copyShareUrl(COMPLETE_SNAPSHOT, {
      origin: "https://example.test",
      pathname: "/merman/",
      async writeClipboardText() {
        throw new Error("denied");
      },
      replaceUrl() {
        historyUpdates += 1;
      },
    }),
    /denied/u
  );
  assert.equal(historyUpdates, 0);
});

test("malformed Base64, URI encoding, and JSON fail closed", () => {
  assert.equal(decodeShareHash("#not base64"), null);
  assert.equal(decodeShareHash(btoa("%E0%A4%A")), null);
  assert.equal(decodeShareHash(btoa(encodeURIComponent("not-json"))), null);
});

function legacyHash(
  hostThemePreset: string,
  extra: Record<string, unknown> = {}
): string {
  return encodedPayload({
    code: "flowchart TD\nA",
    theme: "default",
    hostThemePreset,
    ...extra,
  });
}

function encodedPayload(payload: Record<string, unknown>): string {
  return btoa(encodeURIComponent(JSON.stringify(payload)));
}
