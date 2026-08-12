export interface AnalysisConfigSchemaProjection {
  version: unknown;
  rule_catalog_method: unknown;
  accepted_roots: unknown;
  configurable_rule_ids: unknown;
}

export const RULE_CATALOG_METHOD = "merman/ruleCatalog";
export const CONFIG_SCHEMA_METHOD = "merman/configSchema";

export interface NegotiatedAnalysisConfig {
  version: 1;
  configurableRuleIds: readonly string[];
}

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
  if (response.version !== 1) {
    throw new Error(
      `Unsupported Merman analysis config schema version ${String(response.version)}.`,
    );
  }
  if (response.rule_catalog_method !== RULE_CATALOG_METHOD) {
    throw new Error("Merman analysis config schema returned an incompatible rule catalog method.");
  }
  if (!Array.isArray(response.accepted_roots) || !response.accepted_roots.includes("analysis")) {
    throw new Error("Merman analysis config schema does not accept the analysis root.");
  }
  if (
    !Array.isArray(response.configurable_rule_ids) ||
    response.configurable_rule_ids.some((ruleId) =>
      typeof ruleId !== "string" || ruleId.length === 0 || ruleId.trim() !== ruleId
    ) ||
    new Set(response.configurable_rule_ids).size !== response.configurable_rule_ids.length
  ) {
    throw new Error("Merman analysis config schema returned invalid configurable rule IDs.");
  }
  return {
    version: 1,
    configurableRuleIds: response.configurable_rule_ids,
  };
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
}
