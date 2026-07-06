import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { npmDistTagForVersion } from "./npm-dist-tag.mjs";

describe("npm dist-tag release helper", () => {
  it("uses latest for stable versions", () => {
    assert.equal(npmDistTagForVersion("1.2.3"), "latest");
  });

  it("uses the prerelease channel as the dist-tag", () => {
    assert.equal(npmDistTagForVersion("1.2.3-alpha.1"), "alpha");
    assert.equal(npmDistTagForVersion("1.2.3-beta.12"), "beta");
    assert.equal(npmDistTagForVersion("1.2.3-rc.2"), "rc");
  });

  it("rejects release tags and unsupported SemVer shapes", () => {
    assert.throws(() => npmDistTagForVersion("v1.2.3"), /version must/);
    assert.throws(() => npmDistTagForVersion("1.2.3-alpha"), /version must/);
    assert.throws(() => npmDistTagForVersion("1.2.3-nightly.1"), /version must/);
  });
});
