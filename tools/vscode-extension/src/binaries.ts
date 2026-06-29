import * as fs from "node:fs";
import * as path from "node:path";

export type MermanBinaryName = "merman-lsp" | "merman-cli";

export type BinaryResolutionSource =
  | "explicit"
  | "cargo"
  | "packaged"
  | "workspace-debug";

export interface BinaryInvocation {
  command: string;
  args: string[];
  cwd?: string;
  source: BinaryResolutionSource;
  label: string;
}

export interface BinaryResolutionRequest {
  binaryName: MermanBinaryName;
  packageName: "merman-lsp" | "merman-cli";
  extensionPath: string;
  workspaceRoots: readonly string[];
  directArgs?: readonly string[];
  explicitPath?: string;
  useCargoRun?: boolean;
  cargoArgs?: readonly string[];
  platform?: NodeJS.Platform;
  arch?: NodeJS.Architecture;
}

export function resolveMermanBinary(request: BinaryResolutionRequest): BinaryInvocation {
  const directArgs = [...(request.directArgs ?? [])];
  const explicitPath = normalizePath(request.explicitPath);
  if (explicitPath) {
    assertExecutable(explicitPath, request.binaryName, "configured path");
    return {
      command: explicitPath,
      args: directArgs,
      source: "explicit",
      label: `${request.binaryName} from configured path`,
    };
  }

  if (request.useCargoRun === true) {
    return resolveCargoInvocation(request, directArgs);
  }

  const packagedBinary = findPackagedBinary(request);
  if (packagedBinary) {
    return {
      command: packagedBinary,
      args: directArgs,
      source: "packaged",
      label: `${request.binaryName} from extension package`,
    };
  }

  const workspaceBinary = findWorkspaceDebugBinary(
    request.binaryName,
    request.workspaceRoots,
    request.platform,
  );
  if (workspaceBinary) {
    return {
      command: workspaceBinary,
      args: directArgs,
      cwd: request.workspaceRoots[0],
      source: "workspace-debug",
      label: `${request.binaryName} from workspace target/debug`,
    };
  }

  throw new Error(
    [
      `Unable to find ${request.binaryName}.`,
      `Install a packaged Merman extension that includes ${request.binaryName}, configure its absolute path, or enable the Cargo development fallback.`,
    ].join(" "),
  );
}

export function binaryFileName(
  baseName: MermanBinaryName,
  platform: NodeJS.Platform = process.platform,
): string {
  return platform === "win32" ? `${baseName}.exe` : baseName;
}

export function platformKey(
  platform: NodeJS.Platform = process.platform,
  arch: NodeJS.Architecture = process.arch,
): string {
  return `${platform}-${arch}`;
}

export function findPackagedBinary(
  request: Pick<BinaryResolutionRequest, "binaryName" | "extensionPath" | "platform" | "arch">,
): string | undefined {
  const fileName = binaryFileName(request.binaryName, request.platform);
  const candidates = [
    path.join(
      request.extensionPath,
      "bin",
      platformKey(request.platform, request.arch),
      fileName,
    ),
    path.join(request.extensionPath, "bin", fileName),
  ];
  return candidates.find(isExecutableFile);
}

export function findWorkspaceDebugBinary(
  binaryName: MermanBinaryName,
  workspaceRoots: readonly string[],
  platform: NodeJS.Platform = process.platform,
): string | undefined {
  const fileName = binaryFileName(binaryName, platform);
  for (const root of workspaceRoots) {
    const binaryPath = path.join(root, "target", "debug", fileName);
    if (isExecutableFile(binaryPath)) {
      return binaryPath;
    }
  }
  return undefined;
}

function resolveCargoInvocation(
  request: BinaryResolutionRequest,
  directArgs: string[],
): BinaryInvocation {
  const cwd = request.workspaceRoots[0];
  if (!cwd) {
    throw new Error(
      `Cannot launch ${request.binaryName} through Cargo because no workspace folder is open.`,
    );
  }

  return {
    command: "cargo",
    args: [
      "run",
      "-p",
      request.packageName,
      ...(request.cargoArgs ?? []),
      "--",
      ...directArgs,
    ],
    cwd,
    source: "cargo",
    label: `${request.binaryName} through Cargo development fallback`,
  };
}

function normalizePath(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : undefined;
}

function assertExecutable(filePath: string, binaryName: MermanBinaryName, source: string): void {
  if (!isExecutableFile(filePath)) {
    throw new Error(`${binaryName} ${source} does not exist or is not a file: ${filePath}`);
  }
}

function isExecutableFile(filePath: string): boolean {
  try {
    return fs.statSync(filePath).isFile();
  } catch {
    return false;
  }
}
