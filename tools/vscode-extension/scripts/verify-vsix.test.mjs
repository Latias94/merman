import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  normalizeBareForwardedArgs,
  normalizeNpmForwardedArgs,
} from "./verify-vsix.mjs";

describe("VSIX verification wrapper", () => {
  it("preserves values belonging to explicitly named options", () => {
    assert.deepEqual(
      normalizeBareForwardedArgs([
        "--vsix",
        "merman.vsix",
        "--target",
        "darwin-arm64",
        "--version",
        "0.8.0-alpha.3",
      ]),
      [
        "--vsix",
        "merman.vsix",
        "--target",
        "darwin-arm64",
        "--version",
        "0.8.0-alpha.3",
      ],
    );
  });

  it("normalizes npm-forwarded verification arguments once", () => {
    assert.deepEqual(
      normalizeNpmForwardedArgs(
        [],
        ["vsix", "target", "version"],
        {
          npm_config_vsix: "merman.vsix",
          npm_config_target: "darwin-arm64",
          npm_config_version: "0.8.0-alpha.3",
        },
      ),
      [
        "--vsix",
        "merman.vsix",
        "--target",
        "darwin-arm64",
        "--version",
        "0.8.0-alpha.3",
      ],
    );
  });
});
