use crate::svg_observation::{SvgArtifactObservation, SvgElementObservation};
use merman::model::family::DiagramFamilyId;

use crate::runner::{
    C6ProofError, C6ProofResult,
    artifact_observation::{local_fragment_id, percent_value},
    c6_sequence_proof::require_exact_writer_declaration,
};

const CYAN: &str = "#53DCE4";
const MAGENTA: &str = "#F28BCB";
const NAVY: &str = "#0B1020";
const SURFACE: &str = "#151E35";
const SURFACE_ALT: &str = "#202B46";
const PRIMARY_TEXT: &str = "#E2EFF7";
const MUTED_TEXT: &str = "#92A9BF";
const SHAPE: &[(f64, &str)] = &[
    (2.0, "rgba(83, 220, 228, 0.2)"),
    (5.0, "rgba(83, 220, 228, 0.1)"),
];
const EDGE: &[(f64, &str)] = &[(1.5, "rgba(83, 220, 228, 0.16)")];

pub(super) fn verify(
    family: DiagramFamilyId,
    receipt: &SvgArtifactObservation,
) -> C6ProofResult<()> {
    c6_ensure!(
        "cyberpunk-native-text",
        receipt.has_native_text() && !receipt.has_foreign_object(),
        "complete profile requires native text"
    );
    canvas(receipt)?;
    for label in receipt
        .elements()
        .iter()
        .filter(|node| matches!(node.tag_name(), "text" | "tspan"))
    {
        no_glow(receipt, label)?;
    }
    match family {
        DiagramFamilyId::FLOWCHART => flowchart(receipt),
        DiagramFamilyId::SEQUENCE => sequence(receipt),
        DiagramFamilyId::XY_CHART => xychart(receipt),
        _ => Err(C6ProofError::new("cyberpunk-scope", "undeclared family")),
    }
}

fn select<'a>(
    receipt: &'a SvgArtifactObservation,
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

fn no_glow(
    receipt: &SvgArtifactObservation,
    terminal: &SvgElementObservation,
) -> C6ProofResult<()> {
    let mut current = Some(terminal);
    while let Some(node) = current {
        for property in ["filter", "text-shadow"] {
            c6_ensure!(
                "cyberpunk-crisp-label",
                [node.attribute(property), node.style_value(property)]
                    .into_iter()
                    .flatten()
                    .all(|value| value == "none"),
                "{} and its ancestors must keep {property} disabled",
                terminal.tag_name()
            );
        }
        current = node.parent_index().and_then(|index| receipt.element(index));
    }
    Ok(())
}

fn by_class<'a>(
    receipt: &'a SvgArtifactObservation,
    tag: &str,
    class: &str,
    count: usize,
) -> C6ProofResult<Vec<&'a SvgElementObservation>> {
    select(receipt, count, class, |node| {
        node.tag_name() == tag && node.has_class(class)
    })
}

fn root_id(receipt: &SvgArtifactObservation) -> C6ProofResult<&str> {
    receipt
        .root_id()
        .ok_or_else(|| C6ProofError::new("cyberpunk-root", "missing root id"))
}

fn linked<'a>(
    receipt: &'a SvgArtifactObservation,
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
    receipt: &SvgArtifactObservation,
    terminal: &SvgElementObservation,
    shadows: &[(f64, &str)],
) -> C6ProofResult<()> {
    check_glow(receipt, terminal, shadows, false)
}

fn check_glow(
    receipt: &SvgArtifactObservation,
    terminal: &SvgElementObservation,
    shadows: &[(f64, &str)],
    depth_only: bool,
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
        nodes.len()
            == if depth_only {
                shadows.len() * 7 - 2
            } else {
                shadows.len() * 6
            },
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
    let mut offset = 0;
    let mut previous_result = "SourceGraphic";
    for (index, &(sigma, color)) in shadows.iter().enumerate() {
        let last_depth_stage = depth_only && index + 1 == shadows.len();
        let block_len = if last_depth_stage { 3 } else { 6 };
        let block = &nodes[offset..offset + block_len];
        offset += block_len;
        c6_ensure!(
            "cyberpunk-filter",
            block.iter().map(|node| node.tag_name()).eq([
                "feGaussianBlur",
                "feFlood",
                "feComposite",
                "feMerge",
                "feMergeNode",
                "feMergeNode"
            ][..block_len]
                .iter()
                .copied()),
            "unexpected filter primitive order"
        );
        let blur = filter_result(block[0])?;
        let flood = filter_result(block[1])?;
        let shadow = filter_result(block[2])?;
        props(block[0], &[("in", previous_result)])?;
        number(block[0], "stdDeviation", sigma)?;
        props(block[1], &[("flood-color", color)])?;
        if block[1].attribute("flood-opacity").is_some() {
            number(block[1], "flood-opacity", 1.0)?;
        }
        props(
            block[2],
            &[("in", flood), ("in2", blur), ("operator", "in")],
        )?;
        if !last_depth_stage {
            props(block[4], &[("in", shadow)])?;
            props(block[5], &[("in", previous_result)])?;
            previous_result = filter_result(block[3])?;
        }
    }
    if depth_only {
        let merge = nodes[offset];
        c6_ensure!(
            "cyberpunk-filter",
            merge.tag_name() == "feMerge",
            "missing final depth merge"
        );
        filter_result(merge)?;
        let shadows = nodes
            .iter()
            .filter(|node| node.tag_name() == "feComposite")
            .rev();
        for (node, shadow) in nodes[offset + 1..].iter().zip(shadows) {
            c6_ensure!(
                "cyberpunk-filter",
                node.tag_name() == "feMergeNode" && node.parent_index() == Some(merge.index()),
                "unexpected final depth input"
            );
            props(node, &[("in", filter_result(shadow)?)])?;
        }
    }
    Ok(())
}

fn body_depth(
    receipt: &SvgArtifactObservation,
    face: &SvgElementObservation,
    depth: &SvgElementObservation,
) -> C6ProofResult<()> {
    c6_ensure!(
        "cyberpunk-depth",
        face.attribute("filter").is_none()
            && face
                .style_value("filter")
                .is_none_or(|value| value == "none"),
        "the foreground body must not repaint depth"
    );
    c6_ensure!(
        "cyberpunk-depth",
        face.tag_name() == depth.tag_name()
            && [
                "x",
                "y",
                "width",
                "height",
                "rx",
                "ry",
                "points",
                "d",
                "transform"
            ]
            .iter()
            .all(|attribute| face.attribute(attribute) == depth.attribute(attribute)),
        "depth must reuse its body's geometry"
    );
    for property in ["fill", "stroke", "stroke-width", "rx", "ry"] {
        c6_ensure!(
            "cyberpunk-depth",
            face.style_value(property) == depth.style_value(property),
            "depth must reuse its body's {property}"
        );
    }
    let owner = depth
        .parent_index()
        .and_then(|index| receipt.element(index))
        .ok_or_else(|| C6ProofError::new("cyberpunk-depth", "missing depth owner"))?;
    props(
        owner,
        &[("aria-hidden", "true"), ("pointer-events", "none")],
    )?;
    c6_ensure!(
        "cyberpunk-depth",
        !receipt.descendants_of(owner.index()).any(|node| {
            matches!(node.tag_name(), "text" | "foreignObject" | "use")
                || node.id().is_some()
                || node.attribute("name").is_some()
        }),
        "depth must contain only the body silhouette"
    );
    check_glow(receipt, depth, SHAPE, true)
}

fn filter_result(node: &SvgElementObservation) -> C6ProofResult<&str> {
    node.attribute("result")
        .filter(|name| !name.is_empty() && *name != "SourceGraphic")
        .ok_or_else(|| C6ProofError::new("cyberpunk-filter", "missing or reserved filter result"))
}

fn canvas(receipt: &SvgArtifactObservation) -> C6ProofResult<()> {
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
            props(pattern, &[("patternUnits", "userSpaceOnUse")])?;
            if x == 0.0 && y == 0.0 {
                c6_ensure!(
                    "cyberpunk-canvas",
                    pattern.attribute("patternTransform").is_none(),
                    "origin-aligned tiles must omit the identity transform"
                );
            } else {
                props(
                    pattern,
                    &[("patternTransform", &format!("translate({x} {y})"))],
                )?;
            }
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
                (0.0, "rgba(83, 220, 228, 0.025)"),
                (70.0, "rgba(83, 220, 228, 0)"),
            ]
        } else {
            &[
                (0.0, "rgba(83, 220, 228, 0.018)"),
                (2.5, "rgba(83, 220, 228, 0.018)"),
                (2.5, "rgba(83, 220, 228, 0)"),
                (100.0, "rgba(83, 220, 228, 0)"),
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

fn flowchart(receipt: &SvgArtifactObservation) -> C6ProofResult<()> {
    let root = root_id(receipt)?;
    for (selector, fill) in [
        (format!("#{root} .label text,#{root} span"), "#28253D"),
        (format!("#{root} .cluster-label text"), PRIMARY_TEXT),
        (
            format!(
                "#{root} .edgeLabel .label,#{root} .edgeLabel .label text,#{root} .edgeLabel span"
            ),
            "#28253D",
        ),
    ] {
        require_exact_writer_declaration(receipt, &selector, "fill", fill)?;
    }
    for (property, value) in [("fill", NAVY), ("opacity", "1")] {
        require_exact_writer_declaration(
            receipt,
            &format!("#{root} .edgeLabel rect"),
            property,
            value,
        )?;
    }
    let depths = select(receipt, 4, "node depth", |node| {
        node.attribute("data-merman-theme-depth") == Some("node")
    })?;
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
        props(shape, &[("fill", SURFACE), ("stroke", CYAN)])?;
        number(shape, "stroke-width", 1.5)?;
        if tag == "rect" {
            number(shape, "rx", 4.0)?;
            number(shape, "ry", 4.0)?;
        }
        let depth_owner = depths
            .iter()
            .find(|depth| depth.attribute("transform") == owner.attribute("transform"))
            .ok_or_else(|| C6ProofError::new("cyberpunk-depth", "missing matching node depth"))?;
        let depth = select(receipt, 1, "node silhouette", |node| {
            node.parent_index() == Some(depth_owner.index()) && node.has_class("label-container")
        })?[0];
        body_depth(receipt, shape, depth)?;
        let label = receipt
            .descendants_of(owner.index())
            .find(|node| node.tag_name() == "text" && node.text() == text)
            .ok_or_else(|| C6ProofError::new("cyberpunk-labels", format!("missing {text}")))?;
        props(label, &[("fill", PRIMARY_TEXT)])?;
        number(label, "font-weight", 600.0)?;
    }
    let cluster = select(receipt, 1, "Checkout", |node| {
        node.attribute("data-et") == Some("cluster")
            && node.attribute("data-id") == Some("Checkout")
    })?[0];
    let surface = select(receipt, 1, "cluster surface", |node| {
        node.tag_name() == "rect"
            && node
                .parent_index()
                .and_then(|index| receipt.element(index))
                .is_some_and(|parent| {
                    parent.has_class("cluster")
                        && parent.attribute("data-merman-theme-depth") == Some("container")
                })
    })?[0];
    props(surface, &[("fill", "#10182B"), ("stroke", CYAN)])?;
    c6_ensure!(
        "cyberpunk-depth-order",
        surface.index() < cluster.index(),
        "cluster paint must precede its title"
    );
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
        c6_ensure!(
            "cyberpunk-depth-order",
            depths.iter().all(|depth| depth.index() < edge.index()),
            "all node depth must precede connections"
        );
        props(edge, &[("stroke", CYAN)])?;
        number(edge, "stroke-width", 1.25)?;
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
    for label in select(receipt, 6, "styled labels", |node| {
        node.tag_name() == "g" && node.has_class("label") && !node.text().trim().is_empty()
    })? {
        no_inline_conflicts(label, &[("fill", PRIMARY_TEXT), ("color", PRIMARY_TEXT)])?;
        let weight = if label.attribute("data-id").is_some() {
            500.0
        } else {
            600.0
        };
        for text in receipt
            .descendants_of(label.index())
            .filter(|node| node.tag_name() == "text")
        {
            props(text, &[("fill", PRIMARY_TEXT)])?;
            number(text, "font-weight", weight)?;
        }
    }
    for text in receipt
        .elements()
        .iter()
        .filter(|node| matches!(node.tag_name(), "text" | "tspan"))
    {
        no_inline_conflicts(text, &[("fill", PRIMARY_TEXT)])?;
    }
    Ok(())
}

fn sequence(receipt: &SvgArtifactObservation) -> C6ProofResult<()> {
    let root = root_id(receipt)?;
    for (suffix, property, value) in [
        (".actor", "fill", SURFACE),
        (".actor", "stroke", CYAN),
        (".actor-line", "stroke", CYAN),
        (".actor-line", "stroke-width", "1.25px"),
        (".note", "fill", SURFACE_ALT),
        (".note", "stroke", MAGENTA),
        (".loopLine", "stroke", CYAN),
        (".loopLine", "stroke-dasharray", "2,2"),
        (".labelBox", "fill", SURFACE_ALT),
        (".labelBox", "stroke", CYAN),
        (".labelBox", "filter", "none"),
        (".messageLine0", "stroke-width", "1.25px"),
        (".messageLine1", "stroke-width", "1.25px"),
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
            SURFACE_ALT,
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
            PRIMARY_TEXT,
        ),
        (
            format!("#{root} .messageText,#{root} .messageText>tspan"),
            "fill",
            PRIMARY_TEXT,
        ),
        (
            format!("#{root} .noteText,#{root} .noteText>tspan"),
            "fill",
            PRIMARY_TEXT,
        ),
        (
            format!(
                "#{root} .loopText,#{root} .loopText>tspan,#{root} .sectionTitle,#{root} .sectionTitle>tspan,#{root} .labelText,#{root} .labelText>tspan"
            ),
            "fill",
            PRIMARY_TEXT,
        ),
    ] {
        require_exact_writer_declaration(receipt, &selector, property, value)?;
    }
    let depths = by_class(receipt, "rect", "actor-depth", 4)?;
    for actor in select(receipt, 4, "actor faces", |node| {
        node.tag_name() == "rect" && node.has_class("actor") && !node.has_class("actor-depth")
    })? {
        no_inline_conflicts(actor, &[("fill", SURFACE), ("stroke", CYAN)])?;
        number(actor, "stroke-width", 1.5)?;
        number(actor, "rx", 4.0)?;
        number(actor, "ry", 4.0)?;
        let depth = depths
            .iter()
            .find(|depth| depth.bounds() == actor.bounds())
            .ok_or_else(|| C6ProofError::new("cyberpunk-depth", "missing matching actor depth"))?;
        c6_ensure!(
            "cyberpunk-depth",
            depth
                .parent_index()
                .and_then(|index| receipt.element(index))
                .is_some_and(|parent| parent.attribute("data-merman-theme-depth") == Some("actor")),
            "missing actor depth owner"
        );
        body_depth(receipt, actor, depth)?;
    }
    for actor in by_class(receipt, "text", "actor", 4)? {
        no_glow(receipt, actor)?;
        number(actor, "font-weight", 600.0)?;
    }
    for lifeline in by_class(receipt, "line", "actor-line", 2)? {
        c6_ensure!(
            "cyberpunk-depth-order",
            depths.iter().all(|depth| depth.index() < lifeline.index()),
            "all actor depth must precede lifelines"
        );
        no_inline_conflicts(lifeline, &[("stroke", CYAN), ("stroke-width", "1.25px")])?;
        glow(receipt, lifeline, EDGE)?;
    }
    let activation = by_class(receipt, "rect", "activation0", 1)?[0];
    no_inline_conflicts(activation, &[("fill", SURFACE_ALT), ("stroke", CYAN)])?;
    number(activation, "stroke-width", 1.5)?;
    c6_ensure!(
        "cyberpunk-activation",
        activation.attribute("filter").is_none(),
        "unexpected activation effect"
    );
    let note = by_class(receipt, "rect", "note", 1)?[0];
    no_inline_conflicts(note, &[("fill", SURFACE_ALT), ("stroke", MAGENTA)])?;
    number(note, "stroke-width", 1.25)?;
    number(note, "rx", 4.0)?;
    number(note, "ry", 4.0)?;
    glow(receipt, note, &[(2.5, "rgba(242, 139, 203, 0.12)")])?;
    no_glow(receipt, by_class(receipt, "text", "noteText", 1)?[0])?;
    for frame in by_class(receipt, "line", "loopLine", 4)? {
        no_inline_conflicts(frame, &[("stroke", CYAN), ("stroke-dasharray", "2,2")])?;
        number(frame, "stroke-width", 1.25)?;
        glow(receipt, frame, &[(1.5, "rgba(83, 220, 228, 0.12)")])?;
    }
    let keyword = by_class(receipt, "polygon", "labelBox", 1)?[0];
    no_inline_conflicts(keyword, &[("fill", SURFACE_ALT), ("stroke", CYAN)])?;
    number(keyword, "stroke-width", 1.25)?;
    no_glow(receipt, keyword)?;
    for class in ["labelText", "loopText"] {
        no_glow(receipt, by_class(receipt, "text", class, 1)?[0])?;
    }
    for class in ["messageLine0", "messageLine1"] {
        let message = by_class(receipt, "line", class, 1)?[0];
        no_inline_conflicts(message, &[("stroke", CYAN), ("stroke-width", "1.25px")])?;
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
        number(
            text,
            "font-weight",
            if text.has_class("actor") {
                600.0
            } else {
                500.0
            },
        )?;
        no_glow(receipt, text)?;
        for leaf in std::iter::once(text).chain(receipt.descendants_of(text.index())) {
            no_inline_conflicts(leaf, &[("fill", PRIMARY_TEXT)])?;
        }
    }
    Ok(())
}

fn xychart(receipt: &SvgArtifactObservation) -> C6ProofResult<()> {
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
            (CYAN, "rgba(83, 220, 228, 0.12)")
        } else {
            ("#E8C078", "rgba(232, 192, 120, 0.12)")
        };
        props(bar, &[("fill", color), ("stroke", color)])?;
        number(bar, "fill-opacity", 0.35)?;
        number(bar, "stroke-width", 1.25)?;
        glow(receipt, bar, &[(2.0, flood)])?;
    }
    let lines = select(receipt, 2, "series lines", |node| {
        node.tag_name() == "path" && node.attribute("filter").is_some()
    })?;
    for (line, (color, flood)) in lines.into_iter().zip([
        (MAGENTA, "rgba(242, 139, 203, 0.16)"),
        (CYAN, "rgba(83, 220, 228, 0.16)"),
    ]) {
        props(line, &[("fill", "none"), ("stroke", color)])?;
        number(line, "stroke-width", 2.0)?;
        glow(receipt, line, &[(1.5, flood)])?;
    }
    for text in select(receipt, 18, "XY labels", |node| node.tag_name() == "text")? {
        let (size, weight, color) = match text.text() {
            "Request Volume" => (18.0, 600.0, PRIMARY_TEXT),
            "Window" | "Requests" => (13.0, 500.0, MUTED_TEXT),
            _ => (14.0, 500.0, MUTED_TEXT),
        };
        props(text, &[("fill", color)])?;
        number(text, "font-size", size)?;
        number(text, "font-weight", weight)?;
        no_glow(receipt, text)?;
    }
    for (class, count) in [("ticks", 15), ("axis-line", 1), ("axisl-line", 1)] {
        for path in select(receipt, count, class, |node| {
            node.tag_name() == "path"
                && node
                    .parent_index()
                    .and_then(|index| receipt.element(index))
                    .is_some_and(|parent| parent.has_class(class))
        })? {
            number(path, "stroke-width", 2.0)?;
            if class == "ticks" {
                props(path, &[("stroke", MUTED_TEXT)])?;
                number(path, "opacity", 0.35)?;
            } else {
                props(path, &[("stroke", CYAN)])?;
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
