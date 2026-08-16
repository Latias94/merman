use std::collections::BTreeMap;

use merman_export::RasterPlan;
use merman_theme_fixtures::ReferenceThemeMechanism;

use crate::observation::C6ObservedMechanismDisposition;

use super::{
    C6BoundTargetProof, C6ProofError, C6ProofResult, C6TargetArtifact, class_contains,
    decode_bounded_png_artifact, parse_c6_hex_rgb, parse_c6_svg_view_box, style_value,
    transformed_c6_svg_rect,
};

const NODE_IDS: [&str; 4] = ["A", "B", "C", "D"];
const CELL_MECHANISMS: [ReferenceThemeMechanism; 6] = [
    ReferenceThemeMechanism::CanvasGradient,
    ReferenceThemeMechanism::CanvasPattern,
    ReferenceThemeMechanism::DashArray,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::RoundedCorners,
    ReferenceThemeMechanism::StrokeStyling,
];
const COLOR_TOLERANCE: u8 = 10;
const MIN_STRIPE_COVERAGE: f64 = 0.08;

#[derive(Clone, Copy)]
pub(crate) struct SpotlessFlowchartProofContract<'a> {
    pub(crate) stripe_colors: [&'a str; 2],
    pub(crate) stop_offsets: [f64; 4],
    pub(crate) period_px: f64,
    pub(crate) stroke: &'a str,
    pub(crate) stroke_width: f64,
    pub(crate) dash_pattern: &'a [u16],
    pub(crate) radius: f64,
    pub(crate) font_family: &'a str,
}

impl SpotlessFlowchartProofContract<'_> {
    fn dasharray(self) -> String {
        self.dash_pattern
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SpotlessFlowchartSvgProof {
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    target_proof: C6BoundTargetProof,
    view_box: [f64; 4],
    canvas_region: [f64; 4],
}

impl SpotlessFlowchartSvgProof {
    pub(crate) fn mechanisms(
        &self,
    ) -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
        self.mechanisms.clone()
    }

    pub(crate) fn target_proof(&self) -> C6BoundTargetProof {
        self.target_proof.clone()
    }
}

pub(crate) fn prove_spotless_flowchart_svg(
    contract: SpotlessFlowchartProofContract<'_>,
    artifact: C6TargetArtifact<'_>,
) -> C6ProofResult<SpotlessFlowchartSvgProof> {
    let mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> =
        CELL_MECHANISMS
            .into_iter()
            .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
            .collect();
    let ((view_box, canvas_region), target_proof) = artifact.check_with(
        "spotless-flowchart-standalone-svg-v1",
        mechanisms.clone(),
        |bytes| check_spotless_flowchart_svg(contract, bytes),
    )?;
    Ok(SpotlessFlowchartSvgProof {
        target_proof,
        mechanisms,
        view_box,
        canvas_region,
    })
}

fn check_spotless_flowchart_svg(
    contract: SpotlessFlowchartProofContract<'_>,
    bytes: &[u8],
) -> C6ProofResult<([f64; 4], [f64; 4])> {
    let svg = std::str::from_utf8(bytes)
        .map_err(|error| C6ProofError::new("spotless-flowchart-svg-utf8", error.to_string()))?;
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("spotless-flowchart-svg-parse", error.to_string()))?;
    let root = document.root_element();
    let view_box = parse_c6_svg_view_box(root.attribute("viewBox").ok_or_else(|| {
        C6ProofError::new(
            "spotless-flowchart-svg-root",
            "Flowchart SVG root lacks a viewBox",
        )
    })?)?;

    prove_repeating_canvas(&document, contract)?;
    let nodes = NODE_IDS
        .into_iter()
        .map(|node_id| prove_terminal_node(&document, node_id, contract))
        .collect::<C6ProofResult<Vec<_>>>()?;
    c6_ensure!(
        "spotless-flowchart-svg-text",
        document.descendants().any(|node| node.has_tag_name("text"))
            && !document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
            && !svg.contains("merman-prepared-"),
        "Spotless Flowchart did not retain native terminal text"
    );

    Ok((view_box, canvas_gap_region(nodes[0], nodes[1])?))
}

pub(crate) fn prove_spotless_flowchart_png(
    contract: SpotlessFlowchartProofContract<'_>,
    svg_proof: &SpotlessFlowchartSvgProof,
    artifact: C6TargetArtifact<'_>,
    plan: RasterPlan,
) -> C6ProofResult<C6BoundTargetProof> {
    let (_, target_proof) = artifact.check_with(
        "spotless-flowchart-png-v1",
        svg_proof.mechanisms(),
        |bytes| {
            let raster = decode_bounded_png_artifact(bytes, plan)?;
            for color in contract.stripe_colors {
                raster.prove_opaque_color_coverage_in_svg_rect(
                    svg_proof.view_box,
                    svg_proof.canvas_region,
                    parse_c6_hex_rgb(color)?,
                    COLOR_TOLERANCE,
                    MIN_STRIPE_COVERAGE,
                    "spotless-flowchart-png-canvas",
                    "Spotless Flowchart repeating stripe",
                )?;
            }
            Ok(())
        },
    )?;
    Ok(target_proof)
}

fn prove_repeating_canvas(
    document: &roxmltree::Document<'_>,
    contract: SpotlessFlowchartProofContract<'_>,
) -> C6ProofResult<()> {
    let bases = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect") && node.attribute("data-merman-theme-canvas") == Some("base")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "spotless-flowchart-svg-canvas",
        bases.len() == 1
            && !document
                .descendants()
                .any(|node| node.attribute("data-merman-theme-canvas-layer").is_some()),
        "Spotless Flowchart requires one repeating canvas base and no additional layers"
    );
    let gradient_id = bases[0]
        .attribute("fill")
        .and_then(local_url_id)
        .ok_or_else(|| {
            C6ProofError::new(
                "spotless-flowchart-svg-canvas",
                "canvas base does not reference a terminal gradient",
            )
        })?;
    let gradients = document
        .descendants()
        .filter(|node| node.attribute("id") == Some(gradient_id))
        .collect::<Vec<_>>();
    c6_ensure!(
        "spotless-flowchart-svg-canvas",
        gradients.len() == 1 && gradients[0].has_tag_name("linearGradient"),
        "canvas gradient reference does not resolve to one local linear gradient"
    );
    let gradient = gradients[0];
    let x1 = numeric_attribute(gradient, "x1")?;
    let y1 = numeric_attribute(gradient, "y1")?;
    let vector_x = numeric_attribute(gradient, "x2")? - x1;
    let vector_y = numeric_attribute(gradient, "y2")? - y1;
    let vector_length = vector_x.hypot(vector_y);
    let stops = gradient
        .children()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    let expected_colors = [
        contract.stripe_colors[0],
        contract.stripe_colors[0],
        contract.stripe_colors[1],
        contract.stripe_colors[1],
    ];
    c6_ensure!(
        "spotless-flowchart-svg-canvas",
        contract.period_px.is_finite()
            && contract.period_px > 0.0
            && gradient.attribute("gradientUnits") == Some("userSpaceOnUse")
            && gradient.attribute("spreadMethod") == Some("repeat")
            && approx_eq(x1, 0.0)
            && approx_eq(y1, 0.0)
            && vector_x > 0.0
            && vector_y < 0.0
            && approx_eq(vector_x, -vector_y)
            && approx_eq(vector_length, contract.period_px)
            && stops.len() == contract.stop_offsets.len()
            && stops
                .iter()
                .zip(contract.stop_offsets.into_iter().zip(expected_colors))
                .all(|(stop, (offset, color))| {
                    stop.has_tag_name("stop")
                        && stop
                            .attribute("offset")
                            .and_then(percent_value)
                            .is_some_and(|actual| approx_eq(actual, offset))
                        && stop.attribute("stop-color") == Some(color)
                }),
        "terminal canvas does not retain the bounded repeating Spotless gradient"
    );
    Ok(())
}

fn prove_terminal_node(
    document: &roxmltree::Document<'_>,
    node_id: &str,
    contract: SpotlessFlowchartProofContract<'_>,
) -> C6ProofResult<[f64; 4]> {
    let wrappers = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some(node_id)
                && node.attribute("data-et") == Some("node")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "spotless-flowchart-svg-node",
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
        "spotless-flowchart-svg-node",
        rects.len() == 1,
        "expected one classic process rect for `{node_id}`, got {}",
        rects.len()
    );
    let rect = rects[0];
    let dasharray = contract.dasharray();
    c6_ensure!(
        "spotless-flowchart-svg-node",
        style_value(rect, "stroke") == Some(contract.stroke)
            && style_number(rect, "stroke-width")
                .is_some_and(|value| approx_eq(value, contract.stroke_width))
            && style_value(rect, "stroke-dasharray") == Some(dasharray.as_str())
            && ["rx", "ry"].into_iter().all(|name| {
                numeric_attribute(rect, name).is_ok_and(|value| approx_eq(value, contract.radius))
            })
            && subtree_style_contains(wrappers[0], "font-family", contract.font_family),
        "Flowchart node `{node_id}` differs from the Spotless terminal style contract"
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
        "spotless-flowchart-png-canvas",
        right - left >= 8.0 && height >= 4.0,
        "Flowchart nodes leave no stable canvas ROI above their connecting edge"
    );
    Ok([left, top, right - left, height])
}

fn local_url_id(value: &str) -> Option<&str> {
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

fn subtree_style_contains(node: roxmltree::Node<'_, '_>, property: &str, value: &str) -> bool {
    node.descendants()
        .filter_map(|descendant| style_value(descendant, property))
        .any(|actual| actual.contains(value))
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

fn numeric_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> C6ProofResult<f64> {
    let value = node
        .attribute(name)
        .ok_or_else(|| {
            C6ProofError::new("spotless-flowchart-svg-geometry", format!("missing {name}"))
        })?
        .parse::<f64>()
        .map_err(|error| C6ProofError::new("spotless-flowchart-svg-geometry", error.to_string()))?;
    c6_ensure!(
        "spotless-flowchart-svg-geometry",
        value.is_finite(),
        "Flowchart SVG {name} is not finite"
    );
    Ok(value)
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> SpotlessFlowchartProofContract<'static> {
        SpotlessFlowchartProofContract {
            stripe_colors: ["#e7e2d6", "#d2ccc0"],
            stop_offsets: [0.0, 50.0, 51.0, 100.0],
            period_px: 20.0,
            stroke: "#2c2416",
            stroke_width: 2.0,
            dash_pattern: &[6, 4],
            radius: 4.0,
            font_family: "Excalifont",
        }
    }

    fn valid_canvas_svg() -> &'static str {
        r##"<svg><rect data-merman-theme-canvas="base" fill="url(#canvas-gradient)"/><defs><linearGradient id="canvas-gradient" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="14.142136" y2="-14.142136" spreadMethod="repeat"><stop offset="0%" stop-color="#e7e2d6"/><stop offset="50%" stop-color="#e7e2d6"/><stop offset="51%" stop-color="#d2ccc0"/><stop offset="100%" stop-color="#d2ccc0"/></linearGradient></defs></svg>"##
    }

    #[test]
    fn repeating_canvas_rejects_mutated_stops_or_a_dangling_reference() {
        let valid = valid_canvas_svg();
        let document = roxmltree::Document::parse(valid).unwrap();
        assert!(prove_repeating_canvas(&document, contract()).is_ok());

        let mutations = [
            valid.replacen("offset=\"50%\"", "offset=\"0%\"", 1),
            valid.replace("url(#canvas-gradient)", "url(#missing-gradient)"),
        ];
        for mutated in mutations {
            let document = roxmltree::Document::parse(&mutated).unwrap();
            assert!(
                prove_repeating_canvas(&document, contract()).is_err(),
                "mutation unexpectedly retained the repeating paint closure: {mutated}"
            );
        }
    }

    #[test]
    fn repeating_canvas_rejects_noncanonical_gradient_geometry() {
        let valid = valid_canvas_svg();
        let mutations = [
            valid.replace(
                "gradientUnits=\"userSpaceOnUse\"",
                "gradientUnits=\"objectBoundingBox\"",
            ),
            valid.replace("x1=\"0\"", "x1=\"1\""),
            valid.replace("y1=\"0\"", "y1=\"1\""),
            valid.replace("x2=\"14.142136\"", "x2=\"10\""),
            valid.replace("x2=\"14.142136\"", "x2=\"-14.142136\""),
            valid.replace("y2=\"-14.142136\"", "y2=\"14.142136\""),
        ];

        for mutated in mutations {
            let document = roxmltree::Document::parse(&mutated).unwrap();
            assert!(
                prove_repeating_canvas(&document, contract()).is_err(),
                "mutation unexpectedly retained canonical repeating geometry: {mutated}"
            );
        }
    }
}
