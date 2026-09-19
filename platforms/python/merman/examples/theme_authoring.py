"""Exercise shared theme JSON through an installed, generated UniFFI consumer."""

from __future__ import annotations

import json
import math
from pathlib import Path
import xml.etree.ElementTree as ET

import merman


FIXTURES = (
    Path(__file__).resolve().parents[4]
    / "crates/merman-theme-authoring-fixtures/fixtures/authoring-v1"
)
SOURCES = {
    "flowchart": "flowchart LR\nA[Alpha] --> B[Beta]\n",
    "state": "stateDiagram-v2\n[*] --> Active\nActive --> [*]\n",
    "sequence": "sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: World\n",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def execute_json(client, operation: str, source: str, options_json: str | None = None):
    result = client.execute(merman.MermanOperationRequestV4(
        operation_id=operation, source=source, uri=None, options_json=options_json, control=None,
    ))
    require(result.operation_id == operation, "operation identity changed")
    require(result.media_type == "application/json", "expected JSON output")
    return json.loads(bytes(result.data))


def fixture_canonical_bytes(value) -> bytes:
    # These frozen vectors contain ASCII keys and integral numbers only. This is an
    # independent oracle for that bounded domain, not a general RFC 8785 implementation.
    def integral_numbers(item):
        if isinstance(item, float):
            require(item.is_integer(), "fixture left the integral-number oracle domain")
            return int(item)
        if isinstance(item, dict):
            require(all(key.isascii() for key in item), "fixture keys must remain ASCII")
            return {key: integral_numbers(value) for key, value in item.items()}
        if isinstance(item, list):
            return [integral_numbers(value) for value in item]
        return item

    return json.dumps(
        integral_numbers(value), sort_keys=True, separators=(",", ":"),
        ensure_ascii=False, allow_nan=False,
    ).encode("utf-8")


def render_definition(client, definition: str, family: str) -> str:
    materialized = execute_json(client, "materialize-theme-json", definition)
    require(
        [materialized[key] for key in (
            "schema_version", "authoring_schema_version", "expansion_version", "spec_schema_version",
        )] == [1, 1, 1, 1],
        "materialization version tuple changed",
    )
    # The intermediate spec is passed directly to the render operation, never persisted.
    return client.render_svg(SOURCES[family], json.dumps({
        "version": 3,
        "theme": {"spec": materialized["spec"]},
        "site_config": {"htmlLabels": False},
        "svg": {"diagram_id": f"python-authoring-{family}"},
    }, allow_nan=False))


def state_fill(svg: str) -> str:
    root = ET.fromstring(svg)
    namespace = "{http://www.w3.org/2000/svg}"
    require(root.tag == namespace + "svg", "expected SVG root")
    states = [element for element in root.iter(namespace + "g")
              if "statediagram-state" in element.get("class", "").split()
              and "".join(element.itertext()).strip() == "Active"]
    require(len(states) == 1, "expected one visible Active state")
    shapes = [element for element in states[0].iter(namespace + "rect")
              if "label-container" in element.get("class", "").split()]
    require(len(shapes) == 1, "expected one Active state shape")
    shape = shapes[0]
    for name in ("width", "height"):
        try:
            value = float(shape.get(name, ""))
        except ValueError:
            raise RuntimeError(f"Active shape needs positive finite {name}") from None
        require(math.isfinite(value) and value > 0,
                f"Active shape needs positive finite {name}")
    # Inspect the writer's terminal declaration; do not recreate stylesheet cascade.
    declarations = dict(part.split(":", 1) for part in shape.get("style", "").split(";")
                        if ":" in part)
    return declarations.get("fill", shape.get("fill", "")).removesuffix("!important").strip()


def run_theme_authoring_smoke() -> None:
    api = merman.Merman()
    engine = merman.MermanEngine(None, None)
    fresh = merman.MermanEngine(None, None)
    try:
        expected_presets = json.loads((FIXTURES / "preset-catalog.json").read_text(encoding="utf-8"))
        for client in (api, engine, fresh):
            catalog = json.loads(client.theme_catalog_json())
            require(catalog["schema_version"] == 1, "unexpected theme catalog schema")
            require(catalog["structured_spec_available"] is True, "theme spec unavailable")
            require(fixture_canonical_bytes(catalog["presets"]) == fixture_canonical_bytes(expected_presets),
                    "preset catalog differs from shared golden")
        error_vectors = json.loads((FIXTURES / "errors.json").read_text(encoding="utf-8"))
        for vector in error_vectors:
            operations = ["materialize-theme-json"]
            if vector["id"] == "encoded-byte-limit":
                operations += ["describe-theme-support-json", "export-theme-preset-json"]
            for operation in operations:
                for client in (api, engine):
                    try:
                        execute_json(client, operation, vector["source"],
                                     vector.get("options_json"))
                    except merman.MermanError.Binding as error:
                        require(error.code_name == vector["code_name"],
                                f"{operation}: wrong outer authoring status")
                        details = json.loads(error.details_json)
                        require(details.get("resource") == vector.get("resource"),
                                f"{operation}: resource rejection differs from shared golden")
                        authoring = details.get("theme_authoring")
                        if authoring is not None:
                            for diagnostic in authoring["diagnostics"]:
                                message = diagnostic.pop("message")
                                require(isinstance(message, str) and bool(message.strip()),
                                        "missing authoring diagnostic message")
                        expected_authoring = (
                            vector["theme_authoring"] if operation == "materialize-theme-json"
                            else None
                        )
                        require(authoring == expected_authoring,
                                f"{operation}/{vector['id']}: authoring envelope differs from shared golden")
                    else:
                        raise RuntimeError(f"{operation}/{vector['id']}: invalid request was accepted")

        definitions = {}
        for name in ("light", "dark"):
            readable = (FIXTURES / f"{name}.definition.json").read_text(encoding="utf-8")
            definition = json.loads(readable)
            expected_definition = (FIXTURES / f"{name}.definition.canonical.json").read_bytes().strip()
            expected_spec = (FIXTURES / f"{name}.spec.canonical.json").read_bytes().strip()
            require(fixture_canonical_bytes(definition) == expected_definition,
                    f"{name} canonical definition differs from the Rust typed vector")
            exported = json.dumps(definition, indent=2, ensure_ascii=False, allow_nan=False)
            require(fixture_canonical_bytes(json.loads(exported)) == expected_definition,
                    f"{name} readable export changed the definition")
            for client in (api, engine, fresh):
                materialized = execute_json(client, "materialize-theme-json", exported)
                require(fixture_canonical_bytes(materialized["spec"]) == expected_spec,
                        f"{name} bound materialization differs from the fixed spec oracle")
            definitions[name] = exported

        for family in SOURCES:
            light = render_definition(engine, definitions["light"], family)
            dark = render_definition(engine, definitions["dark"], family)
            require(light != dark, f"{family} ignored light/dark tokens")
            require(light == render_definition(engine, definitions["light"], family),
                    f"{family} leaked dark state into reused light render")
            require(light == render_definition(fresh, definitions["light"], family),
                    f"{family} disagrees across independent engines")
            require(light == render_definition(api, definitions["light"], family),
                    f"{family} one-shot render disagrees with engine render")
            ET.fromstring(dark)
            if family == "state":
                require(state_fill(light) == "#fef3c7", "light surface missed its terminal")
                require(state_fill(dark) == "#172554", "dark surface missed its terminal")

        edited = json.loads(definitions["light"])
        edited["styles"] = [{
            "kind": "rule", "target": "state", "family": "state",
            "style": {"fill": "#123abc"},
        }]
        require(state_fill(render_definition(engine, json.dumps(edited), "state")) == "#123abc",
                "family-scoped rule failed to override the token-derived terminal fill")
        require(render_definition(engine, json.dumps(edited), "sequence")
                == render_definition(engine, definitions["light"], "sequence"),
                "State rule leaked into Sequence")
        require(state_fill(render_definition(engine, definitions["light"], "state")) == "#fef3c7",
                "State override leaked into the next unmodified theme")

        cold_spec = {"styles": [{
            "kind": "rule", "target": "state", "family": "state",
            "style": {"fill": "#456def"},
        }]}
        cold_engine = merman.MermanEngine(None, None)
        try:
            cold_svg = cold_engine.render_svg(SOURCES["state"], json.dumps({
                "version": 3, "theme": {"spec": cold_spec},
                "site_config": {"htmlLabels": False},
            }))
            require(state_fill(cold_svg) == "#456def",
                    "complete spec must cold-start without a materialized definition")
        finally:
            cold_engine.close()

        for preset in ("brutalist", "spotless", "cyberpunk"):
            exported = execute_json(api, "export-theme-preset-json", preset)
            require(exported["kind"] == "complete_spec", "preset must export a closed recipe")
            require(exported["schema_version"] == 1, "unexpected recipe schema version")
            spec = exported["complete_spec"]
            require("assets" not in spec and "mermaid" not in spec,
                    "native preset export must not embed fonts or Mermaid compatibility")
            options = {"version": 3, "site_config": {"htmlLabels": False},
                       "svg": {"diagram_id": f"python-preset-{preset}"}}
            from_preset = engine.render_svg(SOURCES["state"], json.dumps({
                **options, "theme": {"preset": preset},
            }))
            from_export = fresh.render_svg(SOURCES["state"], json.dumps({
                **options, "theme": json.loads(json.dumps(exported)),
            }))
            require(from_preset == from_export, f"{preset} direct import changed the rendered recipe")
            if preset == "cyberpunk":
                for family in ("flowchart", "sequence", "xychart"):
                    source = (FIXTURES.parents[2] / "merman-theme-fixtures/fixtures/public-cyberpunk"
                              / f"{family}.mmd").read_text(encoding="utf-8")
                    scene_options = {**options, "svg": {"diagram_id": f"python-cyberpunk-{family}"}}
                    original = engine.render_svg(source, json.dumps({
                        **scene_options, "theme": {"preset": preset},
                    }))
                    restored = fresh.render_svg(source, json.dumps({
                        **scene_options, "theme": json.loads(json.dumps(exported)),
                    }))
                    ET.fromstring(restored)
                    require(original == restored, f"{family}: complete recipe import changed SVG")
                unversioned = {key: value for key, value in exported.items() if key != "schema_version"}
                for invalid in (
                    unversioned, {**exported, "schema_version": 0}, {**exported, "schema_version": 2},
                    {**exported, "preset": preset}, {**exported, "spec": {}},
                ):
                    try:
                        fresh.render_svg(SOURCES["state"], json.dumps({**options, "theme": invalid}))
                    except merman.MermanError.Binding as error:
                        require(error.code_name == "MERMAN_OPTIONS_JSON_ERROR",
                                "invalid recipe returned the wrong error")
                    else:
                        raise RuntimeError("invalid recipe schema or mixed selection was accepted")

        support_vectors = json.loads((FIXTURES / "support.json").read_text(encoding="utf-8"))
        for vector in support_vectors:
            for client in (api, engine):
                support = execute_json(client, "describe-theme-support-json",
                                       json.dumps(vector["query"]))
                require(support == vector["expected"],
                        f"{vector['id']} support descriptor differs from shared golden")
        budgeted_inputs = {
            "materialize-theme-json": definitions["light"],
            "describe-theme-support-json": json.dumps(support_vectors[0]["query"]),
            "export-theme-preset-json": "editor-light",
        }
        for client in (api, engine):
            for operation, source in budgeted_inputs.items():
                baseline = execute_json(client, operation, source)
                # Export also admits the expanded recipe, so cover its encoded input too.
                encoded_budget = len((
                    json.dumps(baseline, allow_nan=False)
                    if operation == "export-theme-preset-json" else source
                ).encode("utf-8"))
                bounded = execute_json(client, operation, source, json.dumps({
                    "resources": {
                        "profile": "constrained",
                        "limits": {"max_theme_encoded_bytes": encoded_budget},
                    },
                }))
                require(bounded == baseline,
                        f"{operation}: valid theme budget changed the result")
        print("Python theme authoring passed: shared vectors, three families, isolation, "
              "rule, cold start, 3 direct preset imports, 3 complete Cyberpunk scenes, "
              "5 recipe rejections, catalog, "
              f"{len(support_vectors)} support vectors, "
              f"{len(budgeted_inputs)} budgeted operations per consumer")
    finally:
        fresh.close()
        engine.close()


if __name__ == "__main__":
    run_theme_authoring_smoke()
