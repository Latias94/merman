import type {
  ExternalDiagramDefinition,
  LayoutLoaderDefinition,
} from "mermaid";

import {
  normalizeMermaidExternalRequirements,
  type MermaidExternalDiagramModuleId,
  type MermaidExternalRequirements,
  type MermaidLayoutModuleId,
} from "./mermaid-requirements.ts";

export interface MermaidModuleRegistrationHost {
  registerExternalDiagrams(
    diagrams: ExternalDiagramDefinition[],
    options: { readonly lazyLoad: false }
  ): void | Promise<void>;
  registerLayoutLoaders(layouts: LayoutLoaderDefinition[]): void;
}

interface RegistrarLoaders {
  readonly externalDiagramLoaders: Readonly<
    Record<
      MermaidExternalDiagramModuleId,
      () => Promise<ExternalDiagramDefinition>
    >
  >;
  readonly layoutModuleLoaders: Readonly<
    Record<
      MermaidLayoutModuleId,
      () => Promise<readonly LayoutLoaderDefinition[]>
    >
  >;
}

interface HostRegistrationState {
  readonly externalDiagrams: Set<MermaidExternalDiagramModuleId>;
  readonly layoutModules: Set<MermaidLayoutModuleId>;
  queue: Promise<void>;
}

export interface ExternalModuleRegistrar {
  register(
    host: MermaidModuleRegistrationHost,
    requirements: MermaidExternalRequirements
  ): Promise<void>;
}

export function createExternalModuleRegistrar(
  loaders: RegistrarLoaders
): ExternalModuleRegistrar {
  const hostStates = new WeakMap<
    MermaidModuleRegistrationHost,
    HostRegistrationState
  >();
  const externalImports = new Map<
    MermaidExternalDiagramModuleId,
    Promise<ExternalDiagramDefinition>
  >();
  const layoutImports = new Map<
    MermaidLayoutModuleId,
    Promise<readonly LayoutLoaderDefinition[]>
  >();

  const loadExternal = (id: MermaidExternalDiagramModuleId) => {
    const existing = externalImports.get(id);
    if (existing) return existing;
    const pending = loaders.externalDiagramLoaders[id]().catch((error) => {
      if (externalImports.get(id) === pending) externalImports.delete(id);
      throw error;
    });
    externalImports.set(id, pending);
    return pending;
  };
  const loadLayout = (id: MermaidLayoutModuleId) => {
    const existing = layoutImports.get(id);
    if (existing) return existing;
    const pending = loaders.layoutModuleLoaders[id]().catch((error) => {
      if (layoutImports.get(id) === pending) layoutImports.delete(id);
      throw error;
    });
    layoutImports.set(id, pending);
    return pending;
  };

  return {
    register(host, rawRequirements) {
      const requirements = normalizeMermaidExternalRequirements(
        rawRequirements
      );
      let state = hostStates.get(host);
      if (!state) {
        state = {
          externalDiagrams: new Set(),
          layoutModules: new Set(),
          queue: Promise.resolve(),
        };
        hostStates.set(host, state);
      }
      const operation = state.queue.then(async () => {
        const missingExternal = requirements.externalDiagrams.filter(
          (id) => !state.externalDiagrams.has(id)
        );
        const missingLayouts = requirements.layoutModules.filter(
          (id) => !state.layoutModules.has(id)
        );
        if (missingExternal.length === 0 && missingLayouts.length === 0) return;

        const [externalDiagrams, layoutGroups] = await Promise.all([
          Promise.all(missingExternal.map(loadExternal)),
          Promise.all(missingLayouts.map(loadLayout)),
        ]);
        if (externalDiagrams.length > 0) {
          await host.registerExternalDiagrams(externalDiagrams, {
            lazyLoad: false,
          });
          for (const id of missingExternal) state.externalDiagrams.add(id);
        }
        if (layoutGroups.length > 0) {
          host.registerLayoutLoaders(layoutGroups.flat());
          for (const id of missingLayouts) state.layoutModules.add(id);
        }
      });
      state.queue = operation.catch(() => undefined);
      return operation;
    },
  };
}

export const mermaidExternalModuleRegistrar = createExternalModuleRegistrar({
  externalDiagramLoaders: {
    zenuml: async () => (await import("@mermaid-js/mermaid-zenuml")).default,
  },
  layoutModuleLoaders: {
    elk: async () => (await import("@mermaid-js/layout-elk")).default,
    "tidy-tree": async () =>
      (await import("@mermaid-js/layout-tidy-tree")).default,
  },
});
