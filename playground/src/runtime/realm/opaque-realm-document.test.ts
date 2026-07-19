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
