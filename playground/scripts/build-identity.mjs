import { execFileSync } from "node:child_process";

/** Read the checkout actually built, including detached CI merge commits. */
export function readBuildIdentity(repositoryRoot, env = process.env) {
  const git = (...args) => execFileSync("git", args, {
    cwd: repositoryRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
  const ci = env.GITHUB_ACTIONS === "true";
  let commit = null;
  let dirty = false;
  try {
    commit = git("rev-parse", "--verify", "HEAD");
    dirty = !ci && git("status", "--porcelain", "--untracked-files=normal") !== "";
  } catch {
    if (ci) throw new Error("Playground CI build requires a readable Git checkout.");
  }
  if (ci && commit !== env.GITHUB_SHA) {
    throw new Error("Playground checkout does not match the GitHub Actions commit.");
  }
  const ref = ci ? env.GITHUB_REF ?? null : null;
  const channel = !ci ? "local"
    : ["refs/heads/main", "refs/heads/master"].includes(ref) ? "main"
    : ref?.startsWith("refs/tags/") ? "tag" : "preview";
  const repository = ci && /^[\w.-]+\/[\w.-]+$/.test(env.GITHUB_REPOSITORY ?? "")
    ? env.GITHUB_REPOSITORY : null;
  return { channel, commit, ref, dirty, repository };
}
