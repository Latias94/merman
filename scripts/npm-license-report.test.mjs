import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { collectProductionLicenses, formatLicenseGroups, readLicenseContent } from "./generate-npm-license-report.mjs";

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "merman-license-report-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return root;
}
function writePackage(directory, manifest, files = {}) {
  fs.mkdirSync(directory, { recursive: true });
  fs.writeFileSync(path.join(directory, "package.json"), JSON.stringify(manifest));
  for (const [name, content] of Object.entries(files)) fs.writeFileSync(path.join(directory, name), content);
}

test("npm inventory includes production peers and links, omits dev-only and absent optional packages", (t) => {
  const root = fixture(t);
  writePackage(root, {
    name: "test-app", version: "1.0.0", dependencies: { runtime: "1.0.0", linked: "file:./linked-source" },
    devDependencies: { peer: "1.0.0", tooling: "1.0.0" }, optionalDependencies: { absent: "1.0.0" },
  });
  writePackage(path.join(root, "node_modules/runtime"), {
    name: "runtime", version: "1.0.0", peerDependencies: { peer: "1.0.0" },
  }, { LICENSE: "runtime license\r\n", NOTICE: "runtime attribution\r\n" });
  writePackage(path.join(root, "node_modules/peer"), { name: "peer", version: "1.0.0" }, { LICENSE: "peer license" });
  writePackage(path.join(root, "node_modules/tooling"), { name: "tooling", version: "1.0.0" }, { LICENSE: "dev license" });
  writePackage(path.join(root, "linked-source"), { name: "linked", version: "1.0.0" }, { LICENSE: "linked license" });
  fs.symlinkSync(path.join(root, "linked-source"), path.join(root, "node_modules/linked"), "junction");
  const groups = collectProductionLicenses(root);
  assert.deepEqual(groups.flatMap((group) => group.names).sort(), ["linked@1.0.0", "peer@1.0.0", "runtime@1.0.0"]);
  assert.equal(groups.find((group) => group.names[0] === "runtime@1.0.0").content,
    "runtime license\n\nWith the following notices:\n\nruntime attribution");
  assert.match(formatLicenseGroups(groups), / - linked@1.0.0/);
});

test("a missing required dependency blocks report generation", (t) => {
  const root = fixture(t);
  writePackage(root, { name: "test-app", version: "1.0.0", dependencies: { missing: "1.0.0" } });
  assert.throws(() => collectProductionLicenses(root), /npm production dependency inventory exited/);
});

test("ambiguous licenses require an explicit replacement while preserving all notices", (t) => {
  const root = fixture(t);
  const manifest = { name: "dual", version: "1.0.0" };
  writePackage(root, manifest, {
    LICENSE: "first", "LICENSE-OTHER": "second", "NOTICE.txt": "attribution",
    "license-update.mjs": "never a license body",
  });
  assert.throws(() => readLicenseContent(root, manifest), /Ambiguous/);
  const canonical = path.join(root, "canonical.txt");
  fs.writeFileSync(canonical, "selected license\r\n");
  assert.equal(readLicenseContent(root, manifest, canonical),
    "selected license\n\nWith the following notices:\n\nattribution");
});

test("declared-only licenses stay explicit and missing or empty evidence is rejected", (t) => {
  const root = fixture(t);
  writePackage(root, { name: "declaration" });
  assert.equal(readLicenseContent(root, { name: "declaration", license: "MIT" }), "MIT");
  assert.throws(() => readLicenseContent(root, { name: "declaration" }), /Missing license/);
  assert.throws(() => readLicenseContent(root, { name: "declaration", license: "SEE LICENSE IN absent" }), /Missing license/);
  fs.writeFileSync(path.join(root, "LICENSE"), "   ");
  assert.throws(() => readLicenseContent(root, { name: "declaration", license: "MIT" }), /Empty license/);
});
