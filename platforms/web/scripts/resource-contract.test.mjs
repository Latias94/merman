import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const webRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const generatedSource = path.join(webRoot, "src", "generated", "resource-contract.ts");

test("interactive resource tightening accepts the generated Rust layout-work ceiling", async () => {
  const contract = await loadGeneratedContract();

  assert.doesNotThrow(() =>
    contract.tightenResourceOptions(
      { profile: "interactive" },
      {
        profile: "interactive",
        limits: { max_layout_work_units: 800_000 },
      },
    ),
  );
  assert.throws(
    () =>
      contract.tightenResourceOptions(
        { profile: "interactive" },
        {
          profile: "interactive",
          limits: { max_layout_work_units: 800_001 },
        },
      ),
    /maximum 800000/,
  );
});

async function loadGeneratedContract() {
  const source = readFileSync(generatedSource, "utf8");
  const javascript = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: generatedSource,
    reportDiagnostics: true,
  });
  assert.deepEqual(javascript.diagnostics ?? [], []);

  return import(
    `data:text/javascript;base64,${Buffer.from(javascript.outputText).toString("base64")}`
  );
}
