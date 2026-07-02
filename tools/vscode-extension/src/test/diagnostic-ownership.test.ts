import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  emptyDocumentDiagnosticReport,
  projectOwnedDiagnostics,
  projectOwnedDocumentDiagnosticReport,
} from "../diagnostic-ownership.js";

describe("diagnostic ownership", () => {
  it("passes diagnostics through when Merman owns Problems output", () => {
    const diagnostics = [{ message: "syntax" }];

    assert.deepEqual(
      projectOwnedDiagnostics(diagnostics, { enabled: true }),
      diagnostics,
    );
  });

  it("suppresses diagnostics when another linter owns Problems output", () => {
    assert.deepEqual(
      projectOwnedDiagnostics([{ message: "syntax" }], { enabled: false }),
      [],
    );
  });

  it("returns an empty full report for pull diagnostics when disabled", () => {
    assert.deepEqual(projectOwnedDocumentDiagnosticReport(
      { kind: "full", resultId: "r1", items: [{ message: "syntax" }] },
      { enabled: false },
    ), emptyDocumentDiagnosticReport());
  });

  it("preserves pull diagnostic reports when enabled", () => {
    assert.deepEqual(projectOwnedDocumentDiagnosticReport(
      { kind: "full", resultId: "r1", items: [{ message: "syntax" }] },
      { enabled: true },
    ), { kind: "full", resultId: "r1", items: [{ message: "syntax" }] });
    assert.deepEqual(projectOwnedDocumentDiagnosticReport(
      { kind: "unChanged", resultId: "r1" },
      { enabled: true },
    ), { kind: "unChanged", resultId: "r1" });
  });
});
