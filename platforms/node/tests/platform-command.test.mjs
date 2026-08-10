import assert from "node:assert/strict";
import test from "node:test";

import { npmExecutable } from "../scripts/verify-packages.mjs";

test("package verification selects the Windows npm command shim", () => {
  assert.equal(npmExecutable("win32"), "npm.cmd");
  assert.equal(npmExecutable("linux"), "npm");
  assert.equal(npmExecutable("darwin"), "npm");
});
