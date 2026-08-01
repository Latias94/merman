import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  minimumSupportedVscodeVersion,
  vscodeTestVersion,
} from "./vscode-test-version.mjs";

describe("VS Code test version", () => {
  it("uses the exact minimum version declared by the extension manifest", () => {
    assert.equal(vscodeTestVersion, "1.121.0");
  });

  it("rejects ranges that cannot identify one reproducible test build", () => {
    assert.throws(
      () => minimumSupportedVscodeVersion(">=1.121.0"),
      /exact caret range/,
    );
  });
});
