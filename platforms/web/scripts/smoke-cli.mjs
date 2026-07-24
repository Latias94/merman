import { assertKnownArgs, parseArgValue } from "./arg-parse.mjs";

export function parseSmokeCli(inputArgs) {
  assertKnownArgs(inputArgs, {
    valueArgs: ["--package-id"],
  });
  const packageId = parseArgValue(inputArgs, "--package-id");
  if (!packageId || !/^[a-z0-9][a-z0-9-]*$/.test(packageId)) {
    throw new Error("--package-id must be a lowercase package identifier.");
  }
  return {
    packageId,
  };
}

export function smokeUsage() {
  return "usage: node scripts/smoke.mjs [--package-id <full|analysis|render|editor|ascii>]";
}
