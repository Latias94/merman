use merman::DiagramFamilyId;
use merman_render::__private::{SvgArtifactReceipt, SvgElementObservation};

use crate::runner::{
    C6ProofError, C6ProofResult,
    artifact_observation::{local_fragment_id, percent_value},
    c6_sequence_proof::require_exact_writer_declaration,
};

const CYAN: &str = "#00f2ff";
const NAVY: &str = "#051423";
const SHAPE: &[(f64, &str)] = &[
    (8.0, "rgba(0, 242, 255, 0.5)"),
    (16.0, "rgba(0, 242, 255, 0.3)"),
];
const EDGE: &[(f64, &str)] = &[(6.0, "rgba(0, 242, 255, 0.6)")];
const TEXT: &[(f64, &str)] = &[(5.0, "rgba(0, 242, 255, 0.5)")];

pub(super) fn verify(family: DiagramFamilyId, receipt: &SvgArtifactReceipt) -> C6ProofResult<()> {
    c6_ensure!(
        "cyberpunk-native-text",
        receipt.has_native_text()
            && !receipt.has_foreign_object()
            && !receipt.has_prepared_tokens(),
        "complete profile requires native text"
    );
    canvas(receipt)?;
    match family {
        DiagramFamilyId::FLOWCHART => flowchart(receipt),
        DiagramFamilyId::SEQUENCE => sequence(receipt),
        DiagramFamilyId::XY_CHART => xychart(receipt),
        _ => Err(C6ProofError::new("cyberpunk-scope", "undeclared family")),
    }
}

fn select<'a>(
    receipt: &'a SvgArtifactReceipt,
    expected: usize,
    label: &str,
    predicate: impl Fn(&SvgElementObservation) -> bool,
) -> C6ProofResult<Vec<&'a SvgElementObservation>> {
    let nodes = receipt
        .elements()
        .iter()
        .filter(|node| predicate(node))
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-inventory",
        nodes.len() == expected,
        "{label}: expected {expected}, got {}",
        nodes.len()
    );
    Ok(nodes)
}

fn props(node: &SvgElementObservation, expected: &[(&str, &str)]) -> C6ProofResult<()> {
    for &(property, value) in expected {
        let actual = node
            .style_value(property)
            .or_else(|| node.attribute(property));
        c6_ensure!(
            "cyberpunk-style",
            actual == Some(value),
            "{} {:?}: {property} = {actual:?}, expected {value}",
            node.tag_name(),
            node.classes()
        );
    }
    Ok(())
}

fn number(node: &SvgElementObservation, property: &str, expected: f64) -> C6ProofResult<()> {
    let actual = node
        .style_value(property)
        .or_else(|| node.attribute(property))
        .and_then(|value| {
            value
                .strip_suffix("px")
                .unwrap_or(value)
                .parse::<f64>()
                .ok()
        });
    c6_ensure!(
        "cyberpunk-number",
        actual.is_some_and(|value| (value - expected).abs() <= 1e-4),
        "{} {property} = {actual:?}, expected {expected}",
        node.tag_name()
    );
    Ok(())
}

fn no_inline_conflicts(
    node: &SvgElementObservation,
    expected: &[(&str, &str)],
) -> C6ProofResult<()> {
    for &(property, value) in expected {
        c6_ensure!(
            "cyberpunk-inline-style",
            node.style_value(property)
                .is_none_or(|actual| actual == value),
            "conflicting inline {property} on {}",
            node.tag_name()
        );
    }
    for property in ["opacity", "fill-opacity", "stroke-opacity"] {
        if node
            .style_value(property)
            .or_else(|| node.attribute(property))
            .is_some()
        {
            number(node, property, 1.0)?;
        }
    }
    Ok(())
}

fn by_class<'a>(
    receipt: &'a SvgArtifactReceipt,
    tag: &str,
    class: &str,
    count: usize,
) -> C6ProofResult<Vec<&'a SvgElementObservation>> {
    select(receipt, count, class, |node| {
        node.tag_name() == tag && node.has_class(class)
    })
}

fn root_id(receipt: &SvgArtifactReceipt) -> C6ProofResult<&str> {
    receipt
        .root_id()
        .ok_or_else(|| C6ProofError::new("cyberpunk-root", "missing root id"))
}

fn linked<'a>(
    receipt: &'a SvgArtifactReceipt,
    value: Option<&str>,
    tag: &str,
) -> C6ProofResult<&'a SvgElementObservation> {
    let id = value.and_then(local_fragment_id).ok_or_else(|| {
        C6ProofError::new(
            "cyberpunk-reference",
            format!("missing local {tag} binding"),
        )
    })?;
    Ok(select(receipt, 1, tag, |node| {
        node.tag_name() == tag && node.id() == Some(id)
    })?[0])
}

fn glow(
    receipt: &SvgArtifactReceipt,
    terminal: &SvgElementObservation,
    shadows: &[(f64, &str)],
) -> C6ProofResult<()> {
    let binding = terminal.attribute("filter");
    c6_ensure!(
        "cyberpunk-filter",
        terminal
            .style_value("filter")
            .is_none_or(|value| Some(value) == binding),
        "conflicting inline filter"
    );
    let filter = linked(receipt, binding, "filter")?;
    props(filter, &[("color-interpolation-filters", "sRGB")])?;
    let nodes = receipt.descendants_of(filter.index()).collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-filter",
        nodes.len() == shadows.len() * 6,
        "unexpected composed filter structure"
    );
    let results = nodes
        .iter()
        .filter_map(|node| node.attribute("result"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-filter",
        results.len() == shadows.len() * 4
            && results
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == results.len(),
        "ambiguous filter result bindings"
    );
    for (index, (block, &(sigma, color))) in nodes.chunks_exact(6).zip(shadows).enumerate() {
        c6_ensure!(
            "cyberpunk-filter",
            block.iter().map(|node| node.tag_name()).eq([
                "feGaussianBlur",
                "feFlood",
                "feComposite",
                "feMerge",
                "feMergeNode",
                "feMergeNode"
            ]),
            "unexpected filter primitive order"
        );
        let input = if index == 0 {
            "SourceGraphic"
        } else {
            filter_result(nodes[(index - 1) * 6 + 3])?
        };
        let blur = filter_result(block[0])?;
        let flood = filter_result(block[1])?;
        let shadow = filter_result(block[2])?;
        filter_result(block[3])?;
        props(block[0], &[("in", input)])?;
        number(block[0], "stdDeviation", sigma)?;
        props(block[1], &[("flood-color", color)])?;
        if block[1].attribute("flood-opacity").is_some() {
            number(block[1], "flood-opacity", 1.0)?;
        }
        props(
            block[2],
            &[("in", flood), ("in2", blur), ("operator", "in")],
        )?;
        props(block[4], &[("in", shadow)])?;
        props(block[5], &[("in", input)])?;
    }
    Ok(())
}

fn filter_result(node: &SvgElementObservation) -> C6ProofResult<&str> {
    node.attribute("result")
        .filter(|name| !name.is_empty() && *name != "SourceGraphic")
        .ok_or_else(|| C6ProofError::new("cyberpunk-filter", "missing or reserved filter result"))
}

fn canvas(receipt: &SvgArtifactReceipt) -> C6ProofResult<()> {
    let base = select(receipt, 1, "canvas base", |node| {
        node.attribute("data-merman-theme-canvas") == Some("base")
    })?[0];
    props(base, &[("fill", NAVY)])?;
    let [x, y, width, height] = receipt.view_box();
    for (property, value) in [("x", x), ("y", y), ("width", width), ("height", height)] {
        number(base, property, value)?;
    }
    let layers = select(receipt, 3, "canvas layers", |node| {
        node.attribute("data-merman-theme-canvas-layer").is_some()
    })?;
    for (index, layer) in layers.into_iter().enumerate() {
        props(
            layer,
            &[
                ("data-merman-theme-canvas-layer", &index.to_string()),
                ("mix-blend-mode", "screen"),
            ],
        )?;
        let rects = receipt
            .descendants_of(layer.index())
            .filter(|node| node.tag_name() == "rect")
            .collect::<Vec<_>>();
        c6_ensure!(
            "cyberpunk-canvas",
            rects.len() == 1,
            "layer needs one surface"
        );
        let paint = rects[0].attribute("fill");
        for (property, value) in [("x", x), ("y", y), ("width", width), ("height", height)] {
            number(rects[0], property, value)?;
        }
        let gradient = if index == 0 {
            let gradient = linked(receipt, paint, "radialGradient")?;
            number(gradient, "cx", x + width / 2.0)?;
            number(gradient, "cy", y + height / 2.0)?;
            number(gradient, "r", width.hypot(height) / 2.0)?;
            gradient
        } else {
            let pattern = linked(receipt, paint, "pattern")?;
            props(
                pattern,
                &[
                    ("patternUnits", "userSpaceOnUse"),
                    ("patternTransform", &format!("translate({x} {y})")),
                ],
            )?;
            number(pattern, "width", 40.0)?;
            number(pattern, "height", 40.0)?;
            let rect = receipt
                .descendants_of(pattern.index())
                .find(|node| node.tag_name() == "rect")
                .ok_or_else(|| C6ProofError::new("cyberpunk-canvas", "missing grid surface"))?;
            let gradient = linked(receipt, rect.attribute("fill"), "linearGradient")?;
            let geometry = if index == 1 {
                [0.0, 20.0, 40.0, 20.0]
            } else {
                [20.0, 0.0, 20.0, 40.0]
            };
            for (property, value) in ["x1", "y1", "x2", "y2"].into_iter().zip(geometry) {
                number(gradient, property, value)?;
            }
            gradient
        };
        props(gradient, &[("gradientUnits", "userSpaceOnUse")])?;
        let expected: &[(f64, &str)] = if index == 0 {
            &[
                (0.0, "rgba(0, 242, 255, 0.05)"),
                (70.0, "rgba(0, 242, 255, 0)"),
            ]
        } else {
            &[
                (0.0, "rgba(0, 242, 255, 0.03)"),
                (2.5, "rgba(0, 242, 255, 0.03)"),
                (2.5, "#00000000"),
                (100.0, "#00000000"),
            ]
        };
        let stops = receipt
            .descendants_of(gradient.index())
            .filter(|node| node.tag_name() == "stop")
            .collect::<Vec<_>>();
        c6_ensure!(
            "cyberpunk-canvas",
            stops.len() == expected.len(),
            "wrong gradient stop count"
        );
        for (stop, &(offset, color)) in stops.into_iter().zip(expected) {
            props(stop, &[("stop-color", color)])?;
            if stop.attribute("stop-opacity").is_some() {
                number(stop, "stop-opacity", 1.0)?;
            }
            c6_ensure!(
                "cyberpunk-canvas",
                stop.attribute("offset")
                    .and_then(percent_value)
                    .is_some_and(|actual| (actual - offset).abs() <= 1e-4),
                "wrong gradient stop position"
            );
        }
    }
    Ok(())
}

fn flowchart(receipt: &SvgArtifactReceipt) -> C6ProofResult<()> {
    let root = root_id(receipt)?;
    for selector in [
        format!("#{root} .label text,#{root} span"),
        format!("#{root} .cluster-label text"),
        format!("#{root} .edgeLabel .label,#{root} .edgeLabel .label text,#{root} .edgeLabel span"),
    ] {
        require_exact_writer_declaration(receipt, &selector, "fill", CYAN)?;
    }
    for (property, value) in [("fill", NAVY), ("opacity", "1")] {
        require_exact_writer_declaration(
            receipt,
            &format!("#{root} .edgeLabel rect"),
            property,
            value,
        )?;
    }
    for (id, text, tag) in [
        ("Browse", "Browse Products", "rect"),
        ("Stock", "Item in Stock?", "polygon"),
        ("Cart", "Add to Cart", "rect"),
        ("Unavailable", "Out of Stock", "rect"),
    ] {
        let owner = select(receipt, 1, id, |node| {
            node.attribute("data-et") == Some("node") && node.attribute("data-id") == Some(id)
        })?[0];
        let shape = select(receipt, 1, id, |node| {
            node.parent_index() == Some(owner.index()) && node.has_class("label-container")
        })?[0];
        c6_ensure!(
            "cyberpunk-shape",
            shape.tag_name() == tag,
            "wrong {id} geometry"
        );
        props(shape, &[("fill", NAVY), ("stroke", CYAN)])?;
        number(shape, "stroke-width", 3.0)?;
        if tag == "rect" {
            number(shape, "rx", 10.0)?;
            number(shape, "ry", 10.0)?;
        }
        glow(receipt, shape, SHAPE)?;
        let label = receipt
            .descendants_of(owner.index())
            .find(|node| node.tag_name() == "text" && node.text() == text)
            .ok_or_else(|| C6ProofError::new("cyberpunk-labels", format!("missing {text}")))?;
        number(label, "font-weight", 600.0)?;
    }
    let cluster = select(receipt, 1, "Checkout", |node| {
        node.attribute("data-et") == Some("cluster")
            && node.attribute("data-id") == Some("Checkout")
    })?[0];
    let surface = select(receipt, 1, "cluster surface", |node| {
        node.parent_index() == Some(cluster.index()) && node.tag_name() == "rect"
    })?[0];
    props(surface, &[("fill", NAVY), ("stroke", CYAN)])?;
    by_class(receipt, "rect", "background", 2)?;
    // The frozen Dagre profile emits one label owner per edge, including the unlabelled edge.
    for (id, label) in [
        ("L_Browse_Stock_0", ""),
        ("L_Stock_Cart_0", "Yes"),
        ("L_Stock_Unavailable_0", "No"),
    ] {
        select(receipt, 1, id, |node| {
            node.tag_name() == "path" && node.attribute("data-id") == Some(id)
        })?;
        let owner = select(receipt, 1, id, |node| {
            node.has_class("label") && node.attribute("data-id") == Some(id)
        })?[0];
        c6_ensure!(
            "cyberpunk-edge-label",
            owner.text() == label,
            "wrong label for {id}"
        );
    }
    for edge in by_class(receipt, "path", "flowchart-link", 3)? {
        props(edge, &[("stroke", CYAN)])?;
        number(edge, "stroke-width", 2.0)?;
        glow(receipt, edge, EDGE)?;
        let marker = linked(receipt, edge.attribute("marker-end"), "marker")?;
        let path = receipt
            .descendants_of(marker.index())
            .find(|node| node.tag_name() == "path")
            .ok_or_else(|| C6ProofError::new("cyberpunk-marker", "missing marker shape"))?;
        props(
            path,
            &[
                ("d", "M 0 0 L 10 5 L 0 10 z"),
                ("fill", CYAN),
                ("stroke", CYAN),
            ],
        )?;
    }
    for label in select(receipt, 6, "text glows", |node| {
        node.tag_name() == "g" && node.attribute("filter").is_some()
    })? {
        glow(receipt, label, TEXT)?;
        for text in receipt
            .descendants_of(label.index())
            .filter(|node| node.tag_name() == "text")
        {
            number(text, "font-weight", 600.0)?;
        }
    }
    for text in receipt
        .elements()
        .iter()
        .filter(|node| matches!(node.tag_name(), "text" | "tspan"))
    {
        no_inline_conflicts(text, &[("fill", CYAN)])?;
    }
    Ok(())
}

fn sequence(receipt: &SvgArtifactReceipt) -> C6ProofResult<()> {
    let root = root_id(receipt)?;
    for (suffix, property, value) in [
        (".actor", "fill", NAVY),
        (".actor", "stroke", CYAN),
        (".actor-line", "stroke", CYAN),
        (".actor-line", "stroke-width", "2px"),
        (".note", "fill", NAVY),
        (".note", "stroke", "#ff00ff"),
        (".loopLine", "stroke", CYAN),
        (".loopLine", "stroke-dasharray", "2,2"),
        (".labelBox", "fill", NAVY),
        (".labelBox", "stroke", CYAN),
        (".messageLine0", "stroke-dasharray", "none"),
    ] {
        require_exact_writer_declaration(receipt, &format!("#{root} {suffix}"), property, value)?;
    }
    for (selector, property, value) in [
        (
            format!("#{root} .messageLine0,#{root} .messageLine1"),
            "stroke",
            CYAN,
        ),
        (
            format!("#{root} .activation0,#{root} .activation1,#{root} .activation2"),
            "fill",
            "rgba(0, 242, 255, 0.1)",
        ),
        (
            format!("#{root} .activation0,#{root} .activation1,#{root} .activation2"),
            "stroke",
            CYAN,
        ),
        (
            format!(
                "#{root} text.actor,#{root} text.actor>tspan,#{root} text.text,#{root} text.text>tspan"
            ),
            "fill",
            CYAN,
        ),
        (
            format!("#{root} .messageText,#{root} .messageText>tspan"),
            "fill",
            CYAN,
        ),
        (
            format!("#{root} .noteText,#{root} .noteText>tspan"),
            "fill",
            "#ff00ff",
        ),
        (
            format!(
                "#{root} .loopText,#{root} .loopText>tspan,#{root} .sectionTitle,#{root} .sectionTitle>tspan,#{root} .labelText,#{root} .labelText>tspan"
            ),
            "fill",
            CYAN,
        ),
    ] {
        require_exact_writer_declaration(receipt, &selector, property, value)?;
    }
    for actor in by_class(receipt, "rect", "actor", 4)? {
        no_inline_conflicts(actor, &[("fill", NAVY), ("stroke", CYAN)])?;
        number(actor, "stroke-width", 3.0)?;
        number(actor, "rx", 10.0)?;
        number(actor, "ry", 10.0)?;
        glow(receipt, actor, SHAPE)?;
    }
    for actor in by_class(receipt, "text", "actor", 4)? {
        glow(receipt, actor, SHAPE)?;
    }
    for lifeline in by_class(receipt, "line", "actor-line", 2)? {
        no_inline_conflicts(lifeline, &[("stroke", CYAN), ("stroke-width", "2px")])?;
        glow(receipt, lifeline, EDGE)?;
    }
    let activation = by_class(receipt, "rect", "activation0", 1)?[0];
    no_inline_conflicts(
        activation,
        &[("fill", "rgba(0, 242, 255, 0.1)"), ("stroke", CYAN)],
    )?;
    number(activation, "stroke-width", 3.0)?;
    c6_ensure!(
        "cyberpunk-activation",
        activation.attribute("filter").is_none(),
        "unexpected activation effect"
    );
    let note = by_class(receipt, "rect", "note", 1)?[0];
    no_inline_conflicts(note, &[("fill", NAVY), ("stroke", "#ff00ff")])?;
    number(note, "stroke-width", 2.0)?;
    number(note, "rx", 10.0)?;
    number(note, "ry", 10.0)?;
    glow(receipt, note, &[(8.0, "rgba(255, 0, 255, 0.4)")])?;
    glow(
        receipt,
        by_class(receipt, "text", "noteText", 1)?[0],
        &[(4.0, "rgba(255, 0, 255, 0.4)")],
    )?;
    for frame in by_class(receipt, "line", "loopLine", 4)? {
        no_inline_conflicts(frame, &[("stroke", CYAN), ("stroke-dasharray", "2,2")])?;
        number(frame, "stroke-width", 2.0)?;
        glow(receipt, frame, &[(4.0, "rgba(0, 242, 255, 0.5)")])?;
    }
    let keyword = by_class(receipt, "polygon", "labelBox", 1)?[0];
    no_inline_conflicts(keyword, &[("fill", NAVY), ("stroke", CYAN)])?;
    number(keyword, "stroke-width", 2.0)?;
    glow(receipt, keyword, &[(6.0, "rgba(0, 242, 255, 0.4)")])?;
    for class in ["labelText", "loopText"] {
        glow(receipt, by_class(receipt, "text", class, 1)?[0], TEXT)?;
    }
    for class in ["messageLine0", "messageLine1"] {
        let message = by_class(receipt, "line", class, 1)?[0];
        no_inline_conflicts(message, &[("stroke", CYAN)])?;
        let (from, to) = if class == "messageLine0" {
            ("Client", "API")
        } else {
            ("API", "Client")
        };
        props(message, &[("data-from", from), ("data-to", to)])?;
        number(message, "stroke-width", 2.0)?;
        if class == "messageLine1" {
            props(message, &[("stroke-dasharray", "3, 3")])?;
        }
        glow(receipt, message, EDGE)?;
        let marker = linked(receipt, message.attribute("marker-end"), "marker")?;
        c6_ensure!(
            "cyberpunk-marker",
            marker.id() == Some(format!("{root}-arrowhead").as_str()),
            "wrong Sequence marker"
        );
        let path = receipt
            .descendants_of(marker.index())
            .find(|node| node.tag_name() == "path")
            .ok_or_else(|| {
                C6ProofError::new("cyberpunk-marker", "missing Sequence marker shape")
            })?;
        props(path, &[("d", "M -1 0 L 10 5 L 0 10 z")])?;
        no_inline_conflicts(path, &[("fill", CYAN), ("stroke", CYAN)])?;
    }
    let markers = [
        "arrowhead",
        "crosshead",
        "filled-head",
        "solidTopArrowHead",
        "solidBottomArrowHead",
    ]
    .map(|marker| format!("#{root} [id=\"{root}-{marker}\"] path"))
    .join(",");
    for property in ["fill", "stroke"] {
        require_exact_writer_declaration(receipt, &markers, property, CYAN)?;
    }
    for text in select(receipt, 9, "Sequence labels", |node| {
        node.tag_name() == "text" && !node.text().trim().is_empty()
    })? {
        number(text, "font-weight", 400.0)?;
        if text.has_class("messageText") {
            c6_ensure!(
                "cyberpunk-message-label",
                text.attribute("filter").is_none(),
                "unexpected message text glow"
            );
        }
        let expected = if text.has_class("noteText") {
            "#ff00ff"
        } else {
            CYAN
        };
        for leaf in std::iter::once(text).chain(receipt.descendants_of(text.index())) {
            no_inline_conflicts(leaf, &[("fill", expected)])?;
        }
    }
    Ok(())
}

fn xychart(receipt: &SvgArtifactReceipt) -> C6ProofResult<()> {
    let series = select(receipt, 10, "series order", |node| {
        matches!(node.tag_name(), "rect" | "path") && node.attribute("filter").is_some()
    })?;
    c6_ensure!(
        "cyberpunk-series-order",
        series.iter().map(|node| node.tag_name()).eq([
            "rect", "rect", "rect", "rect", "path", "rect", "rect", "rect", "rect", "path"
        ]),
        "series must preserve bar/line/bar/line declaration order"
    );
    let bars = select(receipt, 8, "bars", |node| {
        node.tag_name() == "rect" && node.attribute("filter").is_some()
    })?;
    for (index, bar) in bars.into_iter().enumerate() {
        let (color, flood) = if index < 4 {
            ("#6cc6cb", "rgba(108, 198, 203, 0.4)")
        } else {
            ("#7ce38b", "rgba(124, 227, 139, 0.4)")
        };
        props(bar, &[("fill", color), ("stroke", color)])?;
        number(bar, "fill-opacity", 0.2)?;
        number(bar, "stroke-width", 2.0)?;
        glow(receipt, bar, &[(8.0, flood)])?;
    }
    let lines = select(receipt, 2, "series lines", |node| {
        node.tag_name() == "path" && node.attribute("filter").is_some()
    })?;
    for (line, (color, flood)) in lines.into_iter().zip([
        ("#c77dff", "rgba(199, 125, 255, 0.5)"),
        ("#6cc6cb", "rgba(108, 198, 203, 0.5)"),
    ]) {
        props(line, &[("fill", "none"), ("stroke", color)])?;
        number(line, "stroke-width", 3.0)?;
        glow(receipt, line, &[(6.0, flood)])?;
    }
    for text in select(receipt, 18, "XY labels", |node| node.tag_name() == "text")? {
        props(text, &[("fill", CYAN)])?;
        let (size, weight, sigma, color) = match text.text() {
            "Request Volume" => (18.0, Some(700.0), 7.5, "rgba(0, 242, 255, 0.8)"),
            "Window" | "Requests" => (13.0, None, 5.0, "rgba(0, 242, 255, 0.6)"),
            _ => (14.0, Some(600.0), 5.0, "rgba(0, 242, 255, 0.5)"),
        };
        number(text, "font-size", size)?;
        if let Some(weight) = weight {
            number(text, "font-weight", weight)?;
        }
        glow(receipt, text, &[(sigma, color)])?;
    }
    for (class, count) in [("ticks", 15), ("axis-line", 1), ("axisl-line", 1)] {
        for path in select(receipt, count, class, |node| {
            node.tag_name() == "path"
                && node
                    .parent_index()
                    .and_then(|index| receipt.element(index))
                    .is_some_and(|parent| parent.has_class(class))
        })? {
            props(path, &[("stroke", CYAN)])?;
            number(path, "stroke-width", 2.0)?;
            if class == "ticks" {
                number(path, "opacity", 0.3)?;
            } else {
                c6_ensure!(
                    "cyberpunk-axis",
                    path.attribute("opacity").is_none(),
                    "axis line must remain opaque"
                );
            }
        }
    }
    Ok(())
}
