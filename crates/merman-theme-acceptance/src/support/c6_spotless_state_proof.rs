use std::collections::{BTreeMap, BTreeSet};

use merman_export::RasterPlan;
use merman_theme_fixtures::ReferenceThemeMechanism;

use crate::observation::C6ObservedMechanismDisposition;

use super::{
    C6BoundTargetProof, C6ProofError, C6ProofResult, C6RasterImage, C6TargetArtifact,
    class_contains, decode_bounded_png_artifact, parse_c6_hex_rgb, parse_c6_svg_view_box,
    style_value,
};

const STATE_LABELS: [(&str, &str); 4] = [
    ("rinse", "RINSE"),
    ("polish", "POLISH"),
    ("inspect", "INSPECT"),
    ("release", "RELEASE"),
];
const CANVAS_LAYER_COUNT: usize = 2;
const CELL_MECHANISMS: [ReferenceThemeMechanism; 8] = [
    ReferenceThemeMechanism::CanvasGradient,
    ReferenceThemeMechanism::CanvasLayering,
    ReferenceThemeMechanism::CanvasPattern,
    ReferenceThemeMechanism::CanvasSolid,
    ReferenceThemeMechanism::CssLetterSpacing,
    ReferenceThemeMechanism::CssTextTransform,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::ThemeVariables,
];
// PNG proves visible overlay composition, not the SVG-only text or base-canvas facts.
const PNG_MECHANISMS: [ReferenceThemeMechanism; 3] = [
    ReferenceThemeMechanism::CanvasGradient,
    ReferenceThemeMechanism::CanvasLayering,
    ReferenceThemeMechanism::CanvasPattern,
];
const MIN_OVERLAY_DIFFERENCE: u8 = 16;
const MIN_OVERLAY_CORE_COVERAGE: f64 = 0.75;

#[derive(Clone, Copy)]
pub(crate) struct SpotlessStateProofContract<'a> {
    pub(crate) canvas: &'a str,
    pub(crate) surface: &'a str,
    pub(crate) text: &'a str,
    pub(crate) font_family: &'a str,
    pub(crate) letter_spacing_px: f64,
    pub(crate) overlay_period_px: f64,
    pub(crate) linear_stops: [(f64, &'a str); 4],
    pub(crate) radial_stops: [(f64, &'a str); 4],
}

#[derive(Clone, Debug)]
pub(crate) struct SpotlessStateSvgProof {
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    target_proof: C6BoundTargetProof,
    view_box: [f64; 4],
    overlay_regions: [[f64; 4]; 2],
}

impl SpotlessStateSvgProof {
    pub(crate) fn target_proof(&self) -> C6BoundTargetProof {
        self.target_proof.clone()
    }
}

pub(crate) fn prove_spotless_state_svg(
    contract: SpotlessStateProofContract<'_>,
    artifact: C6TargetArtifact<'_>,
) -> C6ProofResult<SpotlessStateSvgProof> {
    let mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> =
        CELL_MECHANISMS
            .into_iter()
            .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
            .collect();
    let ((view_box, overlay_regions), target_proof) = artifact.check_with(
        "spotless-state-standalone-svg-v1",
        mechanisms.clone(),
        |bytes| check_spotless_state_svg(contract, bytes),
    )?;
    Ok(SpotlessStateSvgProof {
        mechanisms,
        target_proof,
        view_box,
        overlay_regions,
    })
}

type SpotlessStateSvgGeometry = ([f64; 4], [[f64; 4]; 2]);

fn check_spotless_state_svg(
    contract: SpotlessStateProofContract<'_>,
    bytes: &[u8],
) -> C6ProofResult<SpotlessStateSvgGeometry> {
    let svg = std::str::from_utf8(bytes)
        .map_err(|error| C6ProofError::new("spotless-state-svg-utf8", error.to_string()))?;
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("spotless-state-svg-parse", error.to_string()))?;
    let root = document.root_element();
    let view_box = parse_c6_svg_view_box(root.attribute("viewBox").ok_or_else(|| {
        C6ProofError::new("spotless-state-svg-root", "State SVG root lacks a viewBox")
    })?)?;

    prove_terminal_canvas(&document, contract)?;
    for (state_id, expected_label) in STATE_LABELS {
        prove_terminal_state(&document, state_id, expected_label, contract)?;
    }
    c6_ensure!(
        "spotless-state-svg-text",
        document.descendants().any(|node| node.has_tag_name("text"))
            && !document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
            && !svg.contains("merman-prepared-"),
        "Spotless State did not retain native terminal text"
    );

    Ok((
        view_box,
        overlay_visibility_regions(view_box, contract.overlay_period_px)?,
    ))
}

pub(crate) fn prove_spotless_state_png(
    contract: SpotlessStateProofContract<'_>,
    svg_proof: &SpotlessStateSvgProof,
    artifact: C6TargetArtifact<'_>,
    plan: RasterPlan,
) -> C6ProofResult<C6BoundTargetProof> {
    let mechanisms = spotless_state_png_mechanisms(&svg_proof.mechanisms)?;
    let (_, target_proof) = artifact.check_with("spotless-state-png-v1", mechanisms, |bytes| {
        let raster = decode_bounded_png_artifact(bytes, plan)?;
        prove_overlay_visibility(
            &raster,
            svg_proof.view_box,
            svg_proof.overlay_regions,
            parse_c6_hex_rgb(contract.canvas)?,
        )?;
        Ok(())
    })?;
    Ok(target_proof)
}

fn spotless_state_png_mechanisms(
    svg_mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> C6ProofResult<BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>> {
    PNG_MECHANISMS
        .into_iter()
        .map(|mechanism| {
            let disposition = svg_mechanisms.get(&mechanism).copied().ok_or_else(|| {
                C6ProofError::new(
                    "spotless-state-png-mechanisms",
                    format!(
                        "Spotless State PNG overlay proof lacks SVG mechanism `{}`",
                        mechanism.id()
                    ),
                )
            })?;
            c6_ensure!(
                "spotless-state-png-mechanisms",
                disposition == C6ObservedMechanismDisposition::Applied,
                "Spotless State PNG cannot report `{}` from a non-Applied SVG mechanism",
                mechanism.id()
            );
            Ok((mechanism, disposition))
        })
        .collect()
}

fn prove_terminal_canvas(
    document: &roxmltree::Document<'_>,
    contract: SpotlessStateProofContract<'_>,
) -> C6ProofResult<()> {
    let bases = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect") && node.attribute("data-merman-theme-canvas") == Some("base")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "spotless-state-svg-canvas",
        bases.len() == 1 && bases[0].attribute("fill") == Some(contract.canvas),
        "Spotless State requires one terminal solid canvas base"
    );

    let layers = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g") && node.attribute("data-merman-theme-canvas-layer").is_some()
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "spotless-state-svg-canvas",
        layers.len() == CANVAS_LAYER_COUNT,
        "Spotless State requires {CANVAS_LAYER_COUNT} terminal canvas layers, got {}",
        layers.len()
    );
    let mut resource_ids = BTreeSet::new();
    let linear = resolve_layer_paint(document, layers[0], 0, "linearGradient", &mut resource_ids)?;
    c6_ensure!(
        "spotless-state-svg-canvas",
        linear.attribute("spreadMethod") == Some("repeat"),
        "Spotless State linear overlay is not a repeating gradient"
    );
    prove_gradient_stops(linear, &contract.linear_stops, "linear")?;

    let pattern = resolve_layer_paint(document, layers[1], 1, "pattern", &mut resource_ids)?;
    let tile = only_element_child(pattern, "Spotless State radial pattern")?;
    c6_ensure!(
        "spotless-state-svg-canvas",
        tile.has_tag_name("rect"),
        "Spotless State radial pattern does not contain one terminal tile"
    );
    let radial = resolve_local_paint(
        document,
        tile,
        "radialGradient",
        "radial tile",
        &mut resource_ids,
    )?;
    prove_tiled_radial_geometry(pattern, tile, radial, contract.overlay_period_px)?;
    prove_gradient_stops(radial, &contract.radial_stops, "radial")?;
    Ok(())
}

fn resolve_layer_paint<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    layer: roxmltree::Node<'document, 'input>,
    layer_index: usize,
    expected_tag: &str,
    resource_ids: &mut BTreeSet<&'document str>,
) -> C6ProofResult<roxmltree::Node<'document, 'input>> {
    let actual_index = layer
        .attribute("data-merman-theme-canvas-layer")
        .and_then(|value| value.parse::<usize>().ok());
    let source = only_element_child(layer, "Spotless State canvas layer")?;
    c6_ensure!(
        "spotless-state-svg-canvas",
        actual_index == Some(layer_index) && source.has_tag_name("rect"),
        "Spotless State canvas layer {layer_index} is not one ordered terminal paint"
    );
    resolve_local_paint(
        document,
        source,
        expected_tag,
        &format!("canvas layer {layer_index}"),
        resource_ids,
    )
}

fn resolve_local_paint<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    source: roxmltree::Node<'document, 'input>,
    expected_tag: &str,
    label: &str,
    resource_ids: &mut BTreeSet<&'document str>,
) -> C6ProofResult<roxmltree::Node<'document, 'input>> {
    let id = source
        .attribute("fill")
        .and_then(local_fragment_id)
        .ok_or_else(|| {
            C6ProofError::new(
                "spotless-state-svg-canvas",
                format!("Spotless State {label} lacks one local paint reference"),
            )
        })?;
    let definitions = document
        .descendants()
        .filter(|node| node.attribute("id") == Some(id))
        .collect::<Vec<_>>();
    c6_ensure!(
        "spotless-state-svg-canvas",
        definitions.len() == 1
            && definitions[0].tag_name().name() == expected_tag
            && resource_ids.insert(id),
        "Spotless State {label} paint `{id}` does not uniquely resolve to {expected_tag}"
    );
    Ok(definitions[0])
}

fn only_element_child<'document, 'input>(
    node: roxmltree::Node<'document, 'input>,
    label: &str,
) -> C6ProofResult<roxmltree::Node<'document, 'input>> {
    let elements = node
        .children()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    c6_ensure!(
        "spotless-state-svg-canvas",
        elements.len() == 1,
        "{label} must contain exactly one element"
    );
    Ok(elements[0])
}

fn prove_tiled_radial_geometry(
    pattern: roxmltree::Node<'_, '_>,
    tile: roxmltree::Node<'_, '_>,
    radial: roxmltree::Node<'_, '_>,
    period_px: f64,
) -> C6ProofResult<()> {
    c6_ensure!(
        "spotless-state-svg-canvas",
        period_px.is_finite() && period_px > 0.0,
        "Spotless State radial overlay period must be finite and positive"
    );
    let expected_bounds = [0.0, 0.0, period_px, period_px];
    c6_ensure!(
        "spotless-state-svg-canvas",
        pattern.attribute("patternUnits") == Some("userSpaceOnUse")
            && geometry_matches(pattern, expected_bounds)?,
        "Spotless State radial pattern differs from its canonical user-space bounds"
    );
    c6_ensure!(
        "spotless-state-svg-canvas",
        geometry_matches(tile, expected_bounds)?,
        "Spotless State radial tile rect differs from its pattern bounds"
    );

    let half_period = period_px / 2.0;
    c6_ensure!(
        "spotless-state-svg-canvas",
        radial.attribute("gradientUnits") == Some("userSpaceOnUse")
            && radial.attribute("spreadMethod").is_none()
            && approx_eq(numeric_attribute(radial, "cx")?, half_period)
            && approx_eq(numeric_attribute(radial, "cy")?, half_period)
            && approx_eq(numeric_attribute(radial, "r")?, half_period),
        "Spotless State radial gradient differs from its canonical tile center and radius"
    );
    Ok(())
}

fn geometry_matches(node: roxmltree::Node<'_, '_>, expected: [f64; 4]) -> C6ProofResult<bool> {
    ["x", "y", "width", "height"]
        .into_iter()
        .zip(expected)
        .try_fold(true, |matches, (name, expected)| {
            Ok(matches && approx_eq(numeric_attribute(node, name)?, expected))
        })
}

fn numeric_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> C6ProofResult<f64> {
    let value = node
        .attribute(name)
        .ok_or_else(|| {
            C6ProofError::new(
                "spotless-state-svg-canvas",
                format!("Spotless State canvas paint is missing `{name}`"),
            )
        })?
        .parse::<f64>()
        .map_err(|error| C6ProofError::new("spotless-state-svg-canvas", error.to_string()))?;
    c6_ensure!(
        "spotless-state-svg-canvas",
        value.is_finite(),
        "Spotless State canvas paint `{name}` is not finite"
    );
    Ok(value)
}

fn prove_gradient_stops(
    gradient: roxmltree::Node<'_, '_>,
    expected: &[(f64, &str)],
    label: &str,
) -> C6ProofResult<()> {
    let stops = gradient
        .children()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    c6_ensure!(
        "spotless-state-svg-canvas",
        stops.len() == expected.len()
            && stops.iter().zip(expected).all(|(stop, (offset, color))| {
                stop.has_tag_name("stop")
                    && stop
                        .attribute("offset")
                        .and_then(percent_value)
                        .is_some_and(|actual| approx_eq(actual, *offset))
                    && stop.attribute("stop-color") == Some(*color)
            }),
        "Spotless State {label} overlay stops differ from the typed paint contract"
    );
    Ok(())
}

fn local_fragment_id(value: &str) -> Option<&str> {
    value
        .strip_prefix("url(#")
        .and_then(|value| value.strip_suffix(')'))
}

fn percent_value(value: &str) -> Option<f64> {
    value
        .strip_suffix('%')?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn prove_terminal_state(
    document: &roxmltree::Document<'_>,
    state_id: &str,
    expected_label: &str,
    contract: SpotlessStateProofContract<'_>,
) -> C6ProofResult<()> {
    let marker = format!("-state-{state_id}-");
    let wrappers = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g") && node.attribute("id").is_some_and(|id| id.contains(&marker))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "spotless-state-svg-node",
        wrappers.len() == 1,
        "expected one State wrapper for `{state_id}`, got {}",
        wrappers.len()
    );
    let wrapper = wrappers[0];
    let rects = wrapper
        .children()
        .filter(|node| {
            node.has_tag_name("rect")
                && class_contains(*node, "basic")
                && class_contains(*node, "label-container")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "spotless-state-svg-node",
        rects.len() == 1 && style_value(rects[0], "fill") == Some(contract.surface),
        "State `{state_id}` does not retain the Spotless surface token"
    );

    let label = wrapper
        .descendants()
        .find(|node| node.has_tag_name("g") && class_contains(*node, "label"))
        .ok_or_else(|| {
            C6ProofError::new(
                "spotless-state-svg-text",
                format!("State `{state_id}` lacks a terminal label group"),
            )
        })?;
    let text = label
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .ok_or_else(|| {
            C6ProofError::new(
                "spotless-state-svg-text",
                format!("State `{state_id}` lacks terminal native text"),
            )
        })?;
    c6_ensure!(
        "spotless-state-svg-text",
        descendant_text(text) == expected_label
            && subtree_style_value(label, "fill") == Some(contract.text)
            && subtree_style_number(label, "letter-spacing")
                .is_some_and(|value| approx_eq(value, contract.letter_spacing_px))
            && subtree_style_contains(label, "font-family", contract.font_family),
        "State `{state_id}` does not retain transformed, spaced, embedded-font terminal text"
    );
    Ok(())
}

fn descendant_text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .collect::<String>()
        .trim()
        .to_string()
}

fn subtree_style_value<'a>(node: roxmltree::Node<'a, '_>, property: &str) -> Option<&'a str> {
    std::iter::once(node)
        .chain(node.descendants())
        .find_map(|descendant| style_value(descendant, property))
}

fn subtree_style_number(node: roxmltree::Node<'_, '_>, property: &str) -> Option<f64> {
    let value = subtree_style_value(node, property)?;
    value
        .strip_suffix("px")
        .unwrap_or(value)
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn subtree_style_contains(node: roxmltree::Node<'_, '_>, property: &str, value: &str) -> bool {
    std::iter::once(node)
        .chain(node.descendants())
        .filter_map(|descendant| style_value(descendant, property))
        .any(|actual| actual.contains(value))
}

fn overlay_visibility_regions(view_box: [f64; 4], period: f64) -> C6ProofResult<[[f64; 4]; 2]> {
    let core_size = period * 0.05;
    let half_core = core_size / 2.0;
    let inset = 2.0;
    let first_center =
        |minimum: f64| ((minimum - period / 2.0) / period).ceil() * period + period / 2.0;
    let last_center =
        |maximum: f64| ((maximum - period / 2.0) / period).floor() * period + period / 2.0;
    let left_center = first_center(view_box[0] + inset + half_core);
    let right_center = last_center(view_box[0] + view_box[2] - inset - half_core);
    let top_center = first_center(view_box[1] + inset + half_core);
    let regions = [
        [
            left_center - half_core,
            top_center - half_core,
            core_size,
            core_size,
        ],
        [
            right_center - half_core,
            top_center - half_core,
            core_size,
            core_size,
        ],
    ];
    c6_ensure!(
        "spotless-state-png-overlay",
        view_box.into_iter().all(f64::is_finite)
            && period.is_finite()
            && period > 0.0
            && core_size.is_finite()
            && core_size > 0.0
            && right_center - left_center >= period
            && regions.iter().all(|region| {
                region[0] >= view_box[0]
                    && region[1] >= view_box[1]
                    && region[0] + region[2] <= view_box[0] + view_box[2]
                    && region[1] + region[3] <= view_box[1] + view_box[3]
            }),
        "Spotless State viewBox cannot provide two disjoint radial-core windows"
    );
    Ok(regions)
}

fn prove_overlay_visibility(
    raster: &C6RasterImage,
    view_box: [f64; 4],
    regions: [[f64; 4]; 2],
    canvas: [u8; 3],
) -> C6ProofResult<()> {
    for (index, region) in regions.into_iter().enumerate() {
        raster.prove_opaque_color_difference_coverage_in_svg_rect(
            view_box,
            region,
            canvas,
            MIN_OVERLAY_DIFFERENCE,
            MIN_OVERLAY_CORE_COVERAGE,
            "spotless-state-png-overlay",
            &format!("Spotless State radial-core window {index}"),
        )?;
    }
    Ok(())
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spotless_state_png_scope_excludes_text_font_canvas_and_foreground_claims() {
        let svg_mechanisms = CELL_MECHANISMS
            .into_iter()
            .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
            .collect();

        assert_eq!(
            spotless_state_png_mechanisms(&svg_mechanisms).unwrap(),
            PNG_MECHANISMS
                .into_iter()
                .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
                .collect()
        );
    }

    #[test]
    fn spotless_state_png_scope_rejects_a_missing_or_non_applied_overlay_mechanism() {
        let mut svg_mechanisms = CELL_MECHANISMS
            .into_iter()
            .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
            .collect::<BTreeMap<_, _>>();
        svg_mechanisms.remove(&ReferenceThemeMechanism::CanvasPattern);
        assert!(spotless_state_png_mechanisms(&svg_mechanisms).is_err());

        svg_mechanisms.insert(
            ReferenceThemeMechanism::CanvasPattern,
            C6ObservedMechanismDisposition::Residual,
        );
        assert!(spotless_state_png_mechanisms(&svg_mechanisms).is_err());
    }

    fn contract() -> SpotlessStateProofContract<'static> {
        SpotlessStateProofContract {
            canvas: "#ede8dc",
            surface: "#f5f1e8",
            text: "#1a1a1a",
            font_family: "Excalifont",
            letter_spacing_px: 0.64,
            overlay_period_px: 20.0,
            linear_stops: [
                (0.0, "#2c241608"),
                (50.0, "#2c241608"),
                (51.0, "#2c24160d"),
                (100.0, "#2c24160d"),
            ],
            radial_stops: [
                (0.0, "#2c24162e"),
                (10.0, "#2c24162e"),
                (11.0, "#2c241600"),
                (100.0, "#2c241600"),
            ],
        }
    }

    fn valid_canvas_svg() -> &'static str {
        r##"<svg>
          <rect data-merman-theme-canvas="base" fill="#ede8dc"/>
          <defs><linearGradient id="layer-0-gradient" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="14.142136" y2="-14.142136" spreadMethod="repeat"><stop offset="0%" stop-color="#2c241608"/><stop offset="50%" stop-color="#2c241608"/><stop offset="51%" stop-color="#2c24160d"/><stop offset="100%" stop-color="#2c24160d"/></linearGradient></defs>
          <defs><radialGradient id="layer-1-gradient" gradientUnits="userSpaceOnUse" cx="10" cy="10" r="10"><stop offset="0%" stop-color="#2c24162e"/><stop offset="10%" stop-color="#2c24162e"/><stop offset="11%" stop-color="#2c241600"/><stop offset="100%" stop-color="#2c241600"/></radialGradient><pattern id="layer-1-pattern" patternUnits="userSpaceOnUse" x="0" y="0" width="20" height="20"><rect x="0" y="0" width="20" height="20" fill="url(#layer-1-gradient)"/></pattern></defs>
          <g data-merman-theme-canvas-layer="0"><rect fill="url(#layer-0-gradient)"/></g>
          <g data-merman-theme-canvas-layer="1"><rect fill="url(#layer-1-pattern)"/></g>
        </svg>"##
    }

    #[test]
    fn canvas_layers_reject_dangling_or_wrong_paint_resources() {
        let valid = valid_canvas_svg();
        let document = roxmltree::Document::parse(valid).unwrap();
        assert!(prove_terminal_canvas(&document, contract()).is_ok());

        let mutations = [
            valid.replace("url(#layer-1-pattern)", "url(#missing-pattern)"),
            valid.replace("radialGradient", "linearGradient"),
        ];
        for mutated in mutations {
            let document = roxmltree::Document::parse(&mutated).unwrap();
            assert!(
                prove_terminal_canvas(&document, contract()).is_err(),
                "mutation unexpectedly retained the paint closure: {mutated}"
            );
        }
    }

    #[test]
    fn tiled_radial_canvas_rejects_mutated_geometry() {
        let valid = valid_canvas_svg();
        let document = roxmltree::Document::parse(valid).unwrap();
        let mut wrong_period = contract();
        wrong_period.overlay_period_px = 21.0;
        assert!(prove_terminal_canvas(&document, wrong_period).is_err());

        let mutations = [
            (
                "pattern units",
                valid.replace(
                    "patternUnits=\"userSpaceOnUse\"",
                    "patternUnits=\"objectBoundingBox\"",
                ),
            ),
            (
                "pattern x",
                valid.replace(
                    "patternUnits=\"userSpaceOnUse\" x=\"0\"",
                    "patternUnits=\"userSpaceOnUse\" x=\"1\"",
                ),
            ),
            (
                "pattern y",
                valid.replace(
                    "patternUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\"",
                    "patternUnits=\"userSpaceOnUse\" x=\"0\" y=\"1\"",
                ),
            ),
            (
                "pattern width",
                valid.replace(
                    "width=\"20\" height=\"20\"><rect",
                    "width=\"21\" height=\"20\"><rect",
                ),
            ),
            (
                "pattern height",
                valid.replace(
                    "width=\"20\" height=\"20\"><rect",
                    "width=\"20\" height=\"21\"><rect",
                ),
            ),
            (
                "tile x",
                valid.replace(
                    "<rect x=\"0\" y=\"0\" width=\"20\"",
                    "<rect x=\"1\" y=\"0\" width=\"20\"",
                ),
            ),
            (
                "tile y",
                valid.replace(
                    "<rect x=\"0\" y=\"0\" width=\"20\"",
                    "<rect x=\"0\" y=\"1\" width=\"20\"",
                ),
            ),
            (
                "tile width",
                valid.replace(
                    "<rect x=\"0\" y=\"0\" width=\"20\"",
                    "<rect x=\"0\" y=\"0\" width=\"21\"",
                ),
            ),
            (
                "tile height",
                valid.replace(
                    "<rect x=\"0\" y=\"0\" width=\"20\" height=\"20\"",
                    "<rect x=\"0\" y=\"0\" width=\"20\" height=\"21\"",
                ),
            ),
            ("radial cx", valid.replace("cx=\"10\"", "cx=\"11\"")),
            ("radial cy", valid.replace("cy=\"10\"", "cy=\"11\"")),
            ("radial radius", valid.replace("r=\"10\"", "r=\"11\"")),
            (
                "radial units",
                valid.replace(
                    "<radialGradient id=\"layer-1-gradient\" gradientUnits=\"userSpaceOnUse\"",
                    "<radialGradient id=\"layer-1-gradient\" gradientUnits=\"objectBoundingBox\"",
                ),
            ),
        ];

        for (label, mutated) in mutations {
            assert_ne!(mutated, valid, "{label} mutation did not alter the fixture");
            let document = roxmltree::Document::parse(&mutated).unwrap();
            assert!(
                prove_terminal_canvas(&document, contract()).is_err(),
                "{label} mutation unexpectedly retained canonical canvas geometry"
            );
        }
    }

    #[test]
    fn base_only_raster_fails_overlay_visibility_even_when_canvas_color_passes() {
        let mut raster = C6RasterImage::solid_for_test(20, 20, [237, 232, 220]);
        assert!(
            raster
                .prove_opaque_color_coverage_in_svg_rect(
                    [0.0, 0.0, 20.0, 20.0],
                    [0.0, 0.0, 20.0, 20.0],
                    [237, 232, 220],
                    0,
                    1.0,
                    "test",
                    "base canvas",
                )
                .is_ok()
        );
        assert!(
            raster
                .prove_opaque_color_difference_coverage_in_svg_rect(
                    [0.0, 0.0, 20.0, 20.0],
                    [8.0, 8.0, 4.0, 4.0],
                    [237, 232, 220],
                    16,
                    0.10,
                    "test",
                    "overlay",
                )
                .is_err()
        );

        for y in 8..12 {
            for x in 8..12 {
                raster.set_rgb_for_test(x, y, [202, 197, 185]);
            }
        }
        assert!(
            raster
                .prove_opaque_color_difference_coverage_in_svg_rect(
                    [0.0, 0.0, 20.0, 20.0],
                    [8.0, 8.0, 4.0, 4.0],
                    [237, 232, 220],
                    16,
                    0.10,
                    "test",
                    "overlay",
                )
                .is_ok()
        );
    }

    #[test]
    fn foreground_in_one_corner_cannot_substitute_for_both_canvas_overlay_windows() {
        let view_box = [0.0, 0.0, 100.0, 100.0];
        let regions = overlay_visibility_regions(view_box, 20.0).unwrap();
        let canvas = [237, 232, 220];
        let mut raster = C6RasterImage::solid_for_test(100, 100, canvas);

        let left = regions[0];
        for y in left[1] as u32..(left[1] + left[3]) as u32 {
            for x in left[0] as u32..(left[0] + left[2]) as u32 {
                raster.set_rgb_for_test(x, y, [26, 26, 26]);
            }
        }

        assert!(prove_overlay_visibility(&raster, view_box, regions, canvas).is_err());
    }
}
