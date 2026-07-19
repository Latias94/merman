import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash, X509Certificate } from "node:crypto";
import {
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";
import ts from "typescript";

const scriptPath = fileURLToPath(import.meta.url);
const playgroundRoot = path.resolve(path.dirname(scriptPath), "..");
const workspaceRoot = path.resolve(playgroundRoot, "..");
const bundlePath = path.join(
  workspaceRoot,
  "tools",
  "upstreams",
  "MERMAID_REFERENCE_BUNDLE.json"
);
const evidencePath = path.join(
  workspaceRoot,
  "tools",
  "upstreams",
  "ZENUML_CORE_CANDIDATE_EVIDENCE.json"
);
const admissionPath = path.join(
  workspaceRoot,
  "tools",
  "upstreams",
  "ZENUML_CORE_ADMISSION.json"
);
const inlineValidatorRelativePaths = Object.freeze([
  "platforms/web/src/svg-safety-policy.ts",
  "platforms/web/src/svg-safety.ts",
]);
const officialNpmRegistry = "https://registry.npmjs.org/";
const publishPredicate =
  "https://github.com/npm/attestation/tree/main/specs/publish/v0.1";
const slsaPredicate = "https://slsa.dev/provenance/v1";
const FIXTURE_COUNT_ADMISSION_SUMMARIES = Object.freeze({
  corpus:
    "The harness materializes both packages with scripts disabled and proves complete-corpus parse agreement.",
  semantic:
    "Participant, message, fragment, group, return, and creation counts agree across the complete corpus; recovery expansions are classified.",
  render:
    "Oracle and candidate renderToSvg SHA-256 values agree across the complete corpus in the same Chromium process.",
  "strict-inline-artifact":
    "Every candidate native SVG passes the same strict inline publication policy used by the Playground.",
});
const attestationArtifactSchemaVersion = 1;
const maxAttestationArtifactBytes = 64 * 1024;
const maxAttestationPayloadBytes = 32 * 1024;

const COMPATIBILITY_CASES = Object.freeze([
  {
    name: "advanced-participant-group",
    sourceReference: "repo-ref/zenuml-core-3.50.1/test/unit/parser/participant.group.spec.js",
    source: "group BusinessService {\n  @Actor A\n  @Boundary B\n}\nA->B: hi",
  },
  {
    name: "advanced-parallel-fragment",
    sourceReference: "repo-ref/zenuml-core-3.50.1/test/unit/parser/par.spec.js",
    source: "par {\n  A.m()\n  B.m()\n}",
  },
  {
    name: "advanced-try-catch-finally",
    sourceReference: "repo-ref/zenuml-core-3.50.1/test/unit/parser/try.catch.finally.spec.js",
    source: "try { A.m() } catch { B.m() } finally { C.m() }",
  },
  {
    name: "advanced-emoji-message",
    sourceReference: "repo-ref/zenuml-core-3.50.1/test/unit/parity/corpus/text-corpus.ts#emoji",
    source: "A->B: deploy 🚀 now",
  },
  {
    name: "advanced-known-number-unit",
    sourceReference: "repo-ref/zenuml-core-3.50.1/test/unit/parser/digit-leading-name.spec.ts",
    source: "if(300ms) { A.m() }",
  },
]);

const BEHAVIOR_PROBES = Object.freeze([
  {
    id: "explicit-starter-implicit-method-origin",
    classification: "source-verified-endpoint-contract",
    source: "@Starter(A)\nmethod()",
    sourceAttribution: {
      paths: [
        "src/parser/OrderedParticipants.ts",
        "src/parser/From.ts",
        "src/svg/walkStatements.ts",
      ],
      rules: ["starter", "Origin", "From", "message endpoint projection"],
    },
    checks: [
      ["oracle", "parse.pass", true],
      ["candidate", "parse.pass", true],
      ["oracle", "render.rendered", true],
      ["candidate", "render.rendered", true],
      ["oracle", "render.semantic.messages", 1],
      ["candidate", "render.semantic.messages", 1],
      ["oracle", "render.topology.participantNames", ["A"]],
      ["candidate", "render.topology.participantNames", ["A"]],
      [
        "oracle",
        "render.topology.messageEndpoints",
        [{ from: "A", to: null, isSelf: false }],
      ],
      [
        "candidate",
        "render.topology.messageEndpoints",
        [{ from: "A", to: null, isSelf: false }],
      ],
    ],
  },
  {
    id: "dangling-async-arrow",
    classification: "source-verified-endpoint-contract",
    source: "A ->",
    sourceAttribution: {
      paths: [
        "src/g4/sequenceParser.g4",
        "src/parser/ToCollector.ts",
        "src/svg/walkStatements.ts",
      ],
      rules: ["asyncMessage", "missing target recovery", "endpoint projection"],
    },
    checks: [
      ["oracle", "parse.pass", true],
      ["candidate", "parse.pass", true],
      ["oracle", "render.rendered", true],
      ["candidate", "render.rendered", true],
      ["oracle", "render.semantic.messages", 0],
      ["candidate", "render.semantic.messages", 0],
      ["oracle", "render.semantic.returns", 0],
      ["candidate", "render.semantic.returns", 0],
      ["oracle", "render.topology.participantNames", ["A"]],
      ["candidate", "render.topology.participantNames", ["A"]],
      ["oracle", "render.topology.messageEndpoints", []],
      ["candidate", "render.topology.messageEndpoints", []],
      ["oracle", "render.topology.returnEndpoints", []],
      ["candidate", "render.topology.returnEndpoints", []],
    ],
  },
  {
    id: "dangling-return-arrow",
    classification: "source-verified-endpoint-contract",
    source: "A -->",
    sourceAttribution: {
      paths: [
        "src/g4/sequenceParser.g4",
        "src/parser/RetContext.js",
        "src/svg/walkStatements.ts",
      ],
      rules: ["returnAsyncMessage", "From", "ReturnTo", "endpoint projection"],
    },
    checks: [
      ["oracle", "parse.pass", true],
      ["candidate", "parse.pass", true],
      ["oracle", "render.rendered", true],
      ["candidate", "render.rendered", true],
      ["oracle", "render.semantic.returns", 1],
      ["candidate", "render.semantic.returns", 1],
      ["oracle", "render.topology.participantNames", ["A"]],
      ["candidate", "render.topology.participantNames", ["A"]],
      [
        "oracle",
        "render.topology.returnEndpoints",
        [{ from: "A", to: null, isSelf: false }],
      ],
      [
        "candidate",
        "render.topology.returnEndpoints",
        [{ from: "A", to: null, isSelf: false }],
      ],
    ],
  },
  {
    id: "explicit-starter-dangling-return-arrow",
    classification: "source-verified-endpoint-contract",
    source: "@Starter(A)\nA -->",
    sourceAttribution: {
      paths: [
        "src/parser/OrderedParticipants.ts",
        "src/parser/RetContext.js",
        "src/svg/walkStatements.ts",
      ],
      rules: ["starter", "From", "ReturnTo", "endpoint projection"],
    },
    checks: [
      ["oracle", "parse.pass", true],
      ["candidate", "parse.pass", true],
      ["oracle", "render.rendered", true],
      ["candidate", "render.rendered", true],
      ["oracle", "render.semantic.returns", 1],
      ["candidate", "render.semantic.returns", 1],
      ["oracle", "render.topology.participantNames", ["A"]],
      ["candidate", "render.topology.participantNames", ["A"]],
      [
        "oracle",
        "render.topology.returnEndpoints",
        [{ from: "A", to: "A", isSelf: true }],
      ],
      [
        "candidate",
        "render.topology.returnEndpoints",
        [{ from: "A", to: "A", isSelf: true }],
      ],
    ],
  },
  {
    id: "digit-leading-participant",
    classification: "candidate-semantic-expansion",
    source: "3Service.m()",
    sourceAttribution: {
      paths: ["src/g4/sequenceLexer.g4", "src/g4/sequenceParser.g4"],
      rules: ["DIGIT_LEADING_NAME", "name", "methodName"],
    },
    checks: [
      ["oracle", "parse.pass", false],
      ["candidate", "parse.pass", true],
      ["candidate", "render.semantic.messages", 1],
      [
        "candidate",
        "render.topology.participantNames",
        ["_STARTER_", "3Service"],
      ],
    ],
  },
  {
    id: "optional-if-brace-recovery",
    classification: "candidate-recovery-expansion",
    source: "if(x) A.m()",
    sourceAttribution: {
      paths: ["src/g4/sequenceParser.g4"],
      rules: ["ifBlock", "braceBlock?"],
    },
    checks: [
      ["oracle", "parse.pass", false],
      ["oracle", "render.semantic.messages", 0],
      ["candidate", "parse.pass", true],
      ["candidate", "render.semantic.messages", 1],
      ["candidate", "render.semantic.fragments", 1],
      ["candidate", "render.topology.participantNames", ["_STARTER_", "A"]],
    ],
  },
  {
    id: "known-number-unit-with-optional-brace",
    classification: "candidate-lexer-and-recovery-expansion",
    source: "if(300ms) A.m()",
    sourceAttribution: {
      paths: ["src/g4/sequenceLexer.g4", "src/g4/sequenceParser.g4"],
      rules: ["NUMBER_UNIT", "KNOWN_UNIT", "textExpr", "ifBlock"],
    },
    checks: [
      ["oracle", "parse.pass", false],
      ["candidate", "parse.pass", true],
      ["candidate", "render.semantic.messages", 1],
      ["candidate", "render.topology.fragmentLabels", ["300ms"]],
    ],
  },
  {
    id: "same-line-message-topology",
    classification: "source-verified-preserved-behavior",
    source: "A.m() B.m()",
    sourceAttribution: {
      paths: ["src/g4/sequenceParser.g4", "src/parser/MessageCollector.ts"],
      rules: ["block", "stat", "AllMessages"],
    },
    checks: [
      ["oracle", "render.semantic.messages", 2],
      ["candidate", "render.semantic.messages", 2],
      ["candidate", "render.topology.messageLabels", ["m()", "m()"]],
    ],
  },
  {
    id: "typed-head-participant-boundary",
    classification: "source-verified-preserved-behavior",
    source: "@Actor A B.m()",
    sourceAttribution: {
      paths: ["src/g4/sequenceParser.g4", "src/parser/OrderedParticipants.ts"],
      rules: ["participant", "head", "OrderedParticipants"],
    },
    checks: [
      ["candidate", "parse.pass", true],
      ["candidate", "render.semantic.messages", 1],
      [
        "candidate",
        "render.topology.participantNames",
        ["_STARTER_", "A", "B"],
      ],
      ["candidate", "render.topology.participantTypes", [null, "Actor", null]],
    ],
  },
  {
    id: "plain-head-participant-boundary",
    classification: "source-verified-preserved-behavior",
    source: "A B.m()",
    sourceAttribution: {
      paths: ["src/g4/sequenceParser.g4", "src/parser/OrderedParticipants.ts"],
      rules: ["participant", "head", "OrderedParticipants"],
    },
    checks: [
      ["candidate", "parse.pass", true],
      ["candidate", "render.semantic.messages", 1],
      [
        "candidate",
        "render.topology.participantNames",
        ["_STARTER_", "A", "B"],
      ],
    ],
  },
  {
    id: "missing-typed-participant-recovery",
    classification: "source-verified-recovery-contract",
    source: "@Actor",
    sourceAttribution: {
      paths: ["src/parser/ToCollector.ts", "test/unit/parser/to-collector.spec.js"],
      rules: ["participantType-only", "Missing `Participant`"],
    },
    checks: [
      ["candidate", "parse.pass", true],
      [
        "candidate",
        "render.topology.participantNames",
        ["Missing `Participant`"],
      ],
      ["candidate", "render.topology.participantTypes", ["Actor"]],
    ],
  },
  {
    id: "missing-stereotyped-participant-recovery",
    classification: "source-verified-recovery-contract",
    source: "<<Service>>",
    sourceAttribution: {
      paths: ["src/parser/ToCollector.ts", "test/unit/parser/to-collector.spec.js"],
      rules: ["stereotype-only", "Missing `Participant`"],
    },
    checks: [
      ["candidate", "parse.pass", true],
      [
        "candidate",
        "render.topology.participantNames",
        ["Missing `Participant`"],
      ],
      ["candidate", "render.topology.participantStereotypes", ["Service"]],
    ],
  },
]);

const arguments_ = new Set(process.argv.slice(2));
for (const argument of arguments_) {
  assert(
    argument === "--online" || argument === "--write",
    "usage: node zenuml-core-candidate-matrix.mjs [--online] [--write]"
  );
}
const online = arguments_.has("--online");
const writeMode = arguments_.has("--write");
assert(!writeMode || online, "--write requires --online verification");

const bundle = JSON.parse(await readFile(bundlePath, "utf8"));
const admission = JSON.parse(await readFile(admissionPath, "utf8"));
const zenuml = bundle.externalDiagrams.find((diagram) => diagram.id === "zenuml");
assert(zenuml, "reference bundle must contain ZenUML");
const sources = await loadCorpusSources();
const harnessSha256 = await fileSha256(scriptPath);
const inlineValidatorSources = await Promise.all(
  inlineValidatorRelativePaths.map(async (relativePath) => ({
    path: relativePath,
    sha256: await fileSha256(path.join(workspaceRoot, relativePath)),
  }))
);
const assertSafeSvgWithMessagePrefix = await loadInlineSvgValidator();

if (!online) {
  const evidence = JSON.parse(await readFile(evidencePath, "utf8"));
  const attestationArtifacts = await loadAttestationArtifacts(zenuml);
  verifyEvidence(evidence, {
    bundle,
    harnessSha256,
    sources,
    zenuml,
    attestationArtifacts,
    inlineValidatorSources,
  });
  verifyAdmissionFixtureCounts(admission, evidence.corpus.fixtureCount);
  process.exit(0);
}

const temporaryRoot = await mkdtemp(
  path.join(tmpdir(), "merman-zenuml-candidate-")
);
let browser;
let server;
try {
  const oracle = await materialize(zenuml.behaviorSource.oracle, "oracle");
  const candidate = await materialize(
    zenuml.behaviorSource.candidate,
    "candidate"
  );
  const supplyChainResults = {
    oracle: await verifyPublishedPackage(oracle),
    candidate: await verifyPublishedPackage(candidate),
  };
  const attestationArtifacts = new Map();
  for (const [name, reference] of [
    ["oracle", zenuml.behaviorSource.oracle],
    ["candidate", zenuml.behaviorSource.candidate],
  ]) {
    const artifact = supplyChainResults[name].attestationArtifact;
    if (writeMode) {
      reference.publishProvenance.attestationArtifact.sha256 = artifact.sha256;
    } else {
      assert.equal(
        reference.publishProvenance.attestationArtifact.sha256,
        artifact.sha256,
        `${name} attestation artifact digest drift`
      );
    }
    attestationArtifacts.set(name, artifact);
  }

  server = await serveModules({
    oracle: oracle.distRoot,
    candidate: candidate.distRoot,
  });
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.goto(`${server.origin}/`);
  const observations = await observeInBrowser(page, server.origin, sources);
  const [oracleObservation, candidateObservation] = observations;
  assert.equal(oracleObservation.version, oracle.reference.version);
  assert.equal(candidateObservation.version, candidate.reference.version);

  const parseAgreementCount = countAgreement(
    oracleObservation.rows,
    candidateObservation.rows,
    (row) => row.parse
  );
  const semanticAgreementCount = countAgreement(
    oracleObservation.rows,
    candidateObservation.rows,
    (row) => row.render
  );
  const renderAgreementCount = countAgreement(
    oracleObservation.rows,
    candidateObservation.rows,
    (row) => row.render.svgSha256
  );
  const semanticTotals = sumSemantic(oracleObservation.rows);
  const requiredTopology = requiredTopologyEvidence(candidateObservation.rows);
  const classifiedBehavior = classifyBehaviorProbes(
    oracleObservation.probes,
    candidateObservation.probes
  );
  const strictInlineSvg = validateStrictInlineSvgCorpus(
    candidateObservation.nativeSvgs,
    sources,
    inlineValidatorSources
  );

  const evidence = {
    schemaVersion: 4,
    harness: "playground/scripts/zenuml-core-candidate-matrix.mjs",
    harnessSha256,
    command: "npm run verify:zenuml-candidate",
    onlineCommand: "npm run verify:zenuml-candidate:online",
    oracle: packageEvidence(oracle, supplyChainResults.oracle.evidence),
    candidate: packageEvidence(candidate, supplyChainResults.candidate.evidence),
    pluginContract: {
      declaredRange: zenuml.behaviorSource.declaredRange,
      workspaceRange: zenuml.behaviorSource.workspaceRange,
      candidateSatisfiesDeclaredRange: satisfiesCaret(
        candidate.reference.version,
        zenuml.behaviorSource.declaredRange
      ),
      candidateSatisfiesWorkspaceRange: satisfiesCaret(
        candidate.reference.version,
        zenuml.behaviorSource.workspaceRange
      ),
    },
    corpus: {
      fixtureCount: sources.length,
      corpusDigest: corpusDigest(sources),
      parseAgreementCount,
      sources: sources.map(({ name, sourceReference }) => ({
        name,
        sourceReference,
      })),
    },
    semantic: {
      agreementCount: semanticAgreementCount,
      totals: semanticTotals,
      requiredTopology,
      classifiedBehavior,
    },
    render: {
      svgAgreementCount: renderAgreementCount,
    },
    strictInlineSvg,
    resource: {
      measurementScope: "runtime-entry",
      runtimeEntryDeltaBytes:
        candidate.runtimeEntryBytes - oracle.runtimeEntryBytes,
      runtimeEntryDeltaBasisPoints: Math.round(
        ((candidate.runtimeEntryBytes - oracle.runtimeEntryBytes) * 10_000) /
          oracle.runtimeEntryBytes
      ),
    },
  };

  verifyEvidence(evidence, {
    bundle,
    harnessSha256,
    sources,
    zenuml,
    attestationArtifacts,
    inlineValidatorSources,
  });
  synchronizeAdmissionFixtureCounts(admission, evidence.corpus.fixtureCount);
  verifyAdmissionFixtureCounts(admission, evidence.corpus.fixtureCount);
  const serialized = `${JSON.stringify(evidence, null, 2)}\n`;
  if (writeMode) {
    for (const artifact of attestationArtifacts.values()) {
      await writeAtomically(
        path.join(workspaceRoot, artifact.relativePath),
        artifact.serialized
      );
    }
    await writeAtomically(bundlePath, `${JSON.stringify(bundle, null, 2)}\n`);
    await writeAtomically(admissionPath, `${JSON.stringify(admission, null, 2)}\n`);
    await writeAtomically(evidencePath, serialized);
  } else {
    for (const artifact of attestationArtifacts.values()) {
      assert.equal(
        await readFile(path.join(workspaceRoot, artifact.relativePath), "utf8"),
        artifact.serialized,
        `${artifact.relativePath} is stale; rerun online with --write`
      );
    }
    assert.equal(
      await readFile(evidencePath, "utf8"),
      serialized,
      "ZenUML candidate evidence is stale; rerun online with --write after reviewing the delta"
    );
  }
} finally {
  await browser?.close();
  await server?.close();
  await rm(temporaryRoot, { recursive: true, force: true });
}

async function loadInlineSvgValidator() {
  const source = await readFile(
    path.join(workspaceRoot, "platforms/web/src/svg-safety-policy.ts"),
    "utf8"
  );
  const transpiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: "svg-safety-policy.ts",
    reportDiagnostics: true,
  });
  const errors = (transpiled.diagnostics ?? []).filter(
    (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error
  );
  assert.deepEqual(errors, [], "strict inline SVG validator failed to transpile");
  const encoded = Buffer.from(transpiled.outputText).toString("base64");
  const module = await import(`data:text/javascript;base64,${encoded}`);
  assert.equal(typeof module.assertSafeSvgWithMessagePrefix, "function");
  return module.assertSafeSvgWithMessagePrefix;
}

async function loadCorpusSources() {
  const fixtureDirectory = path.join(workspaceRoot, "fixtures", "zenuml");
  const fixtureNames = (await readdir(fixtureDirectory))
    .filter((name) => name.endsWith(".mmd"))
    .sort();
  const fixtures = await Promise.all(
    fixtureNames.map(async (name) => ({
      name,
      sourceReference: `fixtures/zenuml/${name}`,
      source: stripMermaidHeader(
        await readFile(path.join(fixtureDirectory, name), "utf8")
      ),
    }))
  );
  return [...fixtures, ...COMPATIBILITY_CASES];
}

async function observeInBrowser(page, origin, browserSources) {
  return page.evaluate(
    async ({ entries, probes, sources: inputs }) => {
      const sha256 = async (value) =>
        Array.from(
          new Uint8Array(
            await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value))
          ),
          (byte) => byte.toString(16).padStart(2, "0")
        ).join("");
      const normalize = async (module, parser, source) => {
        const parse = await parser.parse(source);
        try {
          const rendered = module.renderToSvg(source);
          const geometry = rendered.geometry;
          const participants = geometry.participants.map((participant) => ({
            name: participant.name ?? null,
            type: participant.type ?? null,
            stereotype: participant.stereotype ?? null,
            groupId: participant.groupId ?? null,
            x: participant.x,
          }));
          const participantNameAt = (x) =>
            participants.find((participant) => participant.x === x)?.name ?? null;
          const endpoints = (items) =>
            items.map((item) => {
              const from = participantNameAt(item.fromX);
              const to = participantNameAt(item.toX);
              const self = item.isSelf ? (from ?? to) : null;
              return {
                from: self ?? from,
                to: self ?? to,
                isSelf: item.isSelf,
              };
            });
          return {
            parse,
            render: {
              rendered: true,
              svgBytes: new TextEncoder().encode(rendered.svg).byteLength,
              svgSha256: await sha256(rendered.svg),
              semantic: {
                participants: geometry.participants.length,
                messages: geometry.messages.length,
                fragments: geometry.fragments.length,
                groups: geometry.groups.length,
                returns: geometry.returns.length,
                creations: geometry.creations.length,
              },
              topology: {
                participantNames: participants.map(({ name }) => name),
                participantTypes: participants.map(({ type }) => type),
                participantStereotypes: participants.map(
                  ({ stereotype }) => stereotype
                ),
                participantGroupIds: participants.map(({ groupId }) => groupId),
                messageLabels: geometry.messages.map(
                  (message) => message.label ?? null
                ),
                messageEndpoints: endpoints(geometry.messages),
                returnEndpoints: endpoints(geometry.returns),
                fragmentLabels: geometry.fragments.map(
                  (fragment) => fragment.label ?? null
                ),
                groupNames: geometry.groups.map((group) => group.name ?? null),
              },
            },
            svg: rendered.svg,
          };
        } catch (error) {
          return {
            parse,
            render: {
              rendered: false,
              error: error instanceof Error ? error.message : String(error),
            },
            svg: null,
          };
        }
      };

      const engines = [];
      for (const [id, url] of entries) {
        const module = await import(url);
        const parser = Object.create(module.default.prototype);
        const rows = [];
        const nativeSvgs = [];
        for (const fixture of inputs) {
          const observed = await normalize(module, parser, fixture.source);
          if (id === "candidate" && observed.svg !== null) {
            nativeSvgs.push({ name: fixture.name, svg: observed.svg });
          }
          rows.push({ name: fixture.name, parse: observed.parse, render: observed.render });
        }
        const probeRows = [];
        for (const probe of probes) {
          const observed = await normalize(module, parser, probe.source);
          probeRows.push({ id: probe.id, parse: observed.parse, render: observed.render });
        }
        engines.push({
          id,
          version: module.default.version,
          rows,
          probes: probeRows,
          nativeSvgs,
        });
      }
      return engines;
    },
    {
      entries: [
        ["oracle", `${origin}/oracle/zenuml.esm.mjs`],
        ["candidate", `${origin}/candidate/zenuml.esm.mjs`],
      ],
      probes: BEHAVIOR_PROBES.map(({ id, source }) => ({ id, source })),
      sources: browserSources,
    }
  );
}

function countAgreement(oracleRows, candidateRows, project) {
  return oracleRows.filter(
    (row, index) =>
      candidateRows[index]?.name === row.name &&
      JSON.stringify(project(row)) === JSON.stringify(project(candidateRows[index]))
  ).length;
}

function sumSemantic(rows) {
  const totals = {};
  for (const row of rows) {
    assert.equal(row.render.rendered, true, `${row.name} did not render`);
    for (const [name, count] of Object.entries(row.render.semantic)) {
      totals[name] = (totals[name] ?? 0) + count;
    }
  }
  return totals;
}

function requiredTopologyEvidence(rows) {
  const byName = new Map(rows.map((row) => [row.name, row]));
  const count = (name, field) => {
    const value = byName.get(name)?.render?.semantic?.[field];
    assert.equal(typeof value, "number", `missing ${name} ${field}`);
    return value;
  };
  return {
    participantGroups: count("advanced-participant-group", "groups"),
    parallelFragments: count("advanced-parallel-fragment", "fragments"),
    tryCatchFinallyFragments: count("advanced-try-catch-finally", "fragments"),
    emojiMessages: count("advanced-emoji-message", "messages"),
    numberUnitConditionFragments: count("advanced-known-number-unit", "fragments"),
  };
}

function classifyBehaviorProbes(oracleRows, candidateRows) {
  const oracle = new Map(oracleRows.map((row) => [row.id, row]));
  const candidate = new Map(candidateRows.map((row) => [row.id, row]));
  return BEHAVIOR_PROBES.map((probe) => {
    const observations = {
      oracle: oracle.get(probe.id),
      candidate: candidate.get(probe.id),
    };
    assert(observations.oracle && observations.candidate, `missing ${probe.id}`);
    const assertions = probe.checks.map(([engine, fact, expected]) => {
      const actual = readPath(observations[engine], fact);
      assert.deepEqual(actual, expected, `${probe.id} ${engine}.${fact}`);
      return { engine, fact, expected, actual };
    });
    return {
      id: probe.id,
      classification: probe.classification,
      source: probe.source,
      sourceAttribution: probe.sourceAttribution,
      assertions,
      oracle: observations.oracle,
      candidate: observations.candidate,
    };
  });
}

function validateStrictInlineSvgCorpus(nativeSvgs, sources, validatorSources) {
  assert.equal(nativeSvgs.length, sources.length);
  assert.deepEqual(
    nativeSvgs.map(({ name }) => name),
    sources.map(({ name }) => name)
  );
  let totalSvgBytes = 0;
  let maxSvgBytes = 0;
  const fixtures = [];
  for (const { name, svg } of nativeSvgs) {
    assert.equal(typeof svg, "string", `${name} has no native SVG output`);
    assertSafeSvgWithMessagePrefix(svg, `ZenUML fixture ${name}`);
    assert.doesNotMatch(
      svg,
      /<foreignObject(?:\s|>)/iu,
      `${name} unexpectedly requires foreignObject publication`
    );
    const bytes = Buffer.byteLength(svg);
    assert(bytes > 0, `${name} produced an empty SVG`);
    totalSvgBytes += bytes;
    maxSvgBytes = Math.max(maxSvgBytes, bytes);
    fixtures.push({
      name,
      svgBytes: bytes,
      svgSha256: createHash("sha256").update(svg).digest("hex"),
      foreignObjectFree: true,
      passed: true,
    });
  }
  return {
    fixtureCount: nativeSvgs.length,
    passedCount: nativeSvgs.length,
    totalSvgBytes,
    maxSvgBytes,
    validatorSources,
    fixtures,
  };
}

async function materialize(reference, id) {
  const root = path.join(temporaryRoot, id);
  await mkdir(root);
  const npm = process.platform === "win32" ? "npm.cmd" : "npm";
  const result = spawnSync(
    npm,
    [
      "install",
      "--ignore-scripts",
      `--registry=${officialNpmRegistry}`,
      "--no-audit",
      "--no-fund",
      "--package-lock=true",
      "--save-exact",
      `${reference.package}@${reference.version}`,
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 128 * 1024 * 1024 }
  );
  assert.equal(result.status, 0, `${id} materialization failed:\n${result.stderr}`);
  const lock = JSON.parse(await readFile(path.join(root, "package-lock.json"), "utf8"));
  const installed = lock.packages[`node_modules/${reference.package}`];
  assert.equal(installed.version, reference.version);
  assert.equal(installed.integrity, reference.integrity);
  assert.equal(installed.resolved, reference.tarballUrl);
  const distRoot = path.join(
    root,
    "node_modules",
    ...reference.package.split("/"),
    "dist"
  );
  const runtimeEntryBytes = (await stat(path.join(distRoot, "zenuml.esm.mjs"))).size;
  return { reference, root, distRoot, runtimeEntryBytes };
}

async function verifyPublishedPackage(materialized) {
  const { reference, root } = materialized;
  const provenance = reference.publishProvenance;
  assert(provenance, `${reference.package}@${reference.version} has no publish provenance`);
  const packageUrl = `${officialNpmRegistry}${encodeURIComponent(reference.package)}`;
  const { json: packument } = await fetchJson(packageUrl);
  const published = packument.versions?.[reference.version];
  assert(published, `${reference.package}@${reference.version} is not published`);
  assert.equal(published.name, reference.package);
  assert.equal(published.version, reference.version);
  assert.equal(published.gitHead, reference.source.commit);
  assert.equal(published.dist.integrity, reference.integrity);
  assert.equal(published.dist.tarball, reference.tarballUrl);
  assert.equal(published.dist.attestations?.url, reference.attestationUrl);
  assert.equal(published.dist.attestations?.provenance?.predicateType, slsaPredicate);

  const tarballResponse = await fetch(reference.tarballUrl, { redirect: "error" });
  assert.equal(tarballResponse.status, 200);
  const tarball = Buffer.from(await tarballResponse.arrayBuffer());
  const tarballSha512Base64 = createHash("sha512").update(tarball).digest("base64");
  const tarballSha512Hex = createHash("sha512").update(tarball).digest("hex");
  assert.equal(reference.integrity, `sha512-${tarballSha512Base64}`);

  const { json: endpoint } = await fetchJson(reference.attestationUrl);
  assert(Array.isArray(endpoint.attestations));
  assert.deepEqual(
    endpoint.attestations.map(({ predicateType }) => predicateType).sort(),
    [publishPredicate, slsaPredicate].sort()
  );

  const npm = process.platform === "win32" ? "npm.cmd" : "npm";
  const npmVersion = spawnSync(npm, ["--version"], {
    encoding: "utf8",
  }).stdout.trim();
  assert(
    versionAtLeast(npmVersion, "11.17.0"),
    "npm 11.17.0 or newer is required"
  );
  const auditResult = spawnSync(
    npm,
    [
      "audit",
      "signatures",
      "--json",
      "--include-attestations",
      `--registry=${officialNpmRegistry}`,
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 128 * 1024 * 1024 }
  );
  assert.equal(auditResult.status, 0, auditResult.stderr || auditResult.stdout);
  const audit = JSON.parse(auditResult.stdout);
  assert.deepEqual(audit.invalid, []);
  assert.deepEqual(audit.missing, []);
  const verified = audit.verified?.find(
    (entry) => entry.name === reference.package && entry.version === reference.version
  );
  assert(verified, `npm did not verify ${reference.package}@${reference.version}`);
  assert.equal(verified.registry, officialNpmRegistry);
  assert.equal(verified.attestations?.url, reference.attestationUrl);
  assert.deepEqual(
    verified.attestationBundles.map(({ predicateType }) => predicateType).sort(),
    [publishPredicate, slsaPredicate].sort()
  );

  const endpointByType = new Map(
    endpoint.attestations.map((attestation) => [attestation.predicateType, attestation])
  );
  const auditByType = new Map(
    verified.attestationBundles.map((attestation) => [
      attestation.predicateType,
      attestation,
    ])
  );
  for (const predicate of [publishPredicate, slsaPredicate]) {
    assert.deepEqual(
      auditByType.get(predicate)?.bundle?.dsseEnvelope,
      endpointByType.get(predicate)?.bundle?.dsseEnvelope,
      `${predicate} endpoint and npm-verified envelope differ`
    );
  }

  const publishAttestation = attestationEvidence(
    auditByType.get(publishPredicate),
    tarballSha512Hex,
    reference
  );
  const slsaAttestation = attestationEvidence(
    auditByType.get(slsaPredicate),
    tarballSha512Hex,
    reference
  );
  const publishStatement = publishAttestation.statement;
  assert.equal(publishStatement.predicate.name, reference.package);
  assert.equal(publishStatement.predicate.version, reference.version);
  assert.equal(publishStatement.predicate.registry, "https://registry.npmjs.org");

  const workflow = slsaAttestation.statement.predicate.buildDefinition.externalParameters.workflow;
  assert.deepEqual(workflow, provenance.workflow);
  const resolved = slsaAttestation.statement.predicate.buildDefinition.resolvedDependencies;
  assert(
    resolved.some(
      (dependency) => dependency.digest?.gitCommit === reference.source.commit
    ),
    "SLSA provenance does not bind the published git commit"
  );
  const certificateBytes = auditByType.get(slsaPredicate).bundle.verificationMaterial
    .certificate.rawBytes;
  const certificate = new X509Certificate(
    Buffer.from(certificateBytes, "base64")
  );
  assert.equal(certificate.subjectAltName, expectedWorkflowSan(provenance.workflow));

  const attestationArtifact = createAttestationArtifact(reference, endpoint.attestations);

  return {
    evidence: {
      attestationArtifact: {
        path: attestationArtifact.relativePath,
        sha256: attestationArtifact.sha256,
      },
      tarballBytes: tarball.byteLength,
      tarballSha512Base64,
      tarballSha512Hex,
      npmAudit: {
        cliVersion: npmVersion,
        registry: verified.registry,
        verified: true,
        predicateTypes: verified.attestationBundles
          .map(({ predicateType }) => predicateType)
          .sort(),
      },
      subject: publishAttestation.subject,
      publish: {
        predicateType: publishPredicate,
        envelopeSha256: publishAttestation.envelopeSha256,
        payloadSha256: publishAttestation.payloadSha256,
        registry: publishStatement.predicate.registry,
      },
      slsa: {
        predicateType: slsaPredicate,
        envelopeSha256: slsaAttestation.envelopeSha256,
        payloadSha256: slsaAttestation.payloadSha256,
        resolvedGitCommit: reference.source.commit,
        workflow,
        certificateIssuer: certificate.issuer,
        certificateSubjectAltName: certificate.subjectAltName,
        certificateFingerprint256: certificate.fingerprint256,
      },
    },
    attestationArtifact,
  };
}

function attestationEvidence(attestation, tarballSha512Hex, reference) {
  assert(attestation?.bundle?.dsseEnvelope);
  const envelope = attestation.bundle.dsseEnvelope;
  assert.equal(envelope.payloadType, "application/vnd.in-toto+json");
  const payloadBytes = Buffer.from(envelope.payload, "base64");
  const statement = JSON.parse(payloadBytes.toString("utf8"));
  assert.equal(statement.predicateType, attestation.predicateType);
  const expectedSubject = `pkg:npm/${reference.package.replace(/^@/u, "%40")}@${reference.version}`;
  assert.equal(statement.subject?.length, 1);
  assert.equal(statement.subject[0].name, expectedSubject);
  assert.equal(statement.subject[0].digest?.sha512, tarballSha512Hex);
  return {
    statement,
    subject: statement.subject[0],
    payloadSha256: createHash("sha256").update(payloadBytes).digest("hex"),
    envelopeSha256: createHash("sha256")
      .update(canonicalJson(envelope))
      .digest("hex"),
  };
}

function createAttestationArtifact(reference, attestations) {
  const descriptor = reference.publishProvenance?.attestationArtifact;
  assert(descriptor, `${reference.package}@${reference.version} has no attestation artifact`);
  assertOwnedRelativePath(descriptor.path);
  const value = {
    schemaVersion: attestationArtifactSchemaVersion,
    package: reference.package,
    version: reference.version,
    attestations: attestations
      .map(({ predicateType, bundle }) => ({ predicateType, bundle }))
      .sort((left, right) => left.predicateType.localeCompare(right.predicateType)),
  };
  const serialized = canonicalPrettyJson(value);
  assert(
    Buffer.byteLength(serialized) <= maxAttestationArtifactBytes,
    `${descriptor.path} exceeds the attestation artifact budget`
  );
  return {
    relativePath: descriptor.path,
    serialized,
    sha256: createHash("sha256").update(serialized).digest("hex"),
    value: JSON.parse(serialized),
  };
}

async function loadAttestationArtifacts(zenuml) {
  const artifacts = new Map();
  for (const [name, reference] of [
    ["oracle", zenuml.behaviorSource.oracle],
    ["candidate", zenuml.behaviorSource.candidate],
  ]) {
    const descriptor = reference.publishProvenance?.attestationArtifact;
    assert(descriptor, `${reference.package}@${reference.version} has no attestation artifact`);
    assertOwnedRelativePath(descriptor.path);
    const serialized = await readFile(
      path.join(workspaceRoot, descriptor.path),
      "utf8"
    );
    assert(
      Buffer.byteLength(serialized) <= maxAttestationArtifactBytes,
      `${descriptor.path} exceeds the attestation artifact budget`
    );
    const value = JSON.parse(serialized);
    assert.equal(
      serialized,
      canonicalPrettyJson(value),
      `${descriptor.path} is not canonical JSON`
    );
    artifacts.set(name, {
      relativePath: descriptor.path,
      serialized,
      sha256: createHash("sha256").update(serialized).digest("hex"),
      value,
    });
  }
  return artifacts;
}

function expectedWorkflowSan(workflow) {
  return `URI:${workflow.repository}/${workflow.path}@${workflow.ref}`;
}

function assertOwnedRelativePath(relativePath) {
  assert.equal(typeof relativePath, "string");
  assert(relativePath.length > 0);
  assert(!path.isAbsolute(relativePath));
  assert(!relativePath.includes("\\"));
  assert(
    relativePath
      .split("/")
      .every((component) => component.length > 0 && component !== "." && component !== "..")
  );
}

function canonicalJson(value) {
  return JSON.stringify(canonicalizeJson(value));
}

function canonicalPrettyJson(value) {
  return `${JSON.stringify(canonicalizeJson(value), null, 2)}\n`;
}

function canonicalizeJson(value) {
  if (Array.isArray(value)) {
    return value.map(canonicalizeJson);
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonicalizeJson(value[key])])
    );
  }
  return value;
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

function packageEvidence(materialized, supplyChain) {
  return {
    version: materialized.reference.version,
    commit: materialized.reference.source.commit,
    integrity: materialized.reference.integrity,
    tarballUrl: materialized.reference.tarballUrl,
    attestationUrl: materialized.reference.attestationUrl,
    runtimeEntryBytes: materialized.runtimeEntryBytes,
    supplyChain,
  };
}

function synchronizeAdmissionFixtureCounts(admission, fixtureCount) {
  for (const entry of admission.matrix) {
    const summary = FIXTURE_COUNT_ADMISSION_SUMMARIES[entry.gate];
    if (!summary) continue;
    entry.evidence.fixtureCount = fixtureCount;
    entry.evidence.summary = summary;
  }
}

function verifyAdmissionFixtureCounts(admission, fixtureCount) {
  for (const [gate, summary] of Object.entries(
    FIXTURE_COUNT_ADMISSION_SUMMARIES
  )) {
    const entry = admission.matrix.find((candidate) => candidate.gate === gate);
    assert(entry, `ZenUML admission has no ${gate} gate`);
    assert.equal(entry.evidence.fixtureCount, fixtureCount);
    assert.equal(entry.evidence.summary, summary);
  }
}

function verifyEvidence(evidence, context) {
  const {
    harnessSha256: expectedHarness,
    sources,
    zenuml,
    attestationArtifacts,
    inlineValidatorSources,
  } = context;
  assert.equal(evidence.schemaVersion, 4);
  assert.equal(evidence.harness, "playground/scripts/zenuml-core-candidate-matrix.mjs");
  assert.equal(evidence.harnessSha256, expectedHarness);
  assert.equal(evidence.command, "npm run verify:zenuml-candidate");
  assert.equal(evidence.onlineCommand, "npm run verify:zenuml-candidate:online");
  for (const [name, reference] of [
    ["oracle", zenuml.behaviorSource.oracle],
    ["candidate", zenuml.behaviorSource.candidate],
  ]) {
    const value = evidence[name];
    assert.equal(value.version, reference.version);
    assert.equal(value.commit, reference.source.commit);
    assert.equal(value.integrity, reference.integrity);
    assert.equal(value.tarballUrl, reference.tarballUrl);
    assert.equal(value.attestationUrl, reference.attestationUrl);
    assert(value.runtimeEntryBytes > 0);
    verifySupplyChainEvidence(
      value.supplyChain,
      reference,
      attestationArtifacts.get(name)
    );
  }
  assert.equal(evidence.pluginContract.candidateSatisfiesDeclaredRange, true);
  assert.equal(evidence.pluginContract.candidateSatisfiesWorkspaceRange, true);
  assert.equal(evidence.corpus.fixtureCount, sources.length);
  assert.equal(evidence.corpus.corpusDigest, corpusDigest(sources));
  assert.equal(evidence.corpus.parseAgreementCount, sources.length);
  assert.equal(evidence.semantic.agreementCount, sources.length);
  assert.equal(evidence.render.svgAgreementCount, sources.length);
  for (const field of [
    "participants",
    "messages",
    "fragments",
    "groups",
    "returns",
    "creations",
  ]) {
    assert(evidence.semantic.totals[field] > 0, `${field} coverage is empty`);
  }
  for (const value of Object.values(evidence.semantic.requiredTopology)) {
    assert(value > 0, "required ZenUML topology coverage is empty");
  }
  assert.equal(
    evidence.semantic.classifiedBehavior.length,
    BEHAVIOR_PROBES.length
  );
  for (const behavior of evidence.semantic.classifiedBehavior) {
    assert(behavior.sourceAttribution.paths.length > 0);
    assert(behavior.sourceAttribution.rules.length > 0);
    assert(behavior.assertions.length > 0);
    for (const assertion of behavior.assertions) {
      assert.deepEqual(assertion.actual, assertion.expected);
    }
  }
  assert.equal(evidence.strictInlineSvg.fixtureCount, sources.length);
  assert.equal(evidence.strictInlineSvg.passedCount, sources.length);
  assert.deepEqual(
    evidence.strictInlineSvg.fixtures.map(({ name }) => name),
    sources.map(({ name }) => name)
  );
  assert(evidence.strictInlineSvg.fixtures.every(({ passed }) => passed === true));
  assert(
    evidence.strictInlineSvg.fixtures.every(
      ({ foreignObjectFree }) => foreignObjectFree === true
    )
  );
  const strictSvgBytes = evidence.strictInlineSvg.fixtures.map(
    ({ svgBytes }) => svgBytes
  );
  assert.equal(
    evidence.strictInlineSvg.totalSvgBytes,
    strictSvgBytes.reduce((total, bytes) => total + bytes, 0)
  );
  assert.equal(evidence.strictInlineSvg.maxSvgBytes, Math.max(...strictSvgBytes));
  for (const fixture of evidence.strictInlineSvg.fixtures) {
    assert(fixture.svgBytes > 0);
    assert.match(fixture.svgSha256, /^[a-f0-9]{64}$/u);
  }
  assert.deepEqual(
    evidence.strictInlineSvg.validatorSources,
    inlineValidatorSources
  );
  const expectedDelta =
    evidence.candidate.runtimeEntryBytes - evidence.oracle.runtimeEntryBytes;
  assert.equal(evidence.resource.runtimeEntryDeltaBytes, expectedDelta);
  assert.equal(
    evidence.resource.runtimeEntryDeltaBasisPoints,
    Math.round((expectedDelta * 10_000) / evidence.oracle.runtimeEntryBytes)
  );
}

function verifySupplyChainEvidence(supply, reference, artifact) {
  assert(artifact, `${reference.package}@${reference.version} has no loaded artifact`);
  const provenance = reference.publishProvenance;
  assert(provenance, `${reference.package}@${reference.version} has no publish provenance`);
  assert.equal(
    provenance.workflow.repository,
    reference.source.repository.replace(/\.git$/u, "")
  );
  assertOwnedRelativePath(provenance.workflow.path);
  assert(provenance.workflow.ref.length > 0);

  assert.equal(supply.attestationArtifact.path, artifact.relativePath);
  assert.equal(supply.attestationArtifact.path, provenance.attestationArtifact.path);
  assert.equal(supply.attestationArtifact.sha256, artifact.sha256);
  assert.equal(supply.attestationArtifact.sha256, provenance.attestationArtifact.sha256);
  assert.match(artifact.sha256, /^[a-f0-9]{64}$/u);
  assert.equal(artifact.serialized, canonicalPrettyJson(artifact.value));
  assert.deepEqual(Object.keys(artifact.value).sort(), [
    "attestations",
    "package",
    "schemaVersion",
    "version",
  ]);
  assert.equal(artifact.value.schemaVersion, attestationArtifactSchemaVersion);
  assert.equal(artifact.value.package, reference.package);
  assert.equal(artifact.value.version, reference.version);
  assert.equal(artifact.value.attestations.length, 2);

  const attestations = new Map(
    artifact.value.attestations.map((attestation) => [
      attestation.predicateType,
      attestation,
    ])
  );
  assert.equal(attestations.size, 2);
  assert.deepEqual([...attestations.keys()].sort(), [publishPredicate, slsaPredicate].sort());
  for (const attestation of attestations.values()) {
    const envelope = attestation.bundle?.dsseEnvelope;
    assert(envelope, `${attestation.predicateType} has no DSSE envelope`);
    assert.equal(envelope.payloadType, "application/vnd.in-toto+json");
    assertValidBase64(envelope.payload, maxAttestationPayloadBytes);
    assert(envelope.signatures.length > 0 && envelope.signatures.length <= 4);
    for (const signature of envelope.signatures) {
      assert.equal(typeof signature.keyid, "string");
      assertValidBase64(signature.sig, 4096);
    }
  }

  const publishAttestation = attestationEvidence(
    attestations.get(publishPredicate),
    supply.tarballSha512Hex,
    reference
  );
  const slsaAttestation = attestationEvidence(
    attestations.get(slsaPredicate),
    supply.tarballSha512Hex,
    reference
  );
  for (const [recorded, derived] of [
    [supply.publish, publishAttestation],
    [supply.slsa, slsaAttestation],
  ]) {
    assert.equal(recorded.envelopeSha256, derived.envelopeSha256);
    assert.equal(recorded.payloadSha256, derived.payloadSha256);
  }
  assert.equal(
    reference.integrity,
    `sha512-${supply.tarballSha512Base64}`
  );
  assert.match(supply.tarballSha512Hex, /^[a-f0-9]{128}$/u);
  assert(supply.tarballBytes > 0);
  assert.deepEqual(supply.subject, publishAttestation.subject);
  assert.deepEqual(slsaAttestation.subject, publishAttestation.subject);
  assert.equal(supply.npmAudit.registry, officialNpmRegistry);
  assert.equal(supply.npmAudit.verified, true);
  assert(Number(supply.npmAudit.cliVersion.split(".")[0]) >= 11);
  assert.deepEqual(supply.npmAudit.predicateTypes, [publishPredicate, slsaPredicate].sort());
  assert.equal(supply.publish.predicateType, publishPredicate);
  assert.equal(supply.publish.registry, "https://registry.npmjs.org");
  assert.equal(publishAttestation.statement.predicate.name, reference.package);
  assert.equal(publishAttestation.statement.predicate.version, reference.version);
  assert.equal(
    publishAttestation.statement.predicate.registry,
    "https://registry.npmjs.org"
  );
  assert.equal(supply.slsa.predicateType, slsaPredicate);
  assert.equal(supply.slsa.resolvedGitCommit, reference.source.commit);
  const buildDefinition = slsaAttestation.statement.predicate.buildDefinition;
  assert.deepEqual(
    buildDefinition.externalParameters.workflow,
    provenance.workflow
  );
  assert(
    buildDefinition.resolvedDependencies.some(
      (dependency) => dependency.digest?.gitCommit === reference.source.commit
    )
  );
  assert.deepEqual(supply.slsa.workflow, provenance.workflow);
  assert.match(supply.slsa.certificateIssuer, /sigstore/u);
  assert.equal(
    supply.slsa.certificateSubjectAltName,
    expectedWorkflowSan(provenance.workflow)
  );
  assert.match(
    supply.slsa.certificateFingerprint256,
    /^(?:[A-F0-9]{2}:){31}[A-F0-9]{2}$/u
  );
}

function assertValidBase64(value, maxDecodedBytes) {
  assert.equal(typeof value, "string");
  assert.match(value, /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u);
  const decoded = Buffer.from(value, "base64");
  assert(decoded.length > 0 && decoded.length <= maxDecodedBytes);
  assert.equal(decoded.toString("base64"), value);
}

function readPath(value, pointer) {
  return pointer.split(".").reduce((current, part) => current?.[part], value);
}

function corpusDigest(inputs) {
  return createHash("sha256").update(JSON.stringify(inputs)).digest("hex");
}

async function fileSha256(file) {
  return createHash("sha256").update(await readFile(file)).digest("hex");
}

async function fetchJson(url) {
  const response = await fetch(url, { redirect: "error" });
  assert.equal(response.status, 200, `${url} returned ${response.status}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  return { bytes, json: JSON.parse(bytes.toString("utf8")) };
}

function stripMermaidHeader(source) {
  return source.replace(/^\s*zenuml\s*(?:\r?\n|$)/u, "");
}

function satisfiesCaret(version, range) {
  assert.match(range, /^\^\d+\.\d+\.\d+$/u);
  const candidate = version.split(".").map(Number);
  const minimum = range.slice(1).split(".").map(Number);
  return (
    candidate[0] === minimum[0] &&
    (candidate[1] > minimum[1] ||
      (candidate[1] === minimum[1] && candidate[2] >= minimum[2]))
  );
}

function versionAtLeast(version, minimum) {
  const parse = (value) => {
    assert.match(value, /^\d+\.\d+\.\d+$/u);
    return value.split(".").map(Number);
  };
  const actual = parse(version);
  const required = parse(minimum);
  return actual.some(
    (component, index) =>
      component > required[index] &&
      actual.slice(0, index).every((value, prefix) => value === required[prefix])
  ) || actual.every((value, index) => value === required[index]);
}

async function serveModules(roots) {
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url, "http://127.0.0.1");
      if (url.pathname === "/") {
        response.setHeader("content-type", "text/html; charset=utf-8");
        response.end("<!doctype html><meta charset=utf-8>");
        return;
      }
      const [, id, ...segments] = url.pathname.split("/");
      const root = roots[id];
      assert(root, "unknown module root");
      const target = path.resolve(root, ...segments.map(decodeURIComponent));
      assert(
        target.startsWith(`${path.resolve(root)}${path.sep}`),
        "path escaped module root"
      );
      response.setHeader("content-type", contentType(target));
      response.end(await readFile(target));
    } catch (error) {
      response.statusCode = 404;
      response.end(String(error));
    }
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  assert(address && typeof address !== "string");
  return {
    origin: `http://127.0.0.1:${address.port}`,
    close: () =>
      new Promise((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve()))
      ),
  };
}

function contentType(file) {
  switch (path.extname(file)) {
    case ".js":
    case ".mjs":
      return "text/javascript; charset=utf-8";
    case ".css":
      return "text/css; charset=utf-8";
    case ".json":
      return "application/json; charset=utf-8";
    case ".ttf":
      return "font/ttf";
    default:
      return "application/octet-stream";
  }
}
