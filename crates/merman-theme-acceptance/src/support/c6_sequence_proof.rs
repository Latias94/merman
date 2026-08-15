use std::collections::BTreeMap;

use merman_export::RasterPlan;
use merman_theme_fixtures::ReferenceThemeMechanism;

use crate::observation::C6ObservedMechanismDisposition;

use super::{
    BrutalistSequenceFixtureContract, C6ProofError, C6ProofResult, C6RasterImage, class_contains,
    decode_bounded_png_artifact,
};

const CELL_MECHANISMS: [ReferenceThemeMechanism; 2] = [
    ReferenceThemeMechanism::StrokeStyling,
    ReferenceThemeMechanism::ThemeVariables,
];

#[derive(Clone, Debug)]
pub(crate) struct BrutalistSequenceSvgProof {
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    view_box: [f64; 4],
    actor_fill_region: [f64; 4],
    note_fill_region: [f64; 4],
    message_stroke_region: [f64; 4],
}

impl BrutalistSequenceSvgProof {
    pub(crate) fn mechanisms(
        &self,
    ) -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
        self.mechanisms.clone()
    }
}

pub(crate) fn prove_brutalist_sequence_svg(
    contract: &BrutalistSequenceFixtureContract<'_>,
    svg: &str,
) -> C6ProofResult<BrutalistSequenceSvgProof> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("sequence-svg-parse", error.to_string()))?;
    let root = document.root_element();
    let root_id = root
        .attribute("id")
        .ok_or_else(|| C6ProofError::new("sequence-svg-root", "Sequence SVG root lacks an id"))?;
    let view_box = parse_view_box(root.attribute("viewBox").ok_or_else(|| {
        C6ProofError::new("sequence-svg-root", "Sequence SVG root lacks a viewBox")
    })?)?;

    let actor_selector = format!("#{root_id} .actor");
    require_stylesheet_property(&document, &actor_selector, "fill", contract.actor_fill())?;
    require_stylesheet_property(&document, &actor_selector, "stroke", contract.stroke())?;
    let note_selector = format!("#{root_id} .note");
    require_stylesheet_property(&document, &note_selector, "fill", contract.note_fill())?;
    require_stylesheet_property(&document, &note_selector, "stroke", contract.stroke())?;
    let activation_selector =
        format!("#{root_id} .activation0,#{root_id} .activation1,#{root_id} .activation2");
    require_stylesheet_property(
        &document,
        &activation_selector,
        "fill",
        contract.actor_fill(),
    )?;
    require_stylesheet_property(&document, &activation_selector, "stroke", contract.stroke())?;
    let message_selector = format!("#{root_id} .messageLine0,#{root_id} .messageLine1");
    require_stylesheet_property(&document, &message_selector, "stroke", contract.stroke())?;

    let actor_rects = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && class_contains(*node, "actor")
                && (class_contains(*node, "actor-top") || class_contains(*node, "actor-bottom"))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-actors",
        actor_rects.len() == 2,
        "Brutalist Sequence requires the regular participant's top and bottom actor rects; got {}",
        actor_rects.len()
    );
    c6_ensure!(
        "sequence-svg-actors",
        document.descendants().any(|node| {
            (node.has_tag_name("circle") || node.has_tag_name("line"))
                && node
                    .ancestors()
                    .any(|ancestor| class_contains(ancestor, "actor-man"))
        }),
        "Brutalist Sequence actor source did not retain an actor-man terminal surface"
    );

    let notes = document
        .descendants()
        .filter(|node| node.has_tag_name("rect") && class_contains(*node, "note"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-note",
        notes.len() == 1,
        "Brutalist Sequence requires one terminal note rect; got {}",
        notes.len()
    );
    let activations = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && ["activation0", "activation1", "activation2"]
                    .into_iter()
                    .any(|class| class_contains(*node, class))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-activation",
        activations.len() == 1,
        "Brutalist Sequence requires one terminal activation rect; got {}",
        activations.len()
    );

    let message_lines = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node.attribute("data-et") == Some("message")
                && (class_contains(*node, "messageLine0") || class_contains(*node, "messageLine1"))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-messages",
        message_lines.len() == 3
            && message_lines
                .iter()
                .filter(|node| class_contains(**node, "messageLine0"))
                .count()
                == 1
            && message_lines
                .iter()
                .filter(|node| class_contains(**node, "messageLine1"))
                .count()
                == 2,
        "Brutalist Sequence requires one solid and two dotted terminal message lines"
    );
    c6_ensure!(
        "sequence-svg-messages",
        message_lines.iter().all(|line| {
            line.attribute("marker-end")
                .is_some_and(|value| value.starts_with("url(#") && value.ends_with("-arrowhead)"))
        }),
        "Sequence message lines do not retain their scoped terminal marker references"
    );
    c6_ensure!(
        "sequence-svg-text",
        document.descendants().any(|node| node.has_tag_name("text"))
            && !document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
            && !svg.contains("merman-prepared-"),
        "Brutalist Sequence did not retain terminal native SVG text"
    );

    Ok(BrutalistSequenceSvgProof {
        mechanisms: applied_cell_mechanisms(),
        view_box,
        actor_fill_region: corner_fill_region(actor_rects[0])?,
        note_fill_region: corner_fill_region(notes[0])?,
        message_stroke_region: middle_line_region(message_lines[0])?,
    })
}

pub(crate) fn prove_brutalist_sequence_png(
    contract: &BrutalistSequenceFixtureContract<'_>,
    svg_proof: &BrutalistSequenceSvgProof,
    bytes: &[u8],
    plan: RasterPlan,
) -> C6ProofResult<()> {
    let raster = decode_bounded_png_artifact(bytes, plan)?;
    prove_roi_color(
        &raster,
        svg_proof.view_box,
        svg_proof.actor_fill_region,
        parse_hex_rgb(contract.actor_fill())?,
        24,
        "sequence-png-actor-fill",
    )?;
    prove_roi_color(
        &raster,
        svg_proof.view_box,
        svg_proof.note_fill_region,
        parse_hex_rgb(contract.note_fill())?,
        24,
        "sequence-png-note-fill",
    )?;
    prove_roi_color(
        &raster,
        svg_proof.view_box,
        svg_proof.message_stroke_region,
        parse_hex_rgb(contract.stroke())?,
        4,
        "sequence-png-message-stroke",
    )?;
    Ok(())
}

fn prove_roi_color(
    raster: &C6RasterImage,
    view_box: [f64; 4],
    region: [f64; 4],
    color: [u8; 3],
    minimum_pixels: usize,
    stage: &'static str,
) -> C6ProofResult<()> {
    let count = raster
        .count_opaque_pixels_near_in_svg_rect(view_box, region, color, 10)
        .ok_or_else(|| C6ProofError::new(stage, "invalid Sequence PNG proof region"))?;
    c6_ensure!(
        stage,
        count >= minimum_pixels,
        "Sequence PNG ROI retained {count} matching pixels, expected at least {minimum_pixels}"
    );
    Ok(())
}

pub(crate) fn applied_cell_mechanisms()
-> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
    CELL_MECHANISMS
        .into_iter()
        .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
        .collect()
}

fn require_stylesheet_property(
    document: &roxmltree::Document<'_>,
    selector: &str,
    property: &str,
    expected: &str,
) -> C6ProofResult<()> {
    let actual = document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .filter_map(|css| css_last_property(css, selector, property))
        .next_back()
        .ok_or_else(|| {
            C6ProofError::new(
                "sequence-svg-stylesheet",
                format!("Sequence stylesheet lacks `{selector}` {property}"),
            )
        })?;
    c6_ensure!(
        "sequence-svg-stylesheet",
        actual == expected,
        "Sequence stylesheet emitted `{actual}` for `{selector}` {property}, expected `{expected}`"
    );
    Ok(())
}

fn css_last_property<'a>(css: &'a str, selector: &str, property: &str) -> Option<&'a str> {
    let marker = format!("{selector}{{");
    let mut cursor = 0;
    let mut winner = None;
    while let Some(relative_start) = css.get(cursor..)?.find(&marker) {
        let body_start = cursor
            .checked_add(relative_start)?
            .checked_add(marker.len())?;
        let relative_end = css.get(body_start..)?.find('}')?;
        let body_end = body_start.checked_add(relative_end)?;
        winner = css
            .get(body_start..body_end)?
            .split(';')
            .fold(winner, |winner, declaration| {
                let Some((name, value)) = declaration.split_once(':') else {
                    return winner;
                };
                (name.trim() == property).then(|| value.trim()).or(winner)
            });
        cursor = body_end.checked_add(1)?;
    }
    winner
}

fn parse_view_box(raw: &str) -> C6ProofResult<[f64; 4]> {
    let values = raw
        .split_ascii_whitespace()
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| C6ProofError::new("sequence-svg-root", error.to_string()))?;
    c6_ensure!(
        "sequence-svg-root",
        values.len() == 4
            && values.iter().all(|value| value.is_finite())
            && values[2] > 0.0
            && values[3] > 0.0,
        "Sequence SVG viewBox is invalid: `{raw}`"
    );
    Ok([values[0], values[1], values[2], values[3]])
}

fn corner_fill_region(node: roxmltree::Node<'_, '_>) -> C6ProofResult<[f64; 4]> {
    let [x, y, width, height] = rect_geometry(node)?;
    let inset = 4.0;
    let available_width = width - inset * 2.0;
    let available_height = height - inset * 2.0;
    c6_ensure!(
        "sequence-svg-roi",
        available_width >= 8.0 && available_height >= 8.0,
        "Sequence fill surface is too small for a stable PNG ROI"
    );
    Ok([
        x + inset,
        y + inset,
        available_width.min(16.0),
        available_height.min(12.0),
    ])
}

fn rect_geometry(node: roxmltree::Node<'_, '_>) -> C6ProofResult<[f64; 4]> {
    let values = ["x", "y", "width", "height"]
        .map(|name| numeric_attribute(node, name))
        .into_iter()
        .collect::<C6ProofResult<Vec<_>>>()?;
    c6_ensure!(
        "sequence-svg-roi",
        values[2] > 0.0 && values[3] > 0.0,
        "Sequence rect has non-positive geometry"
    );
    Ok([values[0], values[1], values[2], values[3]])
}

fn middle_line_region(line: roxmltree::Node<'_, '_>) -> C6ProofResult<[f64; 4]> {
    let x1 = numeric_attribute(line, "x1")?;
    let y1 = numeric_attribute(line, "y1")?;
    let x2 = numeric_attribute(line, "x2")?;
    let y2 = numeric_attribute(line, "y2")?;
    let width = (x2 - x1).abs();
    c6_ensure!(
        "sequence-svg-roi",
        width > 0.0 && (y2 - y1).abs() <= 1e-6,
        "Sequence message proof requires a horizontal line"
    );
    Ok([x1.min(x2) + width * 0.3, y1 - 2.0, width * 0.4, 4.0])
}

fn numeric_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> C6ProofResult<f64> {
    let value = node
        .attribute(name)
        .ok_or_else(|| C6ProofError::new("sequence-svg-roi", format!("missing {name}")))?
        .parse::<f64>()
        .map_err(|error| C6ProofError::new("sequence-svg-roi", error.to_string()))?;
    c6_ensure!(
        "sequence-svg-roi",
        value.is_finite(),
        "Sequence SVG {name} is not finite"
    );
    Ok(value)
}

fn parse_hex_rgb(value: &str) -> C6ProofResult<[u8; 3]> {
    let hex = value.strip_prefix('#').ok_or_else(|| {
        C6ProofError::new(
            "sequence-png-color",
            format!("expected a six-digit hex color, got `{value}`"),
        )
    })?;
    c6_ensure!(
        "sequence-png-color",
        hex.len() == 6,
        "expected a six-digit hex color, got `{value}`"
    );
    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&hex[range], 16)
            .map_err(|error| C6ProofError::new("sequence-png-color", error.to_string()))
    };
    Ok([channel(0..2)?, channel(2..4)?, channel(4..6)?])
}
