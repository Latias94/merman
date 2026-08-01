import { existsSync, readFileSync, realpathSync, statSync } from "node:fs";
import path from "node:path";
import ts from "typescript";

export function collectStaticModuleGraph({
  entry,
  root,
  mode = "runtime",
}) {
  if (mode !== "runtime" && mode !== "declaration") {
    throw new Error(`Unsupported static module graph mode: ${mode}.`);
  }

  const requestedRoot = path.resolve(root);
  const requestedEntry = path.resolve(entry);
  assertInsideRoot(requestedEntry, requestedRoot, "entry");
  const graphRoot = realpathSync.native(requestedRoot);
  const entryFile = resolveEntry(
    path.resolve(graphRoot, path.relative(requestedRoot, requestedEntry)),
    mode,
  );
  assertInsideRoot(entryFile, graphRoot, "entry");

  const files = new Set();
  const dynamicImports = [];
  const externalImports = [];
  const queue = [entryFile];

  while (queue.length > 0) {
    const importer = queue.shift();
    if (files.has(importer)) continue;
    files.add(importer);

    const source = readFileSync(importer, "utf8");
    const sourceFile = ts.createSourceFile(
      importer,
      source,
      ts.ScriptTarget.Latest,
      true,
      scriptKindFor(importer),
    );
    if (sourceFile.parseDiagnostics.length !== 0) {
      const diagnostic = sourceFile.parseDiagnostics[0];
      throw new Error(
        `Cannot parse ${relativePath(graphRoot, importer)}: ${ts.flattenDiagnosticMessageText(
          diagnostic.messageText,
          "\n",
        )}`,
      );
    }

    for (const specifier of moduleDependencies(sourceFile, mode)) {
      if (!specifier.startsWith(".")) {
        externalImports.push(specifier);
        continue;
      }
      const resolved = resolveRelativeModule(importer, specifier, mode);
      assertInsideRoot(resolved, graphRoot, "dependency");
      if (!files.has(resolved)) queue.push(resolved);
    }

    for (const specifier of dynamicImportSpecifiers(sourceFile)) {
      dynamicImports.push(specifier);
    }
  }

  return Object.freeze({
    entry: entryFile,
    root: graphRoot,
    files: Object.freeze([...files].sort()),
    dynamicImports: Object.freeze(dynamicImports.sort()),
    externalImports: Object.freeze(externalImports.sort()),
  });
}

export function relativeModuleFiles(graph) {
  return graph.files.map((file) => relativePath(graph.root, file));
}

function moduleDependencies(sourceFile, mode) {
  const dependencies = [];
  for (const statement of sourceFile.statements) {
    if (ts.isImportDeclaration(statement)) {
      if (mode === "runtime" && importIsTypeOnly(statement.importClause)) {
        continue;
      }
      dependencies.push(
        stringModuleSpecifier(statement.moduleSpecifier, sourceFile),
      );
      continue;
    }
    if (ts.isExportDeclaration(statement) && statement.moduleSpecifier) {
      if (mode === "runtime" && exportIsTypeOnly(statement)) {
        continue;
      }
      dependencies.push(
        stringModuleSpecifier(statement.moduleSpecifier, sourceFile),
      );
    }
  }
  if (mode === "declaration") {
    const visit = (node) => {
      if (ts.isImportTypeNode(node)) {
        const argument = node.argument;
        if (
          !ts.isLiteralTypeNode(argument) ||
          !ts.isStringLiteralLike(argument.literal)
        ) {
          throw new Error(
            `Import type in ${sourceFile.fileName} must use a string literal.`,
          );
        }
        dependencies.push(argument.literal.text);
        return;
      }
      ts.forEachChild(node, visit);
    };
    visit(sourceFile);
  }
  return dependencies;
}

function importIsTypeOnly(importClause) {
  if (!importClause) return false;
  if (importClause.isTypeOnly) return true;
  if (importClause.name) return false;
  const bindings = importClause.namedBindings;
  return (
    bindings !== undefined &&
    ts.isNamedImports(bindings) &&
    bindings.elements.length > 0 &&
    bindings.elements.every((element) => element.isTypeOnly)
  );
}

function exportIsTypeOnly(declaration) {
  if (declaration.isTypeOnly) return true;
  const clause = declaration.exportClause;
  return (
    clause !== undefined &&
    ts.isNamedExports(clause) &&
    clause.elements.length > 0 &&
    clause.elements.every((element) => element.isTypeOnly)
  );
}

function dynamicImportSpecifiers(sourceFile) {
  const imports = [];
  const visit = (node) => {
    if (
      ts.isCallExpression(node) &&
      node.expression.kind === ts.SyntaxKind.ImportKeyword
    ) {
      const [argument] = node.arguments;
      if (!argument || !ts.isStringLiteralLike(argument)) {
        throw new Error(
          `Dynamic import in ${sourceFile.fileName} must use a string literal.`,
        );
      }
      imports.push(argument.text);
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return imports;
}

function stringModuleSpecifier(node, sourceFile) {
  if (!ts.isStringLiteralLike(node)) {
    throw new Error(
      `Module specifier in ${sourceFile.fileName} must use a string literal.`,
    );
  }
  return node.text;
}

function resolveEntry(entry, mode) {
  if (isFile(entry)) return entry;
  const candidates =
    mode === "declaration"
      ? declarationCandidates(entry)
      : runtimeCandidates(entry);
  return firstExistingFile(candidates, `Cannot resolve static module graph entry ${entry}.`);
}

function resolveRelativeModule(importer, specifier, mode) {
  if (!specifier.endsWith(".js")) {
    throw new Error(
      `Local module ${specifier} imported by ${importer} must use an explicit .js specifier.`,
    );
  }
  const requested = path.resolve(path.dirname(importer), specifier);
  const candidates =
    mode === "declaration"
      ? declarationCandidates(requested)
      : runtimeCandidates(requested);
  return firstExistingFile(
    candidates,
    `Cannot resolve ${specifier} imported by ${importer}.`,
  );
}

function runtimeCandidates(requested) {
  return [requested, requested.slice(0, -3) + ".ts"];
}

function declarationCandidates(requested) {
  return [requested.slice(0, -3) + ".d.ts"];
}

function firstExistingFile(candidates, message) {
  for (const candidate of candidates) {
    if (isFile(candidate)) return realpathSync.native(path.resolve(candidate));
  }
  throw new Error(`${message} Tried: ${candidates.join(", ")}.`);
}

function isFile(file) {
  return existsSync(file) && statSync(file).isFile();
}

function scriptKindFor(file) {
  if (file.endsWith(".ts")) {
    return ts.ScriptKind.TS;
  }
  return ts.ScriptKind.JS;
}

function assertInsideRoot(file, root, label) {
  const relative = path.relative(root, file);
  if (
    relative === "" ||
    (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative))
  ) {
    return;
  }
  throw new Error(`Static module graph ${label} escapes ${root}: ${file}.`);
}

function relativePath(root, file) {
  return path.relative(root, file).split(path.sep).join("/");
}
