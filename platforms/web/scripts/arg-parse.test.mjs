import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { ArgParseError, assertKnownArgs, parseArgValue } from "./arg-parse.mjs";

describe("web script argument parsing", () => {
  it("parses split and equals values", () => {
    assert.equal(parseArgValue(["--preset", "browser-core"], "--preset"), "browser-core");
    assert.equal(parseArgValue(["--preset=browser-core"], "--preset"), "browser-core");
    assert.equal(parseArgValue([], "--preset"), null);
  });

  it("rejects missing or empty values", () => {
    assert.throws(() => parseArgValue(["--preset"], "--preset"), ArgParseError);
    assert.throws(() => parseArgValue(["--preset", "--out-dir-rel"], "--preset"), ArgParseError);
    assert.throws(() => parseArgValue(["--preset="], "--preset"), ArgParseError);
  });

  it("rejects unknown arguments", () => {
    assert.doesNotThrow(() =>
      assertKnownArgs(["--preset", "browser-core"], { valueArgs: ["--preset"] }),
    );
    assert.throws(
      () => assertKnownArgs(["--preset", "browser-core", "--extra"], { valueArgs: ["--preset"] }),
      ArgParseError,
    );
  });
});
