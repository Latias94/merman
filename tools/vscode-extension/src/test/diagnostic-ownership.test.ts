import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { projectOwnedDiagnostics } from "../diagnostic-ownership.js";

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
});
