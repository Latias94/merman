"""Validate an ordered JSON array of SVG results or renderer diagnostics on stdin."""

import json
import math
import re
import sys
import xml.etree.ElementTree as ET


EXPECTED_OUTCOMES = ("draw", "allow-empty", "reject", "unreviewed")
_NUMBER = r"[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?"
_LENGTH = re.compile(rf"({_NUMBER})(?:%|px|em|rem|ex|ch|cm|mm|q|in|pt|pc|vw|vh|vmin|vmax)?", re.I)


def validate_rejection(code, message):
    """Recognize diagram-parser diagnostics from the public Node products and CLI tools."""
    # Merman reports maxEdges through MERMAN_PARSE_ERROR as well. A code (or CLI
    # exit 1) identifies a candidate; the diagnostic must describe a grammar error.
    candidate = code in (None, "MERMAN_PARSE_ERROR", "syntax", "SYNTAX") or (type(code) is int and code == 1)
    rejected = False
    if candidate and isinstance(message, str):
        text = message.strip().replace("\r\n", "\n")
        text = text.removeprefix("Generating single mermaid chart\n").lstrip().removeprefix("Error: ")
        header, _, details = text.partition("\n")
        if re.fullmatch(r"Parse error on line [0-9]+:", header):  # Pele and mmdc (Jison)
            rejected = re.search(r"^Expecting .+, got .+$", details, re.MULTILINE) is not None
        else:
            rejected = bool(re.fullmatch(
                r"Diagram parse error \([^()]+\): unexpected (?:end of input|"
                r"(?:extra )?(?:token|identifier|statement separator|edge operator|`[^`]+`))(?:; expected .+)?"
                r"|Error at line [0-9]+, column [0-9]+: Expected .+, got .+"  # Mermaid v13
                r"|Lexical error on line [0-9]+\. Unrecognized text\.",
                header,
            ))
    return {"status": "expected-rejection" if rejected else "render-error", "code": code, "message": message}


def _sized_viewport(root):
    """Check explicit root dimensions; a painted legacy SVG may omit its intrinsic size."""
    view_box = root.get("viewBox")
    if view_box is not None:
        parts = re.split(r"\s*,\s*|\s+", view_box.strip())
        if len(parts) != 4 or not all(re.fullmatch(_NUMBER, part) for part in parts):
            raise ValueError("invalid SVG viewBox")
        values = [float(part) for part in parts]
        if not all(map(math.isfinite, values)) or min(values[2:]) <= 0:
            raise ValueError("SVG viewBox must be finite with positive width and height")

    dimensions = {key: root.get(key, "") for key in ("width", "height", "max-width", "max-height")}
    important = set()
    for declaration in root.get("style", "").split(";"):
        name, _, value = declaration.partition(":")
        name = name.strip().lower()
        if name in dimensions:
            value, priority = re.subn(r"!\s*important\s*$", "", value, flags=re.I)
            if priority or name not in important:
                dimensions[name] = value
            if priority:
                important.add(name)

    sized = set()
    for name, text in dimensions.items():
        length = _LENGTH.fullmatch(text.strip())
        if length:
            value = float(length[1])
            if not math.isfinite(value) or value <= 0:
                raise ValueError(f"SVG root {name} must be finite and positive")
            sized.add(name)
    return view_box is not None or {"width", "height"} <= sized


def _label_options(fact):
    return fact if isinstance(fact, list) else (fact,)


def valid_expected_labels(labels) -> bool:
    """Each required fact is a nonblank string or a nonempty list of nonblank alternatives."""
    if not isinstance(labels, list):
        return False
    for fact in labels:
        options = _label_options(fact)
        if not options or not all(isinstance(text, str) and text.strip() for text in options):
            return False
    return True


def label_text(element):
    """Collect text runs, lines and joined inline text within one SVG/HTML label."""

    def parts(node):
        tag = node.tag.rsplit("}", 1)[-1]
        if tag in {"defs", "style", "script", "title", "desc", "metadata", "template"}:
            return
        if node.text:
            yield node.text
        last_line = None
        for child in node:
            position = None
            if child.tag in {"tspan", "{http://www.w3.org/2000/svg}tspan"} and "x" in child.attrib and "y" in child.attrib:
                position = child.get("x"), child.get("y")
                if last_line and position[0] == last_line[0] and position[1] != last_line[1]:
                    yield None  # SVG wraps can reset x on a new baseline without a whitespace node.
            last_line = position
            yield from parts(child)
            if child.tail:
                yield child.tail
        if tag in {"br", "p", "div", "li"}:
            yield "\n"

    runs = list(parts(element))
    joined = "".join(run for run in runs if run is not None)
    texts = [run for run in runs if run is not None] + joined.splitlines() + [joined]
    if None in runs:
        texts.append("".join(run if run is not None else " " for run in runs))
    return {" ".join(text.split()) for text in texts if text.strip()}


def validate_svg(svg, expected_labels=(), expected_outcome="draw"):
    """Classify one rendered string or file's original bytes, independently of other batch members."""
    if expected_outcome not in EXPECTED_OUTCOMES:
        return {"status": "invalid-request", "message": f"invalid expected_outcome: {expected_outcome!r}"}
    if not isinstance(svg, (bytes, str)):
        return {"status": "invalid-svg", "message": "expected an SVG string"}
    if not svg.strip():
        return {"status": "empty-svg", "message": "empty SVG diagram"}
    try:
        root = ET.fromstring(svg)
    except (ET.ParseError, ValueError, LookupError) as error:
        return {"status": "invalid-svg", "message": str(error)}
    namespace = "{http://www.w3.org/2000/svg}"
    if root.tag not in {"svg", namespace + "svg"}:
        return {"status": "invalid-svg", "message": "output root is not SVG"}
    try:
        sized = _sized_viewport(root)
    except ValueError as error:
        return {"status": "invalid-svg", "message": str(error)}
    # Mermaid also draws its fixed error diagram on renderer exceptions. The SVG
    # alone does not identify a parser rejection, even for a reviewed reject input.
    if root.get("aria-roledescription", "").strip().lower() == "error":
        return {"status": "error-svg", "message": "Mermaid error diagram"}

    elements = [root]
    visited = set()
    ids = None
    painted = False
    missing = set(range(len(expected_labels)))
    facts_by_text = {}
    for index, fact in enumerate(expected_labels):
        for text in _label_options(fact):
            facts_by_text.setdefault(" ".join(text.split()), []).append(index)
    while elements:
        element = elements.pop()
        if element in visited:
            continue
        visited.add(element)
        tag = element.tag.removeprefix(namespace)
        classes = element.get("class", "").split()
        if (tag == "path" and "error-icon" in classes) or (tag == "text" and "error-text" in classes):
            return {"status": "error-svg", "message": "Mermaid error diagram"}
        if tag in {"svg", "g", "a", "switch", "symbol"}:
            elements.extend(child for child in element if child.tag.removeprefix(namespace) != "symbol")
        elif tag == "use":
            href = element.get("href", element.get("{http://www.w3.org/1999/xlink}href", ""))
            if href.startswith("#"):
                if ids is None:
                    ids = {n.get("id"): n for n in root.iter() if n.get("id")}
                if href[1:] in ids:
                    elements.append(ids[href[1:]])
            elif href:
                painted = True
        elif tag in {"text", "tspan", "foreignObject"} and (missing or not painted):
            texts = label_text(element)
            for text in texts:
                missing.difference_update(facts_by_text.pop(text, ()))
            if texts or (
                tag == "foreignObject"
                and any(
                    child.tag.rsplit("}", 1)[-1] in {"img", "svg", "canvas", "video"}
                    for child in element.iter()
                )
            ):
                painted = True
        elif tag in {
            "path",
            "rect",
            "line",
            "polyline",
            "polygon",
            "circle",
            "ellipse",
            "image",
        }:
            painted = True
    if not painted and (expected_outcome == "draw" or not sized):
        return {"status": "empty-svg", "message": "empty SVG diagram"}
    if missing:
        labels = [fact for index, fact in enumerate(expected_labels) if index in missing]
        return {
            "status": "missing-labels",
            "missing_labels": labels,
            "message": f"missing expected SVG labels: {labels!r}",
        }
    if expected_outcome == "reject":
        return {"status": "unexpected-success", "message": "renderer produced an SVG for an expected rejection"}
    if expected_outcome in ("allow-empty", "unreviewed"):
        return {"status": "ok", "output_class": "draw" if painted else "empty"}
    return {"status": "ok"}


def _validate_job(job):
    outcome = job.get("expected_outcome", "draw")
    if "error" in job and outcome in EXPECTED_OUTCOMES:
        error = job["error"]
        if not isinstance(error, dict) or "svg" in job:
            return {"status": "invalid-request", "message": "expected one renderer diagnostic or SVG"}
        verdict = validate_rejection(error.get("code"), error.get("message"))
        if outcome != "reject":
            verdict["status"] = "render-error"
        return verdict
    return validate_svg(job.get("svg"), job.get("expected_labels", ()), outcome)


if __name__ == "__main__":
    jobs = json.load(sys.stdin.buffer)
    results = [_validate_job(job) for job in jobs]
    print(json.dumps(results, separators=(",", ":")))
