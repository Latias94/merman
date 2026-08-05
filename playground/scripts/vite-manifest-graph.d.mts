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

export function parseViteManifest(value: unknown): ViteManifestGraph;
export function manifestKeysForSource(
  graph: ViteManifestGraph,
  source: string,
): string[];
export function manifestChunk(
  graph: ViteManifestGraph,
  key: string,
): ViteManifestChunk;
