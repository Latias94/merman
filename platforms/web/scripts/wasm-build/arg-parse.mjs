import { lstatSync } from "node:fs";
import path from "node:path";

export class ArgParseError extends Error {
  constructor(message) {
    super(message);
    this.name = "ArgParseError";
  }
}

export function hasHelpFlag(args) {
  return args.includes("--help") || args.includes("-h");
}

export function parseArgValue(args, name) {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === name) {
      const value = args[index + 1];
      if (!isPresentValue(value)) {
        throw new ArgParseError(`Missing value for ${name}.`);
      }
      return value;
    }
    if (arg.startsWith(`${name}=`)) {
      const value = arg.slice(name.length + 1);
      if (!isPresentValue(value)) {
        throw new ArgParseError(`Missing value for ${name}.`);
      }
      return value;
    }
  }
  return null;
}

export function assertKnownArgs(args, { valueArgs = [], booleanArgs = [] } = {}) {
  const valueArgSet = new Set(valueArgs);
  const booleanArgSet = new Set(booleanArgs);

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (booleanArgSet.has(arg)) {
      continue;
    }
    if (valueArgSet.has(arg)) {
      const value = args[index + 1];
      if (!isPresentValue(value)) {
        throw new ArgParseError(`Missing value for ${arg}.`);
      }
      index += 1;
      continue;
    }

    const equalsIndex = arg.indexOf("=");
    if (equalsIndex > 0) {
      const name = arg.slice(0, equalsIndex);
      if (valueArgSet.has(name)) {
        const value = arg.slice(equalsIndex + 1);
        if (!isPresentValue(value)) {
          throw new ArgParseError(`Missing value for ${name}.`);
        }
        continue;
      }
    }

    throw new ArgParseError(`Unknown argument: ${arg}.`);
  }
}

export function resolvePackageSubdir(packageRoot, relativeDir, optionName = "package directory") {
  if (typeof relativeDir !== "string" || relativeDir.trim().length === 0) {
    throw new ArgParseError(`Missing value for ${optionName}.`);
  }

  const normalizedRelativeDir = relativeDir.trim();
  if (path.isAbsolute(normalizedRelativeDir)) {
    throw new ArgParseError(`${optionName} must be relative to the web package root.`);
  }
  if (normalizedRelativeDir.split(/[\\/]+/u).includes("..")) {
    throw new ArgParseError(`${optionName} must not contain .. path segments.`);
  }

  const pkgRoot = path.resolve(packageRoot, "pkg");
  const resolved = path.resolve(packageRoot, normalizedRelativeDir);
  const relativeToPkg = path.relative(pkgRoot, resolved);
  if (
    relativeToPkg === "" ||
    (!relativeToPkg.startsWith("..") && !path.isAbsolute(relativeToPkg))
  ) {
    assertNoLinkedPackagePathComponents(pkgRoot, resolved, optionName);
    return {
      absolute: resolved,
      relative: path.relative(packageRoot, resolved),
    };
  }

  throw new ArgParseError(`${optionName} must resolve to pkg or a subdirectory of pkg.`);
}

function assertNoLinkedPackagePathComponents(pkgRoot, resolved, optionName) {
  const root = path.resolve(pkgRoot);
  const target = path.resolve(resolved);
  const relative = path.relative(root, target);
  const segments = relative === "" ? [] : relative.split(path.sep).filter(Boolean);
  let current = root;

  for (const segment of ["", ...segments]) {
    if (segment !== "") {
      current = path.join(current, segment);
    }
    let stat;
    try {
      stat = lstatSync(current);
    } catch (error) {
      if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") {
        continue;
      }
      throw error;
    }
    if (stat.isSymbolicLink()) {
      throw new ArgParseError(
        `${optionName} must not pass through a symlink or junction inside pkg: ${current}.`,
      );
    }
  }
}

function isPresentValue(value) {
  return typeof value === "string" && value.length > 0 && !value.startsWith("--");
}
