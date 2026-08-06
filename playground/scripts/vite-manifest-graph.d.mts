export interface ViteManifestChunk {
  readonly file: string;
  readonly src?: string;
  readonly isEntry?: boolean;
  readonly isDynamicEntry?: boolean;
  readonly imports: readonly string[];
  readonly dynamicImports: readonly string[];
  readonly assets: readonly string[];
  readonly css: readonly string[];
}

export interface ViteManifestGraph {
  readonly chunks: Readonly<Record<string, ViteManifestChunk>>;
}

export interface ViteManifestOutput {
  readonly key: string;
  readonly kind: "file" | "css" | "asset";
  readonly file: string;
}

export interface HtmlStaticAsset {
  readonly kind: "script" | "modulepreload" | "stylesheet";
  readonly url: string;
}

export function parseViteManifest(value: unknown): ViteManifestGraph;
export function manifestKeysForSource(
  graph: ViteManifestGraph,
  source: string,
): string[];
export function manifestChunk(
  graph: ViteManifestGraph,
  key: string,
): ViteManifestChunk;
export function emittedResources(
  graph: ViteManifestGraph,
  keys: Iterable<string>,
): Set<string>;
export function missingStaticStylesheets(
  graph: ViteManifestGraph,
  keys: Iterable<string>,
  linkedStylesheets: Iterable<string>,
): readonly string[];
export function manifestOutputs(
  graph: ViteManifestGraph,
): readonly ViteManifestOutput[];
export function missingManifestOutputs(
  graph: ViteManifestGraph,
  isAvailable: (file: string) => boolean,
): readonly ViteManifestOutput[];
export function htmlStaticAssets(html: string): readonly HtmlStaticAsset[];
