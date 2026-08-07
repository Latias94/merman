import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { loadVerifiedBrowserAdmissionEvidence } from "./zenuml-browser-admission.mjs";

test("browser admission verifies observations without pinning implementation bytes", async () => {
  const { evidence, summaries } = await loadVerifiedBrowserAdmissionEvidence();

  assert.equal(evidence.schemaVersion, 2);
  assert.equal(Object.hasOwn(evidence, "sourceFiles"), false);
  assert.equal(summaries.security.passedObservationCount, summaries.security.observationCount);
  assert.equal(
    summaries["execution-isolation"].passedObservationCount,
    summaries["execution-isolation"].observationCount,
  );
});

test("browser admission ignores source drift but keeps contract and observation binding", async () => {
  const repositoryRoot = path.resolve(import.meta.dirname, "../..");
  const temporaryRoot = await mkdtemp(
    path.join(tmpdir(), "merman-zenuml-admission-")
  );
  const contractPath = "tools/upstreams/ZENUML_BROWSER_ADMISSION_PROBES.json";
  const evidencePath = "tools/upstreams/ZENUML_BROWSER_SECURITY_EVIDENCE.json";
  try {
    for (const relativePath of [contractPath, evidencePath]) {
      const target = path.join(temporaryRoot, relativePath);
      await mkdir(path.dirname(target), { recursive: true });
      await writeFile(
        target,
        await readFile(path.join(repositoryRoot, relativePath), "utf8")
      );
    }

    const historicalSource = path.join(
      temporaryRoot,
      "playground/src/runtime/merman-core.ts"
    );
    await mkdir(path.dirname(historicalSource), { recursive: true });
    await writeFile(historicalSource, "implementation bytes intentionally changed\n");
    await loadVerifiedBrowserAdmissionEvidence(temporaryRoot);

    const evidenceTarget = path.join(temporaryRoot, evidencePath);
    const evidence = JSON.parse(await readFile(evidenceTarget, "utf8"));
    evidence.security.probes[0].observations[0].passed = false;
    await writeFile(evidenceTarget, `${JSON.stringify(evidence, null, 2)}\n`);
    await assert.rejects(
      loadVerifiedBrowserAdmissionEvidence(temporaryRoot),
      /false !== true/u
    );

    await writeFile(
      evidenceTarget,
      await readFile(path.join(repositoryRoot, evidencePath), "utf8")
    );
    const contractTarget = path.join(temporaryRoot, contractPath);
    const contract = JSON.parse(await readFile(contractTarget, "utf8"));
    contract.categories.security[0].description += " changed";
    await writeFile(contractTarget, `${JSON.stringify(contract, null, 2)}\n`);
    await assert.rejects(
      loadVerifiedBrowserAdmissionEvidence(temporaryRoot),
      /actual.*expected|Expected values to be strictly equal/u
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});
