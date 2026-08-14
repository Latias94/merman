interface MermaidQueryProfile {
  readonly relativePath: 'queries/portable/highlights.scm';
  readonly path: string;
  readonly source: string;
}

interface MermaidLanguage {
  readonly name: 'mermaid';
  readonly language: object;
  readonly nodeTypeInfo: readonly object[];
  readonly artifactReceipt: Readonly<Record<string, unknown>>;
  readonly queryProfiles: Readonly<{
    portable: Readonly<{
      highlights: MermaidQueryProfile;
    }>;
  }>;
}

declare const mermaid: MermaidLanguage;

export = mermaid;
