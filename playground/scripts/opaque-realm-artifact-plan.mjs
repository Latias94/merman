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
  schemaVersion: 2,
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
      entry: "src/main.tsx",
      cspProfile: "playground-v1",
      inlineRealms: ["compare-mermaid", "benchmark-mermaid"],
    },
    {
      key: "benchmarkCorpus",
      source: "benchmark-corpus.html",
      entry: "src/benchmark/corpus-browser.ts",
      cspProfile: "benchmark-corpus-v1",
      inlineRealms: ["benchmark-mermaid"],
    },
    {
      key: "benchmarkRealm",
      source: "benchmark.html",
      entry: "src/benchmark/realm/trusted-merman-entry.ts",
      cspProfile: "trusted-benchmark-v1",
      inlineRealms: [],
    },
  ],
});

export function defineOpaqueRealmArtifactPlan(input) {
  if (!isRecord(input) || input.schemaVersion !== 2 || !isRecord(input.roots)) {
    throw new Error("Opaque realm artifact plan must use schema version 2.");
  }
  const roots = Object.freeze({
    generated: relativePath(input.roots.generated, "generated root"),
    publicEngines: relativePath(
      input.roots.publicEngines,
      "public engine root",
    ),
  });
  const publicRoot = roots.publicEngines;
  if (!publicRoot.startsWith("public/") || publicRoot === "public/") {
    throw new Error("Public engine root must be inside public/.");
  }
  const browserMetadataModule = typescriptModule(
    input.browserMetadataModule,
    "browser metadata module",
  );
  const engines = Object.freeze(
    requiredArray(input.engines, "engines").map(projectEngine),
  );
  const engineIds = new Set(engines.map((engine) => engine.id));
  const realms = Object.freeze(
    requiredArray(input.realms, "realms").map((realm) =>
      projectRealm(realm, engineIds),
    ),
  );
  const realmKeys = new Set(realms.map((realm) => realm.key));
  const pages = Object.freeze(
    requiredArray(input.pages, "pages").map((page) =>
      projectPage(page, realms, realmKeys),
    ),
  );
  const plan = Object.freeze({
    schemaVersion: 2,
    roots,
    browserMetadataModule,
    engines,
    realms,
    pages,
  });
  assertPlanOwnership(plan);
  return plan;
}

function projectEngine(value, index) {
  if (!isRecord(value)) throw new Error(`Engine ${index} must be an object.`);
  const id = name(value.id, `engine ${index} id`);
  const exports = Object.freeze([
    ...stringArray(value.exports, `engine ${id} exports`),
  ]);
  if (exports.length === 0 || exports.some((item) => !EXPORT_NAME.test(item))) {
    throw new Error(`Engine ${id} exports are invalid.`);
  }
  assertUnique(exports, `engine ${id} export`);
  if (typeof value.publish !== "boolean") {
    throw new Error(`Engine ${id} publish policy must be boolean.`);
  }
  positiveInteger(value.maxBytes, `engine ${id} byte budget`);
  const projection =
    value.browserProjection === undefined
      ? undefined
      : projectBrowserProjection(value.browserProjection, `engine ${id}`);
  return Object.freeze({
    id,
    entry: typescriptModule(value.entry, `engine ${id} entry`),
    outputBase: outputBase(value.outputBase, `engine ${id} output`),
    publish: value.publish,
    maxBytes: value.maxBytes,
    resourcePolicy: enumValue(
      value.resourcePolicy,
      RESOURCE_POLICIES,
      `engine ${id} resource policy`,
    ),
    exports,
    ...(projection === undefined ? {} : { browserProjection: projection }),
  });
}

function projectRealm(value, engineIds) {
  if (!isRecord(value)) throw new Error("Realm must be an object.");
  const key = name(value.key, "realm key");
  const engine = name(value.engine, `realm ${key} engine`);
  if (!engineIds.has(engine)) {
    throw new Error(`Realm ${key} references unknown engine ${engine}.`);
  }
  const common = {
    key,
    kind: enumValue(
      value.kind,
      new Set(["compare", "benchmark"]),
      `realm ${key} kind`,
    ),
    engine,
  };
  if (isRecord(value.bootstrap)) {
    if (value.page !== undefined) {
      throw new Error(`Opaque realm ${key} cannot declare a page.`);
    }
    return Object.freeze({
      ...common,
      bootstrap: projectRealmBootstrap(value.bootstrap, key),
    });
  }
  if (value.bootstrap !== undefined) {
    throw new Error(`Realm ${key} bootstrap must be an object.`);
  }
  return Object.freeze({
    ...common,
    page: name(value.page, `realm ${key} page`),
  });
}

function projectRealmBootstrap(value, realmKey) {
  if (!CSP_PLACEHOLDER.test(String(value.cspPlaceholder))) {
    throw new Error(`Realm ${realmKey} CSP placeholder is invalid.`);
  }
  positiveInteger(
    value.maxBytes,
    `realm ${realmKey} bootstrap byte budget`,
  );
  return Object.freeze({
    entry: typescriptModule(
      value.entry,
      `realm ${realmKey} bootstrap entry`,
    ),
    outputBase: outputBase(
      value.outputBase,
      `realm ${realmKey} bootstrap output`,
    ),
    cspPlaceholder: value.cspPlaceholder,
    maxBytes: value.maxBytes,
    browserProjection: projectBrowserProjection(
      value.browserProjection,
      `realm ${realmKey}`,
    ),
  });
}

function projectPage(value, realms, realmKeys) {
  if (!isRecord(value)) throw new Error("Page must be an object.");
  const key = name(value.key, "page key");
  const source = relativePath(value.source, `page ${key} source`);
  const entry = typescriptModule(value.entry, `page ${key} entry`);
  if (!source.endsWith(".html")) {
    throw new Error(`Page ${key} source must be HTML.`);
  }
  const inlineRealms = Object.freeze([
    ...stringArray(value.inlineRealms, `page ${key} inline realms`),
  ]);
  assertUnique(inlineRealms, `page ${key} inline realm`);
  for (const realmKey of inlineRealms) {
    const realm = realms.find((candidate) => candidate.key === realmKey);
    if (!realmKeys.has(realmKey)) {
      throw new Error(`Page ${key} references unknown realm ${realmKey}.`);
    }
    if (!realm?.bootstrap) {
      throw new Error(
        `Page ${key} cannot inline same-origin realm ${realmKey}.`,
      );
    }
  }
  return Object.freeze({
    key,
    source,
    entry,
    cspProfile: enumValue(
      value.cspProfile,
      CSP_PROFILES,
      `page ${key} CSP profile`,
    ),
    inlineRealms,
  });
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
  assertUnique(plan.pages.map((page) => page.entry), "page entry");

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

function projectBrowserProjection(value, owner) {
  if (
    !isRecord(value) ||
    typeof value.exportName !== "string" ||
    !EXPORT_NAME.test(value.exportName)
  ) {
    throw new Error(`${owner} browser projection is invalid.`);
  }
  return Object.freeze({
    module: typescriptModule(
      value.module,
      `${owner} browser projection module`,
    ),
    exportName: value.exportName,
  });
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

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
