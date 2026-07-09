import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  buildVscePackageArgs,
  normalizeNpmForwardedArgs,
  parseSourceVersion,
} from "./package-vsix.mjs";

describe("VSIX package wrapper", () => {
  it("packages prerelease source versions with a stable manifest version and prerelease metadata", () => {
    const result = buildVscePackageArgs({
      manifestVersion: "0.8.0",
      releaseVersion: "0.8.0-alpha.3",
      userArgs: ["--target", "linux-x64", "--out", "merman.vsix"],
      env: {},
    });

    assert.deepEqual(result.args, [
      "package",
      "--pre-release",
      "--target",
      "linux-x64",
      "--out",
      "merman.vsix",
    ]);
    assert.match(result.message ?? "", /0\.8\.0-alpha\.3/);
  });

  it("packages stable source versions without prerelease metadata", () => {
    const result = buildVscePackageArgs({
      manifestVersion: "0.8.0",
      releaseVersion: "0.8.0",
      userArgs: ["linux-x64", "merman.vsix"],
      env: {},
    });

    assert.deepEqual(result.args, [
      "package",
      "--target",
      "linux-x64",
      "--out",
      "merman.vsix",
    ]);
    assert.equal(result.message, null);
  });

  it("rejects prerelease VSIX manifest versions", () => {
    assert.throws(
      () =>
        buildVscePackageArgs({
          manifestVersion: "0.8.0-alpha.3",
          releaseVersion: "0.8.0-alpha.3",
          userArgs: [],
          env: {},
        }),
      /stable VSIX manifest version/,
    );
  });

  it("normalizes npm-forwarded target and output arguments", () => {
    assert.deepEqual(
      normalizeNpmForwardedArgs([], {
        npm_config_target: "darwin-arm64",
        npm_config_out: "merman-darwin-arm64.vsix",
      }),
      ["--target", "darwin-arm64", "--out", "merman-darwin-arm64.vsix"],
    );
  });

  it("parses SemVer build metadata without changing the VSIX manifest version", () => {
    assert.deepEqual(parseSourceVersion("1.2.3-beta.4+sha", "release version"), {
      raw: "1.2.3-beta.4+sha",
      stableVersion: "1.2.3",
      preRelease: "beta.4",
    });
  });
});
