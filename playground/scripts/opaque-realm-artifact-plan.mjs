const NAME = /^[a-z][A-Za-z0-9-]*$/u;
const EXPORT_NAME = /^[A-Za-z_$][A-Za-z0-9_$]*$/u;
const CSP_PLACEHOLDER = /^__MERMAN_[A-Z0-9_]+_CSP_HASH__$/u;
const RESOURCE_POLICIES = new Set(["none-v1", "same-origin-wasm-v1"]);
const CSP_PROFILES = new Set([
  "playground-v1",
  "benchmark-corpus-v1",
  "trusted-benchmark-v1",
]);

export const OPAQUE_REALM_ARTIFACT_PLAN = defineOpaqueRealmArtifactPlan({
  schemaVersion: 1,
  roots: {
    generated: ".runtime",
    publicEngines: "public/opaque-realm",
  },
  browserMetadataModule:
    "src/runtime/realm/generated/opaque-realm-plan.generated.ts",
  engines: [
    {
      id: "mermaid",
      entry: "src/runtime/realm/engines/mermaid-engine-artifact-entry.ts",
      outputBase: "mermaid-engine",
      publish: true,
      maxBytes: 12 * 1024 * 1024,
      resourcePolicy: "none-v1",
      exports: ["benchmarkEngineAdapter", "renderWithMermaid"],
    },
    {
      id: "benchmark-merman",
      entry:
        "src/benchmark/realm/engines/benchmark-merman-artifact-entry.ts",
      outputBase: "benchmark-merman-engine",
      publish: true,
      maxBytes: 256 * 1024,
      resourcePolicy: "same-origin-wasm-v1",
      exports: ["benchmarkEngineAdapter"],
      browserProjection: {
        module:
          "src/benchmark/realm/generated/benchmark-merman.generated.ts",
        exportName: "BENCHMARK_MERMAN_ARTIFACT_PROJECTION",
      },
    },
  ],
  realms: [
    {
      key: "compare-mermaid",
      kind: "compare",
      engine: "mermaid",
      bootstrap: {
        entry: "src/runtime/realm/opaque-compare-entry.ts",
        outputBase: "opaque-compare-bootstrap",
        cspPlaceholder: "__MERMAN_COMPARE_BOOTSTRAP_CSP_HASH__",
        maxBytes: 256 * 1024,
        browserProjection: {
          module:
            "src/runtime/realm/generated/compare-mermaid.generated.ts",
          exportName: "COMPARE_MERMAID_ARTIFACT_PROJECTION",
        },
      },
    },
    {
      key: "benchmark-mermaid",
      kind: "benchmark",
      engine: "mermaid",
      bootstrap: {
        entry: "src/benchmark/realm/opaque-mermaid-entry.ts",
        outputBase: "opaque-benchmark-mermaid-bootstrap",
        cspPlaceholder: "__MERMAN_BENCHMARK_BOOTSTRAP_CSP_HASH__",
        maxBytes: 256 * 1024,
        browserProjection: {
          module:
            "src/benchmark/realm/generated/benchmark-mermaid.generated.ts",
          exportName: "BENCHMARK_MERMAID_ARTIFACT_PROJECTION",
        },
      },
    },
    {
      key: "benchmark-merman",
      kind: "benchmark",
      engine: "benchmark-merman",
      page: "benchmarkRealm",
    },
  ],
  pages: [
    {
      key: "playground",
      source: "index.html",
      cspProfile: "playground-v1",
      inlineRealms: ["compare-mermaid", "benchmark-mermaid"],
    },
    {
      key: "benchmarkCorpus",
      source: "benchmark-corpus.html",
      cspProfile: "benchmark-corpus-v1",
      inlineRealms: ["benchmark-mermaid"],
    },
    {
      key: "benchmarkRealm",
      source: "benchmark.html",
      cspProfile: "trusted-benchmark-v1",
      inlineRealms: [],
    },
  ],
});

export function defineOpaqueRealmArtifactPlan(input) {
  if (!isRecord(input) || input.schemaVersion !== 1 || !isRecord(input.roots)) {
    throw new Error("Opaque realm artifact plan must use schema version 1.");
  }
  const plan = structuredClone(input);
  relativePath(plan.roots.generated, "generated root");
  const publicRoot = relativePath(
    plan.roots.publicEngines,
    "public engine root",
  );
  if (!publicRoot.startsWith("public/") || publicRoot === "public/") {
    throw new Error("Public engine root must be inside public/.");
  }
  typescriptModule(plan.browserMetadataModule, "browser metadata module");
  requiredArray(plan.engines, "engines");
  requiredArray(plan.realms, "realms");
  requiredArray(plan.pages, "pages");

  for (const [index, engine] of plan.engines.entries()) {
    if (!isRecord(engine)) throw new Error(`Engine ${index} must be an object.`);
    name(engine.id, `engine ${index} id`);
    typescriptModule(engine.entry, `engine ${engine.id} entry`);
    outputBase(engine.outputBase, `engine ${engine.id} output`);
    if (typeof engine.publish !== "boolean") {
      throw new Error(`Engine ${engine.id} publish policy must be boolean.`);
    }
    positiveInteger(engine.maxBytes, `engine ${engine.id} byte budget`);
    enumValue(
      engine.resourcePolicy,
      RESOURCE_POLICIES,
      `engine ${engine.id} resource policy`,
    );
    const exports = stringArray(engine.exports, `engine ${engine.id} exports`);
    if (exports.length === 0 || exports.some((item) => !EXPORT_NAME.test(item))) {
      throw new Error(`Engine ${engine.id} exports are invalid.`);
    }
    assertUnique(exports, `engine ${engine.id} export`);
    if (engine.browserProjection !== undefined) {
      browserProjection(engine.browserProjection, `engine ${engine.id}`);
    }
  }

  const engineIds = new Set(plan.engines.map((engine) => engine.id));
  for (const realm of plan.realms) {
    if (!isRecord(realm)) throw new Error("Realm must be an object.");
    name(realm.key, "realm key");
    enumValue(
      realm.kind,
      new Set(["compare", "benchmark"]),
      `realm ${realm.key} kind`,
    );
    name(realm.engine, `realm ${realm.key} engine`);
    if (!engineIds.has(realm.engine)) {
      throw new Error(
        `Realm ${realm.key} references unknown engine ${realm.engine}.`,
      );
    }
    if (isRecord(realm.bootstrap)) {
      if (realm.page !== undefined) {
        throw new Error(`Opaque realm ${realm.key} cannot declare a page.`);
      }
      typescriptModule(
        realm.bootstrap.entry,
        `realm ${realm.key} bootstrap entry`,
      );
      outputBase(
        realm.bootstrap.outputBase,
        `realm ${realm.key} bootstrap output`,
      );
      if (!CSP_PLACEHOLDER.test(String(realm.bootstrap.cspPlaceholder))) {
        throw new Error(`Realm ${realm.key} CSP placeholder is invalid.`);
      }
      positiveInteger(
        realm.bootstrap.maxBytes,
        `realm ${realm.key} bootstrap byte budget`,
      );
      browserProjection(realm.bootstrap.browserProjection, `realm ${realm.key}`);
    } else {
      if (realm.bootstrap !== undefined) {
        throw new Error(`Realm ${realm.key} bootstrap must be an object.`);
      }
      name(realm.page, `realm ${realm.key} page`);
    }
  }

  const realmKeys = new Set(plan.realms.map((realm) => realm.key));
  for (const page of plan.pages) {
    if (!isRecord(page)) throw new Error("Page must be an object.");
    name(page.key, "page key");
    const source = relativePath(page.source, `page ${page.key} source`);
    if (!source.endsWith(".html")) {
      throw new Error(`Page ${page.key} source must be HTML.`);
    }
    enumValue(page.cspProfile, CSP_PROFILES, `page ${page.key} CSP profile`);
    const inlineRealms = stringArray(
      page.inlineRealms,
      `page ${page.key} inline realms`,
    );
    assertUnique(inlineRealms, `page ${page.key} inline realm`);
    for (const key of inlineRealms) {
      const realm = plan.realms.find((candidate) => candidate.key === key);
      if (!realmKeys.has(key)) {
        throw new Error(`Page ${page.key} references unknown realm ${key}.`);
      }
      if (!isRecord(realm.bootstrap)) {
        throw new Error(`Page ${page.key} cannot inline same-origin realm ${key}.`);
      }
    }
  }

  assertPlanOwnership(plan);
  return deepFreeze(plan);
}

export function engineForId(plan, id) {
  const engine = plan.engines.find((candidate) => candidate.id === id);
  if (!engine) throw new Error(`Unknown artifact engine ${id}.`);
  return engine;
}

export function realmForKey(plan, key) {
  const realm = plan.realms.find((candidate) => candidate.key === key);
  if (!realm) throw new Error(`Unknown artifact realm ${key}.`);
  return realm;
}

export function pageForKey(plan, key) {
  const page = plan.pages.find((candidate) => candidate.key === key);
  if (!page) throw new Error(`Unknown artifact page ${key}.`);
  return page;
}

export function artifactOutputFiles(plan) {
  return Object.freeze(
    [
      ...plan.engines.map((engine) => engine.outputBase),
      ...plan.realms.flatMap((realm) => realm.bootstrap?.outputBase ?? []),
    ]
      .flatMap((base) => [`${base}.js`, `${base}.json`])
      .sort(),
  );
}

export function publicEngineFiles(plan) {
  return Object.freeze(
    plan.engines
      .filter((engine) => engine.publish)
      .map((engine) => `${engine.outputBase}.js`)
      .sort(),
  );
}

export function publicEngineDirectory(plan) {
  return plan.roots.publicEngines.slice("public/".length);
}

export function publicEnginePath(plan, engine) {
  return `${publicEngineDirectory(plan)}/${engine.outputBase}.js`;
}

function assertPlanOwnership(plan) {
  assertUnique(plan.engines.map((engine) => engine.id), "engine");
  assertUnique(plan.realms.map((realm) => realm.key), "realm");
  assertUnique(plan.pages.map((page) => page.key), "page");
  assertUnique(plan.engines.map((engine) => engine.entry), "engine entry");
  assertUnique(
    plan.realms.flatMap((realm) => realm.bootstrap?.entry ?? []),
    "bootstrap entry",
  );
  assertUnique(
    [
      ...plan.engines.map((engine) => engine.outputBase),
      ...plan.realms.flatMap((realm) => realm.bootstrap?.outputBase ?? []),
    ],
    "artifact output",
  );
  assertUnique(
    [
      plan.browserMetadataModule,
      ...plan.engines.flatMap(
        (engine) => engine.browserProjection?.module ?? [],
      ),
      ...plan.realms.flatMap(
        (realm) => realm.bootstrap?.browserProjection.module ?? [],
      ),
    ],
    "browser projection module",
  );
  assertUnique(
    plan.realms.flatMap(
      (realm) => realm.bootstrap?.cspPlaceholder ?? [],
    ),
    "CSP placeholder",
  );
  assertUnique(plan.pages.map((page) => page.source), "page source");

  const pageKeys = new Set(plan.pages.map((page) => page.key));
  for (const realm of plan.realms) {
    if (realm.page && !pageKeys.has(realm.page)) {
      throw new Error(`Realm ${realm.key} references unknown page ${realm.page}.`);
    }
    if (
      realm.bootstrap &&
      !plan.pages.some((page) => page.inlineRealms.includes(realm.key))
    ) {
      throw new Error(`Opaque realm ${realm.key} has no owning page.`);
    }
    if (!engineForId(plan, realm.engine).publish) {
      throw new Error(`Realm ${realm.key} engine must be published.`);
    }
  }
}

function browserProjection(value, owner) {
  if (
    !isRecord(value) ||
    typeof value.exportName !== "string" ||
    !EXPORT_NAME.test(value.exportName)
  ) {
    throw new Error(`${owner} browser projection is invalid.`);
  }
  typescriptModule(value.module, `${owner} browser projection module`);
}

function requiredArray(value, label) {
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error(`Opaque realm artifact plan requires ${label}.`);
  }
  return value;
}

function assertUnique(values, label) {
  const seen = new Set();
  for (const value of values) {
    if (seen.has(value)) throw new Error(`Duplicate ${label}: ${value}.`);
    seen.add(value);
  }
}

function name(value, label) {
  if (typeof value !== "string" || !NAME.test(value)) {
    throw new Error(`Invalid ${label}.`);
  }
  return value;
}

function outputBase(value, label) {
  const normalized = relativePath(value, label);
  if (normalized.includes("/") || /\.(?:js|json)$/u.test(normalized)) {
    throw new Error(`${label} must be an extensionless file name.`);
  }
  return normalized;
}

function typescriptModule(value, label) {
  const normalized = relativePath(value, label);
  if (!/\.[cm]?tsx?$/u.test(normalized)) {
    throw new Error(`${label} must be a TypeScript module.`);
  }
  return normalized;
}

function relativePath(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.includes("\\") ||
    value.startsWith("/") ||
    value.split("/").some((part) => part === "" || part === "." || part === "..")
  ) {
    throw new Error(`Invalid ${label}.`);
  }
  return value;
}

function enumValue(value, values, label) {
  if (typeof value !== "string" || !values.has(value)) {
    throw new Error(`Invalid ${label}: ${String(value)}.`);
  }
  return value;
}

function positiveInteger(value, label) {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`${label} must be a positive safe integer.`);
  }
}

function stringArray(value, label) {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    throw new Error(`${label} must be a string array.`);
  }
  return value;
}

function deepFreeze(value) {
  if (!isRecord(value) && !Array.isArray(value)) return value;
  for (const nested of Object.values(value)) deepFreeze(nested);
  return Object.freeze(value);
}

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
