import { createRequire } from "node:module";

const requireFromModule = createRequire(import.meta.url);
const contract = requireFromModule("./generated/node-wire-contract.json");

validateContract(contract);

export const NODE_WIRE_CONTRACT = deepFreeze(contract);
export const NODE_TRANSPORT_LIMITS = NODE_WIRE_CONTRACT.documents;
export const NODE_TRANSPORT_FIELD_LIMITS = NODE_WIRE_CONTRACT.fields;

function validateContract(value) {
  if (
    !value ||
    value.schema_version !== 1 ||
    value.package_id !== "@mermanjs/node" ||
    value.artifact_id !== "merman-node-static-svg" ||
    value.transport_api_version !== 1 ||
    value.binding_result_payload_version !== 1 ||
    !value.artifact ||
    !value.documents ||
    !value.fields
  ) {
    throw new Error("The generated Node wire contract is invalid.");
  }
  for (const field of [
    "capability_ids",
    "output_ids",
    "system_adapter_ids",
    "operation_ids",
    "metadata_ids",
    "option_group_ids",
    "constructor_service_ids",
    "text_measurement_provider_ids",
  ]) {
    const ids = value.artifact[field];
    if (
      !Array.isArray(ids) ||
      ids.some((id) => typeof id !== "string" || id.length === 0) ||
      ids.some((id, index) => index > 0 && ids[index - 1] >= id)
    ) {
      throw new Error(`The generated Node artifact field \`${field}\` is invalid.`);
    }
  }
  if (
    !Array.isArray(value.artifact.output_contracts) ||
    value.artifact.output_contracts.length !== value.artifact.output_ids.length ||
    value.artifact.output_contracts.some((contract, index) =>
      !contract ||
      contract.id !== value.artifact.output_ids[index] ||
      typeof contract.media_type !== "string" ||
      contract.media_type.length === 0 ||
      !(contract.system_fonts === null || isObject(contract.system_fonts)) ||
      !(contract.embedded_images === null || isObject(contract.embedded_images))
    )
  ) {
    throw new Error("The generated Node artifact output contracts are invalid.");
  }
  for (const [id, limits] of Object.entries(value.documents)) {
    if (
      !limits ||
      !Number.isSafeInteger(limits.max_utf8_bytes) ||
      !Number.isSafeInteger(limits.max_depth) ||
      !Number.isSafeInteger(limits.max_members) ||
      !Number.isSafeInteger(limits.max_tokens) ||
      !Number.isSafeInteger(limits.max_string_utf8_bytes) ||
      Object.values(limits).some((limit) => limit <= 0)
    ) {
      throw new Error(`The generated Node wire document limit \`${id}\` is invalid.`);
    }
  }
  for (const [id, limit] of Object.entries(value.fields)) {
    if (!Number.isSafeInteger(limit) || limit <= 0) {
      throw new Error(`The generated Node wire field limit \`${id}\` is invalid.`);
    }
  }
}

function isObject(value) {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

function deepFreeze(value) {
  if (!value || typeof value !== "object" || Object.isFrozen(value)) return value;
  for (const child of Object.values(value)) deepFreeze(child);
  return Object.freeze(value);
}
