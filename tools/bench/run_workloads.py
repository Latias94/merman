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

from corpus_utils import load_corpus, select_corpus_fixtures
from workload_drawing import drawing_case, observe_drawings
from workload_svg import EXPECTED_OUTCOMES, valid_expected_labels, validate_rejection, validate_svg
from workload_topology import SCC_SHAPES, topology_source


ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
FAMILY_MANIFEST = ROOT / "playground/examples/manifest.json"
FAMILY_LABELS = HERE / "workload_family_labels.json"
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


def family_scenarios():
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

    reviews = parse_workload_json(FAMILY_LABELS.read_bytes(), FAMILY_LABELS)
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
    catalog = parse_workload_json((HERE / "workloads.json").read_bytes(), "workloads.json")
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
        cases.extend(family_scenarios())

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
            if name in args.scenario or nodes in series.get("render_nodes", []):
                labels = [f"C{i}" for i in range(nodes)] if shape in SCC_SHAPES else ["N0", f"N{nodes - 1}"]
                cases.append(scenario(name, "merman/topology", [(name, source)], {name: labels}))
    names = [s["id"] for s in cases]
    if len(names) != len(set(names)):
        raise ValueError("duplicate scenario ID in workload sources")
    if args.scenario:
        by_id = {s["id"]: s for s in cases}
        cases = [by_id[name] for name in args.scenario]
    return {"schema_version": 1, "suite": args.suite, "scenarios": cases, "topologies": topologies}


def build_runner(metric):
    name = "public_workload_memory" if metric == "memory" else "public_workloads"
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
    result = subprocess.run(
        [runner, "--workload-stdin"], input=json.dumps(job).encode(), stdout=subprocess.PIPE, check=True
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


def run_rust(args):
    raw = args.plan.read_bytes()
    plan = json.loads(raw)
    if plan["schema_version"] != 1:
        raise ValueError("unsupported workload plan")
    work = args.metric == "work"
    exact = args.verify_measured_svg
    if exact and (args.metric != "clock" or args.operation not in SVG_OPERATIONS):
        raise ValueError("--verify-measured-svg requires an SVG operation and --metric clock")
    if args.check_drawings and (work or args.operation not in SVG_OPERATIONS):
        raise ValueError("--check-drawings requires an SVG operation")
    if work and (args.site_layout or args.site_css_bytes or args.operation != "svg-data"):
        raise ValueError(
            "work observes default Flowchart geometry and accepts no site or operation override"
        )
    site = {}
    if args.site_css_bytes:
        rule = ".r{opacity:0.8;}"
        count, pad = divmod(args.site_css_bytes, len(rule))
        site["themeCSS"] = rule * count + " " * pad
    if args.site_layout:
        site["layout"] = args.site_layout
    selected = plan["topologies" if work else "scenarios"]
    by_id = {r["id"]: r for r in selected}
    if not selected or len(by_id) != len(selected):
        raise ValueError("empty selection or duplicate workload ID")
    if args.scenario:
        selected = [by_id[name] for name in args.scenario]
    if exact and any(len(row["requests"]) != 1 for row in selected):
        raise ValueError("--verify-measured-svg requires one request in each selected scenario")
    runner = args.runner.resolve() if args.runner else build_runner(args.metric)
    options = {
        "metric": args.metric,
        "operation": "layout-work" if work else args.operation,
        "warmups": 0 if work else args.warmups,
        "site_css_bytes": args.site_css_bytes,
        "site_layout": args.site_layout,
        "samples": 1 if work else args.samples,
        **({"clock_boundary": "returned-svg"} if exact else {}),
    }
    rows = {}
    pending_drawings = []
    fingerprints = set()
    validation_modes = set()
    validator_sha256 = digest((HERE / "workload_svg.py").read_bytes())

    def observe(job):
        row = child(runner, job)
        fingerprints.add(row.pop("harness_sha256"))
        if len(fingerprints) != 1:
            raise ValueError("workload runner changed during observation")
        return row

    def verify(job, row=None):
        outputs = observe({**job, "verify_only": True})["outputs"]
        signatures, shared = rust_signatures(outputs, job["requests"], job["operation"])
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
        return signatures

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
        }
        row = rows[case["id"]] = {"expected_outcomes": outcomes}
        signatures = verify(job, row)
        statuses = [s.get("status", "ok") for s in signatures]
        row.update(completed=statuses.count("ok"), outputs=signatures)
        if any(status != "ok" for status in statuses):
            row.update(status="expected-rejection" if all(s in ("ok", "expected-rejection") for s in statuses) else "failed",
                       samples=[], expected_rejections=statuses.count("expected-rejection"))
        else:
            row.update(measure_rust_samples(observe, job, signatures, args.samples, exact))
        if verify(job) != signatures:
            row.update(status="measurement-mismatch", error="verification changed after sampling", completed=0, samples=[])
        if row.get("error"):
            print(f"{case['id']}: {row['error']}", file=sys.stderr)
    observer = observe_drawings(pending_drawings, args.chrome)
    return {
        "schema_version": 1,
        "surface": "rust",
        "options": options,
        "plan_sha256": digest(raw),
        "host": [platform.system(), platform.machine(), platform.node()],
        "runner_sha256": digest(runner.read_bytes()),
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
    if base.get("surface") != "rust" or head.get("surface") != "rust":
        from workload_reference import compare_reference

        return compare_reference(base, head, drawing_products=drawing_products)
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
    matrix = sub.add_parser("node-matrix", help="collect and compare the locked Node products")
    matrix.add_argument("--plan", type=Path, required=True)
    matrix.add_argument("--out-dir", type=Path, required=True, help="directory for a new result bundle")
    matrix.add_argument("--peer", action="append", help="select a registered peer (repeatable, default: all)")
    matrix.add_argument("--samples", type=int, default=3, help="warm batches inside each scenario worker")
    matrix.add_argument("--process-rounds", type=int,
                        help="even number of independent rounds with balanced product order (minimum 2)")
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
    run.add_argument("--plan", required=True, type=Path)
    run.add_argument("--metric", choices=["clock", "memory", "work"], default="memory")
    run.add_argument("--operation", choices=OPERATIONS, default="svg-data")
    run.add_argument(
        "--warmups", type=int, default=1, help="untimed workload passes before each observation"
    )
    run.add_argument(
        "--samples",
        type=int,
        default=3,
        help="fresh processes per workload (work is exact and runs once)",
    )
    run.add_argument("--site-css-bytes", type=int, default=0)
    run.add_argument("--site-layout", choices=["dagre", "elk"])
    run.add_argument("--verify-measured-svg", action="store_true",
                     help="time a single public SVG return and verify its actual bytes after the clock stops")
    run.add_argument("--check-drawings", action="store_true", help="inspect declared drawing facts on verified SVGs after sampling")
    run.add_argument("--chrome", type=Path, help="Chromium executable for --check-drawings")
    run.add_argument("--require-complete", action="store_true",
                     help="fail after writing the report if a request missed its expected outcome, or default geometry or SVG failed for a work probe")
    run.add_argument(
        "--runner", type=Path, help="use a frozen executable built with the matching metric"
    )
    run.add_argument("--revision", help="revision label for a frozen runner")
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
    for command in (plan, run):
        command.add_argument(
            "--scenario", action="append", default=[], help="select an ID (repeatable)"
        )
    for command in (plan, run, diff, drawings):
        command.add_argument("--out", required=True, type=Path)
    args = parser.parse_args(argv)
    if args.action in ("node-matrix", "rust") and args.chrome and not args.check_drawings:
        parser.error("--chrome requires --check-drawings")
    if args.action == "compare" and args.chrome and not args.drawing_plan:
        parser.error("--chrome requires --drawing-plan")
    if hasattr(args, "scenario") and len(set(args.scenario)) != len(args.scenario):
        parser.error("a scenario may only be selected once")
    if args.action == "rust" and (args.samples <= 0 or args.warmups < 0 or args.site_css_bytes < 0):
        parser.error("samples must be positive; warmups and site CSS size must be nonnegative")
    try:
        if args.action == "node-matrix":
            from workload_matrix import collect_node_matrix

            print(collect_node_matrix(args))
            return 0
        if args.action == "drawings":
            from workload_drawing import audit_drawings

            if args.out.resolve() in {path.resolve() for path in [args.plan, *args.report]}:
                raise ValueError("choose a drawing report output distinct from its inputs")
            result = audit_drawings(args.plan, [(path, load(path)) for path in args.report], chrome=args.chrome)
        elif args.action == "plan":
            result = materialize(args)
        elif args.action == "rust":
            result = run_rust(args)
        elif args.action == "coverage":
            from workload_coverage import summarize_coverage

            comparisons = (load(path) for path in args.comparison)
            result = summarize_coverage(args.plan.read_bytes(), comparisons)
        else:
            inputs = [args.base, args.head]
            if any(path and args.out.resolve() == path.resolve() for path in (*inputs, args.drawing_plan)):
                raise ValueError("choose a comparison output distinct from its inputs")
            base, head = (load(path) for path in inputs)
            if args.drawing_plan:
                from workload_drawing import audit_drawings

                audit = audit_drawings(args.drawing_plan, list(zip(inputs, (base, head))), chrome=args.chrome)
                result = compare(base, head, drawing_products={p["engine"]: p["scenarios"] for p in audit["products"]})
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
        if args.action == "rust":
            rows = result["rows"].values()
            if any(row.get("status") == "measurement-mismatch" for row in rows):
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
