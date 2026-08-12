import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import {
  validateContract,
  validateEvidence,
} from "./zenuml-browser-admission.mjs";

const contractPath = path.resolve(
  import.meta.dirname,
  "../../tools/upstreams/ZENUML_BROWSER_ADMISSION_PROBES.json"
);

function passingEvidence(contract) {
  const evidence = {
    schemaVersion: 1,
    artifactKind: "zenuml-browser-admission-report",
    generatedBy: "playground/scripts/zenuml-browser-admission.mjs",
    probeContract: {
      path: "tools/upstreams/ZENUML_BROWSER_ADMISSION_PROBES.json",
      sha256: "placeholder",
    },
    projects: contract.projects,
  };
  for (const [category, probes] of Object.entries(contract.categories)) {
    const field = category.replace(/-([a-z])/gu, (_, letter) => letter.toUpperCase());
    const observations = probes.map(({ id, description }) => ({
      id,
      description,
      observations: contract.projects.map((project) => ({
        project,
        testTitle: `${category} ${id}`,
        expected: true,
        observed: true,
        passed: true,
      })),
    }));
    evidence[field] = {
      projectCount: contract.projects.length,
      probeCount: probes.length,
      observationCount: probes.length * contract.projects.length,
      passedObservationCount: probes.length * contract.projects.length,
      probes: observations,
    };
  }
  return evidence;
}

test("browser admission keeps a non-empty exact probe contract", async () => {
  const contract = JSON.parse(await readFile(contractPath, "utf8"));
  validateContract(contract);
});

test("browser admission rejects a failed observation", async () => {
  const contract = JSON.parse(await readFile(contractPath, "utf8"));
  const evidence = passingEvidence(contract);
  const crypto = await import("node:crypto");
  evidence.probeContract.sha256 = crypto
    .createHash("sha256")
    .update(`${JSON.stringify(contract, null, 2)}\n`)
    .digest("hex");
  const category = Object.keys(contract.categories)[0];
  const field = category.replace(/-([a-z])/gu, (_, letter) => letter.toUpperCase());
  evidence[field].probes[0].observations[0].passed = false;
  assert.throws(() => validateEvidence(evidence, contract), /false !== true/u);
});
