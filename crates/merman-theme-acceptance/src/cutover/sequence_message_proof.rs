use super::*;

const EXPECTED_MESSAGE_LINES: [MessageLineWitness; 7] = [
    MessageLineWitness::new("i1", "messageLine0", Some("arrowhead"), Some("arrowhead")),
    MessageLineWitness::new("i2", "messageLine1", None, Some("crosshead")),
    MessageLineWitness::new("i3", "messageLine1", None, Some("filled-head")),
    MessageLineWitness::new("i4", "messageLine0", None, Some("solidTopArrowHead")),
    MessageLineWitness::new("i5", "messageLine0", None, Some("solidBottomArrowHead")),
    MessageLineWitness::new("i6", "messageLine0", None, Some("stickTopArrowHead")),
    MessageLineWitness::new("i7", "messageLine0", None, Some("stickBottomArrowHead")),
];

#[derive(Clone, Copy)]
struct MessageLineWitness {
    data_id: &'static str,
    class: &'static str,
    marker_start: Option<&'static str>,
    marker_end: Option<&'static str>,
}

impl MessageLineWitness {
    const fn new(
        data_id: &'static str,
        class: &'static str,
        marker_start: Option<&'static str>,
        marker_end: Option<&'static str>,
    ) -> Self {
        Self {
            data_id,
            class,
            marker_start,
            marker_end,
        }
    }
}

pub(super) fn prove_sequence_message_svg(
    case: CutoverCase,
    routes: &[ThemeRouteCutoverDescriptor],
    svg: &str,
) -> C6ProofResult<SvgCutoverProof> {
    let route = routes.first().copied().ok_or_else(|| {
        C6ProofError::new("route-svg-proof", "Sequence Message case has no route")
    })?;
    c6_ensure!(
        "route-svg-proof",
        routes.len() == 1
            && route == case.id.route()
            && route.family_id() == DiagramFamilyId::SEQUENCE
            && route.target() == ThemeTarget::Message
            && route.facet() == ThemeRouteCutoverFacet::Stroke,
        "Sequence Message proof received a non-canonical route set"
    );

    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("route-svg-parse", error.to_string()))?;
    let root = document.root_element();
    let root_id = root
        .attribute("id")
        .ok_or_else(|| C6ProofError::new("route-svg-proof", "Sequence SVG root lacks an id"))?;
    let view_box = parse_view_box(root.attribute("viewBox").ok_or_else(|| {
        C6ProofError::new("route-svg-proof", "Sequence SVG root lacks a viewBox")
    })?)?;
    let expected_value = expected_svg_value(route)?;

    let line_selector = format!("#{root_id} .messageLine0,#{root_id} .messageLine1");
    let line_value = stylesheet_property(&document, &line_selector, "stroke")?;
    c6_ensure!(
        "route-svg-proof",
        line_value == expected_value,
        "Sequence Message lines emitted `{line_value}`, expected `{expected_value}`"
    );

    let closed_marker_selector = format!(
        "#{root_id} [id$=\"-arrowhead\"] path,#{root_id} [id$=\"-crosshead\"] path,#{root_id} [id$=\"-filled-head\"] path,#{root_id} [id$=\"-solidTopArrowHead\"] path,#{root_id} [id$=\"-solidBottomArrowHead\"] path"
    );
    for property in ["fill", "stroke"] {
        let value = stylesheet_property(&document, &closed_marker_selector, property)?;
        c6_ensure!(
            "route-svg-proof",
            value == expected_value,
            "Sequence closed Message markers emitted `{value}` for {property}, expected `{expected_value}`"
        );
    }
    let stick_marker_selector = format!(
        "#{root_id} [id$=\"-stickTopArrowHead\"] path,#{root_id} [id$=\"-stickBottomArrowHead\"] path"
    );
    let stick_value = stylesheet_property(&document, &stick_marker_selector, "stroke")?;
    c6_ensure!(
        "route-svg-proof",
        stick_value == expected_value,
        "Sequence stick Message markers emitted `{stick_value}`, expected `{expected_value}`"
    );
    let sequence_number_selector = format!("#{root_id} [id$=\"-sequencenumber\"]");
    let sequence_number_value = stylesheet_property(&document, &sequence_number_selector, "fill")?;
    c6_ensure!(
        "route-svg-proof",
        sequence_number_value == expected_value,
        "Sequence number marker emitted `{sequence_number_value}`, expected `{expected_value}`"
    );

    let message_lines = document
        .descendants()
        .filter(|node| {
            node.attribute("data-et") == Some("message")
                && (class_contains(*node, "messageLine0") || class_contains(*node, "messageLine1"))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        message_lines.len() == EXPECTED_MESSAGE_LINES.len(),
        "Sequence Message witness emitted {} lines, expected {}",
        message_lines.len(),
        EXPECTED_MESSAGE_LINES.len()
    );

    let mut terminal = b"merman.c6-route-sequence-message-surfaces.v1\0".to_vec();
    let mut target_regions = Vec::with_capacity(message_lines.len());
    for (line, expected) in message_lines.iter().copied().zip(EXPECTED_MESSAGE_LINES) {
        c6_ensure!(
            "route-svg-proof",
            line.has_tag_name("line")
                && line.attribute("data-id") == Some(expected.data_id)
                && class_contains(line, expected.class)
                && line.attribute("stroke") == Some("none")
                && style_value(line, "fill") == Some("none"),
            "Sequence Message {} did not retain its expected line surface",
            expected.data_id
        );
        let marker_start = marker_reference(line, "marker-start")?;
        let marker_end = marker_reference(line, "marker-end")?;
        prove_expected_marker_reference(
            &document,
            root_id,
            marker_start.as_deref(),
            expected.marker_start,
        )?;
        prove_expected_marker_reference(
            &document,
            root_id,
            marker_end.as_deref(),
            expected.marker_end,
        )?;

        let region = middle_half_line_region(line)?;
        append_len_prefixed(&mut terminal, expected.data_id.as_bytes());
        append_len_prefixed(&mut terminal, expected.class.as_bytes());
        append_len_prefixed(
            &mut terminal,
            marker_start.as_deref().unwrap_or_default().as_bytes(),
        );
        append_len_prefixed(
            &mut terminal,
            marker_end.as_deref().unwrap_or_default().as_bytes(),
        );
        append_rect(&mut terminal, region);
        target_regions.push(region);
    }

    let sequence_number_marker_id = format!("{root_id}-sequencenumber");
    let sequence_number_marker_url = format!("url(#{sequence_number_marker_id})");
    let sequence_number_marker = marker_by_id(&document, &sequence_number_marker_id)?;
    c6_ensure!(
        "route-svg-proof",
        sequence_number_marker
            .children()
            .any(|child| child.has_tag_name("circle")),
        "Sequence marker {sequence_number_marker_id} lacks its expected circle child"
    );
    let sequence_number_carriers = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node.attribute("stroke-width") == Some("0")
                && node.attribute("marker-start") == Some(sequence_number_marker_url.as_str())
        })
        .count();
    let sequence_number_labels = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && class_contains(*node, "sequenceNumber"))
        .count();
    c6_ensure!(
        "route-svg-proof",
        sequence_number_carriers == EXPECTED_MESSAGE_LINES.len()
            && sequence_number_labels == EXPECTED_MESSAGE_LINES.len(),
        "Sequence Message witness did not emit one sequence number marker and label per line"
    );

    append_len_prefixed(&mut terminal, sequence_number_marker_id.as_bytes());
    append_len_prefixed(&mut terminal, b"circle");
    append_len_prefixed(&mut terminal, line_selector.as_bytes());
    append_len_prefixed(&mut terminal, line_value.as_bytes());
    append_len_prefixed(&mut terminal, closed_marker_selector.as_bytes());
    append_len_prefixed(&mut terminal, stick_marker_selector.as_bytes());
    append_len_prefixed(&mut terminal, sequence_number_selector.as_bytes());
    terminal.extend_from_slice(&usize_to_u64(sequence_number_carriers).to_be_bytes());
    terminal.extend_from_slice(&usize_to_u64(sequence_number_labels).to_be_bytes());

    let terminal_digest = sha256(terminal);
    let mut assertion = b"merman.c6-route-sequence-message-svg-assertion.v1\0".to_vec();
    append_witness(&mut assertion, case.id);
    append_len_prefixed(&mut assertion, expected_value.as_bytes());
    assertion.extend_from_slice(&terminal_digest);
    let assertions = BTreeMap::from([(route, sha256(assertion))]);
    let target_underlay_colors = vec![Vec::new(); target_regions.len()];

    Ok(SvgCutoverProof {
        assertions,
        view_box,
        target_regions,
        target_underlay_colors,
    })
}

fn stylesheet_property<'a>(
    document: &'a roxmltree::Document<'a>,
    selector: &str,
    property: &str,
) -> C6ProofResult<&'a str> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .filter_map(|css| css_last_property(css, selector, property))
        .next_back()
        .ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Sequence stylesheet lacks {selector} {property}"),
            )
        })
}

fn marker_reference(
    node: roxmltree::Node<'_, '_>,
    attribute: &str,
) -> C6ProofResult<Option<String>> {
    let Some(raw) = node.attribute(attribute) else {
        return Ok(None);
    };
    let marker = raw
        .strip_prefix("url(#")
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Sequence Message line has invalid {attribute} `{raw}`"),
            )
        })?;
    Ok(Some(marker.to_owned()))
}

fn prove_expected_marker_reference(
    document: &roxmltree::Document<'_>,
    root_id: &str,
    actual: Option<&str>,
    expected_suffix: Option<&str>,
) -> C6ProofResult<()> {
    let expected = expected_suffix.map(|suffix| format!("{root_id}-{suffix}"));
    c6_ensure!(
        "route-svg-proof",
        actual == expected.as_deref(),
        "Sequence Message marker reference differs: actual={actual:?}, expected={expected:?}"
    );
    if let Some(marker_id) = actual {
        let marker = marker_by_id(document, marker_id)?;
        c6_ensure!(
            "route-svg-proof",
            marker.children().any(|child| child.has_tag_name("path")),
            "Sequence Message marker {marker_id} lacks its expected path child"
        );
    }
    Ok(())
}

fn marker_by_id<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    marker_id: &str,
) -> C6ProofResult<roxmltree::Node<'a, 'input>> {
    document
        .descendants()
        .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
        .ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Sequence marker reference #{marker_id} does not resolve"),
            )
        })
}

fn middle_half_line_region(line: roxmltree::Node<'_, '_>) -> C6ProofResult<[f64; 4]> {
    let x1 = number_attribute(line, "x1")?;
    let y1 = number_attribute(line, "y1")?;
    let x2 = number_attribute(line, "x2")?;
    let y2 = number_attribute(line, "y2")?;
    let width = (x2 - x1).abs();
    c6_ensure!(
        "route-svg-proof",
        width > 0.0 && (y2 - y1).abs() <= 1e-6,
        "Sequence Message proof requires a non-empty horizontal line"
    );
    Ok([x1.min(x2) + width * 0.25, y1 - 1.0, width * 0.5, 2.0])
}

fn number_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> C6ProofResult<f64> {
    let value = node
        .attribute(name)
        .ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Sequence Message line lacks {name}"),
            )
        })?
        .parse::<f64>()
        .map_err(|error| C6ProofError::new("route-svg-proof", error.to_string()))?;
    c6_ensure!(
        "route-svg-proof",
        value.is_finite(),
        "Sequence Message line has non-finite {name}"
    );
    Ok(value)
}

fn parse_view_box(raw: &str) -> C6ProofResult<[f64; 4]> {
    let values = raw
        .split_ascii_whitespace()
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| C6ProofError::new("route-svg-proof", error.to_string()))?;
    c6_ensure!(
        "route-svg-proof",
        values.len() == 4
            && values.iter().all(|value| value.is_finite())
            && values[2] > 0.0
            && values[3] > 0.0,
        "Sequence SVG viewBox is invalid: `{raw}`"
    );
    Ok([values[0], values[1], values[2], values[3]])
}
