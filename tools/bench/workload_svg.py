"""Validate workload SVGs. On stdin, accept an ordered JSON array of {svg, expected_labels} jobs."""

import json
import sys
import xml.etree.ElementTree as ET


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


def validate_svg(svg, expected_labels=()):
    """Classify one rendered string or file's original bytes, independently of other batch members."""
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
    if root.get("aria-roledescription", "").strip().lower() == "error":
        return {"status": "error-svg", "message": "Mermaid error diagram"}

    elements = [root]
    visited = set()
    ids = None
    painted = False
    missing = {" ".join(label.split()) for label in expected_labels}
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
            missing.difference_update(texts)
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
    if not painted:
        return {"status": "empty-svg", "message": "empty SVG diagram"}
    if missing:
        labels = [label for label in expected_labels if " ".join(label.split()) in missing]
        return {
            "status": "missing-labels",
            "missing_labels": labels,
            "message": f"missing expected SVG labels: {labels!r}",
        }
    return {"status": "ok"}


if __name__ == "__main__":
    jobs = json.load(sys.stdin.buffer)
    results = [validate_svg(job["svg"], job.get("expected_labels", ())) for job in jobs]
    print(json.dumps(results, separators=(",", ":")))
