import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const playgroundRoot = path.resolve(path.dirname(scriptPath), "..");
const defaultWorkspaceRoot = path.resolve(playgroundRoot, "..");
const probeContractRelativePath =
  "tools/upstreams/ZENUML_BROWSER_ADMISSION_PROBES.json";
const reporterPath = path.join(
  playgroundRoot,
  "scripts",
  "zenuml-admission-reporter.mjs"
);

export function validateContract(contract) {
  assert.equal(contract.schemaVersion, 1);
  assert.deepEqual(Object.keys(contract.categories).sort(), [
    "execution-isolation",
    "security",
  ]);
  assert.deepEqual([...contract.projects].sort(), contract.projects);
  assert.equal(new Set(contract.projects).size, contract.projects.length);
  assert(contract.projects.length > 0);
  for (const [category, probes] of Object.entries(contract.categories)) {
    assert(probes.length > 0, `${category} probe contract is empty`);
    const ids = probes.map(({ id }) => id);
    assert.equal(new Set(ids).size, ids.length, `${category} has duplicate probes`);
    for (const probe of probes) {
      assert.match(probe.id, /^[a-z0-9]+(?:-[a-z0-9]+)*$/u);
      assert.equal(probe.description.trim(), probe.description);
      assert(probe.description.length > 0);
    }
  }
}

export function validateEvidence(evidence, contract) {
  assert.equal(evidence.schemaVersion, 1);
  assert.equal(evidence.artifactKind, "zenuml-browser-admission-report");
  assert.equal(
    evidence.generatedBy,
    "playground/scripts/zenuml-browser-admission.mjs"
  );
  assert.deepEqual(evidence.projects, contract.projects);
  assert.equal(evidence.probeContract.path, probeContractRelativePath);
  assert.equal(evidence.probeContract.sha256, sha256(canonicalJson(contract)));
  for (const category of Object.keys(contract.categories)) {
    validateCategoryEvidence(
      evidence[camelCategory(category)],
      category,
      contract.categories[category],
      contract.projects
    );
  }
}

function validateCategoryEvidence(value, category, requiredProbes, projects) {
  assert(value, `missing ${category} evidence`);
  assert.equal(value.probeCount, requiredProbes.length);
  assert.equal(value.projectCount, projects.length);
  assert.equal(value.observationCount, requiredProbes.length * projects.length);
  assert.equal(value.passedObservationCount, value.observationCount);
  assert.deepEqual(
    value.probes.map(({ id, description }) => ({ id, description })),
    requiredProbes
  );
  for (const probe of value.probes) {
    assert.equal(probe.observations.length, projects.length);
    assert.deepEqual(
      probe.observations.map(({ project }) => project),
      projects
    );
    for (const observation of probe.observations) {
      assert.equal(observation.passed, true);
      assert.deepEqual(observation.observed, observation.expected);
      assert.equal(observation.testTitle.trim(), observation.testTitle);
      assert(observation.testTitle.length > 0);
    }
  }
}

export async function runBrowserAdmission(
  workspaceRoot = defaultWorkspaceRoot
) {
  const temporaryReport = path.join(
    tmpdir(),
    `merman-zenuml-browser-admission-${process.pid}.json`
  );
  await rm(temporaryReport, { force: true });
  const require = createRequire(path.join(playgroundRoot, "tests", "package.json"));
  const playwrightCli = require.resolve("@playwright/test/cli");
  const child = spawnSync(
    process.execPath,
    [
      playwrightCli,
      "test",
      "benchmark.realm.spec.ts",
      "--project=chromium-desktop",
      `--reporter=${reporterPath}`,
    ],
    {
      cwd: path.join(playgroundRoot, "tests"),
      env: {
        ...process.env,
        MERMAN_ZENUML_ADMISSION_REPORT: temporaryReport,
      },
      stdio: "inherit",
    }
  );
  if (child.error || child.status !== 0) {
    throw new Error(
      `ZenUML browser admission failed with status ${String(child.status)}: ${
        child.error?.message ?? "Playwright reported a failing probe"
      }`
    );
  }
  try {
    const report = await readJson(temporaryReport);
    assert.equal(report.status, "passed");
    const contract = await readJson(
      path.join(workspaceRoot, probeContractRelativePath)
    );
    validateContract(contract);
    const evidence = buildEvidence(report, contract);
    validateEvidence(evidence, contract);
    return evidence;
  } finally {
    await rm(temporaryReport, { force: true });
  }
}

function buildEvidence(report, contract) {
  const observations = new Map();
  for (const category of Object.keys(contract.categories)) {
    observations.set(
      category,
      new Map(contract.categories[category].map(({ id }) => [id, []]))
    );
  }
  const seen = new Set();
  for (const record of report.records) {
    assert(contract.projects.includes(record.project));
    assert.equal(record.status, "passed");
    const { attachment } = record;
    assert.equal(attachment.schemaVersion, 1);
    const required = observations.get(attachment.category);
    assert(required, `unknown admission category ${attachment.category}`);
    for (const probe of attachment.probes) {
      assert(required.has(probe.id), `unknown ${attachment.category} probe ${probe.id}`);
      const key = `${record.project}\0${attachment.category}\0${probe.id}`;
      assert(!seen.has(key), `duplicate admission observation ${key}`);
      seen.add(key);
      assert.equal(probe.passed, true, `${probe.id} did not pass`);
      assert.deepEqual(probe.observed, probe.expected, `${probe.id} observation drift`);
      required.get(probe.id).push({
        project: record.project,
        testTitle: record.testTitle,
        expected: probe.expected,
        observed: probe.observed,
        passed: probe.passed,
      });
    }
  }

  const evidence = {
    schemaVersion: 1,
    artifactKind: "zenuml-browser-admission-report",
    generatedBy: "playground/scripts/zenuml-browser-admission.mjs",
    probeContract: {
      path: probeContractRelativePath,
      sha256: sha256(canonicalJson(contract)),
    },
    projects: contract.projects,
  };
  for (const [category, requiredProbes] of Object.entries(contract.categories)) {
    const byId = observations.get(category);
    const probes = requiredProbes.map(({ id, description }) => ({
      id,
      description,
      observations: byId
        .get(id)
        .sort(
          (left, right) =>
            contract.projects.indexOf(left.project) -
            contract.projects.indexOf(right.project)
        ),
    }));
    const observationCount = probes.reduce(
      (count, probe) => count + probe.observations.length,
      0
    );
    evidence[camelCategory(category)] = {
      projectCount: contract.projects.length,
      probeCount: probes.length,
      observationCount,
      passedObservationCount: probes.reduce(
        (count, probe) =>
          count + probe.observations.filter(({ passed }) => passed).length,
        0
      ),
      probes,
    };
  }
  return evidence;
}

function camelCategory(category) {
  return category.replace(/-([a-z])/gu, (_, letter) => letter.toUpperCase());
}

function canonicalJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

async function readJson(target) {
  return JSON.parse(await readFile(target, "utf8"));
}

function parseArguments(arguments_) {
  if (arguments_.length === 0) return { output: null };
  if (arguments_.length === 2 && arguments_[0] === "--output" && arguments_[1]) {
    return { output: arguments_[1] };
  }
  throw new Error(
    "usage: node zenuml-browser-admission.mjs [--output <path>]"
  );
}

async function main() {
  const { output } = parseArguments(process.argv.slice(2));
  const serialized = canonicalJson(await runBrowserAdmission());
  if (output) {
    const target = path.resolve(output);
    await mkdir(path.dirname(target), { recursive: true });
    await writeFile(target, serialized, { flag: "wx" });
  } else {
    process.stdout.write(serialized);
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  await main();
}
