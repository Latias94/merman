import assert from "node:assert/strict";

// Supply the installed public module so both transports prove the actual Rust registry path.
export async function runIconRegistrySmoke(module) {
  const marker = "node-icon-registry-smoke";
  const json = JSON.stringify({
    prefix: "original",
    icons: {
      rocket: { body: `<path data-icon="${marker}" d="M0 0H16V16H0z"/>` },
      database: { body: `<path data-icon="${marker}-override" d="M0 0H16V16H0z"/>` },
    },
  });
  const iconPacks = [
    { json, registrationName: "fleet" },
    { json, registrationName: "mermaid-architecture" },
  ];
  const engines = [];
  const counts = { svg_renders: 0, constructor_errors: 0, isolation_checks: 0, dispose_checks: 0 };
  try {
    const scoped = await module.createNodeEngine({ iconPacks });
    engines.push(scoped);
    const plain = await module.createNodeEngine();
    engines.push(plain);
    const empty = await module.createNodeEngine({ iconPacks: [] });
    engines.push(empty);
    assert.ok(scoped.runtimeCatalog.constructor_service_ids.includes("icon-registry"));
    const contract = scoped.runtimeCatalog.constructor_service_contracts.find(
      ({ id }) => id === "icon-registry",
    );
    assert.ok(contract.resource_limits.some(({ id }) => id === "max_icon_registry_input_bytes"));
    // The constructor snapshots loaded data; caller mutations do not change an admitted registry.
    iconPacks[0].json = "{}";
    const source = 'flowchart TD\nA@{ icon: "fleet:rocket", label: "A" }';
    const options = { optionsJson: JSON.stringify({
      site_config: { htmlLabels: false }, svg: { diagram_id: "node-icon-registry" },
    }) };
    const registered = await scoped.renderSvg(source, options);
    assert.match(registered, new RegExp(`data-icon="${marker}"`));
    assert.equal(scoped.renderSvgSync(source, options), registered);
    const unregistered = await plain.renderSvg(source, options);
    assert.ok(!unregistered.includes(marker));
    assert.equal(await empty.renderSvg(source, options), unregistered);
    assert.equal(await scoped.renderSvg(source, options), registered);
    counts.svg_renders += 5;
    counts.isolation_checks += 3;

    const architecture = '%%{init: {"architecture": {"numIter": 1, "randomize": false}}}%%\n'
      + "architecture-beta\nservice db(database)[Database]\n";
    const overridden = await scoped.renderSvg(architecture, options);
    assert.match(overridden, new RegExp(`data-icon="${marker}-override"`));
    const builtin = await plain.renderSvg(architecture, options);
    assert.ok(!builtin.includes(marker));
    counts.svg_renders += 2;
    counts.isolation_checks += 1;

    for (const [packs, codeName, kind, limitId] of [
      [[{ json: '{"prefix":"test","icons":{"bad":{"body":"<path>"}}}' }],
        "MERMAN_INVALID_ARGUMENT", "invalid_xml"],
      [[{ json: '{"prefix":"test","icons":{},"icons":{}}' }],
        "MERMAN_INVALID_ARGUMENT", "duplicate_json_key"],
      [[{ json: '{"icons":{}}', registrationName: "a".repeat(65) }],
        "MERMAN_RESOURCE_LIMIT_EXCEEDED", "resource_limit_exceeded", "max_icon_registry_prefix_bytes"],
      [Array.from({ length: 17 }, () => ({ json: '{"prefix":"test","icons":{}}' })),
        "MERMAN_RESOURCE_LIMIT_EXCEEDED", "resource_limit_exceeded", "max_icon_registry_packs"],
    ]) {
      await assert.rejects(module.createNodeEngine({ iconPacks: packs }), (error) => {
        assert.ok(error instanceof module.MermanOperationError);
        assert.equal(error.codeName, codeName);
        assert.equal(error.iconRegistryDetails?.kind_id, kind);
        assert.equal(error.iconRegistryDetails?.pack_index, limitId === "max_icon_registry_packs" ? 16 : 0);
        if (limitId) {
          assert.equal(error.resourceDetails?.limit_id, limitId);
          assert.equal(error.resourceDetails?.profile, "constructor-fixed");
        }
        return true;
      });
      counts.constructor_errors += 1;
    }
    assert.equal(await scoped.renderSvg(source, options), registered);
    counts.svg_renders += 1;
    await scoped.dispose();
    await scoped.dispose();
    assert.throws(() => scoped.renderSvgSync(source, options), module.MermanDisposedError);
    await assert.rejects(scoped.renderSvg(source, options), module.MermanDisposedError);
    assert.equal(await plain.renderSvg(source, options), unregistered);
    counts.svg_renders += 1;
    counts.dispose_checks += 3;
    return counts;
  } finally {
    for (const engine of engines) await engine.dispose();
  }
}
