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
  const interactiveLayoutWorkCeiling = 14_100_000;

  assert.doesNotThrow(() =>
    contract.tightenResourceOptions(
      { profile: "interactive" },
      {
        profile: "interactive",
        limits: { max_layout_work_units: interactiveLayoutWorkCeiling },
      },
    ),
  );
  assert.throws(
    () =>
      contract.tightenResourceOptions(
        { profile: "interactive" },
        {
          profile: "interactive",
          limits: { max_layout_work_units: interactiveLayoutWorkCeiling + 1 },
        },
      ),
    /maximum 14100000/,
  );
});

test("resource tightening preserves an explicitly stricter profile", async () => {
  const contract = await loadGeneratedContract();

  assert.deepEqual(
    contract.tightenResourceOptions(
      { profile: "interactive" },
      { profile: "constrained" },
    ),
    { profile: "constrained" },
  );
});

test("resource tightening rejects a looser profile hidden by generic overrides", async () => {
  const contract = await loadGeneratedContract();
  const smallestGenericLimits = Object.fromEntries(
    contract.RESOURCE_OVERRIDE_IDS.map((id) => [id, 1]),
  );

  assert.throws(
    () =>
      contract.tightenResourceOptions(
        { profile: "constrained" },
        { profile: "interactive", limits: smallestGenericLimits },
      ),
    /ceiling profile: requested interactive, ceiling constrained/,
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
