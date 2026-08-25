import assert from "node:assert/strict";
import test from "node:test";
import { unzlibSync, zlibSync } from "fflate";

import {
  copyShareUrl,
  createWorkspaceShareUrl,
  decodeShareHash,
  encodeShareHash,
  SHARE_LIMITS,
  WORKSPACE_V2_DEFAULTS,
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
  themePresetId: "future-theme",
  svgPipeline: "readable",
  textMeasurementMode: "headless",
};

test("round-trips one complete workspace snapshot through the s2 envelope", () => {
  const hash = encodeShareHash(COMPLETE_SNAPSHOT);
  assert.match(hash, /^#s2:[A-Za-z0-9_-]+$/u);
  assert.deepEqual(decodeShareHash(hash), COMPLETE_SNAPSHOT);
  assert.equal(Object.hasOwn(decodeS2Payload(hash), "renderViewportMode"), false);
});

test("keeps the complete v2 defaults immutable and independent from caller defaults", () => {
  assert.equal(Object.isFrozen(WORKSPACE_V2_DEFAULTS), true);
  assert.deepEqual(WORKSPACE_V2_DEFAULTS, DEFAULT_WORKSPACE_SNAPSHOT);

  const callerDefaults = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    diagramTheme: "forest" as const,
  };
  assert.deepEqual(
    decodeShareHash(s2Payload({}), callerDefaults),
    WORKSPACE_V2_DEFAULTS
  );
});

test("round-trips Unicode without changing byte-oriented validation", () => {
  const snapshot = {
    ...COMPLETE_SNAPSHOT,
    code: "flowchart TD\nA[中文] --> B[日本語]",
    mermaidConfig: '{"title":"café"}',
  };
  assert.deepEqual(decodeShareHash(encodeShareHash(snapshot)), snapshot);
});

test("rejects the removed alpha host-theme share field", () => {
  assert.equal(
    decodeShareHash(
      encodedPayload({
        code: "flowchart TD\nA",
        theme: "default",
        hostThemePreset: "editor-light",
      }),
    ),
    null,
  );
});

test("uses present current fields and defaults omitted optional fields", () => {
  assert.deepEqual(
    decodeShareHash(
      encodedPayload({
        code: "flowchart TD\nA",
        theme: "default",
        themePresetId: "future-theme",
      }),
    ),
    {
      ...DEFAULT_WORKSPACE_SNAPSHOT,
      code: "flowchart TD\nA",
      themePresetId: "future-theme",
    },
  );
});

test("inherits caller defaults when every optional render field is absent", () => {
  const defaults: WorkspaceSnapshot = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    themePresetId: "default-theme",
    svgPipeline: "readable",
  };
  assert.deepEqual(
    decodeShareHash(
      encodedPayload({ code: "flowchart TD\nA", theme: "forest" }),
      defaults,
    ),
    { ...defaults, code: "flowchart TD\nA", diagramTheme: "forest" },
  );
});

test("ignores validated Host viewport state in legacy Base64 and s2 payloads", () => {
  const expected = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    code: "flowchart TD\nA",
  };

  assert.deepEqual(
    decodeShareHash(
      encodedPayload({
        code: expected.code,
        theme: "default",
        renderViewportMode: "host",
      }),
    ),
    expected,
  );
  assert.deepEqual(
    decodeShareHash(
      s2Payload({
        code: expected.code,
        renderViewportMode: "host",
      }),
    ),
    expected,
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
    { ...valid, renderViewportMode: "fluid" },
    { ...valid, themePresetId: false },
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
      }),
    ),
    null,
  );
  assert.equal(
    decodeShareHash(
      encodedPayload({
        code: "flowchart TD\nA",
        config: "x".repeat(SHARE_LIMITS.configBytes + 1),
        theme: "default",
      }),
    ),
    null,
  );
  assert.equal(
    decodeShareHash(
      encodedPayload({
        code: "flowchart TD\nA",
        config: "{}",
        theme: "default",
        ignored: "x".repeat(SHARE_LIMITS.jsonBytes),
      }),
    ),
    null,
  );
});

test("rejects an oversized current theme preset ID", () => {
  const oversizedId = "x".repeat(SHARE_LIMITS.idBytes + 1);
  assert.equal(
    decodeShareHash(
      encodedPayload({
        code: "flowchart TD\nA",
        theme: "default",
        themePresetId: oversizedId,
      }),
    ),
    null,
  );
});

test("refuses to serialize a workspace that cannot be decoded", () => {
  assert.throws(
    () =>
      encodeShareHash({
        ...COMPLETE_SNAPSHOT,
        code: "x".repeat(SHARE_LIMITS.sourceBytes + 1),
      }),
    /share URL contract/u,
  );
});

test("bounds both the encoded s2 envelope and streamed decompression", () => {
  assert.equal(
    decodeShareHash(`#s2:${"A".repeat(SHARE_LIMITS.encodedBytes + 1)}`),
    null
  );

  assert.throws(
    () =>
      encodeShareHash({
        ...COMPLETE_SNAPSHOT,
        code: deterministicNoise(700 * 1024),
      }),
    /share URL contract/u
  );

  const decompressionBomb = s2Payload({
    ignored: "x".repeat(SHARE_LIMITS.jsonBytes),
  });
  assert.ok(decompressionBomb.length < SHARE_LIMITS.encodedBytes);
  assert.equal(decodeShareHash(decompressionBomb), null);
});

test("keeps representative links shorter than legacy and compresses repetition by half", () => {
  const representativeV2 = encodeShareHash(COMPLETE_SNAPSHOT);
  const representativeLegacy = `#${legacySnapshotHash(COMPLETE_SNAPSHOT)}`;
  assert.ok(representativeV2.length <= representativeLegacy.length);

  const repetitiveSnapshot = {
    ...COMPLETE_SNAPSHOT,
    code: `flowchart TD\n${"A --> B\n".repeat(13_000)}`,
  };
  assert.ok(new TextEncoder().encode(repetitiveSnapshot.code).length >= 100 * 1024);
  const repetitiveV2 = encodeShareHash(repetitiveSnapshot);
  const repetitiveLegacy = `#${legacySnapshotHash(repetitiveSnapshot)}`;
  assert.ok(repetitiveV2.length <= repetitiveLegacy.length / 2);
});

test("copy is a pure supplied-snapshot command that only writes the clipboard", async () => {
  const events: string[] = [];
  let historyUpdates = 0;
  let storeNotifications = 0;
  const unsubscribe = useAppStore.subscribe(() => {
    storeNotifications += 1;
  });
  const environment = {
    origin: "https://example.test",
    pathname: "/merman/",
    async writeClipboardText(value) {
      events.push(`clipboard:${value}`);
    },
    replaceUrl() {
      historyUpdates += 1;
    },
  } satisfies ShareCommandEnvironment & { replaceUrl(value: string): void };

  await copyShareUrl(COMPLETE_SNAPSHOT, environment);
  await copyShareUrl(COMPLETE_SNAPSHOT, environment);
  unsubscribe();

  assert.equal(events.length, 2);
  assert.match(
    events[0] ?? "",
    /^clipboard:https:\/\/example\.test\/merman\/#s2:/u
  );
  assert.equal(events[1], events[0]);
  assert.equal(historyUpdates, 0);
  assert.equal(storeNotifications, 0);
});

test("propagates clipboard permission failures without another side effect", async () => {
  await assert.rejects(
    copyShareUrl(COMPLETE_SNAPSHOT, {
      origin: "https://example.test",
      pathname: "/merman/",
      async writeClipboardText() {
        throw new Error("denied");
      },
    }),
    /denied/u,
  );
});

test("creates a canonical workspace URL without inheriting a current query", () => {
  assert.equal(
    createWorkspaceShareUrl(COMPLETE_SNAPSHOT, {
      origin: "https://example.test",
      pathname: "/merman/",
    }),
    `https://example.test/merman/${encodeShareHash(COMPLETE_SNAPSHOT)}`
  );
});

test("malformed Base64, URI encoding, and JSON fail closed", () => {
  assert.equal(decodeShareHash("#not base64"), null);
  assert.equal(decodeShareHash(btoa("%E0%A4%A")), null);
  assert.equal(decodeShareHash(btoa(encodeURIComponent("not-json"))), null);
});

function encodedPayload(payload: Record<string, unknown>): string {
  return btoa(encodeURIComponent(JSON.stringify(payload)));
}

function legacySnapshotHash(snapshot: WorkspaceSnapshot): string {
  return encodedPayload({
    code: snapshot.code,
    theme: snapshot.diagramTheme,
    config: snapshot.mermaidConfig,
    themePresetId: snapshot.themePresetId,
    renderViewportMode: "host",
    svgPipeline: snapshot.svgPipeline,
    textMeasurementMode: snapshot.textMeasurementMode,
    diagramFont: snapshot.diagramFont,
  });
}

function decodeS2Payload(hash: string): Record<string, unknown> {
  const encoded = hash.slice("#s2:".length);
  const normalized = encoded.replaceAll("-", "+").replaceAll("_", "/");
  const binary = atob(
    normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "="),
  );
  const compressed = Uint8Array.from(binary, (character) =>
    character.charCodeAt(0),
  );
  return JSON.parse(new TextDecoder().decode(unzlibSync(compressed))) as Record<
    string,
    unknown
  >;
}

function s2Payload(payload: Record<string, unknown>): string {
  const compressed = zlibSync(new TextEncoder().encode(JSON.stringify(payload)));
  let binary = "";
  for (const byte of compressed) binary += String.fromCharCode(byte);
  return `#s2:${btoa(binary)
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replace(/=+$/u, "")}`;
}

function deterministicNoise(length: number): string {
  const characters = new Array<string>(length);
  let state = 0x12345678;
  for (let index = 0; index < length; index += 1) {
    state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
    characters[index] = String.fromCharCode(32 + (state % 95));
  }
  return characters.join("");
}
