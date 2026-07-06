import { fileURLToPath } from "node:url";

const releaseVersionPattern = /^[0-9]+\.[0-9]+\.[0-9]+(?:-(alpha|beta|rc)\.[0-9]+)?$/;

export function npmDistTagForVersion(version) {
  const match = releaseVersionPattern.exec(version);
  if (!match) {
    throw new Error("version must be X.Y.Z or X.Y.Z-<prerelease>.N");
  }
  return match[1] ?? "latest";
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    process.stdout.write(`${npmDistTagForVersion(process.argv[2] ?? "")}\n`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(2);
  }
}
