import assert from "node:assert/strict";
import test from "node:test";

import { assertOfficialSignatureAuditResult } from "./zenuml-core-candidate-matrix.mjs";

test("official npm signature verification fails explicitly on a nonzero status", () => {
  assert.throws(
    () =>
      assertOfficialSignatureAuditResult(
        {
          status: 1,
          stdout: '{"error":"invalid signature"}',
          stderr: "signature verification failed",
        },
        "@zenuml/core@9.9.9"
      ),
    /official npm signature verification failed.*status 1/u
  );
});

test("official npm signature verification rejects invalid JSON", () => {
  assert.throws(
    () =>
      assertOfficialSignatureAuditResult(
        { status: 0, stdout: "not-json", stderr: "" },
        "@zenuml/core@9.9.9"
      ),
    /returned invalid JSON/u
  );
});
