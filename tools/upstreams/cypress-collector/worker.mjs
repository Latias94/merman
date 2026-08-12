import { createHash } from "node:crypto";
import { parentPort, workerData } from "node:worker_threads";

const { allowedRuntimeEffects, code, sourceSpec } = workerData;
const NativeBuffer = Buffer;
const runtimeEffectSet = new Set(allowedRuntimeEffects);
const registrations = [];
const calls = [];
const runtimeEffects = [];
const titleStack = [];
const registrationIds = new Set();
let currentRegistration = null;

const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const stableJson = (value) => {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(",")}]`;
  }
  return `{${Object.keys(value)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`)
    .join(",")}}`;
};

const fail = (message) => {
  throw new Error(`[${sourceSpec}] ${message}`);
};

const requireString = (value, description) => {
  if (typeof value !== "string" || value.length === 0) {
    fail(`${description} must be a non-empty string`);
  }
  return value;
};

const cloneJsonObject = (value, description) => {
  if (value === undefined) {
    return {};
  }
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${description} must be an object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    fail(`${description} must be a plain object`);
  }
  let serialized;
  try {
    serialized = JSON.stringify(value);
  } catch (error) {
    fail(`${description} is not JSON serializable: ${error.message}`);
  }
  if (serialized === undefined) {
    fail(`${description} is not JSON serializable`);
  }
  return JSON.parse(serialized);
};

const fullTitle = (title) => [...titleStack, title].join(" > ");

const runRegistration = (title, callback, skipped) => {
  requireString(title, "test title");
  if (typeof callback !== "function") {
    fail(`test ${JSON.stringify(title)} must register a callback`);
  }
  const id = fullTitle(title);
  if (!registrationIds.add(id)) {
    fail(`duplicate test registration ${JSON.stringify(id)}`);
  }
  const registration = {
    ordinal: registrations.length + 1,
    id,
    title,
    skipped,
  };
  registrations.push(registration);
  if (skipped) {
    return;
  }

  const previous = currentRegistration;
  currentRegistration = registration;
  try {
    const result = callback();
    if (result && typeof result.then === "function") {
      fail(`async test registration ${JSON.stringify(id)} is unsupported`);
    }
  } finally {
    currentRegistration = previous;
  }
};

const describe = (title, callback) => {
  requireString(title, "describe title");
  if (typeof callback !== "function") {
    fail(`describe ${JSON.stringify(title)} must register a callback`);
  }
  titleStack.push(title);
  try {
    const result = callback();
    if (result && typeof result.then === "function") {
      fail(`async describe ${JSON.stringify(fullTitle(""))} is unsupported`);
    }
  } finally {
    titleStack.pop();
  }
};

describe.skip = (title) => fail(`describe.skip is unsupported: ${JSON.stringify(title)}`);
describe.only = (title) => fail(`describe.only is unsupported: ${JSON.stringify(title)}`);

const it = (title, callback) => runRegistration(title, callback, false);
it.skip = (title, callback) => runRegistration(title, callback, true);
it.only = (title) => fail(`it.only is unsupported: ${JSON.stringify(title)}`);

const captureHelper = (helper, args) => {
  if (currentRegistration === null) {
    fail(`${helper} was called outside an active test registration`);
  }
  if (args.length > 4) {
    fail(`${helper} received ${args.length} arguments; at most four are supported`);
  }
  const diagram = requireString(args[0], `${helper} diagram`);
  const options = cloneJsonObject(args[1], `${helper} options`);
  const api = args[2] === undefined ? false : args[2];
  if (typeof api !== "boolean") {
    fail(`${helper} api argument must be a boolean`);
  }
  const validation = args[3] === undefined ? "absent" : "present";
  if (args[3] !== undefined && typeof args[3] !== "function") {
    fail(`${helper} validation argument must be a function when present`);
  }
  const helperOrdinal =
    calls.filter((call) => call.registration === currentRegistration.id).length + 1;
  const rawIdentity = stableJson({
    api,
    diagram,
    helper,
    options,
    registration: currentRegistration.id,
    validation,
  });
  calls.push({
    registration: currentRegistration.id,
    helperOrdinal,
    helper,
    diagram,
    options,
    api,
    validation,
    rawSha256: sha256(rawIdentity),
  });
};

const passiveHelper = (helper) => {
  fail(`passive helper ${helper} was executed during collection`);
};

const recordRuntimeEffect = (operation, details) => {
  if (currentRegistration === null) {
    fail(`runtime effect ${operation} occurred outside an active test registration`);
  }
  if (!runtimeEffectSet.has(operation)) {
    fail(`runtime effect ${operation} is not allowed for this collector scope`);
  }
  runtimeEffects.push({
    registration: currentRegistration.id,
    operation,
    ...details,
  });
};

const cy = new Proxy(
  {},
  {
    get(_target, property) {
      if (property !== "get") {
        fail(`unsupported Cypress runtime access cy.${String(property)}`);
      }
      return (selector) => {
        requireString(selector, "cy.get selector");
        let completed = false;
        return new Proxy(
          {},
          {
            get(_chainTarget, chainProperty) {
              if (chainProperty !== "should") {
                fail(`unsupported Cypress chain cy.get(...).${String(chainProperty)}`);
              }
              return (...args) => {
                if (completed) {
                  fail("cy.get(...).should(...) may be called only once during collection");
                }
                completed = true;
                if (args.length === 0) {
                  fail("cy.get(...).should(...) requires an assertion argument");
                }
                recordRuntimeEffect("cy.get.should", {
                  selector,
                  argumentKinds: args.map((argument) => typeof argument),
                });
                return undefined;
              };
            },
          }
        );
      };
    },
  }
);

const forbidden = (name) => () => fail(`${name} is unavailable during collection`);
const forbiddenObject = (name) =>
  new Proxy(Object.create(null), {
    get(_target, property) {
      fail(`${name}.${String(property)} is unavailable during collection`);
    },
    set(_target, property) {
      fail(`${name}.${String(property)} cannot be mutated during collection`);
    },
  });
const restrictedMath = new Proxy(Math, {
  get(target, property, receiver) {
    if (property === "random") {
      return forbidden("Math.random");
    }
    return Reflect.get(target, property, receiver);
  },
});
class RestrictedDate {
  constructor() {
    fail("Date is unavailable during collection");
  }

  static now() {
    fail("Date.now is unavailable during collection");
  }

  static parse() {
    fail("Date.parse is unavailable during collection");
  }

  static UTC() {
    fail("Date.UTC is unavailable during collection");
  }
}

Object.assign(globalThis, {
  __mermanCypressCollector: {
    capture: captureHelper,
    passive: passiveHelper,
  },
  describe,
  it,
  cy,
  before: forbidden("before"),
  after: forbidden("after"),
  beforeEach: forbidden("beforeEach"),
  afterEach: forbidden("afterEach"),
  context: forbidden("context"),
  specify: forbidden("specify"),
  test: forbidden("test"),
  fetch: forbidden("fetch"),
  setTimeout: forbidden("setTimeout"),
  setInterval: forbidden("setInterval"),
  setImmediate: forbidden("setImmediate"),
  queueMicrotask: forbidden("queueMicrotask"),
});
for (const [name, value] of Object.entries({
  process: forbiddenObject("process"),
  crypto: forbiddenObject("crypto"),
  performance: forbiddenObject("performance"),
  Buffer: forbiddenObject("Buffer"),
  Date: RestrictedDate,
  Math: restrictedMath,
  eval: forbidden("eval"),
})) {
  Object.defineProperty(globalThis, name, {
    value,
    writable: false,
    enumerable: false,
    configurable: false,
  });
}

const forbiddenConsole = new Proxy(
  {},
  {
    get(_target, property) {
      return forbidden(`console.${String(property)}`);
    },
  }
);
globalThis.console = forbiddenConsole;

try {
  const moduleUrl = `data:text/javascript;base64,${NativeBuffer.from(code).toString("base64")}`;
  await import(moduleUrl);
  parentPort.postMessage({ registrations, calls, runtimeEffects });
} catch (error) {
  parentPort.postMessage({
    error: error instanceof Error ? `${error.message}\n${error.stack ?? ""}` : String(error),
  });
}
