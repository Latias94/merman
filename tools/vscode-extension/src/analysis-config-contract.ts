import { BUNDLED_ANALYSIS_CONFIG_PROJECTION } from "./generated/analysis-config-baseline.js";

export interface AnalysisConfigSchemaProjection {
  version: unknown;
  rule_catalog_method: unknown;
  accepted_roots: unknown;
  profiles: unknown;
  severities: unknown;
  configurable_rule_ids: unknown;
  constraints: unknown;
  schema: unknown;
}

export const RULE_CATALOG_METHOD = "merman/ruleCatalog";
export const CONFIG_SCHEMA_METHOD = "merman/configSchema";

export type AnalysisConfigChangeScope = "diagnostics_only" | "snapshot_affecting";
export type AnalysisConfigClientValueSet =
  | "profiles"
  | "severities"
  | "configurable_rule_ids";

export type AnalysisConfigClientRuntimeConstraint =
  | { kind: "canonical_civil_date" }
  | {
      kind: "representable_local_midnight";
      offsetSettingPath: string;
    };

export interface AnalysisConfigClientObjectField {
  name: string;
  required: boolean;
  normalization: AnalysisConfigClientSettingNormalization;
}

export type AnalysisConfigClientSettingNormalization =
  | {
      kind: "string";
      pattern?: string;
      matcher?: RegExp;
      values?: AnalysisConfigClientValueSet;
    }
  | {
      kind: "integer";
      minimum: number;
      maximum: number;
    }
  | { kind: "object" }
  | { kind: "rule_id_list" }
  | {
      kind: "rule_severity_overrides";
      fields: readonly AnalysisConfigClientObjectField[];
    };

export interface AnalysisConfigClientSetting {
  path: string;
  changeScope: AnalysisConfigChangeScope;
  runtimeConstraints: readonly AnalysisConfigClientRuntimeConstraint[];
  normalization: AnalysisConfigClientSettingNormalization;
}

export interface NegotiatedAnalysisConfig {
  version: 2;
  acceptedRoots: readonly string[];
  profiles: readonly string[];
  severities: readonly string[];
  configurableRuleIds: readonly string[];
  settings: readonly AnalysisConfigClientSetting[];
}

export const BUNDLED_ANALYSIS_CONFIG = parseAnalysisConfigClientProjection(
  BUNDLED_ANALYSIS_CONFIG_PROJECTION,
);

export function assertAnalysisConfigCapability(initializeResult: unknown): void {
  const result = asRecord(initializeResult);
  const capabilities = asRecord(result?.capabilities);
  const experimental = asRecord(capabilities?.experimental);
  const merman = asRecord(experimental?.merman);
  const requests = asRecord(merman?.requests);
  if (requests?.configSchema !== CONFIG_SCHEMA_METHOD) {
    throw new Error(
      `Merman language server does not advertise ${CONFIG_SCHEMA_METHOD}.`,
    );
  }
}

export function negotiateAnalysisConfig(
  response: AnalysisConfigSchemaProjection,
): NegotiatedAnalysisConfig {
  if (response.version !== 2) {
    throw new Error(
      `Unsupported Merman analysis config schema version ${String(response.version)}.`,
    );
  }
  if (response.rule_catalog_method !== RULE_CATALOG_METHOD) {
    throw new Error("Merman analysis config schema returned an incompatible rule catalog method.");
  }
  return parseAnalysisConfigClientProjection(response);
}

export function parseAnalysisConfigClientProjection(
  value: unknown,
): NegotiatedAnalysisConfig {
  const projection = requiredRecord(value, "client projection");
  const acceptedRoots = stringList(projection.accepted_roots, "accepted roots");
  if (!acceptedRoots.includes("analysis")) {
    throw new Error("Merman analysis config schema does not accept the analysis root.");
  }
  const profiles = stringList(projection.profiles, "profiles");
  const severities = stringList(projection.severities, "severities");
  const configurableRuleIds = stringList(
    projection.configurable_rule_ids,
    "configurable rule IDs",
  );
  const constraints = requiredRecord(projection.constraints, "client constraints");
  if (constraints.version !== 1) {
    throw new Error(
      `Unsupported Merman analysis client constraints version ${String(constraints.version)}.`,
    );
  }
  const settings = settingList(constraints.settings);
  validateSettingDependencies(settings);

  return {
    version: 2,
    acceptedRoots,
    profiles,
    severities,
    configurableRuleIds,
    settings,
  };
}

export function analysisConfigValues(
  contract: NegotiatedAnalysisConfig,
  valueSet: AnalysisConfigClientValueSet,
): readonly string[] {
  switch (valueSet) {
    case "profiles":
      return contract.profiles;
    case "severities":
      return contract.severities;
    case "configurable_rule_ids":
      return contract.configurableRuleIds;
  }
}

function settingList(value: unknown): AnalysisConfigClientSetting[] {
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error("Merman analysis config schema returned invalid client settings.");
  }
  const paths = new Set<string>();
  const settings = value.map((entry) => {
    const setting = requiredRecord(entry, "client setting");
    const path = settingPath(setting.path);
    if (paths.has(path)) {
      throw new Error("Merman analysis config schema returned duplicate client setting paths.");
    }
    paths.add(path);
    return {
      path,
      changeScope: changeScope(setting.change_scope),
      runtimeConstraints: runtimeConstraintList(setting.runtime_constraints),
      normalization: settingNormalization(setting.normalization, path),
    };
  });

  for (const path of paths) {
    if ([...paths].some((candidate) => candidate.startsWith(`${path}.`))) {
      throw new Error("Merman analysis config schema returned overlapping client setting paths.");
    }
  }
  return settings;
}

function settingPath(value: unknown): string {
  const path = requiredString(value, "client setting path");
  const segments = path.split(".");
  if (
    segments.some((segment) =>
      !/^[a-z_][a-z0-9_]*$/u.test(segment) ||
      ["__proto__", "constructor", "prototype"].includes(segment)
    )
  ) {
    throw new Error("Merman analysis config schema returned an invalid client setting path.");
  }
  return path;
}

function changeScope(value: unknown): AnalysisConfigChangeScope {
  if (value === "diagnostics_only" || value === "snapshot_affecting") {
    return value;
  }
  throw new Error("Merman analysis config schema returned an invalid change scope.");
}

function settingNormalization(
  value: unknown,
  path: string,
): AnalysisConfigClientSettingNormalization {
  const normalization = requiredRecord(value, `${path} normalization`);
  switch (normalization.kind) {
    case "string": {
      const pattern = optionalString(normalization.pattern, `${path} pattern`);
      let matcher: RegExp | undefined;
      if (pattern) {
        try {
          matcher = new RegExp(pattern, "u");
        } catch {
          throw new Error(`Merman analysis config schema returned an invalid ${path} pattern.`);
        }
      }
      const values = normalization.values === undefined
        ? undefined
        : clientValueSet(normalization.values);
      return { kind: "string", pattern, matcher, values };
    }
    case "integer": {
      const { minimum, maximum } = integerRange(normalization, path);
      return { kind: "integer", minimum, maximum };
    }
    case "object":
      return { kind: "object" };
    case "rule_id_list":
      return { kind: "rule_id_list" };
    case "rule_severity_overrides":
      return {
        kind: "rule_severity_overrides",
        fields: objectFieldList(normalization.fields, path),
      };
    default:
      throw new Error(`Merman analysis config schema returned an invalid ${path} normalizer.`);
  }
}

function objectFieldList(
  value: unknown,
  path: string,
): AnalysisConfigClientObjectField[] {
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error(`Merman analysis config schema returned invalid ${path} object fields.`);
  }
  const names = new Set<string>();
  return value.map((entry) => {
    const field = requiredRecord(entry, `${path} object field`);
    const name = objectFieldName(field.name, path);
    if (names.has(name)) {
      throw new Error(`Merman analysis config schema returned duplicate ${path} object fields.`);
    }
    names.add(name);
    if (typeof field.required !== "boolean") {
      throw new Error(`Merman analysis config schema returned invalid ${path}.${name} requiredness.`);
    }
    return {
      name,
      required: field.required,
      normalization: settingNormalization(field.normalization, `${path}.${name}`),
    };
  });
}

function objectFieldName(value: unknown, path: string): string {
  const name = requiredString(value, `${path} object field name`);
  if (
    !/^[a-z_][a-z0-9_]*$/u.test(name) ||
    ["__proto__", "constructor", "prototype"].includes(name)
  ) {
    throw new Error(`Merman analysis config schema returned an invalid ${path} object field name.`);
  }
  return name;
}

function clientValueSet(value: unknown): AnalysisConfigClientValueSet {
  if (
    value === "profiles" ||
    value === "severities" ||
    value === "configurable_rule_ids"
  ) {
    return value;
  }
  throw new Error("Merman analysis config schema returned an invalid value catalog reference.");
}

function runtimeConstraintList(value: unknown): AnalysisConfigClientRuntimeConstraint[] {
  if (!Array.isArray(value)) {
    throw new Error("Merman analysis config schema returned invalid runtime constraints.");
  }
  return value.map((entry) => {
    const constraint = requiredRecord(entry, "runtime constraint");
    switch (constraint.kind) {
      case "canonical_civil_date":
        return { kind: "canonical_civil_date" };
      case "representable_local_midnight":
        return {
          kind: "representable_local_midnight",
          offsetSettingPath: settingPath(constraint.offset_setting_path),
        };
      default:
        throw new Error("Merman analysis config schema returned an invalid runtime constraint.");
    }
  });
}

function validateSettingDependencies(settings: readonly AnalysisConfigClientSetting[]): void {
  const byPath = new Map(settings.map((setting) => [setting.path, setting]));
  for (const setting of settings) {
    for (const constraint of setting.runtimeConstraints) {
      if (constraint.kind !== "representable_local_midnight") {
        continue;
      }
      const offset = byPath.get(constraint.offsetSettingPath);
      if (offset?.normalization.kind !== "integer") {
        throw new Error(
          "Merman analysis config schema returned an invalid local-midnight offset dependency.",
        );
      }
    }
  }
}

function stringList(value: unknown, label: string): string[] {
  if (
    !Array.isArray(value) ||
    value.length === 0 ||
    value.some((entry) =>
      typeof entry !== "string" || entry.length === 0 || entry.trim() !== entry
    ) ||
    new Set(value).size !== value.length
  ) {
    throw new Error(`Merman analysis config schema returned invalid ${label}.`);
  }
  return value as string[];
}

function integerRange(
  value: Record<string, unknown>,
  label: string,
): { minimum: number; maximum: number } {
  const minimum = value.minimum;
  const maximum = value.maximum;
  if (
    typeof minimum !== "number" ||
    typeof maximum !== "number" ||
    !Number.isSafeInteger(minimum) ||
    !Number.isSafeInteger(maximum) ||
    minimum > maximum
  ) {
    throw new Error(`Merman analysis config schema returned an invalid ${label} range.`);
  }
  return { minimum, maximum };
}

function requiredRecord(value: unknown, label: string): Record<string, unknown> {
  const record = asRecord(value);
  if (!record) {
    throw new Error(`Merman analysis config schema returned an invalid ${label}.`);
  }
  return record;
}

function requiredString(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new Error(`Merman analysis config schema returned an invalid ${label}.`);
  }
  return value;
}

function optionalString(value: unknown, label: string): string | undefined {
  return value === undefined ? undefined : requiredString(value, label);
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
}
