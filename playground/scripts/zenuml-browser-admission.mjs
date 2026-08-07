import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const playgroundRoot = path.resolve(path.dirname(scriptPath), "..");
const defaultWorkspaceRoot = path.resolve(playgroundRoot, "..");
const probeContractRelativePath =
  "tools/upstreams/ZENUML_BROWSER_ADMISSION_PROBES.json";
const evidenceRelativePath =
  "tools/upstreams/ZENUML_BROWSER_SECURITY_EVIDENCE.json";
const reporterPath = path.join(
  playgroundRoot,
  "scripts",
  "zenuml-admission-reporter.mjs"
);
export async function loadVerifiedBrowserAdmissionEvidence(
  workspaceRoot = defaultWorkspaceRoot
) {
  const contract = await readJson(
    path.join(workspaceRoot, probeContractRelativePath)
  );
  validateContract(contract);
  const evidencePath = path.join(workspaceRoot, evidenceRelativePath);
  const serialized = await readFile(evidencePath, "utf8");
  const evidence = JSON.parse(serialized);
  validateEvidence(evidence, contract);
  assert.equal(serialized, canonicalJson(evidence));
  return {
    evidence,
    relativePath: evidenceRelativePath,
    sha256: sha256(serialized),
    summaries: Object.fromEntries(
      Object.keys(contract.categories).map((category) => [
        category,
        categorySummary(evidence, category),
      ])
    ),
  };
}

function validateContract(contract) {
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

function validateEvidence(evidence, contract) {
  assert.equal(evidence.schemaVersion, 2);
  assert.equal(evidence.command, "npm run test:zenuml-browser-admission");
  assert.equal(
    evidence.generatedBy,
    "playground/scripts/zenuml-browser-admission.mjs"
  );
  assert.deepEqual(evidence.projects, contract.projects);
  assert.equal(evidence.probeContract.path, probeContractRelativePath);
  assert.equal(
    evidence.probeContract.sha256,
    sha256(canonicalJson(contract))
  );
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

function categorySummary(evidence, category) {
  const value = evidence[camelCategory(category)];
  return {
    artifact: evidenceRelativePath,
    probeContract: probeContractRelativePath,
    projectCount: value.projectCount,
    probeCount: value.probeCount,
    observationCount: value.observationCount,
    passedObservationCount: value.passedObservationCount,
  };
}

async function runBrowserAdmission(workspaceRoot) {
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
  assert.equal(child.status, 0, "ZenUML browser admission Playwright run failed");
  try {
    const report = await readJson(temporaryReport);
    assert.equal(report.status, "passed");
    const contract = await readJson(
      path.join(workspaceRoot, probeContractRelativePath)
    );
    validateContract(contract);
    return buildEvidence(report, contract);
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
    schemaVersion: 2,
    generatedBy: "playground/scripts/zenuml-browser-admission.mjs",
    command: "npm run test:zenuml-browser-admission",
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
        .sort((left, right) =>
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
  validateEvidence(evidence, contract);
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

async function writeAtomically(target, contents) {
  await mkdir(path.dirname(target), { recursive: true });
  const temporary = `${target}.${process.pid}.tmp`;
  try {
    await writeFile(temporary, contents, { flag: "wx" });
    await rename(temporary, target);
  } finally {
    await rm(temporary, { force: true });
  }
}

async function main() {
  const arguments_ = new Set(process.argv.slice(2));
  for (const argument of arguments_) {
    assert(
      argument === "--run" || argument === "--write",
      "usage: node zenuml-browser-admission.mjs [--run [--write]]"
    );
  }
  const run = arguments_.has("--run");
  const write = arguments_.has("--write");
  assert(!write || run, "--write requires --run");
  if (!run) {
    await loadVerifiedBrowserAdmissionEvidence();
    return;
  }
  const evidence = await runBrowserAdmission(defaultWorkspaceRoot);
  const serialized = canonicalJson(evidence);
  const target = path.join(defaultWorkspaceRoot, evidenceRelativePath);
  if (write) {
    await writeAtomically(target, serialized);
    return;
  }
  assert.equal(
    await readFile(target, "utf8"),
    serialized,
    "ZenUML browser admission evidence is stale; rerun with --run --write"
  );
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  await main();
}
