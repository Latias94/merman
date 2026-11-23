"""Publish a portable measurement receipt and render its reader-facing Markdown."""

from datetime import date
import html
import json
import os
from pathlib import Path
import re
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
    parser.add_argument("--note", help="public run note for the Overview. Start a paragraph with 'CLI:' to place it after the CLI results")
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


def report_notes(note):
    overview, cli = [], []
    for paragraph in re.split(r"\n\s*\n", (note or "").strip()):
        if paragraph.lstrip().startswith("CLI:"):
            cli.append(text(paragraph.lstrip().removeprefix("CLI:")))
        else:
            overview.append(text(paragraph))
    return "\n\n".join(overview), "\n\n".join(cli)


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
    return f"[Download the raw audit data](<{address}>)."


def host_label(host):
    system = (host or {}).get("platform") or (host or {}).get("os")
    if not system:
        return "Host details unavailable"
    platform = {"win32": "Windows", "darwin": "macOS", "linux": "Linux"}.get(system, system)
    runtime = host.get("runtime")
    platform = " ".join(v for v in (platform, host.get("arch")) if v)
    return text(" · ".join(v for v in (platform, host.get("cpu"), f"Node {runtime}" if runtime else None) if v))


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
    blocks = [f"The **{text(raw['suite'] or 'custom')}** plan contains {plan['requests']} requests in {plan['scenarios']} "
              f"scenarios with {plan['distinct_sources']} distinct sources. Each occurrence counts, including repeated "
              "diagrams in a document. Repeatability means each product returned the same bytes on repeated calls "
              "and met the source's output and label expectations.", node_context(raw)]
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
    blocks.append("Higher fractions mean broader coverage of these inputs. Expected rejections count separately from "
                  "renders. The family counts use one reviewed example per "
                  "family. The final column counts scenarios where both products passed every declared drawing property "
                  "in this sweep.")
    if gaps:
        blocks.append("\n".join(gaps))
    return "\n\n".join(blocks)


def node_reading(row):
    peer = PRODUCTS[row["peer"]]
    planned, eligible = row["planned_pairs"], row["eligible_pairs"]
    checks = row.get("visual_checks")
    if checks is None:
        drawing = "Drawing checks unreported for this row"
    elif checks["planned"] == 0:
        drawing = "SVG and any declared labels only"
    else:
        drawing = (f"Checks declared for {checks['planned']}/{row['requests_per_batch']} requests. "
                   f"Both passed all declared properties in {checks['passed_pairs']}/{planned} pairs.")

    reasons = row.get("reasons", [])
    if "expected-rejection" in reasons:
        direction = f"Warm ratio withheld. The batch includes an expected rejection ({eligible}/{planned} eligible pairs)."
        return "expected_rejection", direction, drawing
    pairs = row["pairs"]
    if row["status"] != "complete" or not row["ratio"] or not planned or eligible != planned or len(pairs) != planned:
        detail = f" ({', '.join(reasons)})" if reasons else ""
        return "insufficient", f"Insufficient data. {eligible}/{planned} eligible pairs{detail}.", drawing

    peer_faster = sum(p["ratio"] < 1 for p in pairs)
    merman_faster = sum(p["ratio"] > 1 for p in pairs)
    if peer_faster == planned or merman_faster == planned:
        faster = peer if peer_faster == planned else "Merman"
        direction = f"{faster} faster in {planned}/{planned} rounds"
        if checks and checks["planned"] > 0 and checks["passed_pairs"] == planned:
            return "checked", f"{direction} with the declared drawing properties verified.", drawing
        return "timing", f"Timing only. {direction}.", drawing

    tied = planned - peer_faster - merman_faster
    if tied == planned:
        return "uncertain", f"No direction. All {planned} pairs tied.", drawing
    direction = f"Uncertain. {peer} faster in {peer_faster}/{planned} rounds, Merman in {merman_faster}/{planned}"
    return "uncertain", direction + (f", {tied} tied." if tied else "."), drawing


def rounds_section(raw, survey):
    if not raw:
        return "", "No independent Node timing rounds were included."
    batches = raw["warm_batches_per_worker"]
    batch_count = f"{batches} warm batches per worker" if batches is not None else "warm batch count unreported"
    blocks = [f"The plan scheduled {raw['process_rounds']} independent process rounds per scenario ({batch_count}). "
              "Each millisecond column is the median of one product's worker medians for a complete warm batch. "
              "Lower times mean the batch finished sooner. "
              "In each round, divide the peer's batch median by Merman's batch median from that same round. "
              "The ratio column shows the median and observed range of those quotients. Below 1 favors the peer, "
              "above 1 favors Merman. The endpoints are the smallest and largest ratios observed on this host. "
              "When the faster product changes between rounds, the direction remains uncertain even if the median favors one."]
    context = node_context(raw)
    if not survey or node_context(survey) != context:
        blocks.append(context)
    rows = []
    counts = dict(checked=0, timing=0, uncertain=0, expected_rejection=0, insufficient=0)
    for row in raw["latency"]:
        result, direction, drawing = node_reading(row)
        counts[result] += 1
        ratio = row["ratio"] if result in ("checked", "timing", "uncertain") else None
        rows.append([row["scenario"], PRODUCTS[row["peer"]], f"{row['requests_per_batch']} / {row.get('distinct_sources', '—')}",
                     number(row["merman_ms"]) if ratio else "—", number(row["peer_ms"]) if ratio else "—",
                     f"{number(ratio['median'])} ({number(ratio['min'])}–{number(ratio['max'])})" if ratio else "—",
                     direction, drawing])
    blocks.append(table(["Scenario", "Peer", "Requests / distinct sources per batch", "Merman ms", "Peer ms",
                         "Peer / Merman (range)", "Observed direction", "Drawing evidence"], rows))
    blocks.append("The batch clock stops before output validation. Eligible pairs have repeatable SVG and pass any "
                  "declared label checks. A timing direction with "
                  "verified drawing properties requires the same faster product and passing checks on both sides "
                  "in every scheduled round. The checks cover only the properties declared for the listed requests. "
                  "A dash marks an unavailable count or aggregate.")
    summary = ", ".join(f"{counts[key]} {label}" for key, label in (
        ("checked", "consistent with verified drawing properties"), ("timing", "consistent on timing alone"),
        ("uncertain", "with mixed or tied rounds"), ("expected_rejection", "withheld for expected rejections"),
        ("insufficient", "with insufficient data")) if counts[key] or key == "checked")
    comparison = "comparison" if len(rows) == 1 else "comparisons"
    return f"{len(rows)} scenario/peer {comparison} in the {raw['planned']['scenarios']}-scenario plan ({summary}).", "\n\n".join(blocks)


def startup_section(rounds):
    if not rounds or not rounds.get("startup"):
        return "This receipt has no Node startup or peak RSS measurements."
    intro = ("Times and peak memory are medians across independent workers for each scenario and product. Initialization covers "
             "package import and engine setup inside an already started Node process. First SVG times a successful "
             "first render call, even when a later request in the batch is rejected as expected. Init through first SVG "
             "measures elapsed time from import start through that call's return, including intervening setup. Both "
             "SVG timers end before output validation. Peak resident set size (RSS) covers the "
             "worker's lifetime through the measurement, including warm batches and output validation. Lower milliseconds "
             "mean less waiting and lower MiB means a smaller process peak. A dash means the measurement is unavailable.")
    rows = [[r["scenario"], PRODUCTS[r["engine"]], f"{r['workers']}/{r['planned_workers']}", number(r["initialize_ms"]),
             number(r["first_svg_ms"]), number(r["init_through_first_svg_ms"]), number(r["peak_rss_mib"])]
            for r in rounds["startup"]]
    return intro + "\n\n" + table(["Scenario", "Product", "Measured / planned workers", "Initialize ms", "First SVG ms",
                                   "Init through first SVG ms", "Peak RSS MiB"], rows)


def cli_section(exports):
    if not exports:
        return "No fresh-process CLI export was measured for this report."
    blocks = ["The file export tables time one fresh CLI process per diagram. Each product runs its batches "
              "sequentially. Times are median wall milliseconds per complete batch, including startup and file output. "
              "Lower times mean a shorter wait for the files. "
              "An mmdc / Merman ratio above 1 favors Merman, below 1 favors mmdc. Source counts deduplicate "
              "identical input within each scenario."]
    for raw in exports:
        left, right = (raw["products"][side] for side in ("left", "right"))
        if (left["engine"], right["engine"]) != ("merman", "mmdc"):
            raise ValueError("pass CLI inputs in Merman, mmdc order")
        cov = raw["coverage"]
        blocks.append(f"### {text(raw['format'].upper())} files")
        blocks.append(f"Merman CLI {text(left['version'].split()[-1])} and mmdc {text(right['version'].split()[-1])} · "
                      f"{host_label(raw['host'])} · {raw['samples']} batches per scenario and product. "
                      f"Both returned repeatable files on {cov['common_stable']}/{cov['planned_requests']} requests "
                      f"in a {cov['planned_scenarios']}-scenario plan with {cov['planned_distinct_sources']} distinct "
                      f"sources. {text(raw['output_scope'])}.")
        rows = []
        notes = []
        for row in raw["rows"]:
            batch = row["complete_batch_ms"] or {}
            qualified = batch.get("right_over_left") is not None and not row["ratio_withheld_reason"]
            left_ms, right_ms, ratio = (number(batch.get(key)) if qualified else "—"
                                        for key in ("left", "right", "right_over_left"))
            sources = len({request["source_sha256"] for request in row["requests"]})
            rows.append([row["id"], f"{row['common_stable']}/{row['planned_requests']}",
                         f"{row['stable_coverage']['distinct_sources']}/{sources}", raw["render_settings"],
                         left_ms, right_ms, ratio])
            if not qualified:
                reason = row.get("complete_batch_withheld_reason") or row["ratio_withheld_reason"] or "No eligible complete batch."
                notes.append(f"- {text(row['id'])} — {text(reason)}")
        blocks.append(table(["Scenario", "Repeatable requests / planned", "Repeatable sources / planned", "Settings",
                             "Merman ms", "mmdc ms", "mmdc / Merman"], rows))
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
    blocks = [f"The corpus pairs Merman Web {engine} with Mermaid.js {upstream} in Chromium ({text(browser)}). Both modes completed "
              f"for {coverage['succeeded_families']}/{coverage['selected_families']} selected family examples. Each "
              "attempted fixture starts a fresh browser process. Each measured block runs both engines. Cold starts in a fresh "
              "realm and warm reuses the engine. The publishable interval includes presentation, waiting for "
              "`requestAnimationFrame` and validation. The table gives median elapsed milliseconds. A Mermaid.js / "
              "Merman ratio above 1 favors Merman, below 1 favors Mermaid.js."]
    rows = []
    for row in raw["rows"]:
        for mode in ("cold", "warm"):
            observation = row[mode]
            timings = observation["median_ms"]
            samples = observation["samples"]
            rows.append([row["id"], mode, number(timings["left"]), number(timings["right"]), number(timings["right_over_left"]),
                         " / ".join(str(samples[side]) if samples[side] is not None else "unknown" for side in ("left", "right")),
                         observation["ratio_withheld_reason"] or "Measured"])
    blocks.append(table(["Fixture", "Publishable SVG", "Merman Web ms", "Mermaid.js ms", "Mermaid.js / Merman",
                         "Samples (Merman / Mermaid.js)", "Result"], rows))
    if any(r["warm"].get("budgeted_svg_ms") is not None for r in raw["rows"]):
        blocks.append("The shorter warm interval (`warmBudgetedSvgMs`) covers the adapter call through the SVG "
                      "byte-budget check. An engine can perform DOM layout inside that call. A dash means the browser "
                      "report supplied no value or ratio for that cell.")
        budgeted_rows = []
        for row in raw["rows"]:
            times = row["warm"].get("budgeted_svg_ms") or {}
            budgeted_rows.append([row["id"], number(times.get("left")), number(times.get("right")), number(times.get("right_over_left"))])
        blocks.append(table(["Fixture", "Merman Web ms through byte check", "Mermaid.js ms through byte check",
                             "Mermaid.js / Merman"], budgeted_rows))
    else:
        blocks.append("This receipt has no warm SVG byte-budget timings.")
    return "\n\n".join(blocks)


def rust_section(raw):
    if not raw:
        return "No native Rust work probe was included."
    counts = raw["coverage"]
    total = counts["reported_topologies"]
    revision = raw["products"]["merman"].get("revision")
    build = f"recorded revision `{text(revision)}`" if revision else "revision unreported"
    blocks = [f"The local Rust runner ({build}) measured {total}/{counts['planned_topologies']} planned Flowchart "
              f"stress controls on {host_label(raw['host'])}. At the default budget, geometry completed on "
              f"**{counts['layout_default'].get('accepted', 0)}/{total}** observed cases and public SVG on "
              f"**{counts['svg_default'].get('accepted', 0)}/{total}**. These synthetic graphs probe the layout "
              "work budget and source edge limit. Work units count charged operations. The minimum geometry budget "
              "succeeds at the listed value and fails one unit below. A lower minimum means geometry needs fewer "
              "charged units for that source. The receipt identifies the executable by digest."]
    high = counts["svg_high"]
    ceiling = f"With the elevated work budget, {high.get('accepted', 0)}/{total} stress cases produced SVG."
    if high.get("source_max_edges_rejected"):
        rejected = high["source_max_edges_rejected"]
        ceiling += f" The separate source edge limit applied to {rejected} {'case' if rejected == 1 else 'cases'}."
    blocks.append(ceiling)
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
                blocks.append(f"`{original['id']}` and `{variant['id']}` describe the same labeled, directed graph. "
                              "Changing statement order changed whether the default ELK budget allowed SVG output.")
                break
    return "\n\n".join(blocks)


def render(data, filename):
    if not isinstance(data, dict) or data.get("format") != FORMAT:
        raise ValueError("unrecognized portable benchmark receipt")
    lanes = data.get("measurements")
    if not isinstance(lanes, dict) or not lanes or lanes.keys() - {"node_coverage", "node_rounds", "cli", "rust_work", "browser"}:
        raise ValueError("unknown or missing public benchmark measurements")
    matrix, rounds = lanes.get("node_coverage"), lanes.get("node_rounds")
    exports, browser, rust = lanes.get("cli"), lanes.get("browser"), lanes.get("rust_work")
    rounds_summary, rounds_detail = rounds_section(rounds, matrix)
    overview = []
    if matrix:
        stable = ", ".join(f"{counts['common_stable']}/{counts['planned_requests']} with {PRODUCTS[peer]}"
                           for peer, counts in matrix["coverage"].items())
        overview.append(f"- **Node coverage.** Merman and its peers returned repeatable SVG on {stable}. "
                        f"The plan contains {matrix['planned']['distinct_sources']} distinct sources.")
    if rounds:
        overview.append(f"- **Warm Node rendering.** {rounds_summary}")
    if exports:
        formats = ", ".join(f"{r['format'].upper()} ({r['coverage']['planned_distinct_sources']} distinct sources planned, "
                            f"{r['samples']} batches per product and scenario)" for r in exports)
        overview.append(f"- **CLI exports.** {formats}.")
    if browser:
        cov = browser["coverage"]
        overview.append(f"- **Browser rendering.** Both modes completed on {cov['succeeded_families']}/{cov['selected_families']} "
                        f"selected family examples, with {browser['samples']} blocks planned per mode.")
    if rust:
        work = rust["coverage"]
        overview.append(f"- **Local Rust layout.** {work['svg_default'].get('accepted', 0)}/{work['reported_topologies']} "
                        "observed Flowchart stress controls produced SVG under the default limits.")
    missing = [label for lane, label in ((matrix, "Node coverage"), (rounds, "Node timing rounds"), (browser, "browser rendering"),
                                        (rust, "native layout work")) if not lane]
    measured_formats = {r['format'] for r in exports or []}
    missing.extend(f"CLI {fmt.upper()} exports" for fmt in ("svg", "png") if fmt not in measured_formats)
    if missing:
        overview.append("\nUnmeasured in this receipt — " + ", ".join(missing) + ".")
    overview_note, cli_note = report_notes(data.get("note"))
    if overview_note:
        overview.append("\n" + overview_note)
    cli = cli_section(exports)
    if cli_note:
        cli += "\n\n**CLI note.** " + cli_note
    revision = rust["products"]["merman"].get("revision") if rust else None
    markdown = Template(TEMPLATE.read_text(encoding="utf-8")).substitute(
        date=date.fromisoformat(data["date"]).isoformat(), overview="\n".join(overview),
        coverage=coverage_section(matrix), rounds=rounds_detail, startup=startup_section(rounds),
        cli=cli, browser=browser_section(browser), rust=rust_section(rust),
        rust_build=f" · local `{text(revision[:9])}`" if revision else "",
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
