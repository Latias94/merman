import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { parseSmokeCli } from "./smoke-cli.mjs";

describe("web package smoke CLI", () => {
  it("accepts one closed package identifier", () => {
    assert.deepEqual(parseSmokeCli(["--package-id", "editor"]), {
      packageId: "editor",
    });
  });

  it("rejects missing, traversal-like, and obsolete arguments", () => {
    assert.throws(() => parseSmokeCli([]), /package-id/);
    assert.throws(() => parseSmokeCli(["--package-id", "../full"]), /package identifier/);
    assert.throws(() => parseSmokeCli(["--pkg-dir-rel", "pkg/full"]), /Unknown argument/);
  });
});
