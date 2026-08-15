interface MermaidQueryProfile {
  readonly relativePath: `queries/${MermaidQueryProfileName}/${MermaidQuerySurfaceName}.scm`;
  readonly path: string;
  readonly source: string;
}

type MermaidQueryProfileName = 'portable' | 'neovim' | 'helix' | 'zed';
type MermaidQuerySurfaceName =
  | 'highlights'
  | 'folds'
  | 'indents'
  | 'injections'
  | 'locals'
  | 'tags'
  | 'brackets'
  | 'outline'
  | 'textobjects';

interface MermaidLanguage {
  readonly name: 'mermaid';
  readonly language: object;
  readonly nodeTypeInfo: readonly object[];
  readonly artifactReceipt: Readonly<Record<string, unknown>>;
  readonly queryProfiles: Readonly<
    Partial<
      Record<
        MermaidQueryProfileName,
        Readonly<Partial<Record<MermaidQuerySurfaceName, MermaidQueryProfile>>>
      >
    >
  >;
}

declare const mermaid: MermaidLanguage;

export = mermaid;
