use merman_export::RasterPlan;
use merman_render::__private::{SvgArtifactReceipt, SvgElementObservation};
#[cfg(test)]
use sha2::Digest as _;

use super::{
    C6ProofError, C6ProofResult, C6RasterImage, C6TargetArtifact,
    artifact_observation::sealed_svg_receipt, decode_bounded_png_artifact, parse_c6_hex_rgb,
};

#[path = "c6_sequence_proof/role_text.rs"]
mod role_text;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SequenceRectProofStyle<'a> {
    pub(crate) fill: &'a str,
    pub(crate) stroke: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SequenceLineProofStyle<'a> {
    pub(crate) stroke: &'a str,
    pub(crate) width_px: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SequenceProofContract<'a> {
    pub(crate) actor: SequenceRectProofStyle<'a>,
    pub(crate) actor_rect_count: usize,
    pub(crate) require_actor_man: bool,
    pub(crate) lifeline: Option<SequenceLineProofStyle<'a>>,
    pub(crate) message_stroke: &'a str,
    pub(crate) message_line_counts: [usize; 2],
    pub(crate) message_text_count: usize,
    pub(crate) note: Option<SequenceRectProofStyle<'a>>,
    pub(crate) activation: Option<SequenceRectProofStyle<'a>>,
    pub(crate) font_family: Option<&'a str>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SequenceRoleTextProofContract<'a> {
    pub(crate) actor_text: &'a str,
    pub(crate) message_text: &'a str,
    pub(crate) note_text: &'a str,
    pub(crate) loop_text: &'a str,
    pub(crate) canvas: &'a str,
    pub(crate) loop_label_surface: &'a str,
}

#[derive(Clone, Debug)]
pub(crate) struct SequenceSvgProof {
    view_box: [f64; 4],
    actor_fill_region: [f64; 4],
    lifeline_stroke_region: Option<[f64; 4]>,
    note_fill_region: Option<[f64; 4]>,
    message_stroke_region: [f64; 4],
}

pub(crate) fn prove_sequence_svg(
    contract: SequenceProofContract<'_>,
    artifact: &C6TargetArtifact<'_>,
) -> C6ProofResult<SequenceSvgProof> {
    let receipt = sealed_svg_receipt(artifact)?;
    let geometry = check_sequence_svg_internal(contract, None, receipt)?;
    Ok(SequenceSvgProof {
        view_box: geometry.view_box,
        actor_fill_region: geometry.actor_fill_region,
        lifeline_stroke_region: geometry.lifeline_stroke_region,
        note_fill_region: geometry.note_fill_region,
        message_stroke_region: geometry.message_stroke_region,
    })
}

struct SequenceSvgGeometry {
    view_box: [f64; 4],
    actor_fill_region: [f64; 4],
    lifeline_stroke_region: Option<[f64; 4]>,
    note_fill_region: Option<[f64; 4]>,
    message_stroke_region: [f64; 4],
}

#[cfg(test)]
fn check_sequence_svg(
    contract: SequenceProofContract<'_>,
    bytes: &[u8],
) -> C6ProofResult<SequenceSvgGeometry> {
    let svg = std::str::from_utf8(bytes)
        .map_err(|error| C6ProofError::new("sequence-svg-utf8", error.to_string()))?;
    let digest = sha2::Sha256::digest(svg.as_bytes()).into();
    let receipt = SvgArtifactReceipt::observe_svg_for_test(svg, digest).ok_or_else(|| {
        C6ProofError::new("sequence-svg-observation", "test SVG was not observed")
    })?;
    check_sequence_svg_internal(contract, None, &receipt)
}

#[cfg(test)]
fn check_sequence_svg_with_role_text(
    contract: SequenceProofContract<'_>,
    role_text: SequenceRoleTextProofContract<'_>,
    bytes: &[u8],
) -> C6ProofResult<SequenceSvgGeometry> {
    let svg = std::str::from_utf8(bytes)
        .map_err(|error| C6ProofError::new("sequence-svg-utf8", error.to_string()))?;
    let digest = sha2::Sha256::digest(svg.as_bytes()).into();
    let receipt = SvgArtifactReceipt::observe_svg_for_test(svg, digest).ok_or_else(|| {
        C6ProofError::new("sequence-svg-observation", "test SVG was not observed")
    })?;
    check_sequence_svg_internal(contract, Some(role_text), &receipt)
}

fn check_sequence_svg_internal(
    contract: SequenceProofContract<'_>,
    role_text: Option<SequenceRoleTextProofContract<'_>>,
    receipt: &SvgArtifactReceipt,
) -> C6ProofResult<SequenceSvgGeometry> {
    let root_id = receipt
        .root_id()
        .ok_or_else(|| C6ProofError::new("sequence-svg-root", "Sequence SVG root lacks an id"))?;
    let view_box = receipt.view_box();

    prove_terminal_styles(receipt, root_id, contract)?;
    let surfaces = prove_terminal_surfaces(receipt, contract)?;
    role_text::prove_native_role_text(
        receipt,
        root_id,
        &surfaces,
        contract.font_family,
        contract.message_text_count,
        contract,
        role_text,
    )?;

    Ok(SequenceSvgGeometry {
        view_box,
        actor_fill_region: corner_fill_region(surfaces.actor_rects[0])?,
        lifeline_stroke_region: contract
            .lifeline
            .map(|_| visible_lifeline_region(receipt, &surfaces.lifelines))
            .transpose()?,
        note_fill_region: contract
            .note
            .map(|_| corner_fill_region(surfaces.note))
            .transpose()?,
        message_stroke_region: middle_line_region(surfaces.message_lines[0])?,
    })
}

fn prove_terminal_styles(
    receipt: &SvgArtifactReceipt,
    root_id: &str,
    contract: SequenceProofContract<'_>,
) -> C6ProofResult<()> {
    let actor_selector = format!("#{root_id} .actor");
    require_stylesheet_property(receipt, &actor_selector, "fill", contract.actor.fill)?;
    require_stylesheet_property(receipt, &actor_selector, "stroke", contract.actor.stroke)?;

    if let Some(lifeline) = contract.lifeline {
        let selector = format!("#{root_id} .actor-line");
        require_stylesheet_property(receipt, &selector, "stroke", lifeline.stroke)?;
        if let Some(width_px) = lifeline.width_px {
            require_stylesheet_number(receipt, &selector, "stroke-width", width_px)?;
        }
    }

    let message_selector = format!("#{root_id} .messageLine0,#{root_id} .messageLine1");
    require_stylesheet_property(
        receipt,
        &message_selector,
        "stroke",
        contract.message_stroke,
    )?;

    if let Some(note) = contract.note {
        let selector = format!("#{root_id} .note");
        require_stylesheet_property(receipt, &selector, "fill", note.fill)?;
        require_stylesheet_property(receipt, &selector, "stroke", note.stroke)?;
    }

    if let Some(activation) = contract.activation {
        let selector =
            format!("#{root_id} .activation0,#{root_id} .activation1,#{root_id} .activation2");
        require_stylesheet_property(receipt, &selector, "fill", activation.fill)?;
        require_stylesheet_property(receipt, &selector, "stroke", activation.stroke)?;
    }

    Ok(())
}

struct SequenceSurfaces<'a> {
    actor_rects: Vec<&'a SvgElementObservation>,
    lifelines: Vec<&'a SvgElementObservation>,
    note: &'a SvgElementObservation,
    message_lines: Vec<&'a SvgElementObservation>,
}

fn prove_terminal_surfaces<'a>(
    receipt: &'a SvgArtifactReceipt,
    contract: SequenceProofContract<'_>,
) -> C6ProofResult<SequenceSurfaces<'a>> {
    let actor_rects = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "rect"
                && node.has_class("actor")
                && (node.has_class("actor-top") || node.has_class("actor-bottom"))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-actors",
        actor_rects.len() == contract.actor_rect_count,
        "Sequence requires {} terminal regular-participant actor rects; got {}",
        contract.actor_rect_count,
        actor_rects.len()
    );
    if contract.require_actor_man {
        c6_ensure!(
            "sequence-svg-actors",
            receipt.elements().iter().any(|node| {
                (node.tag_name() == "circle" || node.tag_name() == "line")
                    && super::artifact_observation::has_ancestor_class(receipt, node, "actor-man")
            }),
            "Sequence actor source did not retain an actor-man terminal surface"
        );
    }

    let lifelines = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "line"
                && node.has_class("actor-line")
                && node.attribute("data-et") == Some("life-line")
        })
        .collect::<Vec<_>>();
    if contract.lifeline.is_some() {
        c6_ensure!(
            "sequence-svg-lifelines",
            lifelines.len() == 2,
            "Sequence requires two terminal actor lifelines; got {}",
            lifelines.len()
        );
    }

    let notes = receipt
        .elements()
        .iter()
        .filter(|node| node.tag_name() == "rect" && node.has_class("note"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-note",
        notes.len() == 1,
        "Sequence requires one terminal note rect; got {}",
        notes.len()
    );
    let activations = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "rect"
                && ["activation0", "activation1", "activation2"]
                    .into_iter()
                    .any(|class| node.has_class(class))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-activation",
        activations.len() == 1,
        "Sequence requires one terminal activation rect; got {}",
        activations.len()
    );

    let message_lines = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "line"
                && node.attribute("data-et") == Some("message")
                && (node.has_class("messageLine0") || node.has_class("messageLine1"))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-messages",
        message_lines.len() == contract.message_line_counts.iter().sum::<usize>()
            && message_lines
                .iter()
                .filter(|node| node.has_class("messageLine0"))
                .count()
                == contract.message_line_counts[0]
            && message_lines
                .iter()
                .filter(|node| node.has_class("messageLine1"))
                .count()
                == contract.message_line_counts[1],
        "Sequence terminal message-line counts differ from {:?}",
        contract.message_line_counts
    );
    c6_ensure!(
        "sequence-svg-messages",
        message_lines.iter().all(|line| {
            line.attribute("marker-end")
                .is_some_and(|value| value.starts_with("url(#") && value.ends_with("-arrowhead)"))
        }),
        "Sequence message lines do not retain their scoped terminal marker references"
    );

    Ok(SequenceSurfaces {
        actor_rects,
        lifelines,
        note: notes[0],
        message_lines,
    })
}

pub(crate) fn prove_sequence_png_with_raster(
    contract: SequenceProofContract<'_>,
    svg_proof: &SequenceSvgProof,
    artifact: C6TargetArtifact<'_>,
    plan: RasterPlan,
) -> C6ProofResult<C6RasterImage> {
    let bytes = artifact.bytes();
    let raster = decode_bounded_png_artifact(bytes, plan)?;
    prove_roi_color(
        &raster,
        svg_proof.view_box,
        svg_proof.actor_fill_region,
        parse_c6_hex_rgb(contract.actor.fill)?,
        24,
        "sequence-png-actor-fill",
    )?;
    if let Some(lifeline) = contract.lifeline {
        prove_roi_color(
            &raster,
            svg_proof.view_box,
            required_region(
                svg_proof.lifeline_stroke_region,
                "sequence-png-lifeline-stroke",
            )?,
            parse_c6_hex_rgb(lifeline.stroke)?,
            4,
            "sequence-png-lifeline-stroke",
        )?;
    }
    if let Some(note) = contract.note {
        prove_roi_color(
            &raster,
            svg_proof.view_box,
            required_region(svg_proof.note_fill_region, "sequence-png-note-fill")?,
            parse_c6_hex_rgb(note.fill)?,
            24,
            "sequence-png-note-fill",
        )?;
    }
    prove_roi_color(
        &raster,
        svg_proof.view_box,
        svg_proof.message_stroke_region,
        parse_c6_hex_rgb(contract.message_stroke)?,
        4,
        "sequence-png-message-stroke",
    )?;
    Ok(raster)
}

fn required_region(region: Option<[f64; 4]>, stage: &'static str) -> C6ProofResult<[f64; 4]> {
    region.ok_or_else(|| C6ProofError::new(stage, "Sequence SVG proof omitted a required PNG ROI"))
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
    let global_count = raster.count_opaque_pixels_near(color, 10);
    c6_ensure!(
        stage,
        count >= minimum_pixels,
        "Sequence PNG ROI retained {count} matching pixels, expected at least {minimum_pixels}; global={global_count}, region={region:?}, viewBox={view_box:?}"
    );
    Ok(())
}

fn require_stylesheet_property(
    receipt: &SvgArtifactReceipt,
    selector: &str,
    property: &str,
    expected: &str,
) -> C6ProofResult<()> {
    let actual = exact_writer_stylesheet_property(receipt, selector, property)?;
    c6_ensure!(
        "sequence-svg-stylesheet",
        actual == expected,
        "Sequence stylesheet emitted `{actual}` for `{selector}` {property}, expected `{expected}`"
    );
    Ok(())
}

fn require_stylesheet_number(
    receipt: &SvgArtifactReceipt,
    selector: &str,
    property: &str,
    expected: f64,
) -> C6ProofResult<()> {
    let value = exact_writer_stylesheet_property(receipt, selector, property)?;
    let actual = value
        .strip_suffix("px")
        .unwrap_or(value)
        .parse::<f64>()
        .ok();
    c6_ensure!(
        "sequence-svg-stylesheet",
        actual.is_some_and(|actual| (actual - expected).abs() <= 1e-6),
        "Sequence stylesheet emitted {actual:?} for `{selector}` {property}, expected {expected}"
    );
    Ok(())
}

pub(crate) fn require_exact_writer_declaration(
    receipt: &SvgArtifactReceipt,
    selector: &str,
    property: &str,
    expected: &str,
) -> C6ProofResult<()> {
    require_stylesheet_property(receipt, selector, property, expected)
}

fn exact_writer_stylesheet_property<'a>(
    receipt: &'a SvgArtifactReceipt,
    selector: &str,
    property: &str,
) -> C6ProofResult<&'a str> {
    let values = receipt
        .stylesheets()
        .iter()
        .filter(|stylesheet| stylesheet.parse_valid())
        .flat_map(|stylesheet| stylesheet.rules())
        .filter(|rule| rule.selector() == selector)
        .filter_map(|rule| {
            rule.declaration(property)
                .map(|declaration| declaration.value())
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-stylesheet",
        values.len() == 1,
        "Sequence stylesheet expected one exact writer declaration for `{selector}` {property}, found {}",
        values.len()
    );
    Ok(values[0])
}

fn corner_fill_region(node: &SvgElementObservation) -> C6ProofResult<[f64; 4]> {
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

fn rect_geometry(node: &SvgElementObservation) -> C6ProofResult<[f64; 4]> {
    let [x, y, width, height] =
        ["x", "y", "width", "height"].map(|name| numeric_attribute(node, name));
    let geometry = [x?, y?, width?, height?];
    c6_ensure!(
        "sequence-svg-roi",
        geometry[2] > 0.0 && geometry[3] > 0.0,
        "Sequence rect has non-positive geometry"
    );
    Ok(geometry)
}

fn middle_line_region(line: &SvgElementObservation) -> C6ProofResult<[f64; 4]> {
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

fn visible_lifeline_region(
    receipt: &SvgArtifactReceipt,
    lifelines: &[&SvgElementObservation],
) -> C6ProofResult<[f64; 4]> {
    let blockers = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "rect"
                && (node.has_class("actor")
                    || node.has_class("note")
                    || ["activation0", "activation1", "activation2"]
                        .iter()
                        .any(|class| node.has_class(class)))
        })
        .map(rect_geometry)
        .collect::<C6ProofResult<Vec<_>>>()?;
    for line in lifelines {
        let [x, top, width, height] = vertical_line_region(line)?;
        let bottom = top + height;
        let mut intervals = blockers
            .iter()
            .filter(|rect| x < rect[0] + rect[2] + 2.0 && rect[0] - 2.0 < x + width)
            .map(|rect| {
                (
                    (rect[1] - 2.0).max(top),
                    (rect[1] + rect[3] + 2.0).min(bottom),
                )
            })
            .filter(|(start, end)| start < end)
            .collect::<Vec<_>>();
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut cursor = top;
        let mut longest = (top, 0.0_f64);
        for (start, end) in intervals
            .into_iter()
            .chain(std::iter::once((bottom, bottom)))
        {
            if start - cursor > longest.1 {
                longest = (cursor, start - cursor);
            }
            cursor = cursor.max(end);
        }
        // A short, interior sample avoids rectangle borders and still contains enough stroke
        // pixels for the existing raster assertion. Fully occluded lines cannot prove a color.
        if longest.1 >= 12.0 {
            let sample_height = longest.1.min(24.0);
            return Ok([
                x,
                longest.0 + (longest.1 - sample_height) / 2.0,
                width,
                sample_height,
            ]);
        }
    }
    Err(C6ProofError::new(
        "sequence-svg-roi",
        "no unoccluded lifeline sample region",
    ))
}

fn vertical_line_region(line: &SvgElementObservation) -> C6ProofResult<[f64; 4]> {
    let x1 = numeric_attribute(line, "x1")?;
    let y1 = numeric_attribute(line, "y1")?;
    let x2 = numeric_attribute(line, "x2")?;
    let y2 = numeric_attribute(line, "y2")?;
    let height = (y2 - y1).abs();
    c6_ensure!(
        "sequence-svg-roi",
        height > 0.0 && (x2 - x1).abs() <= 1e-6,
        "Sequence lifeline proof requires a vertical line"
    );
    // Exclude endpoints; callers subtract the observed occluding rectangles.
    Ok([x1 - 2.0, y1.min(y2) + 2.0, 4.0, height - 4.0])
}

fn numeric_attribute(node: &SvgElementObservation, name: &str) -> C6ProofResult<f64> {
    let value = node
        .numeric_attribute(name)
        .ok_or_else(|| C6ProofError::new("sequence-svg-roi", format!("missing {name}")))?;
    c6_ensure!(
        "sequence-svg-roi",
        value.is_finite(),
        "Sequence SVG {name} is not finite"
    );
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifeline_sample_uses_visible_segment_beyond_top_occlusion() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><line class="actor-line" x1="50" y1="0" x2="50" y2="100"/><rect class="activation0" x="45" y="0" width="10" height="40"/></svg>"#;
        let receipt = SvgArtifactReceipt::observe_svg_for_test(
            svg,
            sha2::Sha256::digest(svg.as_bytes()).into(),
        )
        .unwrap();
        let lines = receipt
            .elements()
            .iter()
            .filter(|node| node.has_class("actor-line"))
            .collect::<Vec<_>>();
        let region = visible_lifeline_region(&receipt, &lines).unwrap();
        assert!(region[1] > 40.0);
        assert!(region[1] + region[3] < 100.0);
        assert!(region[3] >= 12.0);
    }

    #[test]
    fn lifeline_sample_rejects_fully_occluded_lines() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 120"><line class="actor-line" x1="50" y1="0" x2="50" y2="100"/><rect class="activation0" x="45" y="0" width="10" height="60"/><rect class="note" x="40" y="40" width="20" height="60"/></svg>"#;
        let receipt = SvgArtifactReceipt::observe_svg_for_test(
            svg,
            sha2::Sha256::digest(svg.as_bytes()).into(),
        )
        .unwrap();
        let lines = receipt
            .elements()
            .iter()
            .filter(|node| node.has_class("actor-line"))
            .collect::<Vec<_>>();
        assert!(visible_lifeline_region(&receipt, &lines).is_err());
    }

    const SPOTLESS_CONTRACT: SequenceProofContract<'static> = SequenceProofContract {
        actor: SequenceRectProofStyle {
            fill: "#f5f1e8",
            stroke: "#2c2416",
        },
        actor_rect_count: 2,
        require_actor_man: true,
        lifeline: Some(SequenceLineProofStyle {
            stroke: "#2c2416",
            width_px: Some(2.0),
        }),
        message_stroke: "#2c2416",
        message_line_counts: [1, 2],
        message_text_count: 3,
        note: None,
        activation: None,
        font_family: Some("sans-serif"),
    };
    const CYBERPUNK_CONTRACT: SequenceProofContract<'static> = SequenceProofContract {
        actor: SequenceRectProofStyle {
            fill: "#22d3ee",
            stroke: "#e0f2fe",
        },
        actor_rect_count: 4,
        require_actor_man: false,
        lifeline: Some(SequenceLineProofStyle {
            stroke: "#22d3ee",
            width_px: Some(2.0),
        }),
        message_stroke: "#22d3ee",
        message_line_counts: [1, 1],
        message_text_count: 2,
        note: Some(SequenceRectProofStyle {
            fill: "#0f172a",
            stroke: "#22d3ee",
        }),
        activation: Some(SequenceRectProofStyle {
            fill: "#0f172a",
            stroke: "#22d3ee",
        }),
        font_family: Some("sans-serif"),
    };
    const CYBERPUNK_ROLE_TEXT: SequenceRoleTextProofContract<'static> =
        SequenceRoleTextProofContract {
            actor_text: "#020617",
            message_text: "#e0f2fe",
            note_text: "#e0f2fe",
            loop_text: "#e0f2fe",
            canvas: "#020617",
            loop_label_surface: "#0f172a",
        };

    #[test]
    fn spotless_sequence_terminal_contract_accepts_complete_svg() {
        let svg = spotless_proof_svg();
        assert!(check_sequence_svg(SPOTLESS_CONTRACT, svg.as_bytes()).is_ok());
    }

    #[test]
    fn spotless_sequence_native_contract_does_not_reimplement_browser_cascade() {
        let svg = spotless_proof_svg().replacen(
            "</svg>",
            "<style>#seq [class~='actor-line']{stroke:#000!important;}</style></svg>",
            1,
        );

        assert!(check_sequence_svg(SPOTLESS_CONTRACT, svg.as_bytes()).is_ok());
    }

    #[test]
    fn spotless_sequence_terminal_contract_rejects_duplicate_writer_declaration() {
        let svg = spotless_proof_svg().replacen(
            "</svg>",
            "<style>#seq .actor-line{stroke:#2c2416;}</style></svg>",
            1,
        );

        assert!(check_sequence_svg(SPOTLESS_CONTRACT, svg.as_bytes()).is_err());
    }

    #[test]
    fn spotless_sequence_terminal_contract_rejects_lifeline_width_mutation() {
        let svg = spotless_proof_svg().replacen("stroke-width:2px", "stroke-width:3px", 1);
        assert!(check_sequence_svg(SPOTLESS_CONTRACT, svg.as_bytes()).is_err());
    }

    #[test]
    fn spotless_sequence_terminal_contract_rejects_role_font_mutation() {
        let svg = spotless_proof_svg().replacen(
            r#"class="loopText" style="font-family:&quot;sans-serif&quot;""#,
            r#"class="loopText" style="font-family:Inter""#,
            1,
        );
        assert!(check_sequence_svg(SPOTLESS_CONTRACT, svg.as_bytes()).is_err());
    }

    #[test]
    fn spotless_sequence_terminal_contract_rejects_missing_lifeline() {
        let svg = spotless_proof_svg();
        let missing_lifeline = svg.replacen(
            r#"<line class="actor-line" data-et="life-line" x1="60" y1="40" x2="60" y2="200"/>"#,
            "",
            1,
        );
        assert!(check_sequence_svg(SPOTLESS_CONTRACT, missing_lifeline.as_bytes()).is_err());
    }

    #[test]
    fn cyberpunk_sequence_role_contract_covers_every_terminal_occurrence() {
        let svg = cyberpunk_role_proof_svg();
        assert!(
            check_sequence_svg_with_role_text(
                CYBERPUNK_CONTRACT,
                CYBERPUNK_ROLE_TEXT,
                svg.as_bytes(),
            )
            .is_ok()
        );
    }

    #[test]
    fn cyberpunk_sequence_role_contract_rejects_one_role_paint_or_surface_mutation() {
        let svg = cyberpunk_role_proof_svg();
        let wrong_note = svg.replacen(
            "#seq .noteText,#seq .noteText>tspan{fill:#e0f2fe;}",
            "#seq .noteText,#seq .noteText>tspan{fill:#000000;}",
            1,
        );
        assert!(
            check_sequence_svg_with_role_text(
                CYBERPUNK_CONTRACT,
                CYBERPUNK_ROLE_TEXT,
                wrong_note.as_bytes(),
            )
            .is_err()
        );

        let wrong_loop_surface = svg.replacen(
            "#seq .labelBox{fill:#0f172a;stroke:#22d3ee;}",
            "#seq .labelBox{fill:#ececff;stroke:#22d3ee;}",
            1,
        );
        assert!(
            check_sequence_svg_with_role_text(
                CYBERPUNK_CONTRACT,
                CYBERPUNK_ROLE_TEXT,
                wrong_loop_surface.as_bytes(),
            )
            .is_err()
        );
    }

    #[test]
    fn cyberpunk_sequence_role_contract_rejects_missing_occurrence() {
        let svg = cyberpunk_role_proof_svg();
        let missing_message = svg.replacen(
            r#"<text class="messageText" style="font-family:&quot;sans-serif&quot;">Breach</text>"#,
            "",
            1,
        );
        assert!(
            check_sequence_svg_with_role_text(
                CYBERPUNK_CONTRACT,
                CYBERPUNK_ROLE_TEXT,
                missing_message.as_bytes(),
            )
            .is_err()
        );
    }

    #[test]
    fn cyberpunk_sequence_native_contract_does_not_reimplement_browser_cascade() {
        let svg = cyberpunk_role_proof_svg().replacen(
            "</svg>",
            "<style>#seq [class~='messageText']{fill:#000!important;}</style></svg>",
            1,
        );

        assert!(
            check_sequence_svg_with_role_text(
                CYBERPUNK_CONTRACT,
                CYBERPUNK_ROLE_TEXT,
                svg.as_bytes(),
            )
            .is_ok()
        );
    }

    #[test]
    fn cyberpunk_sequence_role_contract_rejects_an_unreadable_actor_pair() {
        let svg = cyberpunk_role_proof_svg().replacen(
            "#seq text.actor,#seq text.actor>tspan,#seq text.text,#seq text.text>tspan{fill:#020617;}",
            "#seq text.actor,#seq text.actor>tspan,#seq text.text,#seq text.text>tspan{fill:#22d3ee;}",
            1,
        );
        let low_contrast = SequenceRoleTextProofContract {
            actor_text: "#22d3ee",
            ..CYBERPUNK_ROLE_TEXT
        };

        assert!(
            check_sequence_svg_with_role_text(CYBERPUNK_CONTRACT, low_contrast, svg.as_bytes())
                .is_err()
        );
    }

    fn spotless_proof_svg() -> String {
        r##"<svg xmlns="http://www.w3.org/2000/svg" id="seq" viewBox="0 0 320 240">

<style>#seq .actor{fill:#f5f1e8;stroke:#2c2416;}#seq .actor-line{stroke:#2c2416;stroke-width:2px;}#seq .messageLine0,#seq .messageLine1{stroke:#2c2416;}</style>
<rect class="actor actor-top" x="220" y="10" width="80" height="30"/><rect class="actor actor-bottom" x="220" y="200" width="80" height="30"/>
<g class="actor-man"><circle cx="60" cy="20" r="5"/></g>
<line class="actor-line" data-et="life-line" x1="60" y1="40" x2="60" y2="200"/><line class="actor-line" data-et="life-line" x1="260" y1="40" x2="260" y2="200"/>
<line class="messageLine0" data-et="message" x1="60" y1="70" x2="260" y2="70" marker-end="url(#seq-arrowhead)"/>
<line class="messageLine1" data-et="message" x1="260" y1="150" x2="60" y2="150" marker-end="url(#seq-arrowhead)"/>
<line class="messageLine1" data-et="message" x1="260" y1="180" x2="60" y2="180" marker-end="url(#seq-arrowhead)"/>
<rect class="note" x="100" y="90" width="120" height="30"/><rect class="activation0" x="255" y="55" width="10" height="120"/>
<text class="actor" style="font-family:&quot;sans-serif&quot;">User</text><text class="actor" style="font-family:&quot;sans-serif&quot;">Service</text><text class="actor" style="font-family:&quot;sans-serif&quot;">User</text><text class="actor" style="font-family:&quot;sans-serif&quot;">Service</text>
<text class="messageText" style="font-family:&quot;sans-serif&quot;">Submit request</text><text class="messageText" style="font-family:&quot;sans-serif&quot;">Processing</text><text class="messageText" style="font-family:&quot;sans-serif&quot;">Complete</text>
<text class="noteText" style="font-family:&quot;sans-serif&quot;">Request accepted</text><text class="loopText" style="font-family:&quot;sans-serif&quot;">Poll status</text>
</svg>"##
            .to_string()
    }

    fn cyberpunk_role_proof_svg() -> String {
        r##"<svg xmlns="http://www.w3.org/2000/svg" id="seq" viewBox="0 0 360 340">

<rect data-merman-theme-canvas="base" x="0" y="0" width="360" height="340" fill="#020617"/>
<style>#seq .actor{fill:#22d3ee;stroke:#e0f2fe;}#seq .actor-line{stroke:#22d3ee;stroke-width:2px;}#seq .messageLine0,#seq .messageLine1{stroke:#22d3ee;}#seq .note{fill:#0f172a;stroke:#22d3ee;}#seq .activation0,#seq .activation1,#seq .activation2{fill:#0f172a;stroke:#22d3ee;}#seq .labelBox{fill:#0f172a;stroke:#22d3ee;}#seq text.actor,#seq text.actor>tspan,#seq text.text,#seq text.text>tspan{fill:#020617;}#seq .messageText,#seq .messageText>tspan{fill:#e0f2fe;}#seq .noteText,#seq .noteText>tspan{fill:#e0f2fe;}#seq .loopText,#seq .loopText>tspan,#seq .sectionTitle,#seq .sectionTitle>tspan,#seq .labelText,#seq .labelText>tspan{fill:#e0f2fe;}</style>
<g><rect class="actor actor-top" name="Operator" x="10" y="10" width="120" height="40"/><text class="actor" style="font-family:&quot;sans-serif&quot;">Operator</text></g>
<g><rect class="actor actor-bottom" name="Operator" x="10" y="290" width="120" height="40"/><text class="actor" style="font-family:&quot;sans-serif&quot;">Operator</text></g>
<g><rect class="actor actor-top" name="Console" x="230" y="10" width="120" height="40"/><text class="actor" style="font-family:&quot;sans-serif&quot;">Console</text></g>
<g><rect class="actor actor-bottom" name="Console" x="230" y="290" width="120" height="40"/><text class="actor" style="font-family:&quot;sans-serif&quot;">Console</text></g>
<line class="actor-line" data-et="life-line" x1="70" y1="50" x2="70" y2="290"/><line class="actor-line" data-et="life-line" x1="290" y1="50" x2="290" y2="290"/>
<g><rect class="activation0" x="285" y="90" width="10" height="150"/></g>
<g data-et="note" data-id="i2"><rect class="note" x="80" y="120" width="200" height="40"/><text class="noteText" style="font-family:&quot;sans-serif&quot;">Access granted</text></g>
<g data-et="control-structure" data-id="i5"><polygon class="labelBox" points="60,170 120,170 120,200 60,200"/><text class="labelText" style="font-family:&quot;sans-serif&quot;">loop</text><text class="loopText" style="font-family:&quot;sans-serif&quot;"><tspan>[Trace]</tspan></text></g>
<g><text class="messageText" style="font-family:&quot;sans-serif&quot;">Breach</text><line class="messageLine0" data-et="message" data-id="i0" x1="70" y1="90" x2="290" y2="90" marker-end="url(#seq-arrowhead)"/></g>
<g><text class="messageText" style="font-family:&quot;sans-serif&quot;">Scan</text><line class="messageLine1" data-et="message" data-id="i4" x1="290" y1="250" x2="70" y2="250" marker-end="url(#seq-arrowhead)"/></g>
</svg>"##
            .to_string()
    }
}
