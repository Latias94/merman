import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";

const root = path.resolve(import.meta.dirname, "..");
const sourcePath = path.join(root, "src", "lib", "mermaid-language.ts");

function loadMermaidLanguageModule() {
  const source = readFileSync(sourcePath, "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      esModuleInterop: true,
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2020,
    },
    fileName: sourcePath,
  });
  const module = { exports: {} };
  const context = {
    console,
    module,
    exports: module.exports,
    require(specifier) {
      throw new Error(`unexpected runtime import while testing mermaid-language.ts: ${specifier}`);
    },
    Uint32Array,
  };
  vm.runInNewContext(outputText, context, { filename: sourcePath });
  return module.exports;
}

test("semantic token encoding follows the provided legend order", () => {
  const { encodeSemanticTokensForLegend } = loadMermaidLanguageModule();
  const data = encodeSemanticTokensForLegend(
    [
      {
        line: 0,
        start: 3,
        length: 5,
        tokenType: "namespace",
        tokenModifier: "entity",
      },
    ],
    {
      tokenTypes: ["string", "namespace"],
      tokenModifiers: ["payload", "entity"],
    },
  );

  assert.deepEqual([...data], [0, 3, 5, 1, 2]);
});

test("Monaco semantic tokens provider encodes with the advertised service legend", () => {
  const {
    registerMermaidLanguage,
    setMermaidEditorService,
  } = loadMermaidLanguageModule();
  let semanticProvider;
  const legend = {
    tokenTypes: ["string", "namespace"],
    tokenModifiers: ["payload", "entity"],
  };

  setMermaidEditorService({
    editor_semantic_token_legend() {
      return legend;
    },
    editor_semantic_tokens() {
      return [
        {
          line: 0,
          start: 0,
          length: 4,
          tokenType: "namespace",
          tokenModifier: "entity",
        },
      ];
    },
  });
  registerMermaidLanguage(fakeMonaco((provider) => {
    semanticProvider = provider;
  }));

  assert.deepEqual(semanticProvider.getLegend(), legend);
  const result = semanticProvider.provideDocumentSemanticTokens({
    getValue: () => "flowchart TD",
  });

  assert.deepEqual([...result.data], [0, 0, 4, 1, 2]);
});

function fakeMonaco(captureSemanticProvider) {
  return {
    Range: class Range {
      constructor(startLineNumber, startColumn, endLineNumber, endColumn) {
        this.startLineNumber = startLineNumber;
        this.startColumn = startColumn;
        this.endLineNumber = endLineNumber;
        this.endColumn = endColumn;
      }
    },
    MarkerSeverity: {
      Error: 8,
      Hint: 1,
      Info: 2,
      Warning: 4,
    },
    editor: {
      setModelMarkers() {},
    },
    languages: {
      CompletionItemInsertTextRule: {
        InsertAsSnippet: 4,
      },
      CompletionItemKind: {
        Keyword: 14,
        Snippet: 27,
        Variable: 12,
      },
      SymbolKind: {
        Class: 4,
        Event: 24,
        Function: 11,
        Module: 2,
        Namespace: 3,
        Object: 19,
        Package: 4,
        Property: 6,
        String: 15,
        Struct: 22,
        Variable: 12,
      },
      getLanguages: () => [],
      register() {},
      registerCodeActionProvider() {},
      registerCompletionItemProvider() {},
      registerDefinitionProvider() {},
      registerDocumentFormattingEditProvider() {},
      registerDocumentSemanticTokensProvider(_languageId, provider) {
        captureSemanticProvider(provider);
      },
      registerDocumentSymbolProvider() {},
      registerHoverProvider() {},
      registerReferenceProvider() {},
      registerRenameProvider() {},
      setLanguageConfiguration() {},
      setMonarchTokensProvider() {},
    },
  };
}
