import assert from "node:assert/strict";
import test from "node:test";
import { unzlibSync, zlibSync } from "fflate";
import { resolveThemeSelection } from "./theme-appearance.ts";
import { themeCatalogFixture } from "./theme-appearance.test-fixture.ts";

import {
  copyShareUrl,
  createWorkspaceShareUrl,
  decodeShareHash,
  encodeShareHash,
  SHARE_LIMITS,
  SHARE_THEME_RECIPE_BYTES,
  SHARE_V3_LIMITS,
  WORKSPACE_V2_DEFAULTS,
  WORKSPACE_V3_DEFAULTS,
  type ShareCommandEnvironment,
} from "./share.ts";
import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  selectWorkspaceSnapshot,
  useAppStore,
  type WorkspaceSnapshot,
} from "../store/index.ts";

const COMPLETE_SNAPSHOT: WorkspaceSnapshot = {
  code: "flowchart TD\nA --> B",
  mermaidConfig: '{"look":"neo"}',
  diagramTheme: "forest",
  diagramFont: "arial",
  themePresetId: "future-theme",
  themeRecipeJson: null,
  svgPipeline: "readable",
  textMeasurementMode: "headless",
};

test("round-trips one complete workspace snapshot through the s2 envelope", () => {
  const hash = encodeShareHash(COMPLETE_SNAPSHOT);
  assert.match(hash, /^#s2:[A-Za-z0-9_-]+$/u);
  assert.deepEqual(decodeShareHash(hash), COMPLETE_SNAPSHOT);
  assert.equal(Object.hasOwn(decodeS2Payload(hash), "renderViewportMode"), false);
});

const CUSTOM_THEME_RECIPE = JSON.stringify({
  schema_version: 1,
  kind: "design",
  design: { tokens: { primaryColor: { type: "color", value: "#ff66aa" } } },
});

test("s5 saves host preference without capturing the sender's application appearance", () => {
  const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, diagramTheme: "neo" as const, themeAppearance: "follow" as const };
  const hash = encodeShareHash(workspace);
  assert.match(hash, /^#s5:/);
  const shared = decodeShareHash(hash)!;
  assert.deepEqual(shared, workspace);
  const catalog = themeCatalogFixture();
  assert.equal(resolveThemeSelection(shared, "light", catalog).mermaidTheme, "neo");
  assert.equal(resolveThemeSelection(shared, "dark", catalog).mermaidTheme, "neo-dark");
  for (const prefix of ["#s2:", "#s3:", "#s4:"]) assert.equal(decodeShareHash(hash.replace("#s5:", prefix)), null);
  const historical = decodeShareHash(s2Payload({ themeSelection: "auto" }))!;
  assert.equal(historical.themeAppearance, undefined);
  assert.equal(resolveThemeSelection(historical, "dark", catalog).mermaidTheme, "auto");
});

test("s5 preserves the complete unique-v1 recipe, original bytes and customization provenance", () => {
  const raw = '{"schema_version":1,"kind":"design","design":{"tokens":{},"appearances":{"light":{},"dark":{}},"default_appearance":"light"}}\n';
  const value = {
    ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: raw, themeAppearance: "dark" as const,
    themeRecipeLabel: { name: "Editor copy", source: "imported" as const, derivedFrom: "preset:editor-dark" },
  };
  assert.deepEqual(decodeShareHash(encodeShareHash(value)), value);
  const duplicated = { ...value, themeRecipeJson: raw.replace('"tokens":{}', '"tokens":{},"tokens":{}') };
  assert.equal(decodeShareHash(encodeShareHash(duplicated))?.themeRecipeJson, duplicated.themeRecipeJson);
  for (const invalid of ["auto", "system", "sepia", null, 1, {}]) {
    const hash = s2Payload({ themeAppearance: invalid }).replace("#s2:", "#s5:");
    assert.equal(decodeShareHash(hash), null);
  }
});

test("round-trips a custom recipe through s3 without changing s2 links", () => {
  const snapshot = {
    ...COMPLETE_SNAPSHOT,
    themePresetId: null,
    themeRecipeJson: CUSTOM_THEME_RECIPE,
  };
  const hash = encodeShareHash(snapshot);
  assert.match(hash, /^#s3:[A-Za-z0-9_-]+$/u);
  assert.deepEqual(decodeShareHash(hash), snapshot);
  assert.equal(decodeShareHash(s2Payload({}))?.themeRecipeJson, null);
  assert.match(encodeShareHash(COMPLETE_SNAPSHOT), /^#s2:/u);
  assert.equal(
    decodeShareHash(s2Payload({ themeRecipeJson: CUSTOM_THEME_RECIPE })),
    null,
  );
});

test("named recipes retain their identity in s4 while old envelopes stay strict", () => {
  const snapshot: WorkspaceSnapshot = {
    ...COMPLETE_SNAPSHOT, themePresetId: null, themeRecipeJson: CUSTOM_THEME_RECIPE,
    themeRecipeLabel: { name: "Nightfall", source: "exploration" },
  };
  const hash = encodeShareHash(snapshot);
  assert.match(hash, /^#s4:[A-Za-z0-9_-]+$/u);
  assert.deepEqual(decodeShareHash(hash), snapshot);
  assert.equal(decodeShareHash(hash.replace("#s4:", "#s3:")), null);
  useAppStore.getState().applyWorkspaceSnapshot(snapshot);
  assert.equal(selectWorkspaceSnapshot(useAppStore.getState()).themeRecipeLabel?.name, "Nightfall");
  useAppStore.getState().applyWorkspaceSnapshot(COMPLETE_SNAPSHOT);
  assert.equal(selectWorkspaceSnapshot(useAppStore.getState()).themeRecipeLabel, undefined);
});

test("share metadata cannot claim a name without a recipe or exceed its display boundary", () => {
  for (const label of [
    { name: " ", source: "imported" },
    { name: "x".repeat(257), source: "imported" },
    { name: "Theme", source: "future" },
    { name: "Theme", source: "exploration", trusted: true },
  ]) {
    assert.throws(() => encodeShareHash({
      ...COMPLETE_SNAPSHOT, themePresetId: null, themeRecipeJson: CUSTOM_THEME_RECIPE,
      themeRecipeLabel: label as WorkspaceSnapshot["themeRecipeLabel"],
    }), /share URL contract/u);
  }
  assert.throws(() => encodeShareHash({ ...COMPLETE_SNAPSHOT, themeRecipeLabel: { name: "Theme", source: "imported" } }), /share URL contract/u);
});


test("s3 defaults do not inherit later application or caller theme choices", () => {
  assert.equal(Object.isFrozen(WORKSPACE_V3_DEFAULTS), true);
  assert.notEqual(WORKSPACE_V3_DEFAULTS, WORKSPACE_V2_DEFAULTS);
  const callerDefaults = {
    ...COMPLETE_SNAPSHOT,
    themePresetId: null,
    themeRecipeJson: CUSTOM_THEME_RECIPE,
  };
  assert.deepEqual(
    decodeShareHash(s3Payload({}), callerDefaults),
    WORKSPACE_V3_DEFAULTS,
  );
  assert.equal(
    decodeShareHash(encodedPayload({
      code: "flowchart LR\nA --> B",
      theme: "default",
    }), callerDefaults)?.themeRecipeJson,
    null,
  );
});

test("rejects mixed preset and custom recipe selections on encode and decode", () => {
  assert.throws(
    () =>
      encodeShareHash({
        ...COMPLETE_SNAPSHOT,
        themeRecipeJson: CUSTOM_THEME_RECIPE,
      }),
    /share URL contract/u,
  );
  assert.equal(
    decodeShareHash(s3Payload({
      themePresetId: "brutalist",
      themeRecipeJson: CUSTOM_THEME_RECIPE,
    })),
    null,
  );
});

test("bounds custom recipes by UTF-8 bytes and checks only their envelope", () => {
  const recipeLimit = SHARE_THEME_RECIPE_BYTES;
  assert.equal(recipeLimit, 256 * 1024);
  const base = JSON.stringify({
    schema_version: 1,
    kind: "complete_spec",
    complete_spec: {},
  });
  const maximumRecipe = base + " ".repeat(recipeLimit - base.length);
  const snapshot = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    themeRecipeJson: maximumRecipe,
  };
  assert.deepEqual(decodeShareHash(encodeShareHash(snapshot)), snapshot);
  for (const themeRecipeJson of [
    maximumRecipe + " ",
    base + "中".repeat(Math.ceil(recipeLimit / 3)),
    "null",
    "[]",
    "not-json",
    JSON.stringify({ schema_version: 2, kind: "design" }),
    JSON.stringify({ schema_version: 1, kind: "unknown" }),
  ]) {
    assert.throws(
      () => encodeShareHash({ ...snapshot, themeRecipeJson }),
      /share URL contract/u,
    );
    assert.equal(decodeShareHash(s3Payload({ themeRecipeJson })), null);
  }
});

test("grants the additional decompression budget only to s3", () => {
  const payload = {
    code: "a".repeat(SHARE_LIMITS.sourceBytes),
    config: "b".repeat(SHARE_LIMITS.configBytes),
    themeRecipeJson: CUSTOM_THEME_RECIPE + " ".repeat(128 * 1024),
  };
  assert.notEqual(decodeShareHash(s3Payload(payload)), null);
  assert.equal(decodeShareHash(s2Payload(payload)), null);
  assert.equal(
    decodeShareHash(`#s3:${"A".repeat(SHARE_V3_LIMITS.encodedBytes + 1)}`),
    null,
  );
  assert.equal(
    decodeShareHash(s3Payload({
      ignored: "x".repeat(SHARE_V3_LIMITS.jsonBytes),
    })),
    null,
  );
});

test("keeps the complete v2 defaults immutable and independent from caller defaults", () => {
  assert.equal(Object.isFrozen(WORKSPACE_V2_DEFAULTS), true);
  assert.deepEqual(WORKSPACE_V2_DEFAULTS, { ...DEFAULT_WORKSPACE_SNAPSHOT, diagramFont: "trebuchet" });

  const callerDefaults = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    diagramTheme: "forest" as const,
  };
  assert.deepEqual(
    decodeShareHash(s2Payload({}), callerDefaults),
    WORKSPACE_V2_DEFAULTS
  );
});

test("shares automatic and explicit default appearance without reinterpreting old links", () => {
  for (const diagramTheme of ["auto", "default", "forest"] as const) {
    const snapshot = { ...DEFAULT_WORKSPACE_SNAPSHOT, diagramTheme };
    assert.deepEqual(decodeShareHash(encodeShareHash(snapshot)), snapshot);
  }
  assert.equal(decodeShareHash(s2Payload({ theme: "default" }))?.diagramTheme, "auto");
  assert.equal(decodeShareHash(s2Payload({ theme: "dark" }))?.diagramTheme, "dark");
  assert.equal(decodeShareHash(s2Payload({ theme: "default", themeSelection: "default" })), null);
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
      diagramFont: "trebuchet",
      code: "flowchart TD\nA",
      themePresetId: "future-theme",
    },
  );
});

test("new inherited fonts are explicit while historical links keep Trebuchet", () => {
  for (const snapshot of [
    { ...DEFAULT_WORKSPACE_SNAPSHOT },
    { ...DEFAULT_WORKSPACE_SNAPSHOT, themeRecipeJson: CUSTOM_THEME_RECIPE },
  ]) {
    assert.equal(snapshot.diagramFont, "inherit");
    const hash = encodeShareHash(snapshot);
    assert.deepEqual(decodeShareHash(hash), snapshot);
    const encoded = hash.slice(hash.indexOf(":") + 1);
    const payload = JSON.parse(new TextDecoder().decode(unzlibSync(Buffer.from(encoded, "base64url"))));
    assert.equal(payload.diagramFont, "inherit");
  }
  assert.equal(decodeShareHash(s2Payload({}))?.diagramFont, "trebuchet");
  assert.equal(decodeShareHash(s3Payload({}))?.diagramFont, "trebuchet");
  assert.equal(decodeShareHash(encodedPayload({ code: "flowchart LR\nA", theme: "default" }))?.diagramFont, "trebuchet");
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

test("rejects the retired viewport field across legacy, s2 and s3 payloads", () => {
  for (const mode of ["canonical", "host", "fluid", null]) {
    const payload = {
      code: "flowchart TD\nA",
      theme: "default",
      renderViewportMode: mode,
    };
    for (const encode of [encodedPayload, s2Payload, s3Payload]) {
      assert.equal(decodeShareHash(encode(payload)), null, `${mode}: ${encode.name}`);
    }
  }
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

function s3Payload(payload: Record<string, unknown>): string {
  return s2Payload(payload).replace("#s2:", "#s3:");
}
