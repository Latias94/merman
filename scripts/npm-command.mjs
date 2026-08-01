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
