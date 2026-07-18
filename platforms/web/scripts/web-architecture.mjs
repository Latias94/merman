import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import ts from "typescript";

const forbiddenPolicyNames = new Set([
  "MermaidExternalRequirements",
  "registerExternalDiagrams",
  "registerLayoutLoaders",
]);

const forbiddenCdnHosts = ["cdn.jsdelivr.net", "unpkg.com", "esm.sh"];

export function scanWebArchitecture(sourceRoot) {
  return sourceFiles(sourceRoot).flatMap((file) =>
    findForbiddenWebArchitecture(readFileSync(file, "utf8"), path.relative(sourceRoot, file))
  );
}

export function findForbiddenWebArchitecture(source, file = "source.ts") {
  const sourceFile = ts.createSourceFile(
    file,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS
  );
  const violations = [];

  const report = (node, rule, detail) => {
    const location = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
    violations.push({
      file,
      line: location.line + 1,
      column: location.character + 1,
      rule,
      detail,
    });
  };

  const checkModuleSpecifier = (node) => {
    if (ts.isStringLiteralLike(node) && isForbiddenMermaidModule(node.text)) {
      report(node, "mermaid-module", node.text);
    }
  };

  const visit = (node) => {
    if (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) {
      if (node.moduleSpecifier) {
        checkModuleSpecifier(node.moduleSpecifier);
      }
    } else if (
      ts.isImportTypeNode(node) &&
      ts.isLiteralTypeNode(node.argument)
    ) {
      checkModuleSpecifier(node.argument.literal);
    } else if (
      ts.isCallExpression(node) &&
      node.expression.kind === ts.SyntaxKind.ImportKeyword &&
      node.arguments.length === 1
    ) {
      checkModuleSpecifier(node.arguments[0]);
    }

    if (ts.isIdentifier(node) && forbiddenPolicyNames.has(node.text)) {
      report(node, "mermaid-policy-name", node.text);
    }

    if (ts.isStringLiteralLike(node)) {
      if (forbiddenCdnHosts.some((host) => node.text.includes(host))) {
        report(node, "mermaid-cdn", node.text);
      }
      if (forbiddenPolicyNames.has(node.text) || node.text.includes("VITE_MERMAID_")) {
        report(node, "mermaid-policy-name", node.text);
      }
    }

    ts.forEachChild(node, visit);
  };

  visit(sourceFile);
  return violations;
}

function isForbiddenMermaidModule(specifier) {
  return (
    specifier === "mermaid" ||
    specifier.startsWith("mermaid/") ||
    specifier.startsWith("@mermaid-js/")
  );
}

function sourceFiles(root) {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const absolute = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...sourceFiles(absolute));
    } else if (entry.isFile() && entry.name.endsWith(".ts")) {
      files.push(absolute);
    }
  }
  return files.sort();
}
