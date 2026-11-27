import { localLayoutCapability } from "../src/transport-contract.mjs";

export function readLocalLayoutFlag(args) {
  const prefix = "--local-";
  const flags = args.filter((arg) => arg.startsWith(prefix));
  if (flags.length > 1) {
    throw new Error("Select one of --local-grid, --local-layered, or --local-shoal.");
  }
  const layout = flags[0]?.slice(prefix.length) ?? null;
  localLayoutCapability(layout);
  return layout;
}
