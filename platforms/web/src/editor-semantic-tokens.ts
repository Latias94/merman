import {
  SEMANTIC_TOKEN_DESCRIPTOR,
  SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  SEMANTIC_TOKEN_RECORD_WIDTH,
  SEMANTIC_TOKEN_VALID_MODIFIER_MASK,
  SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX,
} from "./generated/token-descriptor.js";
import type {
  EditorSemanticTokenDescriptor,
  WasmSemanticTokenDescriptor,
} from "./public-types.js";

export function validateSemanticTokenDescriptor(
  value: unknown
): EditorSemanticTokenDescriptor {
  if (!isRecord(value)) fail("descriptor must be an object");
  const descriptor = value as unknown as WasmSemanticTokenDescriptor;
  requireEqual("schemaVersion", descriptor.schemaVersion, SEMANTIC_TOKEN_DESCRIPTOR.schemaVersion);
  requireEqual("digest", descriptor.digest, SEMANTIC_TOKEN_DESCRIPTOR_DIGEST);
  requireEqual(
    "validTypeCodeMax",
    descriptor.validTypeCodeMax,
    SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX
  );
  requireEqual(
    "validModifierMask",
    descriptor.validModifierMask,
    SEMANTIC_TOKEN_VALID_MODIFIER_MASK
  );

  validateTokenTypes(descriptor.tokenTypes);
  validateModifiers(descriptor.modifiers);
  validatePackedDescriptor(descriptor.packed);
  return SEMANTIC_TOKEN_DESCRIPTOR;
}

export function validatePackedSemanticTokens(value: unknown): Uint32Array {
  if (!(value instanceof Uint32Array)) {
    fail("semantic token payload must be a Uint32Array");
  }
  if (value.length % SEMANTIC_TOKEN_RECORD_WIDTH !== 0) {
    fail(`semantic token payload length must be divisible by ${SEMANTIC_TOKEN_RECORD_WIDTH}`);
  }

  for (let offset = 0; offset < value.length; offset += SEMANTIC_TOKEN_RECORD_WIDTH) {
    if (value[offset + 2] === 0) {
      fail(`semantic token at word ${offset} has zero UTF-16 length`);
    }
    if (value[offset + 3] > SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX) {
      fail(`semantic token at word ${offset} has an unknown type code`);
    }
    if (((value[offset + 4] & ~SEMANTIC_TOKEN_VALID_MODIFIER_MASK) >>> 0) !== 0) {
      fail(`semantic token at word ${offset} has unknown modifier bits`);
    }
  }
  return value;
}

function validateTokenTypes(value: unknown): void {
  if (!Array.isArray(value) || value.length !== SEMANTIC_TOKEN_DESCRIPTOR.tokenTypes.length) {
    fail("token type descriptor count does not match the generated descriptor");
  }
  for (let index = 0; index < value.length; index += 1) {
    const actual = value[index];
    const expected = SEMANTIC_TOKEN_DESCRIPTOR.tokenTypes[index];
    if (!isRecord(actual)) fail(`tokenTypes[${index}] must be an object`);
    requireEqual(`tokenTypes[${index}].id`, actual.id, expected.id);
    requireEqual(`tokenTypes[${index}].code`, actual.code, expected.code);
    requireEqual(`tokenTypes[${index}].lspName`, actual.lspName, expected.lspName);
    requireEqual(`tokenTypes[${index}].lspIndex`, actual.lspIndex, expected.lspIndex);
  }
}

function validateModifiers(value: unknown): void {
  if (!Array.isArray(value) || value.length !== SEMANTIC_TOKEN_DESCRIPTOR.modifiers.length) {
    fail("modifier descriptor count does not match the generated descriptor");
  }
  for (let index = 0; index < value.length; index += 1) {
    const actual = value[index];
    const expected = SEMANTIC_TOKEN_DESCRIPTOR.modifiers[index];
    if (!isRecord(actual)) fail(`modifiers[${index}] must be an object`);
    requireEqual(`modifiers[${index}].id`, actual.id, expected.id);
    requireEqual(`modifiers[${index}].index`, actual.index, expected.index);
    requireEqual(`modifiers[${index}].bit`, actual.bit, expected.bit);
    requireEqual(`modifiers[${index}].lspName`, actual.lspName, expected.lspName);
    requireEqual(`modifiers[${index}].lspIndex`, actual.lspIndex, expected.lspIndex);
  }
}

function validatePackedDescriptor(value: unknown): void {
  if (!isRecord(value)) fail("packed descriptor must be an object");
  requireEqual("packed.encoding", value.encoding, SEMANTIC_TOKEN_DESCRIPTOR.packed.encoding);
  requireEqual(
    "packed.wordWidthBits",
    value.wordWidthBits,
    SEMANTIC_TOKEN_DESCRIPTOR.packed.wordWidthBits
  );
  requireEqual("packed.recordWidth", value.recordWidth, SEMANTIC_TOKEN_RECORD_WIDTH);
  if (
    !Array.isArray(value.fieldOrder) ||
    value.fieldOrder.length !== SEMANTIC_TOKEN_DESCRIPTOR.packed.fieldOrder.length ||
    value.fieldOrder.some(
      (field, index) => field !== SEMANTIC_TOKEN_DESCRIPTOR.packed.fieldOrder[index]
    )
  ) {
    fail("packed.fieldOrder does not match the generated descriptor");
  }
}

function requireEqual(label: string, actual: unknown, expected: unknown): void {
  if (actual !== expected) {
    fail(`${label} does not match the generated descriptor`);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function fail(message: string): never {
  throw new Error(`Invalid Merman semantic token contract: ${message}.`);
}
