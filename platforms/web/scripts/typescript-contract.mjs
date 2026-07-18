import path from "node:path";
import ts from "typescript";

export function loadTypeScriptContract({
  tsconfigPath,
  extraRootNames = [],
}) {
  const config = ts.readConfigFile(tsconfigPath, ts.sys.readFile);
  if (config.error) {
    throw new Error(formatDiagnostics([config.error]));
  }
  const parsed = ts.parseJsonConfigFileContent(
    config.config,
    ts.sys,
    path.dirname(tsconfigPath),
    undefined,
    tsconfigPath,
  );
  if (parsed.errors.length > 0) {
    throw new Error(formatDiagnostics(parsed.errors));
  }

  const rootNames = uniqueAbsolutePaths([
    ...parsed.fileNames,
    ...extraRootNames,
  ]);
  const program = ts.createProgram({
    rootNames,
    options: parsed.options,
  });
  const checker = program.getTypeChecker();

  return new TypeScriptContract(program, checker);
}

class TypeScriptContract {
  constructor(program, checker) {
    this.program = program;
    this.checker = checker;
  }

  diagnostics() {
    return ts.getPreEmitDiagnostics(this.program);
  }

  formatDiagnostics(diagnostics = this.diagnostics()) {
    return formatDiagnostics(diagnostics);
  }

  exportedValueNames(file) {
    return this.#exportedNames(file, ts.SymbolFlags.Value);
  }

  exportedTypeNames(file) {
    return this.#exportedNames(file, ts.SymbolFlags.Type);
  }

  exportedNames(file) {
    return new Set(this.#moduleExports(file).map((symbol) => symbol.name));
  }

  declaredValueExportNames(file) {
    const source = this.#sourceFile(file);
    const names = new Set();
    for (const statement of source.statements) {
      if (ts.isExportDeclaration(statement)) {
        if (statement.isTypeOnly || !statement.exportClause) continue;
        if (ts.isNamedExports(statement.exportClause)) {
          for (const element of statement.exportClause.elements) {
            if (!element.isTypeOnly) names.add(element.name.text);
          }
        }
        continue;
      }
      if (!hasExportModifier(statement)) continue;
      if (ts.isVariableStatement(statement)) {
        for (const declaration of statement.declarationList.declarations) {
          collectBindingNames(declaration.name, names);
        }
      } else if (
        (ts.isFunctionDeclaration(statement) ||
          ts.isClassDeclaration(statement) ||
          ts.isEnumDeclaration(statement)) &&
        statement.name
      ) {
        names.add(statement.name.text);
      }
    }
    return names;
  }

  typeOnlyStarExportSpecifiers(file) {
    return this.#starExportSpecifiers(file, true);
  }

  valueStarExportSpecifiers(file) {
    return this.#starExportSpecifiers(file, false);
  }

  exportedTypePropertyNames(file, exportName) {
    const type = this.#declaredExportType(file, exportName);
    return new Set(type.getProperties().map((property) => property.name));
  }

  exportedTypePropertyText(file, exportName, propertyName) {
    const type = this.#declaredExportType(file, exportName);
    const property = type.getProperty(propertyName);
    if (!property) {
      throw new Error(`${relativeFile(file)}: ${exportName}.${propertyName} is not declared`);
    }
    const declaration = property.valueDeclaration ?? property.declarations?.[0];
    if (!declaration) {
      throw new Error(`${relativeFile(file)}: ${exportName}.${propertyName} has no declaration`);
    }
    const propertyType = this.checker.getTypeOfSymbolAtLocation(property, declaration);
    return this.checker.typeToString(
      propertyType,
      declaration,
      ts.TypeFormatFlags.NoTruncation,
    );
  }

  exportedStringLiteralMembers(file, exportName) {
    const symbol = this.#resolvedExport(file, exportName);
    const type = this.#declaredExportType(file, exportName);
    const members = type.isUnion() ? type.types : [type];
    const literals = new Set(
      members
        .filter((member) => member.isStringLiteral())
        .map((member) => member.value),
    );
    for (const declaration of symbol.declarations ?? []) {
      if (!ts.isTypeAliasDeclaration(declaration)) continue;
      collectDeclaredStringLiterals(declaration.type, literals);
    }
    return literals;
  }

  exportedFunctionReturnPropertyNames(file, exportName) {
    const symbol = this.#resolvedExport(file, exportName);
    const declaration = symbol.valueDeclaration ?? symbol.declarations?.[0];
    if (!declaration) {
      throw new Error(`${relativeFile(file)}: ${exportName} has no value declaration`);
    }
    const type = this.checker.getTypeOfSymbolAtLocation(symbol, declaration);
    const signatures = type.getCallSignatures();
    if (signatures.length === 0) {
      throw new Error(`${relativeFile(file)}: ${exportName} is not callable`);
    }

    const names = new Set();
    for (const signature of signatures) {
      for (const property of this.checker.getReturnTypeOfSignature(signature).getProperties()) {
        names.add(property.name);
      }
    }
    return names;
  }

  #declaredExportType(file, exportName) {
    const symbol = this.#resolvedExport(file, exportName);
    return this.checker.getDeclaredTypeOfSymbol(symbol);
  }

  #exportedNames(file, requiredFlags) {
    const names = this.#moduleExports(file)
      .filter((symbol) => (this.#resolveAlias(symbol).flags & requiredFlags) !== 0)
      .map((symbol) => symbol.name);
    return new Set(names);
  }

  #resolvedExport(file, exportName) {
    const exported = this.#moduleExports(file).find((symbol) => symbol.name === exportName);
    if (!exported) {
      throw new Error(`${relativeFile(file)}: missing export ${exportName}`);
    }
    return this.#resolveAlias(exported);
  }

  #resolveAlias(symbol) {
    return (symbol.flags & ts.SymbolFlags.Alias) !== 0
      ? this.checker.getAliasedSymbol(symbol)
      : symbol;
  }

  #moduleExports(file) {
    const source = this.#sourceFile(file);
    const moduleSymbol = this.checker.getSymbolAtLocation(source);
    if (!moduleSymbol) {
      throw new Error(`${relativeFile(file)} is not an external module`);
    }
    return this.checker.getExportsOfModule(moduleSymbol);
  }

  #sourceFile(file) {
    const source = this.program.getSourceFile(path.resolve(file));
    if (!source) {
      throw new Error(`${relativeFile(file)} is not part of the TypeScript program`);
    }
    return source;
  }

  #starExportSpecifiers(file, typeOnly) {
    const specifiers = new Set();
    for (const statement of this.#sourceFile(file).statements) {
      if (
        ts.isExportDeclaration(statement) &&
        statement.isTypeOnly === typeOnly &&
        !statement.exportClause &&
        statement.moduleSpecifier &&
        ts.isStringLiteralLike(statement.moduleSpecifier)
      ) {
        specifiers.add(statement.moduleSpecifier.text);
      }
    }
    return specifiers;
  }
}

function uniqueAbsolutePaths(files) {
  return [...new Set(files.map((file) => path.resolve(file)))];
}

function relativeFile(file) {
  return path.relative(process.cwd(), file) || path.basename(file);
}

function formatDiagnostics(diagnostics) {
  return ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCanonicalFileName: (fileName) => fileName,
    getCurrentDirectory: ts.sys.getCurrentDirectory,
    getNewLine: () => ts.sys.newLine,
  });
}

function collectDeclaredStringLiterals(node, output) {
  if (
    ts.isLiteralTypeNode(node) &&
    ts.isStringLiteralLike(node.literal)
  ) {
    output.add(node.literal.text);
    return;
  }
  if (ts.isUnionTypeNode(node)) {
    for (const member of node.types) {
      collectDeclaredStringLiterals(member, output);
    }
  } else if (ts.isParenthesizedTypeNode(node)) {
    collectDeclaredStringLiterals(node.type, output);
  }
}

function hasExportModifier(node) {
  return node.modifiers?.some((modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword) ?? false;
}

function collectBindingNames(name, output) {
  if (ts.isIdentifier(name)) {
    output.add(name.text);
    return;
  }
  for (const element of name.elements) {
    if (!ts.isOmittedExpression(element)) {
      collectBindingNames(element.name, output);
    }
  }
}
