import * as fs from "node:fs";
import * as path from "node:path";

export type MermanBinaryName = "merman-lsp" | "merman-cli";

export type BinaryResolutionSource =
  | "explicit"
  | "cargo"
  | "packaged";

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
  workspaceTrusted?: boolean;
  platform?: NodeJS.Platform;
  arch?: NodeJS.Architecture;
}

export function resolveMermanBinary(request: BinaryResolutionRequest): BinaryInvocation {
  const directArgs = [...(request.directArgs ?? [])];
  const explicitPath = normalizePath(request.explicitPath);
  if (explicitPath) {
    assertExecutable(explicitPath, request.binaryName, "configured path");
    assertTrustedWorkspaceExecutable(explicitPath, request);
    return {
      command: explicitPath,
      args: directArgs,
      source: "explicit",
      label: `${request.binaryName} from configured path`,
    };
  }

  if (request.useCargoRun === true) {
    assertTrustedCargoFallback(request);
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

  throw new Error(
    [
      `Unable to find ${request.binaryName}.`,
      `Install a packaged Merman extension that includes ${request.binaryName}, configure its absolute path, or enable the trusted Cargo development fallback.`,
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

function assertTrustedWorkspaceExecutable(
  filePath: string,
  request: Pick<BinaryResolutionRequest, "binaryName" | "workspaceRoots" | "workspaceTrusted">,
): void {
  if (!isWorkspaceLocalPath(filePath, request.workspaceRoots)) {
    return;
  }
  if (request.workspaceTrusted === true) {
    return;
  }
  throw new Error(
    `${request.binaryName} configured path points inside the current workspace and requires a trusted workspace.`,
  );
}

function assertTrustedCargoFallback(
  request: Pick<BinaryResolutionRequest, "binaryName" | "workspaceRoots" | "workspaceTrusted">,
): void {
  if (request.workspaceRoots.length === 0 || request.workspaceTrusted === true) {
    return;
  }
  throw new Error(
    `${request.binaryName} Cargo development fallback requires a trusted workspace.`,
  );
}

function isWorkspaceLocalPath(filePath: string, workspaceRoots: readonly string[]): boolean {
  const resolvedFilePath = path.resolve(filePath);
  return workspaceRoots.some((root) => isPathInside(resolvedFilePath, root));
}

function isPathInside(filePath: string, root: string): boolean {
  const resolvedRoot = path.resolve(root);
  const relative = path.relative(resolvedRoot, filePath);
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

function isExecutableFile(filePath: string): boolean {
  try {
    return fs.statSync(filePath).isFile();
  } catch {
    return false;
  }
}
