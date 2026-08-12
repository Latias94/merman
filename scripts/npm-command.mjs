import process from "node:process";
import { spawnSync } from "node:child_process";

export function npmCommand(
  args,
  {
    platform = process.platform,
    execPath = process.execPath,
    env = process.env,
  } = {},
) {
  const npmExecPath =
    typeof env.npm_execpath === "string" ? env.npm_execpath.trim() : "";
  if (npmExecPath && !/\.(?:bat|cmd|exe)$/iu.test(npmExecPath)) {
    return {
      command: execPath,
      args: [npmExecPath, ...args],
    };
  }

  if (platform === "win32") {
    return {
      command: env.ComSpec || env.COMSPEC || "cmd.exe",
      args: ["/d", "/s", "/c", "npm.cmd", ...args],
    };
  }

  return {
    command: "npm",
    args: [...args],
  };
}

export function spawnNpmSync(
  args,
  {
    platform = process.platform,
    execPath = process.execPath,
    env = process.env,
    ...spawnOptions
  } = {},
) {
  const invocation = npmCommand(args, { platform, execPath, env });
  return spawnSync(invocation.command, invocation.args, {
    ...spawnOptions,
    env,
  });
}

export function assertSuccessfulNpmSpawn(result, description = "npm") {
  if (!result || typeof result !== "object") {
    throw new TypeError("spawn result must be an object");
  }
  const label = typeof description === "string" && description.trim()
    ? description.trim()
    : "npm";
  if (result.error) {
    throw new Error(`${label} could not start: ${result.error.message}`, {
      cause: result.error,
    });
  }
  if (result.signal !== null && result.signal !== undefined) {
    throw new Error(`${label} was terminated by signal ${result.signal}.`);
  }
  if (result.status === null || result.status === undefined) {
    throw new Error(`${label} ended without an exit status.`);
  }
  if (!Number.isInteger(result.status)) {
    throw new Error(`${label} returned an invalid exit status: ${String(result.status)}.`);
  }
  if (result.status !== 0) {
    const detail = [result.stderr, result.stdout]
      .map((value) => value === undefined || value === null ? "" : String(value).trim())
      .find(Boolean);
    throw new Error(
      `${label} exited with status ${result.status}${detail ? `: ${detail}` : "."}`,
    );
  }
  return result;
}
