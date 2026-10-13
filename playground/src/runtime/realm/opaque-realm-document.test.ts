import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import { buildOpaqueRealmDocument } from "./opaque-realm-document.ts";

const SCRIPT = "globalThis.__opaqueRealmStarted = true;";
const ARTIFACT = {
  schemaVersion: 1 as const,
  id: "compare",
  bytes: Buffer.byteLength(SCRIPT),
  sha256: createHash("sha256").update(SCRIPT).digest("hex"),
  cspHash: `sha256-${createHash("sha256").update(SCRIPT).digest("base64")}`,
  script: SCRIPT,
};

test("opaque realm document has a hash-locked no-network CSP and local boot data", () => {
  const document = buildOpaqueRealmDocument(
    {
      kind: "compare",
      realmId: "r".repeat(43),
      bootNonce: "b".repeat(43),
    },
    ARTIFACT
  );
  assert.match(
    document,
    new RegExp(`script-src '${ARTIFACT.cspHash}' blob:`)
  );
  assert.match(document, /connect-src 'none'/);
  assert.match(document, /object-src 'none'/);
  assert.match(document, /worker-src 'none'/);
  assert.match(document, /merman-realm-boot/);
  assert.match(document, /presentation-host/);
  assert.match(document, /__opaqueRealmStarted/);
  assert.doesNotMatch(document, /allow-same-origin/);
  assert.doesNotMatch(document, /https?:\/\//);
});

test("opaque realm document rejects mismatched or HTML-breaking artifacts", () => {
  const boot = {
    kind: "compare" as const,
    realmId: "r".repeat(43),
    bootNonce: "b".repeat(43),
  };
  assert.throws(
    () => buildOpaqueRealmDocument(boot, { ...ARTIFACT, id: "benchmark" }),
    /artifact is invalid/
  );
  assert.throws(
    () =>
      buildOpaqueRealmDocument(boot, {
        ...ARTIFACT,
        script: "</script><script>alert(1)</script>",
      }),
    /artifact is invalid/
  );
});

test("opaque realm shell preserves host language for compare and benchmark font fallback", () => {
  for (const kind of ["compare", "benchmark"] as const) {
    for (const language of ["en", "zh-Hant-TW", ""]) {
      const document = buildOpaqueRealmDocument(
        { kind, realmId: "r".repeat(43), bootNonce: "b".repeat(43) },
        { ...ARTIFACT, id: kind },
        language,
      );
      assert.ok(document.startsWith(`<!doctype html><html lang="${language}">`));
      assert.match(document, new RegExp(`script-src '${ARTIFACT.cspHash}' blob:`));
      assert.ok(document.includes(`<script>${SCRIPT}</script>`));
    }
  }
});

test("opaque realm shell escapes host language without adding executable attributes", () => {
  const document = buildOpaqueRealmDocument(
    { kind: "compare", realmId: "r".repeat(43), bootNonce: "b".repeat(43) },
    ARTIFACT,
    'en" onload="<script>&',
  );
  assert.ok(document.startsWith(
    '<!doctype html><html lang="en&quot; onload=&quot;&lt;script&gt;&amp;">',
  ));
  assert.doesNotMatch(document, /lang="en" onload=/);
});
