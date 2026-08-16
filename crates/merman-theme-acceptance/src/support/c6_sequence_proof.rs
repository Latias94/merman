use std::collections::BTreeMap;

use merman_export::RasterPlan;
use merman_theme_fixtures::ReferenceThemeMechanism;

use crate::observation::C6ObservedMechanismDisposition;

use super::{
    BrutalistSequenceFixtureContract, C6BoundTargetProof, C6ProofError, C6ProofResult,
    C6RasterImage, C6TargetArtifact, class_contains, decode_bounded_png_artifact, parse_c6_hex_rgb,
    parse_c6_svg_view_box, style_value,
};

const BRUTALIST_CELL_MECHANISMS: [ReferenceThemeMechanism; 1] =
    [ReferenceThemeMechanism::ThemeVariables];

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

#[derive(Clone, Debug)]
pub(crate) struct SequenceSvgProof {
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    target_proof: C6BoundTargetProof,
    view_box: [f64; 4],
    actor_fill_region: [f64; 4],
    lifeline_stroke_region: Option<[f64; 4]>,
    note_fill_region: Option<[f64; 4]>,
    message_stroke_region: [f64; 4],
}

impl SequenceSvgProof {
    pub(crate) fn mechanisms(
        &self,
    ) -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
        self.mechanisms.clone()
    }

    pub(crate) fn target_proof(&self) -> C6BoundTargetProof {
        self.target_proof.clone()
    }
}

pub(crate) type BrutalistSequenceSvgProof = SequenceSvgProof;

pub(crate) fn prove_brutalist_sequence_svg(
    contract: &BrutalistSequenceFixtureContract<'_>,
    artifact: C6TargetArtifact<'_>,
) -> C6ProofResult<BrutalistSequenceSvgProof> {
    prove_sequence_svg(
        brutalist_contract(contract),
        &BRUTALIST_CELL_MECHANISMS,
        "brutalist-sequence-standalone-svg-v1",
        artifact,
    )
}

pub(crate) fn prove_brutalist_sequence_png(
    contract: &BrutalistSequenceFixtureContract<'_>,
    svg_proof: &BrutalistSequenceSvgProof,
    artifact: C6TargetArtifact<'_>,
    plan: RasterPlan,
) -> C6ProofResult<C6BoundTargetProof> {
    prove_sequence_png(
        brutalist_contract(contract),
        svg_proof,
        "brutalist-sequence-png-v1",
        artifact,
        plan,
    )
}

fn brutalist_contract<'a>(
    contract: &'a BrutalistSequenceFixtureContract<'a>,
) -> SequenceProofContract<'a> {
    let actor = SequenceRectProofStyle {
        fill: contract.actor_fill(),
        stroke: contract.stroke(),
    };
    SequenceProofContract {
        actor,
        actor_rect_count: 2,
        require_actor_man: true,
        lifeline: None,
        message_stroke: contract.stroke(),
        message_line_counts: [1, 2],
        message_text_count: 3,
        note: Some(SequenceRectProofStyle {
            fill: contract.note_fill(),
            stroke: contract.stroke(),
        }),
        activation: Some(actor),
        font_family: None,
    }
}

pub(crate) fn prove_sequence_svg(
    contract: SequenceProofContract<'_>,
    cell_mechanisms: &[ReferenceThemeMechanism],
    semantic_assertion_id: &'static str,
    artifact: C6TargetArtifact<'_>,
) -> C6ProofResult<SequenceSvgProof> {
    let mechanisms = applied_mechanisms(cell_mechanisms);
    let (geometry, target_proof) =
        artifact.check_with(semantic_assertion_id, mechanisms.clone(), |bytes| {
            check_sequence_svg(contract, bytes)
        })?;
    Ok(SequenceSvgProof {
        mechanisms,
        target_proof,
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

fn check_sequence_svg(
    contract: SequenceProofContract<'_>,
    bytes: &[u8],
) -> C6ProofResult<SequenceSvgGeometry> {
    let svg = std::str::from_utf8(bytes)
        .map_err(|error| C6ProofError::new("sequence-svg-utf8", error.to_string()))?;
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("sequence-svg-parse", error.to_string()))?;
    let root = document.root_element();
    let root_id = root
        .attribute("id")
        .ok_or_else(|| C6ProofError::new("sequence-svg-root", "Sequence SVG root lacks an id"))?;
    let view_box = parse_c6_svg_view_box(root.attribute("viewBox").ok_or_else(|| {
        C6ProofError::new("sequence-svg-root", "Sequence SVG root lacks a viewBox")
    })?)?;

    prove_terminal_styles(&document, root_id, contract)?;
    let surfaces = prove_terminal_surfaces(&document, contract)?;
    prove_native_role_text(
        &document,
        svg,
        contract.font_family,
        contract.message_text_count,
    )?;

    Ok(SequenceSvgGeometry {
        view_box,
        actor_fill_region: corner_fill_region(surfaces.actor_rects[0])?,
        lifeline_stroke_region: contract
            .lifeline
            .map(|_| vertical_line_region(surfaces.lifelines[0]))
            .transpose()?,
        note_fill_region: contract
            .note
            .map(|_| corner_fill_region(surfaces.note))
            .transpose()?,
        message_stroke_region: middle_line_region(surfaces.message_lines[0])?,
    })
}

fn prove_terminal_styles(
    document: &roxmltree::Document<'_>,
    root_id: &str,
    contract: SequenceProofContract<'_>,
) -> C6ProofResult<()> {
    let actor_selector = format!("#{root_id} .actor");
    require_stylesheet_property(document, &actor_selector, "fill", contract.actor.fill)?;
    require_stylesheet_property(document, &actor_selector, "stroke", contract.actor.stroke)?;

    if let Some(lifeline) = contract.lifeline {
        let selector = format!("#{root_id} .actor-line");
        require_stylesheet_property(document, &selector, "stroke", lifeline.stroke)?;
        if let Some(width_px) = lifeline.width_px {
            require_stylesheet_number(document, &selector, "stroke-width", width_px)?;
        }
    }

    let message_selector = format!("#{root_id} .messageLine0,#{root_id} .messageLine1");
    require_stylesheet_property(
        document,
        &message_selector,
        "stroke",
        contract.message_stroke,
    )?;

    if let Some(note) = contract.note {
        let selector = format!("#{root_id} .note");
        require_stylesheet_property(document, &selector, "fill", note.fill)?;
        require_stylesheet_property(document, &selector, "stroke", note.stroke)?;
    }

    if let Some(activation) = contract.activation {
        let selector =
            format!("#{root_id} .activation0,#{root_id} .activation1,#{root_id} .activation2");
        require_stylesheet_property(document, &selector, "fill", activation.fill)?;
        require_stylesheet_property(document, &selector, "stroke", activation.stroke)?;
    }

    Ok(())
}

struct SequenceSurfaces<'document, 'input> {
    actor_rects: Vec<roxmltree::Node<'document, 'input>>,
    lifelines: Vec<roxmltree::Node<'document, 'input>>,
    note: roxmltree::Node<'document, 'input>,
    message_lines: Vec<roxmltree::Node<'document, 'input>>,
}

fn prove_terminal_surfaces<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    contract: SequenceProofContract<'_>,
) -> C6ProofResult<SequenceSurfaces<'document, 'input>> {
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
        actor_rects.len() == contract.actor_rect_count,
        "Sequence requires {} terminal regular-participant actor rects; got {}",
        contract.actor_rect_count,
        actor_rects.len()
    );
    if contract.require_actor_man {
        c6_ensure!(
            "sequence-svg-actors",
            document.descendants().any(|node| {
                (node.has_tag_name("circle") || node.has_tag_name("line"))
                    && node
                        .ancestors()
                        .any(|ancestor| class_contains(ancestor, "actor-man"))
            }),
            "Sequence actor source did not retain an actor-man terminal surface"
        );
    }

    let lifelines = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && class_contains(*node, "actor-line")
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
        message_lines.len() == contract.message_line_counts.iter().sum::<usize>()
            && message_lines
                .iter()
                .filter(|node| class_contains(**node, "messageLine0"))
                .count()
                == contract.message_line_counts[0]
            && message_lines
                .iter()
                .filter(|node| class_contains(**node, "messageLine1"))
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

fn prove_native_role_text(
    document: &roxmltree::Document<'_>,
    svg: &str,
    font_family: Option<&str>,
    message_text_count: usize,
) -> C6ProofResult<()> {
    c6_ensure!(
        "sequence-svg-text",
        document.descendants().any(|node| node.has_tag_name("text"))
            && !document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
            && !svg.contains("merman-prepared-"),
        "Brutalist Sequence did not retain terminal native SVG text"
    );

    let Some(font_family) = font_family else {
        return Ok(());
    };
    for (role, minimum) in [
        ("actor", 4usize),
        ("messageText", message_text_count),
        ("noteText", 1),
        ("loopText", 1),
    ] {
        let texts = document
            .descendants()
            .filter(|node| node.has_tag_name("text") && class_contains(*node, role))
            .collect::<Vec<_>>();
        c6_ensure!(
            "sequence-svg-font",
            texts.len() >= minimum
                && texts.iter().all(|node| {
                    style_value(*node, "font-family")
                        .is_some_and(|actual| css_family_matches(actual, font_family))
                }),
            "Sequence requires at least {minimum} native `{role}` texts with `{font_family}`"
        );
    }

    let typed_font_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("style") && node.attribute("data-merman-typed-fonts") == Some("v1")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-font",
        typed_font_styles.len() == 1
            && typed_font_styles[0].text().is_some_and(|css| {
                css.contains("@font-face")
                    && css.contains("data:font/")
                    && css.contains(font_family)
            }),
        "Sequence standalone SVG lacks its sealed embedded `{font_family}` font face"
    );
    Ok(())
}

pub(crate) fn prove_sequence_png(
    contract: SequenceProofContract<'_>,
    svg_proof: &SequenceSvgProof,
    semantic_assertion_id: &'static str,
    artifact: C6TargetArtifact<'_>,
    plan: RasterPlan,
) -> C6ProofResult<C6BoundTargetProof> {
    let (_, target_proof) =
        artifact.check_with(semantic_assertion_id, svg_proof.mechanisms(), |bytes| {
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
            Ok(())
        })?;
    Ok(target_proof)
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

fn applied_mechanisms(
    cell_mechanisms: &[ReferenceThemeMechanism],
) -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
    cell_mechanisms
        .iter()
        .copied()
        .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
        .collect()
}

fn require_stylesheet_property(
    document: &roxmltree::Document<'_>,
    selector: &str,
    property: &str,
    expected: &str,
) -> C6ProofResult<()> {
    let actual = stylesheet_property(document, selector, property).ok_or_else(|| {
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

fn require_stylesheet_number(
    document: &roxmltree::Document<'_>,
    selector: &str,
    property: &str,
    expected: f64,
) -> C6ProofResult<()> {
    let actual = stylesheet_property(document, selector, property).and_then(|value| {
        value
            .strip_suffix("px")
            .unwrap_or(value)
            .parse::<f64>()
            .ok()
    });
    c6_ensure!(
        "sequence-svg-stylesheet",
        actual.is_some_and(|actual| (actual - expected).abs() <= 1e-6),
        "Sequence stylesheet emitted {actual:?} for `{selector}` {property}, expected {expected}"
    );
    Ok(())
}

fn stylesheet_property<'input>(
    document: &'input roxmltree::Document<'input>,
    selector: &str,
    property: &str,
) -> Option<&'input str> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .filter_map(|css| css_last_property(css, selector, property))
        .next_back()
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

fn css_family_matches(actual: &str, expected: &str) -> bool {
    actual
        .split(',')
        .next()
        .map(str::trim)
        .map(|family| family.trim_matches(['\'', '"']))
        == Some(expected)
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

fn vertical_line_region(line: roxmltree::Node<'_, '_>) -> C6ProofResult<[f64; 4]> {
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
    // The middle of a lifeline can be intentionally covered by notes and activation bars.
    Ok([x1 - 2.0, y1.min(y2) + height * 0.05, 4.0, height * 0.15])
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

#[cfg(test)]
mod tests {
    use super::*;

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
        font_family: Some("Excalifont"),
    };

    #[test]
    fn spotless_sequence_terminal_contract_accepts_complete_svg() {
        let svg = spotless_proof_svg();
        assert!(check_sequence_svg(SPOTLESS_CONTRACT, svg.as_bytes()).is_ok());
    }

    #[test]
    fn spotless_sequence_terminal_contract_rejects_lifeline_width_mutation() {
        let svg = spotless_proof_svg().replacen("stroke-width:2px", "stroke-width:3px", 1);
        assert!(check_sequence_svg(SPOTLESS_CONTRACT, svg.as_bytes()).is_err());
    }

    #[test]
    fn spotless_sequence_terminal_contract_rejects_role_font_mutation() {
        let svg = spotless_proof_svg().replacen(
            r#"class="loopText" style="font-family:&quot;Excalifont&quot;""#,
            r#"class="loopText" style="font-family:Inter""#,
            1,
        );
        assert!(check_sequence_svg(SPOTLESS_CONTRACT, svg.as_bytes()).is_err());
    }

    #[test]
    fn spotless_sequence_terminal_contract_rejects_missing_lifeline_or_font_seal() {
        let svg = spotless_proof_svg();
        let missing_lifeline = svg.replacen(
            r#"<line class="actor-line" data-et="life-line" x1="60" y1="40" x2="60" y2="200"/>"#,
            "",
            1,
        );
        assert!(check_sequence_svg(SPOTLESS_CONTRACT, missing_lifeline.as_bytes()).is_err());

        let missing_font_seal = svg.replacen(" data-merman-typed-fonts=\"v1\"", "", 1);
        assert!(check_sequence_svg(SPOTLESS_CONTRACT, missing_font_seal.as_bytes()).is_err());
    }

    fn spotless_proof_svg() -> String {
        r##"<svg xmlns="http://www.w3.org/2000/svg" id="seq" viewBox="0 0 320 240">
<style data-merman-typed-fonts="v1">@font-face{font-family:"Excalifont";src:url(data:font/ttf;base64,AA==)}</style>
<style>#seq .actor{fill:#f5f1e8;stroke:#2c2416;}#seq .actor-line{stroke:#2c2416;stroke-width:2px;}#seq .messageLine0,#seq .messageLine1{stroke:#2c2416;}</style>
<rect class="actor actor-top" x="220" y="10" width="80" height="30"/><rect class="actor actor-bottom" x="220" y="200" width="80" height="30"/>
<g class="actor-man"><circle cx="60" cy="20" r="5"/></g>
<line class="actor-line" data-et="life-line" x1="60" y1="40" x2="60" y2="200"/><line class="actor-line" data-et="life-line" x1="260" y1="40" x2="260" y2="200"/>
<line class="messageLine0" data-et="message" x1="60" y1="70" x2="260" y2="70" marker-end="url(#seq-arrowhead)"/>
<line class="messageLine1" data-et="message" x1="260" y1="150" x2="60" y2="150" marker-end="url(#seq-arrowhead)"/>
<line class="messageLine1" data-et="message" x1="260" y1="180" x2="60" y2="180" marker-end="url(#seq-arrowhead)"/>
<rect class="note" x="100" y="90" width="120" height="30"/><rect class="activation0" x="255" y="55" width="10" height="120"/>
<text class="actor" style="font-family:&quot;Excalifont&quot;">User</text><text class="actor" style="font-family:&quot;Excalifont&quot;">Service</text><text class="actor" style="font-family:&quot;Excalifont&quot;">User</text><text class="actor" style="font-family:&quot;Excalifont&quot;">Service</text>
<text class="messageText" style="font-family:&quot;Excalifont&quot;">Submit request</text><text class="messageText" style="font-family:&quot;Excalifont&quot;">Processing</text><text class="messageText" style="font-family:&quot;Excalifont&quot;">Complete</text>
<text class="noteText" style="font-family:&quot;Excalifont&quot;">Request accepted</text><text class="loopText" style="font-family:&quot;Excalifont&quot;">Poll status</text>
</svg>"##
            .to_string()
    }
}
