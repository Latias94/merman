use std::collections::{BTreeMap, BTreeSet};

use merman_export::RasterPlan;
use merman_theme_fixtures::ReferenceThemeMechanism;

use crate::observation::C6ObservedMechanismDisposition;

use super::{
    C6BoundTargetProof, C6ProofError, C6ProofResult, C6RasterImage, C6TargetArtifact,
    class_contains, decode_bounded_png_artifact, parse_c6_hex_rgb, parse_c6_svg_view_box,
    style_value, transformed_c6_svg_rect,
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
    pub(crate) fn mechanisms(
        &self,
    ) -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
        self.mechanisms.clone()
    }

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
    let ((view_box, canvas_region), target_proof) = artifact.check_with(
        "cyberpunk-flowchart-standalone-svg-v1",
        mechanisms.clone(),
        |bytes| check_cyberpunk_flowchart_svg(contract, bytes),
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
    bytes: &[u8],
) -> C6ProofResult<([f64; 4], [f64; 4])> {
    let svg = std::str::from_utf8(bytes)
        .map_err(|error| C6ProofError::new("cyberpunk-flowchart-svg-utf8", error.to_string()))?;
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("cyberpunk-flowchart-svg-parse", error.to_string()))?;
    let root = document.root_element();
    let view_box = parse_c6_svg_view_box(root.attribute("viewBox").ok_or_else(|| {
        C6ProofError::new(
            "cyberpunk-flowchart-svg-root",
            "Flowchart SVG root lacks a viewBox",
        )
    })?)?;

    prove_terminal_canvas(&document, contract)?;
    let nodes = NODE_IDS
        .into_iter()
        .map(|node_id| prove_terminal_node(&document, node_id, contract))
        .collect::<C6ProofResult<Vec<_>>>()?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-text",
        document.descendants().any(|node| node.has_tag_name("text"))
            && !document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
            && !svg.contains("merman-prepared-"),
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
    let (_, target_proof) = artifact.check_with(
        "cyberpunk-flowchart-png-v1",
        svg_proof.mechanisms(),
        |bytes| {
            let raster = decode_bounded_png_artifact(bytes, plan)?;
            prove_canvas_visibility(
                &raster,
                svg_proof.view_box,
                svg_proof.canvas_region,
                parse_c6_hex_rgb(contract.canvas)?,
            )
        },
    )?;
    Ok(target_proof)
}

fn prove_terminal_canvas(
    document: &roxmltree::Document<'_>,
    contract: CyberpunkFlowchartProofContract<'_>,
) -> C6ProofResult<()> {
    let bases = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect") && node.attribute("data-merman-theme-canvas") == Some("base")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        bases.len() == 1 && bases[0].attribute("fill") == Some(contract.canvas),
        "Cyberpunk Flowchart requires one terminal solid canvas base"
    );

    let layers = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g") && node.attribute("data-merman-theme-canvas-layer").is_some()
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
                && style_value(*layer, "mix-blend-mode") == Some(contract.blend_mode)
                && layer.attribute("opacity").is_none()
                && layer.attribute("transform").is_none(),
            "Cyberpunk Flowchart canvas layer {index} lost its order or blend mode"
        );
    }

    let mut resource_ids = BTreeSet::new();
    prove_tiled_linear_layer(document, layers[0], contract, &mut resource_ids)?;
    prove_linear_wash_layer(document, layers[1], contract, &mut resource_ids)?;
    prove_radial_layer(document, layers[2], contract, &mut resource_ids)?;
    Ok(())
}

fn prove_tiled_linear_layer<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    layer: roxmltree::Node<'document, 'input>,
    contract: CyberpunkFlowchartProofContract<'_>,
    resource_ids: &mut BTreeSet<&'document str>,
) -> C6ProofResult<()> {
    let source = only_element_child(layer, "Cyberpunk Flowchart grid layer")?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        source.has_tag_name("rect"),
        "Cyberpunk Flowchart grid layer must contain one terminal rect"
    );
    let pattern = resolve_local_paint(document, source, "pattern", "grid layer", resource_ids)?;
    let [tile_width, tile_height] = contract.tile_size_px;
    c6_ensure!(
        "cyberpunk-flowchart-svg-pattern",
        pattern.attribute("patternUnits") == Some("userSpaceOnUse")
            && numeric_attribute(pattern, "x").is_ok_and(|value| approx_eq(value, 0.0))
            && numeric_attribute(pattern, "y").is_ok_and(|value| approx_eq(value, 0.0))
            && numeric_attribute(pattern, "width").is_ok_and(|value| approx_eq(value, tile_width))
            && numeric_attribute(pattern, "height")
                .is_ok_and(|value| approx_eq(value, tile_height)),
        "Cyberpunk Flowchart grid pattern geometry differs from the fixed tile"
    );
    let tile = only_element_child(pattern, "Cyberpunk Flowchart grid pattern")?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-pattern",
        tile.has_tag_name("rect")
            && numeric_attribute(tile, "x").is_ok_and(|value| approx_eq(value, 0.0))
            && numeric_attribute(tile, "y").is_ok_and(|value| approx_eq(value, 0.0))
            && numeric_attribute(tile, "width").is_ok_and(|value| approx_eq(value, tile_width))
            && numeric_attribute(tile, "height").is_ok_and(|value| approx_eq(value, tile_height)),
        "Cyberpunk Flowchart grid tile does not cover the fixed pattern bounds"
    );
    let gradient =
        resolve_local_paint(document, tile, "linearGradient", "grid tile", resource_ids)?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-pattern",
        gradient.attribute("gradientUnits") == Some("userSpaceOnUse")
            && gradient.attribute("spreadMethod").is_none()
            && numeric_attribute(gradient, "x1").is_ok_and(|value| approx_eq(value, 0.0))
            && numeric_attribute(gradient, "y1")
                .is_ok_and(|value| approx_eq(value, tile_height / 2.0))
            && numeric_attribute(gradient, "x2").is_ok_and(|value| approx_eq(value, tile_width))
            && numeric_attribute(gradient, "y2")
                .is_ok_and(|value| approx_eq(value, tile_height / 2.0)),
        "Cyberpunk Flowchart grid must retain one non-repeating tile-local linear gradient"
    );
    prove_gradient_stops(gradient, &contract.grid_stops, "grid")
}

fn prove_linear_wash_layer<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    layer: roxmltree::Node<'document, 'input>,
    contract: CyberpunkFlowchartProofContract<'_>,
    resource_ids: &mut BTreeSet<&'document str>,
) -> C6ProofResult<()> {
    let source = only_element_child(layer, "Cyberpunk Flowchart linear wash layer")?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        source.has_tag_name("rect"),
        "Cyberpunk Flowchart linear wash layer must contain one terminal rect"
    );
    let gradient = resolve_local_paint(
        document,
        source,
        "linearGradient",
        "linear wash layer",
        resource_ids,
    )?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        gradient.attribute("gradientUnits") == Some("userSpaceOnUse")
            && gradient.attribute("spreadMethod").is_none()
            && linear_vector(gradient)
                .is_some_and(|[x, y]| { x > 0.0 && y > 0.0 && approx_eq(x, y) }),
        "Cyberpunk Flowchart linear wash lacks finite terminal geometry"
    );
    prove_gradient_stops(gradient, &contract.wash_stops, "linear wash")
}

fn prove_radial_layer<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    layer: roxmltree::Node<'document, 'input>,
    contract: CyberpunkFlowchartProofContract<'_>,
    resource_ids: &mut BTreeSet<&'document str>,
) -> C6ProofResult<()> {
    let source = only_element_child(layer, "Cyberpunk Flowchart radial layer")?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-canvas",
        source.has_tag_name("rect"),
        "Cyberpunk Flowchart radial layer must contain one terminal rect"
    );
    let gradient = resolve_local_paint(
        document,
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
                .all(|name| numeric_attribute(gradient, name).is_ok())
            && numeric_attribute(gradient, "r").is_ok_and(|value| value > 0.0),
        "Cyberpunk Flowchart radial glow lacks finite terminal geometry"
    );
    prove_gradient_stops(gradient, &contract.radial_stops, "radial glow")
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
                "cyberpunk-flowchart-svg-resource",
                format!("Cyberpunk Flowchart {label} lacks one local paint reference"),
            )
        })?;
    let definitions = document
        .descendants()
        .filter(|node| node.attribute("id") == Some(id))
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-resource",
        definitions.len() == 1
            && definitions[0].tag_name().name() == expected_tag
            && resource_ids.insert(id),
        "Cyberpunk Flowchart {label} paint `{id}` does not uniquely resolve to {expected_tag}"
    );
    Ok(definitions[0])
}

fn only_element_child<'document, 'input>(
    node: roxmltree::Node<'document, 'input>,
    label: &str,
) -> C6ProofResult<roxmltree::Node<'document, 'input>> {
    let elements = node
        .children()
        .filter(|child| child.is_element())
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
    gradient: roxmltree::Node<'_, '_>,
    expected: &[(f64, &str); N],
    label: &str,
) -> C6ProofResult<()> {
    let stops = gradient
        .children()
        .filter(|child| child.is_element())
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-gradient",
        stops.len() == expected.len()
            && stops.iter().zip(expected).all(|(stop, (offset, color))| {
                stop.has_tag_name("stop")
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
    document: &roxmltree::Document<'_>,
    node_id: &str,
    contract: CyberpunkFlowchartProofContract<'_>,
) -> C6ProofResult<[f64; 4]> {
    let marker = format!("-flowchart-{node_id}-");
    let wrappers = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g") && node.attribute("id").is_some_and(|id| id.contains(&marker))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-flowchart-svg-node",
        wrappers.len() == 1,
        "expected one Flowchart wrapper for `{node_id}`, got {}",
        wrappers.len()
    );
    let rects = wrappers[0]
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && class_contains(*node, "basic")
                && class_contains(*node, "label-container")
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
        style_value(rect, "stroke") == Some(contract.node_stroke)
            && style_number(rect, "stroke-width")
                .is_some_and(|value| approx_eq(value, contract.node_stroke_width))
            && subtree_style_contains(wrappers[0], "font-family", contract.font_family),
        "Flowchart node `{node_id}` differs from the Cyberpunk stroke contract"
    );
    transformed_c6_svg_rect(rect)
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

fn local_fragment_id(value: &str) -> Option<&str> {
    value.strip_prefix("url(#")?.strip_suffix(')')
}

fn percent_value(value: &str) -> Option<f64> {
    value
        .strip_suffix('%')?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn style_number(node: roxmltree::Node<'_, '_>, property: &str) -> Option<f64> {
    let value = style_value(node, property)?;
    value
        .strip_suffix("px")
        .unwrap_or(value)
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn subtree_style_contains(node: roxmltree::Node<'_, '_>, property: &str, value: &str) -> bool {
    node.descendants()
        .filter_map(|descendant| style_value(descendant, property))
        .any(|actual| actual.contains(value))
}

fn numeric_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> C6ProofResult<f64> {
    let value = node
        .attribute(name)
        .ok_or_else(|| {
            C6ProofError::new(
                "cyberpunk-flowchart-svg-geometry",
                format!("missing {name}"),
            )
        })?
        .parse::<f64>()
        .map_err(|error| {
            C6ProofError::new("cyberpunk-flowchart-svg-geometry", error.to_string())
        })?;
    c6_ensure!(
        "cyberpunk-flowchart-svg-geometry",
        value.is_finite(),
        "Flowchart SVG {name} is not finite"
    );
    Ok(value)
}

fn linear_vector(node: roxmltree::Node<'_, '_>) -> Option<[f64; 2]> {
    Some([
        numeric_attribute(node, "x2").ok()? - numeric_attribute(node, "x1").ok()?,
        numeric_attribute(node, "y2").ok()? - numeric_attribute(node, "y1").ok()?,
    ])
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn valid_canvas_svg() -> &'static str {
        r##"<svg><rect data-merman-theme-canvas="base" fill="#020617"/><defs><linearGradient id="layer-0-gradient" gradientUnits="userSpaceOnUse" x1="0" y1="12" x2="24" y2="12"><stop offset="0%" stop-color="#22d3ee33"/><stop offset="50%" stop-color="#22d3ee33"/><stop offset="51%" stop-color="#22d3ee00"/><stop offset="100%" stop-color="#22d3ee00"/></linearGradient><pattern id="layer-0-pattern" patternUnits="userSpaceOnUse" x="0" y="0" width="24" height="24"><rect x="0" y="0" width="24" height="24" fill="url(#layer-0-gradient)"/></pattern></defs><defs><linearGradient id="layer-1-gradient" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="100" y2="100"><stop offset="0%" stop-color="#0f172a"/><stop offset="100%" stop-color="#164e63"/></linearGradient></defs><defs><radialGradient id="layer-2-gradient" gradientUnits="userSpaceOnUse" cx="50" cy="40" r="50"><stop offset="0%" stop-color="#67e8f955"/><stop offset="100%" stop-color="#67e8f900"/></radialGradient></defs><g data-merman-theme-canvas-layer="0" style="mix-blend-mode:screen"><rect fill="url(#layer-0-pattern)"/></g><g data-merman-theme-canvas-layer="1" style="mix-blend-mode:screen"><rect fill="url(#layer-1-gradient)"/></g><g data-merman-theme-canvas-layer="2" style="mix-blend-mode:screen"><rect fill="url(#layer-2-gradient)"/></g></svg>"##
    }

    #[test]
    fn canvas_rejects_missing_reordered_or_unblended_layers() {
        let valid = valid_canvas_svg();
        let document = roxmltree::Document::parse(valid).unwrap();
        assert!(prove_terminal_canvas(&document, contract()).is_ok());

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
            assert_ne!(mutated, valid, "mutation did not alter the fixture");
            let document = roxmltree::Document::parse(&mutated).unwrap();
            assert!(
                prove_terminal_canvas(&document, contract()).is_err(),
                "mutation unexpectedly retained layer ordering and blend: {mutated}"
            );
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
            assert_ne!(mutated, valid, "mutation did not alter the fixture");
            let document = roxmltree::Document::parse(&mutated).unwrap();
            assert!(
                prove_terminal_canvas(&document, contract()).is_err(),
                "mutation unexpectedly retained the resource closure: {mutated}"
            );
        }
    }

    #[test]
    fn terminal_node_rejects_mutated_stroke() {
        let valid = r##"<svg><g id="fixture-flowchart-A-0"><rect class="basic label-container" x="0" y="0" width="20" height="10" style="fill:#0f172a;stroke:#22d3ee;stroke-width:2px"/><text style="font-family:Excalifont">A</text></g></svg>"##;
        let document = roxmltree::Document::parse(valid).unwrap();
        assert!(prove_terminal_node(&document, "A", contract()).is_ok());

        for mutated in [
            valid.replace("stroke:#22d3ee", "stroke:#e0f2fe"),
            valid.replace("stroke-width:2px", "stroke-width:1px"),
            valid.replace("font-family:Excalifont", "font-family:sans-serif"),
        ] {
            let document = roxmltree::Document::parse(&mutated).unwrap();
            assert!(prove_terminal_node(&document, "A", contract()).is_err());
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
