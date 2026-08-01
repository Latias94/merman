import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const rustSourceRoot = path.resolve(nodeRoot, "..", "..", "crates", "merman-node", "src");

test("the napi lint boundary still rejects handwritten unsafe code", (context) => {
  const crateRoot = readFileSync(path.join(rustSourceRoot, "lib.rs"), "utf8");
  assert.match(
    crateRoot,
    /#!\[cfg_attr\(not\(feature = "transport-napi"\), forbid\(unsafe_code\)\)\]/,
  );
  assert.match(
    crateRoot,
    /#!\[cfg_attr\(feature = "transport-napi", deny\(unsafe_code\)\)\]/,
  );

  for (const entry of readdirSync(rustSourceRoot, { withFileTypes: true })) {
    if (!entry.isFile() || path.extname(entry.name) !== ".rs") continue;
    const source = readFileSync(path.join(rustSourceRoot, entry.name), "utf8");
    assert.doesNotMatch(source, /\bunsafe\b/, `${entry.name} contains handwritten unsafe code`);
  }

  const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "merman-node-unsafe-lint-"));
  context.after(() => rmSync(temporaryRoot, { recursive: true, force: true }));
  const probe = path.join(temporaryRoot, "probe.rs");
  writeFileSync(
    probe,
    `${crateRoot.split("\n").slice(0, 2).join("\n")}
fn main() {
    unsafe { core::ptr::read_volatile(&0_u8); }
}
`,
  );
  const result = spawnSync(
    "rustc",
    ["--edition", "2024", "--cfg", 'feature="transport-napi"', probe],
    { cwd: temporaryRoot, encoding: "utf8" },
  );
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /usage of an .*unsafe.* block/i);
});
