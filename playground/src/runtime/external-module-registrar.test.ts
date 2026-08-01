import assert from "node:assert/strict";
import test from "node:test";
import type {
  ExternalDiagramDefinition,
  LayoutLoaderDefinition,
} from "mermaid";

import { createExternalModuleRegistrar } from "./external-module-registrar.ts";

test("registrar loads in parallel and registers canonical module order once", async () => {
  const events: string[] = [];
  const pendingElk = Promise.withResolvers<readonly LayoutLoaderDefinition[]>();
  const registrar = createExternalModuleRegistrar({
    externalDiagramLoaders: {
      zenuml: async () => {
        events.push("load:zenuml");
        return diagram("zenuml");
      },
    },
    layoutModuleLoaders: {
      elk: async () => {
        events.push("load:elk");
        return pendingElk.promise;
      },
      "tidy-tree": async () => {
        events.push("load:tidy-tree");
        return [layout("tidy-tree")];
      },
    },
  });
  const host = fakeHost(events);
  const registration = registrar.register(host, {
    externalDiagrams: ["zenuml"],
    layoutModules: ["elk", "tidy-tree"],
  });
  await Promise.resolve();
  assert.deepEqual(events, ["load:zenuml", "load:elk", "load:tidy-tree"]);
  pendingElk.resolve([layout("elk")]);
  await registration;
  await registrar.register(host, {
    externalDiagrams: ["zenuml"],
    layoutModules: ["elk", "tidy-tree"],
  });
  assert.deepEqual(events, [
    "load:zenuml",
    "load:elk",
    "load:tidy-tree",
    "register:diagrams:zenuml",
    "register:layouts:elk,tidy-tree",
  ]);
});

test("registrar serializes host mutations and retries a rejected import", async () => {
  const events: string[] = [];
  let attempts = 0;
  const registrar = createExternalModuleRegistrar({
    externalDiagramLoaders: {
      zenuml: async () => {
        attempts += 1;
        if (attempts === 1) throw new Error("offline");
        return diagram("zenuml");
      },
    },
    layoutModuleLoaders: {
      elk: async () => [layout("elk")],
      "tidy-tree": async () => [layout("tidy-tree")],
    },
  });
  const host = fakeHost(events);
  const requirements = {
    externalDiagrams: ["zenuml"] as const,
    layoutModules: [] as const,
  };
  await assert.rejects(registrar.register(host, requirements), /offline/);
  await registrar.register(host, requirements);
  assert.equal(attempts, 2);
  assert.deepEqual(events, ["register:diagrams:zenuml"]);
});

function fakeHost(events: string[]) {
  return {
    async registerExternalDiagrams(diagrams: readonly { id: string }[]) {
      events.push(`register:diagrams:${diagrams.map((item) => item.id).join(",")}`);
    },
    registerLayoutLoaders(layouts: readonly { name: string }[]) {
      events.push(`register:layouts:${layouts.map((item) => item.name).join(",")}`);
    },
  };
}

function diagram(id: string): ExternalDiagramDefinition {
  return {
    id,
    detector: () => true,
    loader: async () => {
      throw new Error("test loader is not executed");
    },
  };
}

function layout(name: string): LayoutLoaderDefinition {
  return {
    name,
    loader: async () => ({ render: async () => {} }),
  };
}
