use std::collections::{BTreeMap, BTreeSet};

use merman_export::RasterPlan;
use merman_render::__private::{SvgArtifactReceipt, SvgElementObservation};
use merman_theme_fixtures::ReferenceThemeMechanism;

use crate::observation::C6ObservedMechanismDisposition;

use super::{
    C6BoundTargetProof, C6ProofError, C6ProofResult, C6RasterImage, C6TargetArtifact,
    artifact_observation::{
        approx_eq, local_fragment_id, numeric_attribute, percent_value, sealed_svg_receipt,
        style_number,
    },
    decode_bounded_png_artifact, parse_c6_hex_rgb,
};

const NODE_IDS: [&str; 4] = ["A", "B", "C", "D"];
const CANVAS_LAYER_COUNT: usize = 3;
const CELL_MECHANISMS: [ReferenceThemeMechanism; 5] = [
    ReferenceThemeMechanism::CanvasBlend,
    ReferenceThemeMechanism::CanvasGradient,
    ReferenceThemeMechanism::CanvasLayering,
    ReferenceThemeMechanism::CanvasPattern,
    ReferenceThemeMechanism::StrokeStyling,
];
const PNG_MECHANISMS: [ReferenceThemeMechanism; 1] = [ReferenceThemeMechanism::CanvasLayering];
const MIN_CANVAS_DIFFERENCE: u8 = 6;
const MIN_CANVAS_DIFFERENCE_COVERAGE: f64 = 0.25;

#[derive(Clone, Copy)]
pub(crate) struct CyberpunkFlowchartProofContract<'a> {
    pub(crate) canvas: &'a str,
    pub(crate) node_stroke: &'a str,
    pub(crate) node_stroke_width: f64,
    pub(crate) blend_mode: &'a str,
    pub(crate) tile_size_px: [f64; 2],
    pub(crate) grid_stops: [(f64, &'a str); 4],
    pub(crate) wash_stops: [(f64, &'a str); 2],
    pub(crate) radial_stops: [(f64, &'a str); 2],
    pub(crate) font_family: &'a str,
}

#[derive(Clone, Debug)]
pub(crate) struct CyberpunkFlowchartSvgProof {
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    target_proof: C6BoundTargetProof,
    view_box: [f64; 4],
    canvas_region: [f64; 4],
}

impl CyberpunkFlowchartSvgProof {
    pub(crate) fn target_proof(&self) -> C6BoundTargetProof {
        self.target_proof.clone()
    }
}

pub(crate) fn prove_cyberpunk_flowchart_svg(
    contract: CyberpunkFlowchartProofContract<'_>,
    artifact: C6TargetArtifact<'_>,
) -> C6ProofResult<CyberpunkFlowchartSvgProof> {
    let mechanisms = CELL_MECHANISMS
        .into_iter()
        .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
        .collect::<BTreeMap<_, _>>();
    let receipt = sealed_svg_receipt(&artifact)?;
    let (view_box, canvas_region) = check_cyberpunk_flowchart_svg(contract, receipt)?;
    let (_, target_proof) = artifact.check_with(
        "cyberpunk-flowchart-standalone-svg-v1",
        mechanisms.clone(),
        |_| Ok::<(), C6ProofError>(()),
    )?;
    Ok(CyberpunkFlowchartSvgProof {
        mechanisms,
        target_proof,
        view_box,
        canvas_region,
    })
}

fn check_cyberpunk_flowchart_svg(
    contract: CyberpunkFlowchartProofContract<'_>,
    receipt: &SvgArtifactReceipt,
) -> C6ProofResult<([f64; 4], [f64; 4])> {
    let view_box = receipt.view_box();
    prove_terminal_canvas(receipt, contract)?;
    let nodes = NODE_IDS
        .into_iter()
        .map(|node_id| prove_terminal_node(receipt, node_id, contract))
        .collect::<C6ProofResult<Vec<_>>>()?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-text",
        receipt.has_native_text()
            && !receipt.has_foreign_object()
            && !receipt.has_prepared_tokens(),
        "Cyberpunk Flowchart did not retain native terminal text"
    );
    Ok((view_box, canvas_gap_region(nodes[0], nodes[1])?))
}

pub(crate) fn prove_cyberpunk_flowchart_png(
    contract: CyberpunkFlowchartProofContract<'_>,
    svg_proof: &CyberpunkFlowchartSvgProof,
    artifact: C6TargetArtifact<'_>,
    plan: RasterPlan,
) -> C6ProofResult<C6BoundTargetProof> {
    let mechanisms = cyberpunk_flowchart_png_mechanisms(&svg_proof.mechanisms)?;
    let (_, target_proof) =
        artifact.check_with("cyberpunk-flowchart-png-v1", mechanisms, |bytes| {
            let raster = decode_bounded_png_artifact(bytes, plan)?;
            prove_canvas_visibility(
                &raster,
                svg_proof.view_box,
                svg_proof.canvas_region,
                parse_c6_hex_rgb(contract.canvas)?,
            )
        })?;
    Ok(target_proof)
}

fn cyberpunk_flowchart_png_mechanisms(
    svg_mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> C6ProofResult<BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>> {
    PNG_MECHANISMS
        .into_iter()
        .map(|mechanism| {
            let disposition = svg_mechanisms.get(&mechanism).copied().ok_or_else(|| {
                C6ProofError::new(
                    "cyberpunk-flowchart-png-mechanisms",
                    format!(
                        "Cyberpunk Flowchart PNG layer proof lacks SVG mechanism `{}`",
                        mechanism.id()
                    ),
                )
            })?;
            c6_ensure!(
                "cyberpunk-flowchart-png-mechanisms",
                disposition == C6ObservedMechanismDisposition::Applied,
                "Cyberpunk Flowchart PNG cannot report `{}` from a non-Applied SVG mechanism",
                mechanism.id()
            );
            Ok((mechanism, disposition))
        })
        .collect()
}

fn prove_terminal_canvas(
    receipt: &SvgArtifactReceipt,
    contract: CyberpunkFlowchartProofContract<'_>,
) -> C6ProofResult<()> {
    let bases = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "rect" && node.attribute("data-merman-theme-canvas") == Some("base")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        bases.len() == 1 && bases[0].attribute("fill") == Some(contract.canvas),
        "Cyberpunk Flowchart requires one terminal solid canvas base"
    );

    let layers = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "g" && node.attribute("data-merman-theme-canvas-layer").is_some()
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        layers.len() == CANVAS_LAYER_COUNT,
        "Cyberpunk Flowchart requires {CANVAS_LAYER_COUNT} ordered canvas layers, got {}",
        layers.len()
    );
    for (index, layer) in layers.iter().enumerate() {
        c6_ensure!(
            "cyberpunk-flowchart-svg-canvas",
            layer
                .attribute("data-merman-theme-canvas-layer")
                .and_then(|value| value.parse::<usize>().ok())
                == Some(index)
                && layer.style_value("mix-blend-mode") == Some(contract.blend_mode)
                && layer.attribute("opacity").is_none()
                && layer.attribute("transform").is_none(),
            "Cyberpunk Flowchart canvas layer {index} lost its order or blend mode"
        );
    }

    let mut resource_ids = BTreeSet::new();
    prove_tiled_linear_layer(receipt, layers[0], contract, &mut resource_ids)?;
    prove_linear_wash_layer(receipt, layers[1], contract, &mut resource_ids)?;
    prove_radial_layer(receipt, layers[2], contract, &mut resource_ids)?;
    Ok(())
}

fn prove_tiled_linear_layer(
    receipt: &SvgArtifactReceipt,
    layer: &SvgElementObservation,
    contract: CyberpunkFlowchartProofContract<'_>,
    resource_ids: &mut BTreeSet<String>,
) -> C6ProofResult<()> {
    let source = only_element_child(receipt, layer, "Cyberpunk Flowchart grid layer")?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        source.tag_name() == "rect",
        "Cyberpunk Flowchart grid layer must contain one terminal rect"
    );
    let pattern = resolve_local_paint(receipt, source, "pattern", "grid layer", resource_ids)?;
    let [tile_width, tile_height] = contract.tile_size_px;
    c6_ensure!(
        "cyberpunk-flowchart-svg-pattern",
        pattern.attribute("patternUnits") == Some("userSpaceOnUse")
            && numeric_attribute(pattern, "x").is_some_and(|value| approx_eq(value, 0.0))
            && numeric_attribute(pattern, "y").is_some_and(|value| approx_eq(value, 0.0))
            && numeric_attribute(pattern, "width")
                .is_some_and(|value| approx_eq(value, tile_width))
            && numeric_attribute(pattern, "height")
                .is_some_and(|value| approx_eq(value, tile_height)),
        "Cyberpunk Flowchart grid pattern geometry differs from the fixed tile"
    );
    let tile = only_element_child(receipt, pattern, "Cyberpunk Flowchart grid pattern")?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-pattern",
        tile.tag_name() == "rect"
            && numeric_attribute(tile, "x").is_some_and(|value| approx_eq(value, 0.0))
            && numeric_attribute(tile, "y").is_some_and(|value| approx_eq(value, 0.0))
            && numeric_attribute(tile, "width").is_some_and(|value| approx_eq(value, tile_width))
            && numeric_attribute(tile, "height").is_some_and(|value| approx_eq(value, tile_height)),
        "Cyberpunk Flowchart grid tile does not cover the fixed pattern bounds"
    );
    let gradient = resolve_local_paint(receipt, tile, "linearGradient", "grid tile", resource_ids)?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-pattern",
        gradient.attribute("gradientUnits") == Some("userSpaceOnUse")
            && gradient.attribute("spreadMethod").is_none()
            && numeric_attribute(gradient, "x1").is_some_and(|value| approx_eq(value, 0.0))
            && numeric_attribute(gradient, "y1")
                .is_some_and(|value| approx_eq(value, tile_height / 2.0))
            && numeric_attribute(gradient, "x2").is_some_and(|value| approx_eq(value, tile_width))
            && numeric_attribute(gradient, "y2")
                .is_some_and(|value| approx_eq(value, tile_height / 2.0)),
        "Cyberpunk Flowchart grid must retain one non-repeating tile-local linear gradient"
    );
    prove_gradient_stops(receipt, gradient, &contract.grid_stops, "grid")
}

fn prove_linear_wash_layer(
    receipt: &SvgArtifactReceipt,
    layer: &SvgElementObservation,
    contract: CyberpunkFlowchartProofContract<'_>,
    resource_ids: &mut BTreeSet<String>,
) -> C6ProofResult<()> {
    let source = only_element_child(receipt, layer, "Cyberpunk Flowchart linear wash layer")?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        source.tag_name() == "rect",
        "Cyberpunk Flowchart linear wash layer must contain one terminal rect"
    );
    let gradient = resolve_local_paint(
        receipt,
        source,
        "linearGradient",
        "linear wash layer",
        resource_ids,
    )?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        gradient.attribute("gradientUnits") == Some("userSpaceOnUse")
            && gradient.attribute("spreadMethod").is_none()
            && linear_vector(gradient).is_some_and(|[x, y]| x > 0.0 && y > 0.0 && approx_eq(x, y)),
        "Cyberpunk Flowchart linear wash lacks finite terminal geometry"
    );
    prove_gradient_stops(receipt, gradient, &contract.wash_stops, "linear wash")
}

fn prove_radial_layer(
    receipt: &SvgArtifactReceipt,
    layer: &SvgElementObservation,
    contract: CyberpunkFlowchartProofContract<'_>,
    resource_ids: &mut BTreeSet<String>,
) -> C6ProofResult<()> {
    let source = only_element_child(receipt, layer, "Cyberpunk Flowchart radial layer")?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        source.tag_name() == "rect",
        "Cyberpunk Flowchart radial layer must contain one terminal rect"
    );
    let gradient = resolve_local_paint(
        receipt,
        source,
        "radialGradient",
        "radial layer",
        resource_ids,
    )?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        gradient.attribute("gradientUnits") == Some("userSpaceOnUse")
            && gradient.attribute("spreadMethod").is_none()
            && ["cx", "cy", "r"]
                .into_iter()
                .all(|name| numeric_attribute(gradient, name).is_some())
            && numeric_attribute(gradient, "r").is_some_and(|value| value > 0.0),
        "Cyberpunk Flowchart radial glow lacks finite terminal geometry"
    );
    prove_gradient_stops(receipt, gradient, &contract.radial_stops, "radial glow")
}

fn resolve_local_paint<'a>(
    receipt: &'a SvgArtifactReceipt,
    source: &SvgElementObservation,
    expected_tag: &str,
    label: &str,
    resource_ids: &mut BTreeSet<String>,
) -> C6ProofResult<&'a SvgElementObservation> {
    let id = source
        .style_value("fill")
        .or_else(|| source.attribute("fill"))
        .and_then(local_fragment_id)
        .ok_or_else(|| {
            C6ProofError::new(
                "cyberpunk-flowchart-svg-resource",
                format!("Cyberpunk Flowchart {label} lacks one local paint reference"),
            )
        })?;
    let definitions = receipt
        .elements()
        .iter()
        .filter(|node| node.attribute("id") == Some(id))
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-resource",
        definitions.len() == 1
            && definitions[0].tag_name() == expected_tag
            && resource_ids.insert(id.to_owned()),
        "Cyberpunk Flowchart {label} paint `{id}` does not uniquely resolve to {expected_tag}"
    );
    Ok(definitions[0])
}

fn only_element_child<'a>(
    receipt: &'a SvgArtifactReceipt,
    node: &SvgElementObservation,
    label: &str,
) -> C6ProofResult<&'a SvgElementObservation> {
    let elements = receipt
        .elements()
        .iter()
        .filter(|child| child.parent_index() == Some(node.index()))
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        elements.len() == 1,
        "{label} must contain exactly one terminal element, got {}",
        elements.len()
    );
    Ok(elements[0])
}

fn prove_gradient_stops<const N: usize>(
    receipt: &SvgArtifactReceipt,
    gradient: &SvgElementObservation,
    expected: &[(f64, &str); N],
    label: &str,
) -> C6ProofResult<()> {
    let stops = receipt
        .elements()
        .iter()
        .filter(|child| child.parent_index() == Some(gradient.index()))
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-gradient",
        stops.len() == expected.len()
            && stops.iter().zip(expected).all(|(stop, (offset, color))| {
                stop.tag_name() == "stop"
                    && stop
                        .attribute("offset")
                        .and_then(percent_value)
                        .is_some_and(|actual| approx_eq(actual, *offset))
                    && stop.attribute("stop-color") == Some(*color)
            }),
        "Cyberpunk Flowchart {label} stops differ from the fixed recipe"
    );
    Ok(())
}

fn prove_terminal_node(
    receipt: &SvgArtifactReceipt,
    node_id: &str,
    contract: CyberpunkFlowchartProofContract<'_>,
) -> C6ProofResult<[f64; 4]> {
    let wrappers = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "g"
                && node.attribute("data-id") == Some(node_id)
                && node.attribute("data-et") == Some("node")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-node",
        wrappers.len() == 1,
        "expected one Flowchart wrapper for `{node_id}`, got {}",
        wrappers.len()
    );
    let rects = receipt
        .descendants_of(wrappers[0].index())
        .filter(|node| {
            node.tag_name() == "rect"
                && node.has_class("basic")
                && node.has_class("label-container")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-node",
        rects.len() == 1,
        "expected one classic process rect for `{node_id}`, got {}",
        rects.len()
    );
    let rect = rects[0];
    c6_ensure!(
        "cyberpunk-flowchart-svg-node",
        rect.style_value("stroke") == Some(contract.node_stroke)
            && style_number(rect, "stroke-width")
                .is_some_and(|value| approx_eq(value, contract.node_stroke_width))
            && receipt
                .descendants_of(wrappers[0].index())
                .filter_map(|node| node.style_value("font-family"))
                .any(|value| value.contains(contract.font_family)),
        "Flowchart node `{node_id}` differs from the Cyberpunk stroke contract"
    );
    rect.bounds().ok_or_else(|| {
        C6ProofError::new(
            "cyberpunk-flowchart-svg-node",
            format!("Flowchart node `{node_id}` lacks finite bounds"),
        )
    })
}

fn canvas_gap_region(first: [f64; 4], second: [f64; 4]) -> C6ProofResult<[f64; 4]> {
    let left = first[0] + first[2] + 2.0;
    let right = second[0] - 2.0;
    let top = first[1].max(second[1]) + 2.0;
    let center = (first[1] + first[3] / 2.0).min(second[1] + second[3] / 2.0);
    let height = (center - top - 3.0).min(12.0);
    c6_ensure!(
        "cyberpunk-flowchart-png-canvas",
        right - left >= 8.0 && height >= 4.0,
        "Flowchart nodes leave no stable canvas ROI above their connecting edge"
    );
    Ok([left, top, right - left, height])
}

fn prove_canvas_visibility(
    raster: &C6RasterImage,
    view_box: [f64; 4],
    region: [f64; 4],
    canvas: [u8; 3],
) -> C6ProofResult<()> {
    raster.prove_opaque_color_difference_coverage_in_svg_rect(
        view_box,
        region,
        canvas,
        MIN_CANVAS_DIFFERENCE,
        MIN_CANVAS_DIFFERENCE_COVERAGE,
        "cyberpunk-flowchart-png-canvas",
        "Cyberpunk Flowchart layered canvas",
    )
}

fn linear_vector(node: &SvgElementObservation) -> Option<[f64; 2]> {
    Some([
        numeric_attribute(node, "x2")? - numeric_attribute(node, "x1")?,
        numeric_attribute(node, "y2")? - numeric_attribute(node, "y1")?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest as _, Sha256};

    #[test]
    fn cyberpunk_flowchart_png_scope_reports_only_visible_layering() {
        let svg_mechanisms = CELL_MECHANISMS
            .into_iter()
            .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            cyberpunk_flowchart_png_mechanisms(&svg_mechanisms).unwrap(),
            BTreeMap::from([(
                ReferenceThemeMechanism::CanvasLayering,
                C6ObservedMechanismDisposition::Applied,
            )])
        );

        let mut missing = svg_mechanisms.clone();
        missing.remove(&ReferenceThemeMechanism::CanvasLayering);
        assert!(cyberpunk_flowchart_png_mechanisms(&missing).is_err());

        let mut non_applied = svg_mechanisms;
        non_applied.insert(
            ReferenceThemeMechanism::CanvasLayering,
            C6ObservedMechanismDisposition::Residual,
        );
        assert!(cyberpunk_flowchart_png_mechanisms(&non_applied).is_err());
    }

    fn contract() -> CyberpunkFlowchartProofContract<'static> {
        CyberpunkFlowchartProofContract {
            canvas: "#020617",
            node_stroke: "#22d3ee",
            node_stroke_width: 2.0,
            blend_mode: "screen",
            tile_size_px: [24.0, 24.0],
            grid_stops: [
                (0.0, "#22d3ee33"),
                (50.0, "#22d3ee33"),
                (51.0, "#22d3ee00"),
                (100.0, "#22d3ee00"),
            ],
            wash_stops: [(0.0, "#0f172a"), (100.0, "#164e63")],
            radial_stops: [(0.0, "#67e8f955"), (100.0, "#67e8f900")],
            font_family: "Excalifont",
        }
    }

    fn test_receipt(svg: &str) -> SvgArtifactReceipt {
        let digest: [u8; 32] = Sha256::digest(svg.as_bytes()).into();
        SvgArtifactReceipt::observe_svg_for_test(svg, digest)
            .expect("test SVG must produce a receipt")
    }

    fn valid_canvas_svg() -> &'static str {
        r##"<svg viewBox="0 0 100 100"><rect data-merman-theme-canvas="base" fill="#020617"/><defs><linearGradient id="layer-0-gradient" gradientUnits="userSpaceOnUse" x1="0" y1="12" x2="24" y2="12"><stop offset="0%" stop-color="#22d3ee33"/><stop offset="50%" stop-color="#22d3ee33"/><stop offset="51%" stop-color="#22d3ee00"/><stop offset="100%" stop-color="#22d3ee00"/></linearGradient><pattern id="layer-0-pattern" patternUnits="userSpaceOnUse" x="0" y="0" width="24" height="24"><rect x="0" y="0" width="24" height="24" fill="url(#layer-0-gradient)"/></pattern></defs><defs><linearGradient id="layer-1-gradient" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="100" y2="100"><stop offset="0%" stop-color="#0f172a"/><stop offset="100%" stop-color="#164e63"/></linearGradient></defs><defs><radialGradient id="layer-2-gradient" gradientUnits="userSpaceOnUse" cx="50" cy="40" r="50"><stop offset="0%" stop-color="#67e8f955"/><stop offset="100%" stop-color="#67e8f900"/></radialGradient></defs><g data-merman-theme-canvas-layer="0" style="mix-blend-mode:screen"><rect fill="url(#layer-0-pattern)"/></g><g data-merman-theme-canvas-layer="1" style="mix-blend-mode:screen"><rect fill="url(#layer-1-gradient)"/></g><g data-merman-theme-canvas-layer="2" style="mix-blend-mode:screen"><rect fill="url(#layer-2-gradient)"/></g></svg>"##
    }

    #[test]
    fn canvas_rejects_missing_reordered_or_unblended_layers() {
        let valid = valid_canvas_svg();
        let receipt = test_receipt(valid);
        assert!(prove_terminal_canvas(&receipt, contract()).is_ok());

        let layer_zero = r##"<g data-merman-theme-canvas-layer="0" style="mix-blend-mode:screen"><rect fill="url(#layer-0-pattern)"/></g>"##;
        let layer_one = r##"<g data-merman-theme-canvas-layer="1" style="mix-blend-mode:screen"><rect fill="url(#layer-1-gradient)"/></g>"##;
        let layer_two = r##"<g data-merman-theme-canvas-layer="2" style="mix-blend-mode:screen"><rect fill="url(#layer-2-gradient)"/></g>"##;
        let mutations = [
            valid.replace(layer_zero, ""),
            valid.replace(
                &format!("{layer_one}{layer_two}"),
                &format!("{layer_two}{layer_one}"),
            ),
            valid.replacen(" style=\"mix-blend-mode:screen\"", "", 1),
        ];
        for mutated in mutations {
            assert!(prove_terminal_canvas(&test_receipt(&mutated), contract()).is_err());
        }
    }

    #[test]
    fn canvas_rejects_mutated_pattern_geometry_urls_or_stops() {
        let valid = valid_canvas_svg();
        let mutations = [
            valid.replace(
                "patternUnits=\"userSpaceOnUse\"",
                "patternUnits=\"objectBoundingBox\"",
            ),
            valid.replace(
                "width=\"24\" height=\"24\"><rect",
                "width=\"25\" height=\"24\"><rect",
            ),
            valid.replace(
                "<rect x=\"0\" y=\"0\" width=\"24\"",
                "<rect x=\"1\" y=\"0\" width=\"24\"",
            ),
            valid.replace("url(#layer-0-pattern)", "url(#missing-pattern)"),
            valid.replace("url(#layer-0-gradient)", "url(#missing-gradient)"),
            valid.replacen("x2=\"24\"", "x2=\"23\"", 1),
            valid.replacen("offset=\"50%\"", "offset=\"49%\"", 1),
            valid.replacen("stop-color=\"#22d3ee33\"", "stop-color=\"#22d3ee34\"", 1),
            valid.replace("url(#layer-1-gradient)", "url(#missing-wash)"),
            valid.replacen("stop-color=\"#0f172a\"", "stop-color=\"#0f172b\"", 1),
            valid.replace("url(#layer-2-gradient)", "url(#missing-radial)"),
            valid.replacen("stop-color=\"#67e8f955\"", "stop-color=\"#67e8f954\"", 1),
        ];
        for mutated in mutations {
            assert!(prove_terminal_canvas(&test_receipt(&mutated), contract()).is_err());
        }
    }

    #[test]
    fn terminal_node_rejects_mutated_stroke() {
        let valid = r##"<svg viewBox="0 0 100 100"><g id="fixture-merman-flowchart-node-0" data-id="A" data-et="node"><rect class="basic label-container" x="0" y="0" width="20" height="10" rx="5" ry="5" style="fill:#0f172a;stroke:#22d3ee;stroke-width:2px"/><text style="font-family:Excalifont">A</text></g></svg>"##;
        assert!(prove_terminal_node(&test_receipt(valid), "A", contract()).is_ok());

        for mutated in [
            valid.replace("stroke:#22d3ee", "stroke:#e0f2fe"),
            valid.replace("stroke-width:2px", "stroke-width:1px"),
            valid.replace("font-family:Excalifont", "font-family:sans-serif"),
        ] {
            assert!(prove_terminal_node(&test_receipt(&mutated), "A", contract()).is_err());
        }
    }

    #[test]
    fn pure_background_png_cannot_prove_the_layered_canvas() {
        let view_box = [0.0, 0.0, 20.0, 20.0];
        let region = [0.0, 0.0, 20.0, 20.0];
        let canvas = [2, 6, 23];
        let mut raster = C6RasterImage::solid_for_test(20, 20, canvas);
        assert!(prove_canvas_visibility(&raster, view_box, region, canvas).is_err());

        for y in 0..10 {
            for x in 0..10 {
                raster.set_rgb_for_test(x, y, [20, 50, 70]);
            }
        }
        assert!(prove_canvas_visibility(&raster, view_box, region, canvas).is_ok());
    }
}
