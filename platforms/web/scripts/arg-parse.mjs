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

function isPresentValue(value) {
  return typeof value === "string" && value.length > 0 && !value.startsWith("--");
}
