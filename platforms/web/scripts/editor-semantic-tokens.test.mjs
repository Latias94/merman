import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";

const root = path.resolve(import.meta.dirname, "..");

test("runtime semantic token descriptor must match the generated contract", () => {
  const contract = loadTypeScriptModule("src/generated/token-descriptor.ts");
  const { validateSemanticTokenDescriptor } = loadTypeScriptModule(
    "src/editor-semantic-tokens.ts"
  );
  const runtime = runtimeDescriptor(contract.SEMANTIC_TOKEN_DESCRIPTOR);

  assert.equal(validateSemanticTokenDescriptor(runtime), contract.SEMANTIC_TOKEN_DESCRIPTOR);

  for (const mutate of [
    (value) => (value.digest = "sha256:obsolete"),
    (value) => (value.packed.recordWidth = 4),
    (value) => (value.tokenTypes[0].code = 9),
    (value) => (value.tokenTypes[0].lspName = "transportOwnedName"),
    (value) => (value.modifiers[0].bit = 2),
  ]) {
    const invalid = structuredClone(runtime);
    mutate(invalid);
    assert.throws(
      () => validateSemanticTokenDescriptor(invalid),
      /generated descriptor|semantic token contract/i
    );
  }
});

test("packed semantic tokens are validated without projection or copying", () => {
  const {
    SEMANTIC_TOKEN_RECORD_WIDTH,
    SEMANTIC_TOKEN_VALID_MODIFIER_MASK,
    SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX,
  } = loadTypeScriptModule("src/generated/token-descriptor.ts");
  const { validatePackedSemanticTokens } = loadTypeScriptModule(
    "src/editor-semantic-tokens.ts"
  );
  const packed = new Uint32Array([
    0,
    0,
    4,
    SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX,
    SEMANTIC_TOKEN_VALID_MODIFIER_MASK,
  ]);

  assert.equal(SEMANTIC_TOKEN_RECORD_WIDTH, 5);
  assert.equal(validatePackedSemanticTokens(packed), packed);
  assert.throws(() => validatePackedSemanticTokens([]), /Uint32Array/);
  assert.throws(
    () => validatePackedSemanticTokens(new Uint32Array([0, 0, 4, 0])),
    /divisible/
  );
  assert.throws(
    () => validatePackedSemanticTokens(new Uint32Array([0, 0, 0, 0, 0])),
    /zero UTF-16 length/
  );
  assert.throws(
    () =>
      validatePackedSemanticTokens(
        new Uint32Array([0, 0, 1, SEMANTIC_TOKEN_VALID_TYPE_CODE_MAX + 1, 0])
      ),
    /type code/
  );
  assert.throws(
    () =>
      validatePackedSemanticTokens(
        new Uint32Array([0, 0, 1, 0, SEMANTIC_TOKEN_VALID_MODIFIER_MASK + 1])
      ),
    /modifier bits/
  );
});

function runtimeDescriptor(descriptor) {
  return {
    schemaVersion: descriptor.schemaVersion,
    digest: descriptor.digest,
    tokenTypes: descriptor.tokenTypes.map(({ id, code, lspName, lspIndex }) => ({
      id,
      code,
      lspName,
      lspIndex,
    })),
    modifiers: descriptor.modifiers.map(
      ({ id, index, bit, lspName, lspIndex }) => ({
        id,
        index,
        bit,
        lspName,
        lspIndex,
      })
    ),
    packed: {
      encoding: descriptor.packed.encoding,
      wordWidthBits: descriptor.packed.wordWidthBits,
      recordWidth: descriptor.packed.recordWidth,
      fieldOrder: [...descriptor.packed.fieldOrder],
    },
    validTypeCodeMax: descriptor.validTypeCodeMax,
    validModifierMask: descriptor.validModifierMask,
  };
}

const moduleCache = new Map();

function loadTypeScriptModule(relativePath) {
  return load(path.join(root, relativePath));
}

function load(sourcePath) {
  const normalizedPath = path.normalize(sourcePath);
  if (moduleCache.has(normalizedPath)) return moduleCache.get(normalizedPath).exports;
  const source = readFileSync(normalizedPath, "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2020,
    },
    fileName: normalizedPath,
  });
  const module = { exports: {} };
  moduleCache.set(normalizedPath, module);
  const context = {
    Error,
    Uint32Array,
    module,
    exports: module.exports,
    require(specifier) {
      if (!specifier.startsWith(".")) {
        throw new Error(`unexpected runtime import: ${specifier}`);
      }
      const requested = path.resolve(path.dirname(normalizedPath), specifier);
      const sourceModule = requested.endsWith(".js")
        ? `${requested.slice(0, -3)}.ts`
        : requested;
      const resolved = existsSync(sourceModule) ? sourceModule : `${sourceModule}.ts`;
      return load(resolved);
    },
  };
  vm.runInNewContext(outputText, context, { filename: normalizedPath });
  return module.exports;
}
