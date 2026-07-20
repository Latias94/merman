import assert from "node:assert/strict";
import test from "node:test";

import {
  WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION,
  parseWebSurfaceDescriptor,
  webSurfaceDescriptor,
} from "./web-surface-descriptor.mjs";

test("checked-in Web descriptor has one valid closed surface graph", () => {
  assert.equal(
    webSurfaceDescriptor.schema_version,
    WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION,
  );
  assert.equal(webSurfaceDescriptor.default_preset, "browser-full");
  assert.deepEqual(
    webSurfaceDescriptor.public_surfaces.map(({ entry }) => entry),
    ["core", "render", "render-only", "ascii", "editor", "full"],
  );

  const bridge = webSurfaceDescriptor.presets.find(
    ({ name }) => name === "browser-bridge",
  );
  assert.deepEqual(bridge?.features, []);
  assert.deepEqual(
    Object.values(bridge?.capabilities ?? {}),
    Array(8).fill(false),
  );

  const editor = webSurfaceDescriptor.presets.find(
    ({ name }) => name === "browser-editor",
  );
  assert.deepEqual(editor?.features, ["core-full", "editor-language"]);
  assert.equal(editor?.capabilities.core_full, true);
  assert.equal(editor?.capabilities.editor_language, true);
  assert.equal(editor?.capabilities.render, false);
});

test("descriptor rejects an unsupported schema", () => {
  const descriptor = cloneDescriptor();
  descriptor.schema_version += 1;

  assert.throws(
    () => parseWebSurfaceDescriptor(descriptor),
    /schema must be 1/,
  );
});

test("descriptor rejects duplicate preset and public surface ownership", () => {
  const duplicatePreset = cloneDescriptor();
  duplicatePreset.presets.push(structuredClone(duplicatePreset.presets[0]));
  assert.throws(
    () => parseWebSurfaceDescriptor(duplicatePreset),
    /Duplicate preset name/,
  );

  const duplicateSurface = cloneDescriptor();
  duplicateSurface.public_surfaces.push(
    structuredClone(duplicateSurface.public_surfaces[0]),
  );
  assert.throws(
    () => parseWebSurfaceDescriptor(duplicateSurface),
    /Duplicate public surface entry/,
  );
});

test("descriptor rejects dangling preset references and unknown runtime profiles", () => {
  const dangling = cloneDescriptor();
  dangling.public_surfaces[0].preset = "browser-missing";
  assert.throws(
    () => parseWebSurfaceDescriptor(dangling),
    /references unknown preset browser-missing/,
  );

  const unknownProfile = cloneDescriptor();
  unknownProfile.public_surfaces[0].runtime_profile = "unknown";
  assert.throws(
    () => parseWebSurfaceDescriptor(unknownProfile),
    /unknown runtime profile unknown/,
  );
});

test("descriptor requires the complete boolean capability record", () => {
  const descriptor = cloneDescriptor();
  delete descriptor.presets[0].capabilities.editor_language;

  assert.throws(
    () => parseWebSurfaceDescriptor(descriptor),
    /capabilities keys must be exactly/,
  );
});

function cloneDescriptor() {
  return structuredClone(webSurfaceDescriptor);
}
