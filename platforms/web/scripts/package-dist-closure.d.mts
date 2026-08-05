export interface PackageDistClosure {
  readonly root: string;
  readonly packageId: string;
  readonly files: readonly string[];
  readonly javascriptModules: readonly string[];
  readonly declarationModules: readonly string[];
}

export interface PackageRuntimeDistClosure {
  readonly root: string;
  readonly packageId: string;
  readonly javascriptModules: readonly string[];
}

export function packageDistClosure(
  distRoot: string,
  packageId: string,
): Readonly<PackageDistClosure>;

export function packageRuntimeDistClosure(
  distRoot: string,
  packageId: string,
): Readonly<PackageRuntimeDistClosure>;
