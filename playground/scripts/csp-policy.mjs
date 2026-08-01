const COMMON_DIRECTIVES = Object.freeze({
  "default-src": ["'none'"],
  "style-src": ["'self'", "'unsafe-inline'"],
  "img-src": ["'self'", "data:", "blob:"],
  "font-src": ["'self'", "data:"],
  "connect-src": ["'self'"],
  "object-src": ["'none'"],
  "base-uri": ["'none'"],
  "form-action": ["'none'"],
});

export function isCspHash(value) {
  return (
    typeof value === "string" &&
    /^sha256-[A-Za-z0-9+/]+={0,2}$/u.test(value)
  );
}

export function createExpectedCspPolicies(hashes) {
  const indexHashes = quotedHashes(hashes["index.html"], "index.html", 2);
  const benchmarkHashes = quotedHashes(
    hashes["benchmark.html"],
    "benchmark.html",
    0,
  );
  const corpusHashes = quotedHashes(
    hashes["benchmark-corpus.html"],
    "benchmark-corpus.html",
    1,
  );
  return Object.freeze({
    "index.html": Object.freeze({
      ...COMMON_DIRECTIVES,
      "script-src": [
        "'self'",
        "blob:",
        ...indexHashes,
        "'wasm-unsafe-eval'",
      ],
      "worker-src": ["'self'"],
      "frame-src": ["'self'"],
    }),
    "benchmark-corpus.html": Object.freeze({
      ...COMMON_DIRECTIVES,
      "script-src": [
        "'self'",
        "blob:",
        ...corpusHashes,
        "'wasm-unsafe-eval'",
      ],
      "worker-src": ["'none'"],
      "frame-src": ["'self'"],
    }),
    "benchmark.html": Object.freeze({
      ...COMMON_DIRECTIVES,
      "script-src": [
        "'self'",
        "blob:",
        ...benchmarkHashes,
        "'wasm-unsafe-eval'",
      ],
      "worker-src": ["'none'"],
      "frame-src": ["'none'"],
    }),
  });
}

export class CspContractError extends Error {
  constructor(message) {
    super(message);
    this.name = "CspContractError";
  }
}

export function parseCspPolicy(content) {
  if (typeof content !== "string" || content.trim() === "") {
    throw new CspContractError("CSP content must be a non-empty string.");
  }

  const directives = new Map();
  for (const rawDirective of content.split(";")) {
    const tokens = rawDirective.trim().split(/\s+/u).filter(Boolean);
    if (tokens.length === 0) continue;
    const [rawName, ...values] = tokens;
    const name = rawName.toLowerCase();
    if (!/^[a-z][a-z0-9-]*$/u.test(name)) {
      throw new CspContractError(`Invalid CSP directive name: ${rawName}`);
    }
    if (directives.has(name)) {
      throw new CspContractError(`Duplicate CSP directive: ${name}`);
    }
    if (new Set(values).size !== values.length) {
      throw new CspContractError(`Duplicate CSP source in directive: ${name}`);
    }
    directives.set(name, values);
  }
  return directives;
}

export function extractMetaCsp(html) {
  if (typeof html !== "string") {
    throw new CspContractError("HTML must be a string.");
  }

  const policies = [];
  for (const match of html.matchAll(/<meta\b(?:[^>"']|"[^"]*"|'[^']*')*>/giu)) {
    const attributes = parseHtmlAttributes(match[0]);
    if (attributes.get("http-equiv")?.toLowerCase() === "content-security-policy") {
      const content = attributes.get("content");
      if (content === undefined) {
        throw new CspContractError("CSP meta element is missing its content attribute.");
      }
      policies.push(content);
    }
  }

  if (policies.length !== 1) {
    throw new CspContractError(
      `Expected exactly one CSP meta element, found ${policies.length}.`,
    );
  }
  return policies[0];
}

export function verifyHtmlCsp(fileName, html, expectedPolicies) {
  const expected = expectedPolicies[fileName];
  if (!expected) {
    return [`No CSP contract is defined for ${fileName}.`];
  }

  let actual;
  try {
    actual = parseCspPolicy(extractMetaCsp(html));
  } catch (error) {
    return [error instanceof Error ? error.message : String(error)];
  }

  const violations = [];
  const expectedNames = new Set(Object.keys(expected));
  for (const name of actual.keys()) {
    if (!expectedNames.has(name)) {
      violations.push(`${fileName} has unexpected CSP directive ${name}.`);
    }
  }

  for (const [name, expectedValues] of Object.entries(expected)) {
    const actualValues = actual.get(name);
    if (!actualValues) {
      violations.push(`${fileName} is missing CSP directive ${name}.`);
      continue;
    }
    if (!sameStringSet(actualValues, expectedValues)) {
      violations.push(
        `${fileName} CSP ${name} must be [${expectedValues.join(", ")}], found [${actualValues.join(", ")}].`,
      );
    }
  }
  return violations;
}

function quotedHashes(hashes, fileName, expectedCount) {
  if (
    !Array.isArray(hashes) ||
    hashes.length !== expectedCount ||
    new Set(hashes).size !== hashes.length ||
    hashes.some((hash) => !isCspHash(hash))
  ) {
    throw new CspContractError(
      `${fileName} has an invalid opaque-realm hash set.`,
    );
  }
  return hashes.map((hash) => `'${hash}'`);
}

function parseHtmlAttributes(tag) {
  const attributes = new Map();
  const pattern = /([A-Za-z_:][A-Za-z0-9_.:-]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+))/gu;
  for (const match of tag.matchAll(pattern)) {
    const name = match[1].toLowerCase();
    if (attributes.has(name)) {
      throw new CspContractError(`Duplicate HTML attribute on CSP meta element: ${name}`);
    }
    attributes.set(name, match[2] ?? match[3] ?? match[4] ?? "");
  }
  return attributes;
}

function sameStringSet(left, right) {
  return left.length === right.length && left.every((value) => right.includes(value));
}
