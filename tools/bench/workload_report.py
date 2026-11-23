"""Publish a portable measurement receipt and render its reader-facing Markdown."""

from datetime import date
import html
import json
import os
from pathlib import Path
from string import Template
from tempfile import TemporaryDirectory
from urllib.parse import quote, urlsplit


TEMPLATE = Path(__file__).with_suffix(".md.in")
FORMAT = "merman-public-renderer-report-v1"
PRODUCTS = {"merman": "Merman", "pele": "Pele", "mermaid-v13": "Mermaid v13", "mmdc": "mmdc"}
BACKENDS = {"napi": "N-API", "javascript": "JavaScript", "node-wasm": "Node-WASM"}


def add_report_parser(sub):
    parser = sub.add_parser("report", help="generate or replay a reader-facing renderer comparison")
    parser.add_argument("--node-matrix", type=Path, metavar="DIR", help="single-pass Node matrix bundle for coverage")
    parser.add_argument("--node-rounds", type=Path, metavar="DIR", help="balanced process-round bundle for Node timings")
    parser.add_argument("--cli", nargs=3, type=Path, metavar=("PLAN", "MERMAN", "MMDC"), action="append",
                        help="frozen plan and both raw CLI reports; repeat for a different output format")
    parser.add_argument("--rust-work", nargs=2, type=Path, metavar=("PLAN", "REPORT"),
                        help="public Rust topology plan and --metric work report")
    parser.add_argument("--browser", type=Path, metavar="JSON", help="playground browser family corpus report")
    parser.add_argument("--date", type=date.fromisoformat, help="measurement date (YYYY-MM-DD)")
    parser.add_argument("--note", help="optional public description of this build or run")
    parser.add_argument("--raw-url", help="public HTTPS reference to the archived raw measurements")
    parser.add_argument("--out", type=Path, help="new Markdown file; a sibling JSON file stores its portable evidence")
    parser.add_argument("--from", type=Path, dest="receipt", metavar="JSON", help="regenerate Markdown beside a published receipt")
    parser.add_argument("--check", action="store_true", help="verify that Markdown matches --from, without writing it")


def text(value):
    value = " ".join(str(value).split())
    escaped = html.escape(value, quote=False)
    for char in "\\|`[]*_~":
        escaped = escaped.replace(char, "\\" + char)
    return escaped


def table(headers, rows):
    def line(values):
        return "| " + " | ".join(text(value) for value in values) + " |"

    return "\n".join([line(headers), line(["---"] * len(headers)), *(line(row) for row in rows)])


def number(value):
    return "—" if value is None else f"{value:,.3f}"


def raw_link(address):
    if not address:
        return "This snapshot has no public raw-bundle link."
    url = urlsplit(address)
    if (url.scheme != "https" or not url.hostname or url.username is not None
            or any(ord(char) < 33 or char in '<>\\' for char in address)):
        raise ValueError("--raw-url must be a public HTTPS link without embedded credentials")
    return f"[Raw measurement archive](<{address}>)."


def host_label(host):
    system = (host or {}).get("platform") or (host or {}).get("os")
    if not system:
        return "Host details unavailable"
    platform = {"win32": "Windows", "darwin": "macOS", "linux": "Linux"}.get(system, system)
    runtime = host.get("runtime")
    return text(" · ".join(v for v in (f"{platform} {host['arch']}", host.get("cpu"), f"Node {runtime}" if runtime else None) if v))


def node_context(raw):
    products = []
    missing = []
    for engine, package in raw["products"].items():
        if package["version"] and package["backend"]:
            products.append(f"{PRODUCTS[engine]} {package['version']} ({BACKENDS[package['backend']]})")
        else:
            products.append(f"{PRODUCTS[engine]} (build unavailable)")
        workers = package["workers"]
        if workers["missing"]:
            missing.append(f"{PRODUCTS[engine]} {workers['missing']}/{workers['planned']}")
    message = f"{host_label(raw['host'])}. The collection scheduled {text(', '.join(products))}."
    if raw["status"] != "complete" or missing:
        message += " Collection incomplete." + (f" Fraction of scenario workers missing by product — {text(', '.join(missing))}." if missing else "")
    return message


def coverage_section(raw):
    if not raw:
        return "No Node coverage sweep was included."
    plan = raw["planned"]
    blocks = [f"The **{text(raw['suite'] or 'custom')}** plan contains **{plan['requests']} requests** in {plan['scenarios']} "
              f"scenarios with {plan['distinct_sources']} distinct sources. Repeated requests retain their "
              "place in a document. A stable request returned the same bytes repeatedly within each product. "
              "Each pair was checked against the source expectations.", node_context(raw)]
    rows, gaps = [], []
    for peer, counts in raw["coverage"].items():
        family = counts["families"]
        drawing = counts["drawing_checks"]
        rows.append([PRODUCTS[peer], f"{counts['common_completed']}/{counts['planned_requests']}",
                     f"{counts['common_stable']}/{counts['planned_requests']}", counts["common_expected_rejections"],
                     f"{family['merman_stable']} / {family['peer_stable']} of {family['planned']}" if family['planned'] else "—",
                     f"{drawing['passed']}/{drawing['planned']}" if drawing['planned'] else "—"])
        gapnames = [g["family"] for g in counts["family_gaps"]]
        if gapnames:
            gaps.append(f"- Merman and {PRODUCTS[peer]} lacked a shared repeatable output for {text(', '.join(gapnames))}.")
        if drawing["failed"] or drawing["unchecked"]:
            gaps.append(f"- Drawing checks for Merman and {PRODUCTS[peer]} had {drawing['failed']} failed "
                        f"and {drawing['unchecked']} unchecked scenarios.")
    blocks.append(table(["Compared with Merman", "Both completed", "Both repeatable", "Both rejected as expected",
                         "Repeatable family examples (Merman / peer)", "Drawing scenarios passing on both"], rows))
    blocks.append("**How to read these counts.** A larger completion or repeatability fraction means more of this "
                  "fixed plan passed for both products; keep the denominator in view. Expected rejections are not "
                  "successful renders or speed samples. Each family column uses one reviewed example, so more repeatable "
                  "examples means broader observed coverage, not a complete support guarantee. Drawing checks cover only "
                  "the declared properties for those scenarios; a stable SVG can still differ in appearance.")
    if gaps:
        blocks.append("\n".join(gaps))
    return "\n\n".join(blocks)


def rounds_section(raw, survey):
    if not raw:
        return "No independent Node timing rounds were included. The coverage sweep supplies no process-paired timing estimate."
    blocks = [f"Each product ran **{raw['process_rounds']} independent process rounds** per scenario, with "
              f"{raw['warm_batches_per_worker']} warm batches per worker. The table reports the median of "
              "the worker medians in milliseconds per complete batch. The ratio is computed within each "
              "paired round before taking its median. A value below 1 means the peer took less time. "
              "The range shows the smallest and largest observed ratios."]
    if not survey or node_context(survey) != node_context(raw):
        blocks.append(node_context(raw))
    rows, withheld = [], []
    for row in raw["latency"]:
        ratio = row["ratio"] if row["status"] == "complete" else None
        pairs = row["pairs"]
        direction = f"{sum(p['ratio'] < 1 for p in pairs)} / {sum(p['ratio'] == 1 for p in pairs)} / {sum(p['ratio'] > 1 for p in pairs)}"
        rows.append([row["scenario"], row["requests_per_batch"], PRODUCTS[row["peer"]],
                     number(row["merman_ms"]) if ratio else "—", number(row["peer_ms"]) if ratio else "—",
                     f"{number(ratio['median'])} ({number(ratio['min'])}–{number(ratio['max'])})" if ratio else "—",
                     direction if ratio else "—", f"{row['eligible_pairs']}/{row['planned_pairs']}"])
        if not ratio:
            reasons = ", ".join(text(reason) for reason in row.get("reasons", []))
            withheld.append(f"- {text(row['scenario'])} with {PRODUCTS[row['peer']]} had {row['eligible_pairs']} "
                            f"eligible pairs out of {row['planned_pairs']}" + (f" ({reasons})." if reasons else "."))
    blocks.append(table(["Scenario", "Requests per batch", "Peer", "Merman ms", "Peer ms", "Peer / Merman (range)",
                         "Peer faster / tie / Merman faster", "Eligible pairs"], rows))
    blocks.append("**How to read this table.** Lower `ms` means a shorter batch for this workload. `Peer / Merman` "
                  "below 1 means the peer was faster; above 1 means Merman was faster. The range is the observed "
                  "minimum–maximum across paired rounds, not a confidence interval. If it crosses 1, neither product "
                  "led in every round. Direction counts are peer-faster / tie / Merman-faster rounds. Compare rows "
                  "with the same requests per batch. A dash means no aggregate was admitted, not a zero-time render.")
    if withheld:
        blocks.append("\n".join(withheld))
    peers = set(raw["coverage"])
    if peers:
        faster = {}
        for row in raw["latency"]:
            if row["status"] == "complete" and row["ratio"]["max"] < 1:
                faster.setdefault(row["scenario"], set()).add(row["peer"])
        consistent = [name for name, observed in faster.items() if observed == peers]
        if consistent:
            ids = ", ".join(text(name) for name in consistent)
            blocks.append(f"**Merman profiling candidates.** In {ids}, every measured peer took less time in every "
                          "paired round. Replay these inputs on two Merman builds and time parsing, layout and SVG "
                          "emission to locate the cost.")
    return "\n\n".join(blocks)


def cli_section(exports):
    if not exports:
        return "No fresh-process CLI export was measured for this report."
    blocks = ["Each request starts a new process and writes a file. The table shows the median wall time for "
              "a complete batch, including startup. PNG exports use each renderer's fonts and dimensions. "
              "These samples run sequentially for each product."]
    for raw in exports:
        left, right = (raw["products"][side] for side in ("left", "right"))
        if (left["engine"], right["engine"]) != ("merman", "mmdc"):
            raise ValueError("pass CLI inputs in Merman, mmdc order")
        cov = raw["coverage"]
        blocks.append(f"### {text(raw['format'].upper())} files")
        blocks.append(f"{text(left['version'])} and mmdc {text(right['version'])} · {host_label(raw['host'])} · "
                      f"{raw['samples']} batches per product. The two products returned repeatable files on "
                      f"{cov['common_stable']} of {cov['planned_requests']} planned requests. Settings reported by "
                      f"the collector were **{text(raw['render_settings'])}**.")
        rows = []
        notes = []
        for row in raw["rows"]:
            batch = row["complete_batch_ms"] or {}
            qualified = batch.get("right_over_left") is not None and not row["ratio_withheld_reason"]
            left_ms, right_ms, ratio = (number(batch.get(key)) if qualified else "—"
                                        for key in ("left", "right", "right_over_left"))
            rows.append([row["id"], str(row["planned_requests"]), left_ms, right_ms, ratio])
            if not qualified:
                reason = row.get("complete_batch_withheld_reason") or row["ratio_withheld_reason"] or "No eligible complete batch."
                notes.append(f"- {text(row['id'])} — {text(reason)}")
        blocks.append(table(["Scenario", "Requests per batch", "Merman ms", "mmdc ms", "mmdc / Merman"], rows))
        blocks.append("**How to read this table.** Lower `ms` means a quicker file export. `mmdc / Merman` above 1 "
                      "means mmdc took longer and Merman was faster; below 1 means mmdc was faster. These fresh-process "
                      "times include CLI startup and output writing. SVG and PNG have separate costs and rendering "
                      "boundaries. A dash means the row was withheld, not a zero-duration render.")
        if notes:
            blocks.append("\n".join(notes))
        if cov["reported_scenarios"] != cov["planned_scenarios"]:
            blocks.append(f"The collectors reported {cov['reported_scenarios']} of {cov['planned_scenarios']} planned scenarios.")
    return "\n\n".join(blocks)


def browser_section(raw):
    if not raw:
        return "No browser corpus was included."
    coverage = raw["coverage"]
    engine, upstream = (text(raw["products"][side]["version"]) for side in ("left", "right"))
    browser = raw["host"].get("chromium_version") or "version unavailable"
    preface = (f"The browser run compared **Merman Web {engine}** with **Mermaid.js {upstream}** in Chromium "
               f"{text(browser)}. Both modes completed for {coverage['succeeded_families']} of {coverage['selected_families']} "
               f"selected family examples. Each fixture has its own browser process. Cold measures the first publishable "
               "SVG in a fresh realm. Warm measures a publishable SVG with the engine already initialized. "
               "Each block runs both products. The table gives the median elapsed time per product and mode. "
               "**How to read browser rows:** lower milliseconds means faster. A Mermaid.js / Merman ratio above 1 favors "
               "Merman; below 1 favors Mermaid.js. Do not combine cold and warm rows.")
    rows = []
    for row in raw["rows"]:
        for mode in ("cold", "warm"):
            observation = row[mode]
            timings = observation["median_ms"]
            ratio = timings["right_over_left"]
            rows.append([row["id"], mode, number(timings["left"]) if ratio is not None else "—",
                         number(timings["right"]) if ratio is not None else "—", number(ratio),
                         " / ".join(str(n) if n is not None else "unknown" for n in observation["samples"].values()),
                         observation["ratio_withheld_reason"] or "Measured"])
    return preface + "\n\n" + table(["Fixture", "Mode", "Merman Web ms", "Mermaid.js ms", "Mermaid.js / Merman",
                                      "Samples (Merman / Mermaid.js)", "Result"], rows)


def rust_section(raw):
    if not raw:
        return "No native Rust work probe was included."
    counts = raw["coverage"]
    total = counts["reported_topologies"]
    blocks = [f"On {host_label(raw['host'])}, the native library observed {total} of "
              f"{counts['planned_topologies']} planned Flowchart topologies. At the default budget, geometry "
              f"completed on **{counts['layout_default'].get('accepted', 0)}/{total}** and public SVG on "
              f"**{counts['svg_default'].get('accepted', 0)}/{total}**. Work units count operations charged "
              "by Merman's layout budget. The minimum is verified on the geometry writer at adjacent passing "
              "and failing ceilings. **How to read layout work:** geometry and SVG counts are successful results / observed "
              "cases; more means broader completion under this budget. 'Work limit' and 'Source edge limit' are "
              "different causes. Fewer minimum geometry units means a lower charged-work threshold for that source, "
              "not a measured rendering-time advantage."]
    labels = {"accepted": "Returned", "layout_work_rejected": "Work limit", "source_max_edges_rejected": "Source edge limit"}
    rows = raw["rows"]
    exceptions = {r["id"] for r in rows if r["layout_default"] != "accepted" or r["svg_default"] != "accepted"}
    controls = {r["canonical_id"] for r in rows if r["id"] in exceptions and "canonical_id" in r}
    selected_ids = exceptions | controls
    selected = [r for r in rows if r["id"] in selected_ids or r.get("canonical_id") in exceptions]
    if selected:
        blocks.append("Cases reaching a default limit and their available statement-order controls appear below. "
                      "The portable measurements include every observed topology.")
        blocks.append(table(["Topology", "Nodes / edges", "Geometry", "SVG", "Minimum geometry units"], [
            [r["id"], f"{r['nodes']} / {r['edges']}", labels[r["layout_default"]], labels[r["svg_default"]],
             f"{r['minimum_passing_budget']:,}" if r["minimum_passing_budget"] is not None else "—"] for r in selected]))
        originals = {r["id"]: r for r in rows}
        for variant in selected:
            original = originals.get(variant.get("canonical_id"))
            if original and original["svg_default"] != variant["svg_default"]:
                blocks.append(f"**A case to profile.** Compare `{original['id']}` and `{variant['id']}` in the default ELK "
                              "layout. They describe the same labeled, directed graph. Changing statement order changed "
                              "whether the SVG returned at the default budget. Profiling both inputs can identify the stage "
                              "responsible for the difference.")
                break
    revision = raw["products"]["merman"].get("revision")
    if revision:
        blocks.append(f"Native runner revision label `{text(revision)}`. Its executable digest is recorded in the JSON.")
    return "\n\n".join(blocks)


def render(data, filename):
    if not isinstance(data, dict) or data.get("format") != FORMAT:
        raise ValueError("unrecognized portable benchmark receipt")
    lanes = data.get("measurements")
    if not isinstance(lanes, dict) or not lanes or lanes.keys() - {"node_coverage", "node_rounds", "cli", "rust_work", "browser"}:
        raise ValueError("unknown or missing public benchmark measurements")
    matrix, rounds = lanes.get("node_coverage"), lanes.get("node_rounds")
    highlights = []
    if matrix:
        plan = matrix["planned"]
        highlights.append(f"- **Coverage.** The Node survey includes {plan['requests']} calls in {plan['scenarios']} scenarios, "
                          f"with {plan['families']} single-example family baselines.")
    if rounds:
        complete = [r for r in rounds["latency"] if r["status"] == "complete"]
        crossing = sum(r["ratio"]["min"] < 1 < r["ratio"]["max"] for r in complete)
        highlights.append(f"- **Warm timings.** {len(complete)} of {len(rounds['latency'])} scenario and peer pairs have every "
                          f"scheduled round. The faster product changed between rounds in {crossing} of those pairs.")
    if lanes.get("rust_work"):
        work = lanes["rust_work"]["coverage"]
        highlights.append(f"- **Native limits.** {work['svg_default'].get('accepted', 0)} of {work['reported_topologies']} "
                          "Flowchart topologies produced SVG at Merman's default layout budget.")
    if lanes.get("cli") and not highlights:
        highlights.append("- **CLI exports.** Measurements cover " + " and ".join(r["format"].upper() for r in lanes["cli"]) + " output files.")
    if lanes.get("browser") and not highlights:
        highlights.append(f"- **Browser rendering.** The selected corpus covers {lanes['browser']['coverage']['selected_families']} family examples.")
    if data.get("note"):
        highlights.append("\n" + text(data["note"]))
    markdown = Template(TEMPLATE.read_text(encoding="utf-8")).substitute(
        date=date.fromisoformat(data["date"]).isoformat(), highlights="\n".join(highlights),
        coverage=coverage_section(matrix), rounds=rounds_section(rounds, matrix), cli=cli_section(lanes.get("cli")),
        browser=browser_section(lanes.get("browser")), rust=rust_section(lanes.get("rust_work")),
        receipt=quote(filename, safe="-._~"), raw_bundle=raw_link(data.get("raw_url")),
    )
    return markdown.encode("utf-8")


def collect(args):
    from workload_report_node import load_matrix, load_rounds
    from workload_report_surfaces import load_browser, load_cli, load_rust

    if not any((args.node_matrix, args.node_rounds, args.cli, args.rust_work, args.browser)):
        raise ValueError("select at least one measured surface")
    if not args.date or not args.out or args.out.suffix != ".md":
        raise ValueError("collection requires --date and a new --out Markdown path ending in .md")
    lanes = {}
    if args.node_matrix:
        lanes["node_coverage"] = load_matrix(args.node_matrix)
    if args.node_rounds:
        lanes["node_rounds"] = load_rounds(args.node_rounds)
    if args.cli:
        lanes["cli"] = [load_cli(*paths) for paths in args.cli]
        formats = [row["format"] for row in lanes["cli"]]
        if len(formats) != len(set(formats)):
            raise ValueError("include one CLI pair per output format in each report")
    if args.rust_work:
        lanes["rust_work"] = load_rust(*args.rust_work)
    if args.browser:
        lanes["browser"] = load_browser(args.browser)
    return {"format": FORMAT, "date": args.date.isoformat(), "note": args.note,
            "raw_url": args.raw_url, "measurements": lanes}


def generate_report(args):
    if args.receipt:
        if any((args.node_matrix, args.node_rounds, args.cli, args.rust_work, args.browser, args.date,
                args.note, args.raw_url, args.out)) or args.receipt.suffix != ".json":
            raise ValueError("--from takes a portable .json receipt and optionally --check")
        receipt = args.receipt
        output = receipt.with_suffix(".md")
        markdown = render(json.loads(receipt.read_bytes()), receipt.name)
        if args.check:
            if output.read_bytes().replace(b"\r\n", b"\n") != markdown:
                raise ValueError(f"generated Markdown differs from {output}")
        else:
            with TemporaryDirectory(dir=output.parent) as temp:
                draft = Path(temp) / "report.md"
                draft.write_bytes(markdown)
                try:
                    os.replace(draft, output)
                except PermissionError:
                    if not output.is_file():
                        raise
                    output.write_bytes(markdown)
        return output
    if args.check:
        raise ValueError("--check requires --from")
    data = collect(args)
    output = args.out
    receipt = output.with_suffix(".json")
    markdown = render(data, receipt.name)
    serialized = (json.dumps(data, indent=2, ensure_ascii=False, allow_nan=False) + "\n").encode()
    if output.exists() or receipt.exists():
        raise FileExistsError("choose a new report name or regenerate the existing receipt with --from")
    output.parent.mkdir(parents=True, exist_ok=True)
    with receipt.open("xb") as destination:
        destination.write(serialized)
    with output.open("xb") as destination:
        destination.write(markdown)
    return output
