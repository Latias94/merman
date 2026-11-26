"""Freeze ordered inputs, observe public Rust calls, and compare matching observations."""

import argparse
import hashlib
from itertools import product
import json
import os
from pathlib import Path
import platform
import re
from statistics import median
import subprocess
import sys
from xml.etree import ElementTree as ET

from .corpus import load_corpus, select_corpus_fixtures
from .drawing import drawing_case, observe_drawings
from .reference import LAYOUTS
from .svg import EXPECTED_OUTCOMES, valid_expected_labels, validate_rejection, validate_svg
from .topology import SCC_SHAPES, flowchart_layout_inventory, flowchart_svg_inventory, topology_source


ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parents[1]
FAMILY_MANIFEST = ROOT / "playground/examples/manifest.json"
WORKLOADS = HERE / "workloads.json"
LAYOUT_EVIDENCE_DAGRE = ("flowchart", "agentflow", "class", "state", "requirement", "usecase")
LAYOUT_EVIDENCE_ENGINES = (*LAYOUT_EVIDENCE_DAGRE, "architecture", "block", "c4", "er", "swimlane")
OPERATIONS = (
    "svg-data",
    "svg",
    "layout-json",
    "prepare-drop",
    "prepare-retained",
    "static-svg",
    "resvg-svg",
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    return json.loads(path.read_bytes())


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(data, ensure_ascii=False, indent=2, allow_nan=False) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def parse_workload_json(data, context="workload"):
    """Reject duplicate object keys while decoding a workload manifest or plan."""
    def unique_fields(pairs):
        fields = {}
        for key, value in pairs:
            if key in fields:
                raise ValueError(f"duplicate JSON key {key!r} in {context}")
            fields[key] = value
        return fields

    return json.loads(data, object_pairs_hook=unique_fields)


def validate_drawing_facts(facts: object, context: str = "request") -> dict:
    """Validate the observable drawing facts declared for one request."""
    field = f"{context}.drawing_facts"
    if not isinstance(facts, dict):
        raise ValueError(f"{field} must be an object")
    unknown = [key for key in facts if key not in {"arrows", "groups", "label_widths", "sequence_messages"}]
    if unknown:
        raise ValueError(f"{field} has unknown fields: {unknown}")
    arrows, groups = facts.get("arrows", []), facts.get("groups", {})
    widths = facts.get("label_widths", {})
    messages = facts.get("sequence_messages", {})
    if not isinstance(messages, dict):
        raise ValueError(f"{field}.sequence_messages must map message text to counts")
    if (
        not isinstance(arrows, list) or not isinstance(groups, dict) or not isinstance(widths, dict)
        or not (arrows or groups or widths or messages)
    ):
        raise ValueError(f"{field} needs at least one fact in arrows, groups, label_widths or sequence_messages")

    for text, count in messages.items():
        if not isinstance(text, str) or not text.strip():
            raise ValueError(f"{field}.sequence_messages requires nonblank message text")
        if type(count) is not int or not 0 < count < 2**53:
            raise ValueError(f"{field}.sequence_messages count for {text!r} must be a positive JS safe integer")

    def valid_id(value):
        return isinstance(value, str) and bool(value) and value == value.strip()

    for node, label in widths.items():
        if not valid_id(node) or node in groups or not valid_id(label) or len(label.splitlines()) != 1:
            raise ValueError(f"{field}.label_widths needs a node ID and nonblank single-line source label")

    edges = set()
    for arrow in arrows:
        if not isinstance(arrow, dict) or arrow.keys() != {"edge", "from", "to"}:
            raise ValueError(f"{field}.arrows entries require exactly edge, from and to")
        if not all(valid_id(value) for value in arrow.values()):
            raise ValueError(f"{field}.arrows needs nonblank IDs without surrounding whitespace")
        edge, start, end = arrow["edge"], arrow["from"], arrow["to"]
        if edge in edges:
            raise ValueError(f"{field}.arrows repeats edge {edge!r}")
        if start == end or start in groups or end in groups:
            raise ValueError(f"{field}.arrows edge {edge!r} needs two distinct node endpoints")
        edges.add(edge)

    members = set()
    for group, nodes in groups.items():
        if not valid_id(group) or not isinstance(nodes, list) or not nodes:
            raise ValueError(f"{field}.groups needs a group ID and a nonempty node list per group")
        for node in nodes:
            if not valid_id(node) or node in groups:
                raise ValueError(f"{field}.groups expects a node ID in {group!r}, got {node!r}")
            if node in members:
                raise ValueError(f"{field}.groups repeats node {node!r} in {group!r}")
            members.add(node)
    return facts


def _fixture_expectations(manifest, key, names, context):
    values = manifest.get(key, {})
    if not isinstance(values, dict):
        raise ValueError(f"{key} in {context} must be an object keyed by fixture name")
    unknown = values.keys() - set(names)
    if unknown:
        raise ValueError(f"{key} must refer to files in {context}, unknown names: {sorted(unknown)}")
    return values


def scenario(name, origin, sources, labels=None, *, drawing_facts=None, outcomes=None, allow_unlabeled=False, diagram_id=None):
    if not re.fullmatch(r"[a-zA-Z0-9_-]+", name):
        raise ValueError(f"invalid scenario ID: {name!r}")
    if diagram_id is not None and (not isinstance(diagram_id, str) or not diagram_id):
        raise ValueError(f"invalid fixed diagram_id in {name!r}")
    requests = []
    for i, (fixture, text) in enumerate(sources):
        if not text.strip():
            raise ValueError(f"empty source in {name}: {fixture}")
        request = {
            "fixture": fixture,
            "source": text,
            "source_sha256": digest(text.encode()),
            "diagram_id": diagram_id or f"wb-{name}-{i + 1}",
        }
        if labels is not None and (fixture in labels or not allow_unlabeled):
            expected = labels.get(fixture)
            if not valid_expected_labels(expected):
                raise ValueError(f"review expected labels for {name}/{fixture}")
            request["expected_labels"] = expected
        if drawing_facts is not None and fixture in drawing_facts:
            request["drawing_facts"] = validate_drawing_facts(
                drawing_facts[fixture], f"{name}/{fixture}"
            )
        reviewed = "expected_labels" in request or "drawing_facts" in request
        outcome = (outcomes or {}).get(fixture, "unreviewed" if allow_unlabeled and not reviewed else "draw")
        if outcome not in EXPECTED_OUTCOMES:
            raise ValueError(f"invalid expected_outcome for {name}/{fixture}: {outcome!r}")
        if outcome in ("reject", "allow-empty") and (request.get("expected_labels") or "drawing_facts" in request):
            raise ValueError(f"{name}/{fixture}: drawing facts or required labels conflict with {outcome}")
        request["expected_outcome"] = outcome
        requests.append(request)
    if not requests:
        raise ValueError(f"empty scenario: {name}")
    return {"id": name, "origin": origin, "requests": requests}


def four_node_control(name):
    """Hold the public diagram ID, source length and graph size fixed over eight requests."""
    edge_sets = ("AB BC CD", "AB AC AD", "AB BC BD", "AB AC CD",
                 "AB CB BD", "AB CB DB", "AB CB CD", "AB DC CA")
    sources, labels, facts = [], {}, {}
    for index in range(8):
        suffix = str(index if name == "four-node-relabel" else 0)
        edges = edge_sets[index if name == "four-node-edges" else 0].split()
        names = {node: label + suffix for node, label in zip("ABCD", ("Plan", "Edit", "Test", "Ship"), strict=True)}
        lines = ["flowchart TD", *(f"{node}{suffix}[{label}]" for node, label in names.items()),
                 *(f"{a}{suffix} --> {b}{suffix}" for a, b in edges)]
        fixture = f"{name}-{index + 1}"
        sources.append((fixture, "\n".join(lines) + "\n"))
        labels[fixture] = list(names.values())
        facts[fixture] = {"label_widths": {node + suffix: label for node, label in names.items()},
                          "arrows": [{"edge": f"L_A{suffix}_B{suffix}_0", "from": f"A{suffix}", "to": f"B{suffix}"}]}
    return scenario(name, "merman/four-node-controls", sources, labels, drawing_facts=facts, diagram_id="four-node-preview")


def document_scenario(manifest):
    doc = parse_workload_json(manifest.read_bytes(), manifest)
    if not isinstance(doc, dict):
        raise ValueError(f"expected a document object in {manifest}")
    unknown = doc.keys() - {"id", "origin", "files", "expected_labels", "drawing_facts", "expected_outcomes", "diagram_id"}
    if unknown:
        raise ValueError(f"unknown document fields in {manifest}: {sorted(unknown)}")
    files = doc["files"]
    if not isinstance(files, list) or not all(isinstance(f, str) and f.strip() for f in files):
        raise ValueError(f"files must be a list of filenames in {manifest}")
    return scenario(
        doc["id"], doc["origin"], [(f, (manifest.parent / f).read_text(encoding="utf-8")) for f in files],
        _fixture_expectations(doc, "expected_labels", files, manifest),
        drawing_facts=_fixture_expectations(doc, "drawing_facts", files, manifest),
        outcomes=_fixture_expectations(doc, "expected_outcomes", files, manifest), allow_unlabeled=True,
        diagram_id=doc.get("diagram_id"),
    )


def family_scenarios(reviews):
    """Project the playground's reviewed baselines and attach their public SVG label facts."""
    manifest = parse_workload_json(FAMILY_MANIFEST.read_bytes(), FAMILY_MANIFEST)
    examples = manifest["examples"]
    rows = [row for row in examples if row["evidence"]["role"] == "family-baseline"]
    if not rows:
        raise ValueError("playground catalog has no family-baseline examples")
    for field in ("diagramType", "id", "order", "fixture"):
        seen = set()
        for row in rows:
            if row[field] in seen:
                raise ValueError(f"duplicate family-baseline {field}: {row[field]!r}")
            seen.add(row[field])
    missing = {row["diagramType"] for row in examples} - {row["diagramType"] for row in rows}
    if missing:
        raise ValueError(f"missing family-baseline for {sorted(missing)}")

    labels, reasons = reviews["expected_labels"], reviews["empty_label_reasons"]
    ids = {row["id"] for row in rows}
    if ids != labels.keys():
        raise ValueError(
            f"family label catalog drift: missing {sorted(ids - labels.keys())}, "
            f"stale {sorted(labels.keys() - ids)}"
        )
    if reasons.keys() != {key for key, values in labels.items() if values == []}:
        raise ValueError("empty_label_reasons must match the family examples with empty expected_labels")

    cases = []
    for row in sorted(rows, key=lambda row: row["order"]):
        example, family, origin = row["id"], row["diagramType"], row["fixture"]
        path = Path(origin)
        if not path.is_relative_to("fixtures") or ".." in path.parts or path.suffix != ".mmd":
            raise ValueError(f"invalid family-baseline fixture path for {example}: {origin!r}")
        source = (ROOT / path).read_text(encoding="utf-8")
        case = scenario(f"family-{family}", origin, [(example, source)], labels)
        expected = labels[example]
        distinct = {frozenset(fact if isinstance(fact, list) else [fact]) for fact in expected}
        if len(expected) != len(distinct):
            raise ValueError(f"duplicate expected label in family example {example}")
        if not expected:
            reason = reasons[example]
            if not isinstance(reason, str) or not reason.strip():
                raise ValueError(f"missing reason for empty expected_labels in {example}")
            case["requests"][0]["empty_label_reason"] = reason
        case.update(family=family, order=row["order"])
        cases.append(case)
    return cases


def materialize(args):
    catalog = parse_workload_json(WORKLOADS.read_bytes(), WORKLOADS)
    corpus = load_corpus(HERE / "corpus.json")
    registered = {f.name: ROOT / f.source for f in corpus.fixtures}
    for name, path in catalog["fixture_files"].items():
        if name in registered:
            raise ValueError(f"workload file shadows a registered corpus fixture: {name}")
        registered[name] = ROOT / path
    inputs = dict(catalog["inputs"])
    for variant in ("repeated", "unique"):
        messages = (f"A->>B: event-{i if variant == 'unique' else 0:03d}\n" for i in range(100))
        inputs[f"sequence-100-{variant}"] = "sequenceDiagram\n" + "".join(messages)
    if inputs.keys() & registered.keys():
        raise ValueError("workload input shadows a fixture file")
    fixture_facts = _fixture_expectations(
        catalog, "drawing_facts", inputs.keys() | registered.keys(), "workloads.json"
    )
    fixture_outcomes = _fixture_expectations(
        catalog, "expected_outcomes", inputs.keys() | registered.keys(), "workloads.json"
    )
    cases = []
    builtin_documents = set()
    for recipe in catalog["scenarios"]:
        included = args.suite == "full" or (args.suite == "quick" and recipe.get("quick"))
        if not included and recipe["id"] not in args.scenario:
            continue
        if "manifest" in recipe:
            manifest = (HERE / recipe["manifest"]).resolve()
            case = document_scenario(manifest)
            if case["id"] != recipe["id"]:
                raise ValueError(f"document manifest ID differs from {recipe['id']!r}")
            cases.append(case)
            builtin_documents.add(manifest)
            continue
        if "document" in recipe:
            path = recipe["document"]
            text = (ROOT / path).read_text(encoding="utf-8")
            # Each document owns its block names in reading order.
            blocks = re.findall(
                r"^```mermaid\n(.*?)^```[ \t]*$", text, flags=re.MULTILINE | re.DOTALL
            )
            labels = recipe["expected_labels"]
            if len(blocks) != len(labels):
                raise ValueError(f"review the changed fence population in {path}")
            sources = [(f"block-{i}", block) for i, block in enumerate(blocks, 1)]
            names = [f for f, _ in sources]
            cases.append(scenario(recipe["id"], path, sources, labels,
                                  drawing_facts=_fixture_expectations(recipe, "drawing_facts", names, path),
                                  outcomes=_fixture_expectations(recipe, "expected_outcomes", names, path)))
            continue
        sequence = recipe["fixtures"] * recipe.get("repeat", 1)
        for fixture in sequence:
            if fixture not in inputs:
                inputs[fixture] = registered[fixture].read_text(encoding="utf-8")
        cases.append(
            scenario(
                recipe["id"],
                recipe.get("origin", "merman"),
                [(f, inputs[f]) for f in sequence],
                catalog["expected_labels"],
                drawing_facts=fixture_facts, outcomes=fixture_outcomes, diagram_id=recipe.get("diagram_id"),
            )
        )

    if args.suite in ("full", "families") or any(name.startswith("family-") for name in args.scenario):
        cases.extend(family_scenarios(catalog["family_labels"]))

    for suite in corpus.suites:
        name = f"corpus-{suite}"
        if name in args.scenario:
            fixtures = select_corpus_fixtures(corpus, suite)
            sources = [(f.name, registered[f.name].read_text(encoding="utf-8")) for f in fixtures]
            cases.append(
                scenario(
                    name, f"tools/bench/corpus.json#{suite}", sources, catalog["expected_labels"],
                    drawing_facts=fixture_facts, outcomes=fixture_outcomes, allow_unlabeled=True,
                )
            )

    cases.extend(four_node_control(name) for name in ("four-node-repeat", "four-node-relabel", "four-node-edges")
                 if name in args.scenario)
    cases.extend(document_scenario(m) for m in args.document if m.resolve() not in builtin_documents)
    if args.pele_fixtures:
        upstream = load(args.pele_fixtures / "manifest.json")
        for entry in upstream["diagrams"]:
            name = entry["id"].replace("/", "-")
            cases.append(
                scenario(
                    name,
                    upstream["source"],
                    [(entry["id"], (args.pele_fixtures / entry["file"]).read_text(encoding="utf-8"))],
                    allow_unlabeled=True,
                )
            )

    topologies = []
    stage_scenarios = []
    topology_suite = args.suite
    if args.suite == "families" or (args.scenario and args.suite == "quick"):
        topology_suite = "full"
    for series in catalog["topologies"][topology_suite]:
        for shape, nodes, order in product(series["shapes"], series["nodes"], series.get("orders", ["canonical"])):
            canonical = f"{shape}-{nodes}"
            name = canonical if order == "canonical" else f"{canonical}-{order}"
            if (args.scenario or args.suite == "families") and name not in args.scenario:
                continue
            source, edges = topology_source(shape, nodes, order)
            topologies.append(
                {
                    "id": name,
                    "nodes": nodes,
                    "edges": edges,
                    "source": source,
                    "source_sha256": digest(source.encode()),
                    **({"canonical_id": canonical, "statement_order": order} if name != canonical else {}),
                }
            )
            render = name in args.scenario or nodes in series.get("render_nodes", [])
            stage_only = nodes in series.get("stage_render_nodes", [])
            if render or stage_only:
                labels = [f"C{i}" for i in range(nodes)] if shape in SCC_SHAPES else ["N0", f"N{nodes - 1}"]
                population = cases if render else stage_scenarios
                population.append(scenario(name, "merman/topology", [(name, source)], {name: labels}))
    names = [s["id"] for s in (*cases, *stage_scenarios)]
    if len(names) != len(set(names)):
        raise ValueError("duplicate scenario ID in workload sources")
    if args.scenario:
        by_id = {s["id"]: s for s in cases}
        cases = [by_id[name] for name in args.scenario]
    return {"schema_version": 1, "suite": args.suite, "scenarios": cases, "topologies": topologies,
            **({"stage_scenarios": stage_scenarios} if stage_scenarios else {})}


def build_runner(metric, *, shoal=False):
    name = "memory" if metric in ("memory", "stage-memory", "heap-profile") else "workloads"
    features = ["benchmark-stages"] if metric in ("stages", "stage-memory") else []
    if metric == "heap-profile":
        features.append("dhat-heap")
    if shoal:
        features.append("layout-shoal")
    # Path dependencies have the same crate names and versions in two revision worktrees.
    # Keep a reusable build slot per checkout even when the caller normally shares targets.
    parent = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    target = (ROOT / parent).resolve() / "public-workloads" / digest(str(ROOT).encode())[:12]
    cmd = [
        "cargo",
        "build",
        "--locked",
        "--release",
        "--target-dir",
        str(target),
        "-p",
        "merman",
        "--bench",
        name,
        *(["--features", ",".join(features)] if features else []),
        "--message-format=json-render-diagnostics",
    ]
    result = subprocess.run(cmd, cwd=ROOT, stdout=subprocess.PIPE, check=True)
    artifacts = [json.loads(line) for line in result.stdout.splitlines()]
    paths = [
        a["executable"]
        for a in artifacts
        if a.get("reason") == "compiler-artifact" and a["target"]["name"] == name and a["executable"]
    ]
    if len(paths) != 1:
        raise ValueError(f"expected one executable for {name}: {paths}")
    return Path(paths[0])


def child(runner, job):
    command = [runner, "--workload-stdin"]
    if job.get("metric") == "stages":
        command.append("--stages")
        if job.get("stage_trace"):
            command.append("--stage-trace")
    elif job.get("metric") == "stage-memory":
        command.append("--stage-memory")
    request = dict(job)
    profile = request.pop("heap_profile", None)
    if profile and not request.get("verify_only"):
        command.extend(("--heap-profile", profile))
    result = subprocess.run(
        command, input=json.dumps(request).encode(), stdout=subprocess.PIPE, check=True
    )
    return json.loads(result.stdout)


SVG_OPERATIONS = {"svg-data", "svg", "static-svg", "resvg-svg"}


def rust_signatures(outputs, requests, operation):
    if not isinstance(outputs, list) or len(outputs) != len(requests):
        raise ValueError("incomplete Rust verification outputs")
    svg = operation in SVG_OPERATIONS
    signatures = []
    frozen = False
    for output, request in zip(outputs, requests):
        outcome = request.get("expected_outcome", "draw")
        if isinstance(output, dict) and isinstance(output.get("render_error"), str):
            message = output["render_error"]
            verdict = validate_rejection(None, message) if outcome == "reject" else {"status": "render-error"}
            signatures.append({**verdict, "message": message})
        elif svg and isinstance(output, str):
            verdict = validate_svg(output, request.get("expected_labels", []), outcome)
            data = output.encode("utf-8")
            signature = dict(bytes=len(data), sha256=digest(data), observed_size=len(data))
            if verdict["status"] != "ok":
                signature.update(verdict)
            elif "output_class" in verdict:
                signature["output_class"] = verdict["output_class"]
            signatures.append(signature)
        else:
            if (not isinstance(output, dict) or type(output.get("bytes")) is not int or output["bytes"] <= 0
                    or type(output.get("observed_size")) is not int or output["observed_size"] <= 0
                    or not re.fullmatch(r"[0-9a-f]{64}", str(output.get("sha256")))):
                raise ValueError("invalid Rust verification identity")
            if svg:  # Frozen runners only return identities from their embedded reader.
                if outcome not in ("draw", "unreviewed") or any(isinstance(label, list) for label in request.get("expected_labels", [])):
                    raise ValueError("source outcomes and alternative label spellings require a Rust runner that returns the SVG")
                if output["observed_size"] != output["bytes"]:
                    raise ValueError("invalid verification identity from the frozen Rust runner")
                frozen = True
            signatures.append({"status": "unexpected-success"} if outcome == "reject" else output)
    return signatures, svg and not frozen


def measure_rust_samples(observe, job, signatures, count, exact):
    sizes = [s["observed_size"] for s in signatures]
    samples, measured_hashes = [], []
    for sample_index in range(count):
        sample = observe({**job, "return_measured_svg": True} if exact else job)
        error = None
        if exact:
            outputs = sample.get("outputs")
            if not isinstance(outputs, list) or len(outputs) != 1 or not isinstance(outputs[0], str):
                error = "exact timing requires the SVG from the actual measured call"
            else:
                actual, _ = rust_signatures(outputs, job["requests"], job["operation"])
                measured_hashes.append(actual[0]["sha256"])
                if actual != signatures:
                    error = f"measured SVG differs from verified output: {actual[0].get('message', 'digest mismatch')}"
        elif sample["sizes"] != sizes:
            error = f"unstable output size: measured {sample['sizes']}, verified {sizes}"
        if error:
            return dict(status="measurement-mismatch", error=error, completed=0, samples=[], failed_sample_index=sample_index,
                        **({"measured_svg_sha256": measured_hashes} if exact else {}))
        samples.append(sample["metrics"])
    return dict(samples=samples, measured_output_check="returned-svg" if exact else "size-only",
                **({"measured_svg_sha256": measured_hashes} if exact else {}))


def check_layout_evidence(response, job, signatures, requested, *, inventory, stages=False):
    receipts = response.get("layout_evidence")
    if not isinstance(receipts, list) or len(receipts) != len(signatures):
        raise ValueError("layout verification requires a runner that returns per-request layout evidence")
    for receipt, request, output, signature in zip(receipts, job["requests"], response["outputs"], signatures, strict=True):
        if isinstance(output, dict) and "render_error" in output and receipt is None:
            continue
        diagram_id = request["diagram_id"]
        if (not isinstance(receipt, dict) or receipt.get("diagram_id") != diagram_id
                or receipt.get("source_sha256") != request["source_sha256"]
                or not isinstance(receipt.get("family"), str)
                or not isinstance(receipt.get("required_capabilities"), list)
                or not all(isinstance(c, str) for c in receipt["required_capabilities"])):
            raise ValueError(f"missing or misidentified layout evidence for {diagram_id}")
        family = receipt["family"]
        required = [c.removeprefix("layout-") for c in receipt["required_capabilities"] if c.startswith("layout-")]
        if stages:
            actual = receipt.get("backend")
            fixed = receipt.get("fixed_backend")
            if (not isinstance(actual, str) or "fixed_backend" not in receipt
                    or fixed not in (None, "native", "swimlane", "cytoscape")
                    or (fixed is not None and requested is None)):
                raise ValueError(f"missing or invalid stage layout witness for {diagram_id}")
            capability = actual.partition(".")[0]
            if capability in ("cytoscape", *LAYOUTS):
                expected = [capability]
            elif actual in ("native", "swimlane", "tidy-tree"):
                expected = []
            else:
                raise ValueError(f"invalid stage layout backend for {diagram_id}")
            if required != expected:
                raise ValueError(f"inconsistent stage layout witness for {diagram_id}")
            receipt["layout_applicability"] = ("default" if requested is None else
                                              "family-fixed" if fixed is not None else "selectable")
            expected_backend = fixed if fixed is not None else requested
            if expected_backend is not None and actual != expected_backend:
                signature.update(status="layout-backend-mismatch",
                                 message=f"requested {requested}, expected {expected_backend}, observed {actual} ({family})")
        else:
            actual = None
            if len(required) == 1 and family in LAYOUT_EVIDENCE_ENGINES:
                actual = required[0]
            elif not required and family in LAYOUT_EVIDENCE_DAGRE:
                actual = "dagre"
            receipt["backend"] = actual
            if actual is None or actual != requested:
                observed = actual or "unconfirmed"
                if family == "er" and not required:
                    observed = "unconfirmed between Dagre and Swimlane"
                signature.update(status="layout-backend-mismatch",
                                 message=f"requested {requested}, observed {observed} ({family})")
        if "status" in signature:
            continue
        if job["operation"] == "layout-json":
            product = output.get("layout_json")
            if not isinstance(product, dict):
                raise ValueError("layout verification requires layout JSON from the runner")
            if product.get("family_id") != family:
                raise ValueError(f"returned layout JSON family differs from layout evidence for {diagram_id}")
            read_inventory = flowchart_layout_inventory
        else:
            if not isinstance(output, str):
                raise ValueError("layout verification requires SVG bytes from the runner")
            product = ET.fromstring(output)
            if product.get("id") != diagram_id:
                raise ValueError(f"returned SVG ID differs from frozen ID {diagram_id!r}")
            read_inventory = flowchart_svg_inventory
        if not inventory:
            continue
        if family not in ("flowchart", "agentflow"):
            signature.update(status="unsupported-inventory", message=f"no cross-backend inventory for {family}")
            continue
        try:
            receipt["inventory"] = read_inventory(product)
        except ValueError as error:
            signature.update(status="invalid-inventory", message=str(error))
    return receipts


def write_chrome_trace(path, report):
    events = []
    for process, (scenario, row) in enumerate(report["rows"].items(), 1):
        events.append({"name": "process_name", "ph": "M", "pid": process, "tid": 0,
                       "args": {"name": scenario}})
        for thread, sample in enumerate(row.get("samples", []), 1):
            events.append({"name": "end_to_end", "cat": "merman", "ph": "X", "pid": process,
                           "tid": thread, "ts": 0, "dur": sample["elapsed_ns"] / 1000})
            for interval in sample.pop("trace", []):
                events.append({"name": interval["name"], "cat": "merman", "ph": "X",
                               "pid": process, "tid": thread, "ts": interval["offset_ns"] / 1000,
                               "dur": interval["duration_ns"] / 1000})
    write(path, {"traceEvents": events})


STAGE_PHASES = {
    "preprocess", "parse", "theme_prepare", "measure", "layout_prepare", "layout_solve", "svg_emit",
    "css_emit", "postprocess",
}
STAGE_MEMORY_COUNTERS = {"allocations", "allocated_bytes", "peak_growth_bytes"}


def summarize_stage_layout(receipts, requested):
    backends = sorted({r["backend"] for r in receipts if r is not None})
    applicability = {r["layout_applicability"] for r in receipts if r is not None}
    return {
        "requested": requested,
        "backends": backends,
        "applicability": "unobserved" if not applicability else (
            next(iter(applicability)) if len(applicability) == 1 else "mixed"
        ),
        "cross_backend_eligible": (requested is not None and backends == [requested]
                                   and applicability == {"selectable"} and None not in receipts),
    }


def validate_stage_timing(sample):
    stages = sample.get("stages") if isinstance(sample, dict) else None
    if not isinstance(stages, dict) or stages.keys() != STAGE_PHASES | {"end_to_end"}:
        raise ValueError("stage timing runner did not return all phases")
    if any(not isinstance(row, dict) or row.keys() != {"elapsed_ns", "calls"}
           or any(type(value) is not int or value < 0 for value in row.values()) for row in stages.values()):
        raise ValueError("stage timing runner returned invalid durations or call counts")
    elapsed = sample.get("elapsed_ns")
    unattributed = sample.get("unattributed_ns")
    if (type(elapsed) is not int or type(unattributed) is not int or unattributed < 0
            or stages["end_to_end"] != {"elapsed_ns": elapsed, "calls": 1}
            or sum(stages[phase]["elapsed_ns"] for phase in STAGE_PHASES) + unattributed != elapsed):
        raise ValueError("stage durations and unattributed time do not match end-to-end time")


def validate_stage_memory(sample):
    stages = sample.get("stages") if isinstance(sample, dict) else None
    if not isinstance(stages, dict) or stages.keys() != STAGE_PHASES | {"unattributed", "end_to_end"}:
        raise ValueError("stage memory runner did not return all allocation phases")
    if any(not isinstance(row, dict) or row.keys() != STAGE_MEMORY_COUNTERS
           or any(type(value) is not int or value < 0 for value in row.values()) for row in stages.values()):
        raise ValueError("stage memory runner returned invalid allocation counters")
    total = stages["end_to_end"]
    if any(sample.get(key) != total[key] for key in STAGE_MEMORY_COUNTERS):
        raise ValueError("stage memory result differs from the end-to-end counters")
    for key in ("allocations", "allocated_bytes"):
        if sum(row[key] for stage, row in stages.items() if stage != "end_to_end") != total[key]:
            raise ValueError(f"stage memory {key} do not sum to the end-to-end counter")


def run_rust(args, *, raw=None, runner=None, memory_runner=None, layout_comparison=False):
    if raw is None:
        raw = args.plan.read_bytes()
    plan = json.loads(raw)
    if plan["schema_version"] != 1:
        raise ValueError("unsupported workload plan")
    work = args.metric == "work"
    exact = args.verify_measured_svg
    shoal = args.site_layout == "shoal" or layout_comparison
    stages = args.metric == "stages"
    heap_profile = getattr(args, "heap_profile", None)
    if heap_profile and (args.metric != "memory" or args.samples != 1):
        raise ValueError("--heap-profile requires --metric memory and --samples 1")
    trace_path = getattr(args, "chrome_trace", None)
    with_memory = getattr(args, "with_memory", False)
    frozen_memory = getattr(args, "memory_runner", None)
    if with_memory and not stages:
        raise ValueError("--with-memory requires --metric stages")
    if (memory_runner is not None or frozen_memory is not None) and not with_memory:
        raise ValueError("a stage memory runner requires --with-memory")
    if stages and args.operation not in SVG_OPERATIONS:
        raise ValueError("stage attribution requires an SVG operation")
    verify_layout = shoal or stages
    resource_profile = getattr(args, "resource_profile", None)
    if exact and (args.metric != "clock" or args.operation not in SVG_OPERATIONS):
        raise ValueError("--verify-measured-svg requires an SVG operation and --metric clock")
    if args.check_drawings and (work or args.operation not in SVG_OPERATIONS):
        raise ValueError("--check-drawings requires an SVG operation")
    if work and (args.site_layout or args.site_css_bytes or resource_profile or args.operation != "svg-data"):
        raise ValueError(
            "work probes default ELK without resource, site or operation overrides; use clock or memory for explicit layouts"
        )
    if verify_layout and args.operation not in SVG_OPERATIONS | {"layout-json"}:
        raise ValueError("layout verification requires an SVG or layout-json operation")
    site = {}
    if args.site_css_bytes:
        rule = ".r{opacity:0.8;}"
        count, pad = divmod(args.site_css_bytes, len(rule))
        site["themeCSS"] = rule * count + " " * pad
    if args.site_layout:
        site["layout"] = args.site_layout
    selected = plan["topologies" if work else "scenarios"]
    if stages:
        selected = [*selected, *plan.get("stage_scenarios", [])]
    by_id = {r["id"]: r for r in selected}
    if not selected or len(by_id) != len(selected):
        raise ValueError("empty selection or duplicate workload ID")
    if args.scenario:
        selected = [by_id[name] for name in args.scenario]
    if (exact or layout_comparison) and any(len(row["requests"]) != 1 for row in selected):
        raise ValueError("--verify-measured-svg requires one request in each selected scenario")
    if heap_profile:
        if len(selected) != 1 or heap_profile.exists():
            raise ValueError("choose one scenario and a new heap profile path")
        heap_profile = heap_profile.resolve()
        heap_profile.parent.mkdir(parents=True, exist_ok=True)
    if runner is None:
        metric = "heap-profile" if heap_profile else args.metric
        runner = args.runner.resolve() if args.runner else build_runner(metric, shoal=shoal)
    if with_memory and memory_runner is None:
        memory_runner = frozen_memory.resolve() if frozen_memory else build_runner("stage-memory", shoal=shoal)
    if with_memory and memory_runner.resolve() == runner.resolve():
        raise ValueError("stage timing and allocation collection require separate executables")
    options = {
        "metric": args.metric,
        "operation": "layout-work" if work else args.operation,
        "warmups": 0 if work else args.warmups,
        "site_css_bytes": args.site_css_bytes,
        "site_layout": args.site_layout,
        "samples": 1 if work else args.samples,
        **({"resource_profile": resource_profile} if resource_profile else {}),
        **({"clock_boundary": "returned-svg"} if exact else {}),
        **({"stage_trace": True} if trace_path is not None else {}),
        **({"with_memory": True} if with_memory else {}),
        **({"heap_profile": True} if heap_profile else {}),
    }
    rows = {}
    pending_drawings = []
    fingerprints = set()
    validation_modes = set()
    validator_sha256 = digest(Path(__file__).with_name("svg.py").read_bytes())

    def observe(job):
        executable = memory_runner if job.get("metric") == "stage-memory" else runner
        row = child(executable, job)
        if resource_profile and row.get("resource_profile") != resource_profile:
            raise ValueError(f"workload runner did not confirm resource profile {resource_profile!r}")
        fingerprints.add(row.pop("harness_sha256"))
        if len(fingerprints) != 1:
            raise ValueError("workload runner harness changed or differs between observation executables")
        return row

    def verify(job, row=None):
        response = observe({**job, "verify_only": True, **({"verify_layout": True} if verify_layout else {})})
        outputs = response["outputs"]
        signatures, shared = rust_signatures(outputs, job["requests"], job["operation"])
        evidence = None
        if verify_layout:
            evidence = check_layout_evidence(response, job, signatures, args.site_layout,
                                             inventory=layout_comparison, stages=stages)
            if job["operation"] == "layout-json":
                signatures = [{k: v for k, v in signature.items() if k != "layout_json"} for signature in signatures]
        validation_modes.add(shared)
        if len(validation_modes) > 1:
            raise ValueError("Rust verification protocol changed during observation")
        if row is not None and job["operation"] in SVG_OPERATIONS:
            for i, (request, output, signature) in enumerate(zip(job["requests"], outputs, signatures, strict=True)):
                if "drawing_facts" not in request:
                    continue
                facts = validate_drawing_facts(request["drawing_facts"], request["diagram_id"])
                check = drawing_case(request, i)
                check["svg_sha256"] = signature.get("sha256")
                row.setdefault("drawing_checks", []).append(check)
                if not args.check_drawings:
                    check["status"] = "not-checked"
                elif isinstance(output, str) and "status" not in signature:
                    pending_drawings.append((check, dict(svg=output, facts=facts)))
        return signatures, evidence

    for case in selected:
        print(case["id"], file=sys.stderr, flush=True)
        if work:
            if digest(case["source"].encode()) != case["source_sha256"]:
                raise ValueError(f"input digest mismatch in {case['id']}")
            rows[case["id"]] = observe({"task": "topology", **case})
            continue
        requests = case["requests"]
        if any(digest(r["source"].encode()) != r["source_sha256"] for r in requests):
            raise ValueError(f"input digest mismatch in {case['id']}")
        if any(not valid_expected_labels(r.get("expected_labels", [])) for r in requests):
            raise ValueError(f"invalid expected_labels in {case['id']}")
        if not requests:
            raise ValueError(f"empty request sequence in {case['id']}")
        outcomes = [r.get("expected_outcome", "draw") for r in requests]
        if any(value not in EXPECTED_OUTCOMES for value in outcomes):
            raise ValueError(f"invalid expected_outcome in {case['id']}")
        job = {
            "task": "measure",
            "metric": args.metric,
            "operation": args.operation,
            "requests": requests,
            "warmups": args.warmups,
            "site": site or None,
            **({"stage_trace": True} if trace_path is not None else {}),
            **({"heap_profile": str(heap_profile)} if heap_profile else {}),
            **({"resource_profile": resource_profile} if resource_profile else {}),
        }
        row = rows[case["id"]] = {"expected_outcomes": outcomes}
        signatures, evidence = verify(job, row)
        if verify_layout:
            row["layout_evidence"] = evidence
        if stages:
            row["stage_layout"] = summarize_stage_layout(evidence, args.site_layout)
        statuses = [s.get("status", "ok") for s in signatures]
        row.update(completed=statuses.count("ok"), outputs=signatures)
        if any(status != "ok" for status in statuses):
            row.update(status="expected-rejection" if all(s in ("ok", "expected-rejection") for s in statuses) else "failed",
                       samples=[], expected_rejections=statuses.count("expected-rejection"))
        else:
            row.update(measure_rust_samples(observe, job, signatures, args.samples, exact))
            if stages:
                for sample in row["samples"]:
                    validate_stage_timing(sample)
        if verify(job) != (signatures, evidence):
            row.update(status="measurement-mismatch", error="verification changed after sampling", completed=0, samples=[])
        if with_memory:
            row["memory_samples"] = []
            if not row.get("status"):
                memory_job = dict(job, metric="stage-memory")
                memory_job.pop("stage_trace", None)
                if verify(memory_job) != (signatures, evidence):
                    raise ValueError(f"{case['id']}: stage memory runner returned different verified output")
                measured = measure_rust_samples(observe, memory_job, signatures, args.samples, False)
                if measured.get("error"):
                    raise ValueError(f"{case['id']}: {measured['error']} during stage memory collection")
                for sample in measured["samples"]:
                    validate_stage_memory(sample)
                if verify(memory_job) != (signatures, evidence):
                    raise ValueError(f"{case['id']}: stage memory verification changed after sampling")
                row["memory_samples"] = measured["samples"]
        if row.get("error"):
            print(f"{case['id']}: {row['error']}", file=sys.stderr)
    if heap_profile and not heap_profile.is_file():
        raise ValueError("workload did not write the requested DHAT profile")
    observer = observe_drawings(pending_drawings, args.chrome)
    return {
        "schema_version": 1,
        "surface": "rust",
        "options": options,
        "plan_sha256": digest(raw),
        "host": [platform.system(), platform.machine(), platform.node()],
        "runner_sha256": digest(runner.read_bytes()),
        **({"memory_runner_sha256": digest(memory_runner.read_bytes())} if with_memory else {}),
        "harness_sha256": fingerprints.pop(),
        **({"drawing_observer": observer} if observer else {}),
        **({"svg_validator_sha256": validator_sha256} if validation_modes == {True} else {}),
        "revision": args.revision
        or (
            None
            if args.runner
            else subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        ),
        "rows": rows,
    }


def verified_native_drawings(row):
    """Bind each passing observation to the SVG identity at its original request position."""
    identities = []
    for check in row.get("drawing_checks", []):
        index = check["request_index"]
        if type(index) is not int or not 0 <= index < len(row["outputs"]):
            raise ValueError("invalid native drawing request index")
        if (check["status"] != "passed" or check["svg_sha256"] != row["outputs"][index]["sha256"]
                or not check["facts"] or any(f["status"] != "passed" for f in check["facts"])):
            return None
        identities.append((index, check["diagram_id"], check["source_sha256"]))
    return identities


def run_rust_layouts(args):
    """Collect both public operations from one executable and one snapshot of the plan."""
    raw = args.plan.read_bytes()
    runner = args.runner.resolve() if args.runner else build_runner("clock", shoal=True)
    runner_sha = digest(runner.read_bytes())
    runs = {}
    for backend in LAYOUTS:
        runs[backend] = {}
        for operation in ("layout-json", "svg"):
            options = argparse.Namespace(**vars(args), site_layout=backend, operation=operation,
                                         verify_measured_svg=operation == "svg")
            report = run_rust(options, raw=raw, runner=runner, layout_comparison=True)
            if report["runner_sha256"] != runner_sha:
                raise ValueError("workload executable changed during layout comparison")
            runs[backend][operation] = report
    cases = {}
    for name in runs["dagre"]["svg"]["rows"]:
        inventories, medians = [], {}
        completed = 0
        reviewed = runs["dagre"]["svg"]["rows"][name]["expected_outcomes"] == ["draw"]
        for backend, reports in runs.items():
            layout, svg = (reports[op]["rows"][name] for op in ("layout-json", "svg"))
            completed += layout["completed"] + svg["completed"]
            if not reviewed or any(row.get("status") for row in (layout, svg)):
                continue
            graph, painting = (row["layout_evidence"][0]["inventory"] for row in (layout, svg))
            if (graph["nodes"] != [node for node, _ in painting["nodes"]]
                    or graph.get("groups", []) != [group for group, _ in painting.get("groups", [])]
                    or [edge[0] for edge in graph["edges"]] != [edge[0] for edge in painting["edges"]]):
                continue
            inventories.append((graph, painting))
            medians[backend] = {op: median(s["elapsed_ns"] for s in row["samples"])
                                for op, row in (("layout-json", layout), ("svg", svg))}
        case = cases[name] = dict(planned=2 * len(LAYOUTS), completed=completed)
        if not reviewed:
            case["status"] = "source-outcome-not-draw"
        elif completed != case["planned"]:
            case["status"] = "incomplete"
        elif len(inventories) != len(LAYOUTS) or any(i != inventories[0] for i in inventories):
            case["status"] = "inventory-mismatch"
        else:
            case.update(status="matching-inventories", median_ns=medians)
    return dict(schema_version=1, surface="rust-layouts", classification="diagnostic", plan_sha256=digest(raw),
                runner_sha256=runner_sha, rows=cases, runs=runs)


def compare_topology(old, new):
    a, b = ((row["boundary"] or {}).get("minimum_passing_budget") for row in (old, new))
    same = old["high_budget"] == new["high_budget"] and old["high"] == new["high"]
    result = {"default_status": [row["default"]["status"] for row in (old, new)],
              "base_minimum": a, "head_minimum": b}
    if "svg" in old or "svg" in new:
        if not all("svg" in row for row in (old, new)):
            raise ValueError("incompatible topology SVG observation protocols")
        result["default_svg_status"] = [row["svg"]["default"]["status"] for row in (old, new)]
        # A work change can preserve the exact returned SVG.
        def high_svg(row):
            return {k: v for k, v in row["svg"]["high"].items() if k != "layout_work_units"}

        same = same and high_svg(old) == high_svg(new)
    result["status"] = "same-high-budget-result" if same else "output-changed"
    result["delta"] = b - a if same and a is not None and b is not None else None
    return result


def compare(base, head, *, drawing_products=None):
    if any(report.get("surface") == "rust-stages" or report.get("options", {}).get("metric") == "stages"
           for report in (base, head)):
        raise ValueError("stage attribution reports do not support compare; use bench.py report --rust-stages")
    if base.get("surface") != "rust" or head.get("surface") != "rust":
        from .reference import compare_reference

        return compare_reference(base, head, drawing_products=drawing_products)
    if any(report.get("options", {}).get("heap_profile") for report in (base, head)):
        raise ValueError("heap profiles are diagnostic evidence")
    for key in ("schema_version", "surface", "plan_sha256", "options", "host"):
        if base[key] != head[key]:
            raise ValueError(f"incompatible reports: {key}")
    if list(base["rows"]) != list(head["rows"]):
        raise ValueError("different workload selections")
    if base["harness_sha256"] != head["harness_sha256"] or base.get("svg_validator_sha256") != head.get("svg_validator_sha256"):
        return {
            "schema_version": 1,
            "classification": "harness-changed",
            "plan_sha256": base["plan_sha256"],
            "base_harness": base["harness_sha256"],
            "head_harness": head["harness_sha256"],
            "base_svg_validator": base.get("svg_validator_sha256"),
            "head_svg_validator": head.get("svg_validator_sha256"),
            "rows": {},
        }
    options = base["options"]
    exact = options.get("clock_boundary") == "returned-svg"
    if "clock_boundary" in options and (not exact or options.get("metric") != "clock" or options.get("operation") not in SVG_OPERATIONS):
        raise ValueError("invalid Rust returned-SVG measurement boundary")
    differences = {}
    for name, old in base["rows"].items():
        new = head["rows"][name]
        if base["options"]["metric"] == "work":
            differences[name] = compare_topology(old, new)
            continue
        if old.get("status") or new.get("status"):
            differences[name] = {"status": old.get("status") if old.get("status") == new.get("status") else "outcome-changed",
                                 "base": old.get("status"), "head": new.get("status"),
                                 "completed": [old["completed"], new["completed"]],
                                 "expected_rejections": [r.get("expected_rejections", 0) for r in (old, new)]}
            if old["outputs"] != new["outputs"]:
                differences[name]["status"] = "outcome-changed"
            continue
        if old["outputs"] != new["outputs"]:
            differences[name] = {"status": "output-changed"}
            continue
        if "expected_outcomes" not in old or "expected_outcomes" not in new:
            differences[name] = {"status": "source-outcomes-unverified"}
            continue
        outcomes = old["expected_outcomes"]
        if outcomes != new["expected_outcomes"] or len(outcomes) != old["completed"] or old["completed"] != new["completed"]:
            raise ValueError(f"incompatible source outcomes for {name}")
        if len(set(outcomes)) != 1 or outcomes[0] not in ("draw", "allow-empty"):
            differences[name] = {"status": "no-comparable-outcome", "expected_outcomes": outcomes}
            continue
        if exact:
            for row in (old, new):
                if (row.get("measured_output_check") != "returned-svg" or len(row["outputs"]) != 1
                        or not re.fullmatch(r"[0-9a-f]{64}", str(row["outputs"][0].get("sha256")))
                        or not all(sample.keys() == {"elapsed_ns"} for sample in row["samples"])
                        or row.get("measured_svg_sha256") != [row["outputs"][0]["sha256"]] * base["options"]["samples"]
                        or len(row["samples"]) != base["options"]["samples"]):
                    raise ValueError(f"missing exact measured SVG evidence for {name}")
        if old.get("drawing_checks") or new.get("drawing_checks"):
            old_drawings = verified_native_drawings(old)
            if (not old_drawings or old_drawings != verified_native_drawings(new) or not base.get("drawing_observer")
                    or base["drawing_observer"] != head.get("drawing_observer")):
                differences[name] = {"status": "drawing-not-verified", "completed": new["completed"]}
                continue
        metrics = {}
        for k in old["samples"][0]:
            a, b = (median(s[k] for s in r["samples"]) for r in (old, new))
            metrics[k] = {"base": a, "head": b, "delta": b - a}
            if a > 0:
                metrics[k]["change_percent"] = 100 * (b - a) / a
        differences[name] = {"status": "same-output" if exact else "matching-verification-outputs",
                             "measured_output_check": "returned-svg" if exact else "size-only",
                             "completed": new["completed"], "metrics": metrics}
    return {
        "schema_version": 1,
        "plan_sha256": base["plan_sha256"],
        "options": base["options"],
        "base_runner": base["runner_sha256"],
        "head_runner": head["runner_sha256"],
        "classification": "diagnostic",
        "rows": differences,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    from .report import add_report_parser

    add_report_parser(sub)
    matrix = sub.add_parser("node-matrix", help="collect and compare Node products or named variants")
    matrix.add_argument("--plan", type=Path, required=True)
    matrix.add_argument("--out-dir", type=Path, required=True, help="directory for a new result bundle")
    matrix.add_argument("--peer", action="append", help="select a registered peer (repeatable, default: all)")
    matrix.add_argument("--variant", action="append", metavar="LABEL=ENGINE[:MODULE][:OPTIONS_JSON]",
                        help="select a named Node variant (repeat at least twice; first is the baseline). "
                             "Use ::OPTIONS_JSON with the locked Merman entry.")
    matrix.add_argument("--variant-direct-id", action="append", metavar="LABEL",
                        help="pass the frozen diagram ID as diagramId for a Merman variant without Options JSON")
    matrix.add_argument("--variant-layout", action="append", metavar="LABEL=LAYOUT",
                        help="set dagre, elk, or shoal once at construction for a Merman variant")
    matrix.add_argument("--samples", type=int, default=3, help="warm batches inside each scenario worker")
    matrix.add_argument("--process-rounds", type=int,
                        help="even number of independent rounds with balanced product order (minimum 2, default for variants: 2)")
    matrix.add_argument("--warmup-calls", type=int, help="maximum calls in the Node warmup (default: 20; 0 skips warmup)")
    matrix.add_argument("--warmup-ms", type=int, help="maximum elapsed ms in the Node warmup (default: 150; 0 skips warmup)")
    matrix.add_argument("--timeout-seconds", type=float, default=300, help="time budget per scenario")
    matrix.add_argument("--merman-module", type=Path, help="public Node entry of a local Merman candidate")
    matrix.add_argument("--check-drawings", action="store_true", help="retain and inspect declared SVG drawing facts after timing")
    matrix.add_argument("--chrome", type=Path, help="Chromium executable for --check-drawings")
    drawings = sub.add_parser("drawings", help="inspect retained SVGs against frozen drawing facts")
    drawings.add_argument("--plan", type=Path, required=True)
    drawings.add_argument("--report", required=True, type=Path, action="append", help="raw Node or SVG CLI report (repeatable)")
    drawings.add_argument("--chrome", type=Path, help="Chromium executable (default: Puppeteer configuration)")
    drawings.add_argument("--require-pass", action="store_true", help="return a failure for missing or failing drawing evidence")
    plan = sub.add_parser("plan", help="freeze portable source bytes and occurrence IDs")
    plan.add_argument("--suite", choices=["quick", "full", "families", "topology-orders"], default="quick")
    plan.add_argument(
        "--document", type=Path, action="append", default=[], help="ordered local .mmd file manifest"
    )
    plan.add_argument(
        "--pele-fixtures", type=Path, help="pinned mermaid-v13 packages/bench/fixtures/pele directory"
    )
    run = sub.add_parser("rust", help="run each sample in a fresh child process")
    layouts = sub.add_parser("rust-layouts", help="compare Dagre, ELK and Shoal on single-request Flowcharts")
    layouts.set_defaults(metric="clock", check_drawings=False, chrome=None, site_css_bytes=0)
    gallery = sub.add_parser("layout-gallery", help="write an offline HTML comparison of Flowchart and Agentflow SVGs")
    gallery.add_argument("sources", nargs="*", type=Path, help="additional Mermaid files to draw without native timing")
    gallery.add_argument("--plan", type=Path, help="frozen single-request Flowchart or Agentflow plan")
    gallery.add_argument("--comparison", type=Path, help="rust-layouts report for this plan and executable")
    gallery.add_argument("--runner", type=Path, help="use a frozen workloads clock executable")
    gallery.add_argument("--pele", action="store_true", help="add the locked Pele renderer and its separate Node.js SVG timing")
    gallery.add_argument("--override-frontmatter-layout", action="store_true",
                         help="lock the native backend for drawings without a recorded comparison, even when the source sets layout")
    gallery.add_argument("--out", required=True, type=Path, help="standalone HTML output file")
    for command in (run, layouts):
        command.add_argument("--plan", required=True, type=Path)
        command.add_argument("--warmups", type=int, default=1, help="untimed workload passes before each observation")
        command.add_argument("--samples", type=int, default=3, help="fresh processes per workload (work runs once)")
        command.add_argument("--runner", type=Path, help="use a frozen executable built with the matching metric")
        command.add_argument("--revision", help="revision label for a frozen runner")
        command.add_argument("--resource-profile", choices=["interactive", "constrained"],
                             help="resource profile for public operations (default: interactive)")
    run.add_argument("--metric", choices=["clock", "memory", "work", "stages"], default="memory")
    run.add_argument("--chrome-trace", type=Path, help="write a Chrome/Perfetto trace for the stages metric")
    run.add_argument("--heap-profile", type=Path, help="write a DHAT heap profile for one workload sample")
    run.add_argument("--with-memory", action="store_true", help="collect stage allocations in a separate executable")
    run.add_argument("--memory-runner", type=Path, help="use a frozen stage memory executable with --with-memory")
    run.add_argument("--operation", choices=OPERATIONS, default="svg-data")
    run.add_argument("--site-css-bytes", type=int, default=0)
    run.add_argument("--site-layout", choices=LAYOUTS, help="site default for rendering (Shoal requires layout evidence)")
    run.add_argument("--verify-measured-svg", action="store_true",
                     help="time a single public SVG return and verify its actual bytes after the clock stops")
    run.add_argument("--check-drawings", action="store_true", help="inspect declared drawing facts on verified SVGs after sampling")
    run.add_argument("--chrome", type=Path, help="Chromium executable for --check-drawings")
    run.add_argument("--require-complete", action="store_true",
                     help="fail after writing the report if a request missed its expected outcome, or default geometry or SVG failed for a work probe")
    diff = sub.add_parser("compare", help="compare two reports for the same surface and plan")
    diff.add_argument("base", type=Path)
    diff.add_argument("head", type=Path)
    diff.add_argument("--drawing-plan", type=Path, help="check retained SVG drawing facts on both reports before comparing")
    diff.add_argument("--chrome", type=Path, help="Chromium executable for --drawing-plan")
    coverage = sub.add_parser("coverage", help="inventory planned sources and verified labels")
    coverage.add_argument("--plan", required=True, type=Path, help="frozen workload plan")
    coverage.add_argument(
        "--comparison", type=Path, action="append", default=[],
        help="external workload comparison JSON (repeatable)",
    )
    coverage.add_argument("--out", type=Path, help="write JSON to a file (default: stdout)")
    for command in (plan, run, layouts):
        command.add_argument(
            "--scenario", action="append", default=[], help="select an ID (repeatable)"
        )
    for command in (plan, run, layouts, diff, drawings):
        command.add_argument("--out", required=True, type=Path)
    args = parser.parse_args(argv)
    if args.action in ("node-matrix", "rust") and args.chrome and not args.check_drawings:
        parser.error("--chrome requires --check-drawings")
    if args.action == "compare" and args.chrome and not args.drawing_plan:
        parser.error("--chrome requires --drawing-plan")
    if hasattr(args, "scenario") and len(set(args.scenario)) != len(args.scenario):
        parser.error("a scenario may only be selected once")
    if args.action in ("rust", "rust-layouts") and (args.samples <= 0 or args.warmups < 0 or args.site_css_bytes < 0):
        parser.error("samples must be positive; warmups and site CSS size must be nonnegative")
    if args.action in ("rust", "rust-layouts") and args.out.resolve() in {p.resolve() for p in (args.plan, args.runner) if p}:
        parser.error("choose a report output distinct from the frozen plan and runner")
    if args.action == "rust" and args.memory_runner and args.out.resolve() == args.memory_runner.resolve():
        parser.error("choose a report output distinct from the frozen memory runner")
    if args.action == "rust" and args.heap_profile and args.heap_profile.resolve() == args.out.resolve():
        parser.error("choose separate output paths for the memory report and heap profile")
    try:
        if args.action == "layout-gallery":
            from .gallery import write_gallery

            write_gallery(args)
            return 0
        if args.action == "report":
            from .report import generate_report

            print(generate_report(args))
            return 0
        if args.action == "node-matrix":
            from .node import collect_node_matrix

            print(collect_node_matrix(args))
            return 0
        if args.action == "drawings":
            from .drawing import audit_drawings

            if args.out.resolve() in {path.resolve() for path in [args.plan, *args.report]}:
                raise ValueError("choose a drawing report output distinct from its inputs")
            result = audit_drawings(args.plan, [(path, load(path)) for path in args.report], chrome=args.chrome)
        elif args.action == "plan":
            result = materialize(args)
        elif args.action == "rust":
            if args.chrome_trace:
                if args.metric != "stages":
                    raise ValueError("--chrome-trace requires --metric stages")
                if args.chrome_trace.resolve() in {p.resolve() for p in (args.plan, args.out, args.runner, args.memory_runner) if p}:
                    raise ValueError("choose a trace output distinct from the plan, report and frozen runners")
            result = run_rust(args)
            if args.chrome_trace:
                write_chrome_trace(args.chrome_trace, result)
        elif args.action == "rust-layouts":
            result = run_rust_layouts(args)
        elif args.action == "coverage":
            from .coverage import summarize_coverage

            comparisons = (load(path) for path in args.comparison)
            result = summarize_coverage(args.plan.read_bytes(), comparisons)
        else:
            inputs = [args.base, args.head]
            if any(path and args.out.resolve() == path.resolve() for path in (*inputs, args.drawing_plan)):
                raise ValueError("choose a comparison output distinct from its inputs")
            base, head = (load(path) for path in inputs)
            if args.drawing_plan:
                from .drawing import audit_drawings
                from .reference import product_label

                audit = audit_drawings(args.drawing_plan, list(zip(inputs, (base, head))), chrome=args.chrome)
                result = compare(base, head, drawing_products={product_label(p): p["scenarios"] for p in audit["products"]})
                result["drawing_audit"] = audit
            else:
                result = compare(base, head)
        if args.out:
            write(args.out, result)
        else:
            text = json.dumps(result, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
            sys.stdout.buffer.write(text.encode("utf-8"))
        if args.action == "drawings" and args.require_pass:
            return int(not result["planned"]["facts"] or any(
                s["status"] in {"failed", "unavailable"} for p in result["products"] for s in p["scenarios"].values()
            ))
        if args.action == "rust-layouts":
            return int(any(row["status"] != "matching-inventories" for row in result["rows"].values()))
        if args.action == "rust":
            rows = result["rows"].values()
            if any(row.get("status") == "measurement-mismatch" or any(
                output.get("status") == "layout-backend-mismatch" for output in row.get("outputs", [])
            ) for row in rows):
                return 1
            if args.require_complete:
                if args.metric == "work":
                    if any("svg" not in row for row in rows):
                        raise ValueError("--require-complete for work needs a runner that observes default SVG")
                    return int(any(row["default"]["status"] != "accepted" or row["svg"]["default"]["status"] != "accepted" for row in rows))
                return int(any(row.get("status") == "failed" or (args.check_drawings and any(
                    check["status"] != "passed" for check in row.get("drawing_checks", [])
                )) for row in rows))
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
