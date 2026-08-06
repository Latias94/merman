import ts from "typescript";

export function assertNoRuntimeModuleRequests(source, fileName) {
  if (typeof source !== "string") {
    throw new TypeError("Runtime module request validation requires source text.");
  }
  const sourceFile = ts.createSourceFile(
    fileName,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.JS,
  );
  const requests = [];

  const visit = (node) => {
    const kind = runtimeModuleRequestKind(node);
    if (kind) {
      const position = sourceFile.getLineAndCharacterOfPosition(
        node.getStart(sourceFile),
      );
      requests.push(
        `${kind} at ${position.line + 1}:${position.character + 1}`,
      );
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);

  if (requests.length > 0) {
    throw new Error(
      `${fileName} contains runtime module requests: ${requests.join(", ")}.`,
    );
  }
}

function runtimeModuleRequestKind(node) {
  if (ts.isImportDeclaration(node)) return "import declaration";
  if (ts.isExportDeclaration(node) && node.moduleSpecifier) {
    return "export-from declaration";
  }
  if (ts.isImportEqualsDeclaration(node)) return "import-equals declaration";
  if (
    ts.isCallExpression(node) &&
    node.expression.kind === ts.SyntaxKind.ImportKeyword
  ) {
    return "dynamic import";
  }
  return null;
}
