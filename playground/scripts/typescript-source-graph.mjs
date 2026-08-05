import path from "node:path";

import ts from "typescript";

export function createTypeScriptSourceGraph({
  rootDir,
  configFile = "tsconfig.json",
  entries,
}) {
  const root = path.resolve(rootDir);
  const configPath = path.resolve(root, configFile);
  const config = ts.readConfigFile(configPath, ts.sys.readFile);
  if (config.error) throw new Error(formatDiagnostics([config.error]));
  const parsed = ts.parseJsonConfigFileContent(
    config.config,
    ts.sys,
    path.dirname(configPath),
    undefined,
    configPath,
  );
  if (parsed.errors.length > 0) {
    throw new Error(formatDiagnostics(parsed.errors));
  }
  if (!Array.isArray(entries) || entries.length === 0) {
    throw new Error("TypeScript source graph requires at least one entry.");
  }
  const entryFiles = entries.map((entry) => {
    const absolute = path.resolve(root, entry);
    const source = ownedSource(root, absolute);
    if (!ts.sys.fileExists(absolute)) {
      throw new Error(`TypeScript source graph entry does not exist: ${source}.`);
    }
    return absolute;
  });
  const program = ts.createProgram({
    rootNames: entryFiles,
    options: parsed.options,
  });
  const cache = ts.createModuleResolutionCache(
    root,
    (file) => ts.sys.useCaseSensitiveFileNames ? file : file.toLowerCase(),
    parsed.options,
  );
  const edges = [];
  const files = new Set();

  for (const sourceFile of program.getSourceFiles()) {
    if (!isOwnedFile(root, sourceFile.fileName)) continue;
    const from = ownedSource(root, sourceFile.fileName);
    files.add(from);
    for (const request of moduleRequests(sourceFile)) {
      const mode = ts.getModeForUsageLocation(
        sourceFile,
        request.literal,
        parsed.options,
      );
      const specifier = request.literal.text;
      const resolution = ts.resolveModuleName(
        stripViteResourceQuery(specifier),
        sourceFile.fileName,
        parsed.options,
        ts.sys,
        cache,
        undefined,
        mode,
      ).resolvedModule;
      const resolvedFileName =
        resolution?.resolvedFileName ??
        resolveRelativeAsset(sourceFile.fileName, specifier);
      const unresolvedExternalResource =
        !resolvedFileName && isExternalViteResource(specifier);
      if (!resolvedFileName && !unresolvedExternalResource) {
        const position = sourceFile.getLineAndCharacterOfPosition(
          request.literal.getStart(sourceFile),
        );
        throw new Error(
          `${from}:${position.line + 1}:${position.character + 1} cannot resolve ${JSON.stringify(specifier)} with ${path.basename(configPath)}.`,
        );
      }
      const external =
        unresolvedExternalResource || !isOwnedFile(root, resolvedFileName);
      const target = external ? null : ownedSource(root, resolvedFileName);
      if (target !== null) files.add(target);
      edges.push(
        Object.freeze({
          from,
          to: target,
          specifier,
          kind: request.kind,
          external,
        }),
      );
    }
  }

  edges.sort((left, right) =>
    [left.from, left.kind, left.specifier, left.to ?? ""].join("\0").localeCompare(
      [right.from, right.kind, right.specifier, right.to ?? ""].join("\0"),
    ),
  );
  return Object.freeze({
    configFile: posixRelative(root, configPath),
    edges: Object.freeze(edges),
    entries: Object.freeze(entryFiles.map((file) => ownedSource(root, file))),
    files,
    rootDir: root,
  });
}

function stripViteResourceQuery(specifier) {
  return specifier.split(/[?#]/u, 1)[0];
}

function resolveRelativeAsset(importer, specifier) {
  const request = stripViteResourceQuery(specifier);
  if (!request.startsWith(".")) return null;
  const candidate = path.resolve(path.dirname(importer), request);
  return ts.sys.fileExists(candidate) ? candidate : null;
}

function isExternalViteResource(specifier) {
  return (
    !specifier.startsWith(".") &&
    !specifier.startsWith("/") &&
    /[?#]/u.test(specifier)
  );
}

export function collectSourceClosure(
  graph,
  roots,
  { includeDynamic = false, includeTypeOnly = false } = {},
) {
  const files = new Set();
  const pending = [...roots];
  const edgesBySource = Map.groupBy(graph.edges, (edge) => edge.from);
  while (pending.length > 0) {
    const source = pending.pop();
    if (files.has(source)) continue;
    if (!graph.files.has(source)) {
      throw new Error(`Unknown TypeScript source graph root: ${source}.`);
    }
    files.add(source);
    for (const edge of edgesBySource.get(source) ?? []) {
      if (edge.external || edge.to === null) continue;
      if (edge.kind === "dynamic" && !includeDynamic) continue;
      if (edge.kind === "type" && !includeTypeOnly) continue;
      pending.push(edge.to);
    }
  }
  return files;
}

function moduleRequests(sourceFile) {
  const requests = [];
  const add = (literal, kind) => {
    if (literal && ts.isStringLiteralLike(literal)) {
      requests.push({ literal, kind });
    }
  };
  const visit = (node) => {
    if (ts.isImportDeclaration(node)) {
      add(node.moduleSpecifier, isTypeOnlyImport(node) ? "type" : "static");
    } else if (ts.isExportDeclaration(node) && node.moduleSpecifier) {
      add(node.moduleSpecifier, isTypeOnlyExport(node) ? "type" : "static");
    } else if (ts.isImportTypeNode(node)) {
      add(
        ts.isLiteralTypeNode(node.argument) ? node.argument.literal : undefined,
        "type",
      );
    } else if (
      ts.isCallExpression(node) &&
      node.expression.kind === ts.SyntaxKind.ImportKeyword
    ) {
      add(node.arguments[0], "dynamic");
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return requests;
}

function isTypeOnlyImport(node) {
  const clause = node.importClause;
  if (!clause) return false;
  if (clause.isTypeOnly) return true;
  return Boolean(
    !clause.name &&
      clause.namedBindings &&
      ts.isNamedImports(clause.namedBindings) &&
      clause.namedBindings.elements.length > 0 &&
      clause.namedBindings.elements.every((element) => element.isTypeOnly),
  );
}

function isTypeOnlyExport(node) {
  if (node.isTypeOnly) return true;
  return Boolean(
    node.exportClause &&
      ts.isNamedExports(node.exportClause) &&
      node.exportClause.elements.length > 0 &&
      node.exportClause.elements.every((element) => element.isTypeOnly),
  );
}

function isOwnedFile(root, file) {
  const relative = path.relative(root, path.resolve(file));
  return (
    relative !== "" &&
    !path.isAbsolute(relative) &&
    relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) &&
    !relative.split(path.sep).includes("node_modules")
  );
}

function ownedSource(root, file) {
  if (!isOwnedFile(root, file)) {
    throw new Error(`TypeScript source graph escaped its root: ${file}.`);
  }
  return posixRelative(root, file);
}

function posixRelative(root, file) {
  return path.relative(root, file).replaceAll(path.sep, "/");
}

function formatDiagnostics(diagnostics) {
  return ts.formatDiagnostics(diagnostics, {
    getCanonicalFileName: (file) => file,
    getCurrentDirectory: () => ts.sys.getCurrentDirectory(),
    getNewLine: () => ts.sys.newLine,
  });
}
