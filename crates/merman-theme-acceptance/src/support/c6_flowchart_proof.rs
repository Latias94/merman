use std::collections::BTreeMap;

use merman_export::RasterPlan;
use merman_theme_fixtures::ReferenceThemeMechanism;

use crate::observation::C6ObservedMechanismDisposition;

use super::{
    BrutalistFlowchartFixtureContract, C6ProofError, C6ProofResult, C6RasterImage, class_contains,
    decode_bounded_png_artifact, parse_c6_hex_rgb, parse_c6_svg_view_box, style_value,
    transformed_c6_svg_rect,
};

const NODE_IDS: [&str; 6] = ["A", "B", "C", "D", "E", "F"];
const CELL_MECHANISMS: [ReferenceThemeMechanism; 3] = [
    ReferenceThemeMechanism::NthChildSelector,
    ReferenceThemeMechanism::RoundedCorners,
    ReferenceThemeMechanism::StrokeStyling,
];
const MIN_FILL_PIXELS: usize = 24;
const MIN_STROKE_PIXELS: usize = 12;
const MIN_CORNER_COVERAGE: f64 = 0.75;

#[derive(Clone, Debug)]
pub(crate) struct BrutalistFlowchartSvgProof {
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    view_box: [f64; 4],
    nodes: Vec<NodeGeometry>,
}

impl BrutalistFlowchartSvgProof {
    pub(crate) fn mechanisms(
        &self,
    ) -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
        self.mechanisms.clone()
    }
}

#[derive(Clone, Copy, Debug)]
struct NodeGeometry {
    rect: [f64; 4],
}

pub(crate) fn prove_brutalist_flowchart_svg(
    contract: &BrutalistFlowchartFixtureContract<'_>,
    svg: &str,
) -> C6ProofResult<BrutalistFlowchartSvgProof> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("flowchart-svg-parse", error.to_string()))?;
    let root = document.root_element();
    let view_box = parse_c6_svg_view_box(root.attribute("viewBox").ok_or_else(|| {
        C6ProofError::new("flowchart-svg-root", "Flowchart SVG root lacks a viewBox")
    })?)?;

    c6_ensure!(
        "flowchart-svg-text",
        document.descendants().any(|node| node.has_tag_name("text"))
            && !document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
            && !svg.contains("merman-prepared-"),
        "Brutalist Flowchart did not retain terminal native SVG text"
    );

    let nodes = NODE_IDS
        .iter()
        .enumerate()
        .map(|(ordinal, node_id)| {
            let marker = format!("-flowchart-{node_id}-");
            let wrappers = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("g")
                        && node.attribute("id").is_some_and(|id| id.contains(&marker))
                })
                .collect::<Vec<_>>();
            c6_ensure!(
                "flowchart-svg-node",
                wrappers.len() == 1,
                "expected one Flowchart wrapper for {node_id}, found {}",
                wrappers.len()
            );
            let wrapper = wrappers[0];
            let rects = wrapper
                .descendants()
                .filter(|node| {
                    node.has_tag_name("rect")
                        && class_contains(*node, "basic")
                        && class_contains(*node, "label-container")
                })
                .collect::<Vec<_>>();
            c6_ensure!(
                "flowchart-svg-node",
                rects.len() == 1,
                "expected one classic process rect for {node_id}, found {}",
                rects.len()
            );
            let rect = rects[0];
            let expected_fill = &contract.palette_colors[ordinal % contract.palette_colors.len()];
            require_style(rect, "fill", expected_fill, node_id)?;
            require_style(rect, "stroke", contract.border.color(), node_id)?;
            let stroke_width = css_px(style_value(rect, "stroke-width").ok_or_else(|| {
                C6ProofError::new(
                    "flowchart-svg-node",
                    format!("Flowchart node {node_id} lacks stroke-width"),
                )
            })?)?;
            c6_ensure!(
                "flowchart-svg-node",
                approx_eq(stroke_width, f64::from(contract.border.width_px())),
                "Flowchart node {node_id} stroke width is {stroke_width}, expected {}",
                contract.border.width_px()
            );
            for name in ["rx", "ry"] {
                let radius = numeric_attribute(rect, name)?;
                c6_ensure!(
                    "flowchart-svg-node",
                    approx_eq(radius, f64::from(contract.radius_px)),
                    "Flowchart node {node_id} {name} is {radius}, expected {}",
                    contract.radius_px
                );
            }

            Ok(NodeGeometry {
                rect: transformed_c6_svg_rect(rect)?,
            })
        })
        .collect::<C6ProofResult<Vec<_>>>()?;

    Ok(BrutalistFlowchartSvgProof {
        mechanisms: applied_cell_mechanisms(),
        view_box,
        nodes,
    })
}

pub(crate) fn prove_brutalist_flowchart_png(
    contract: &BrutalistFlowchartFixtureContract<'_>,
    svg_proof: &BrutalistFlowchartSvgProof,
    bytes: &[u8],
    plan: RasterPlan,
) -> C6ProofResult<()> {
    let raster = decode_bounded_png_artifact(bytes, plan)?;
    for (ordinal, node) in svg_proof.nodes.iter().enumerate() {
        prove_roi_color(
            &raster,
            svg_proof.view_box,
            fill_region(
                node.rect,
                f64::from(contract.radius_px),
                f64::from(contract.border.width_px()),
            )?,
            parse_c6_hex_rgb(&contract.palette_colors[ordinal % contract.palette_colors.len()])?,
            MIN_FILL_PIXELS,
            "flowchart-png-node-fill",
        )?;
    }

    let first = svg_proof.nodes[0].rect;
    prove_roi_color(
        &raster,
        svg_proof.view_box,
        stroke_region(
            first,
            f64::from(contract.radius_px),
            f64::from(contract.border.width_px()),
        )?,
        parse_c6_hex_rgb(contract.border.color())?,
        MIN_STROKE_PIXELS,
        "flowchart-png-node-stroke",
    )?;

    let coverage = raster
        .rounded_corner_coverage_in_svg_rect(
            svg_proof.view_box,
            first,
            f64::from(contract.radius_px),
            f64::from(contract.border.width_px()),
            parse_c6_hex_rgb(contract.tokens.background())?,
            10,
        )
        .ok_or_else(|| {
            C6ProofError::new(
                "flowchart-png-node-rounding",
                "invalid Flowchart rounded-corner ROI",
            )
        })?;
    c6_ensure!(
        "flowchart-png-node-rounding",
        coverage.inside_total() >= 3 && coverage.outside_total() >= 3,
        "rounded corner retained too few classified pixels; inside={}, outside={}",
        coverage.inside_total(),
        coverage.outside_total()
    );
    c6_ensure!(
        "flowchart-png-node-rounding",
        coverage.inside_ratio() >= MIN_CORNER_COVERAGE
            && coverage.outside_ratio() >= MIN_CORNER_COVERAGE,
        "rounded corner coverage is incomplete; inside={:.3}, outside={:.3}",
        coverage.inside_ratio(),
        coverage.outside_ratio()
    );
    Ok(())
}

fn applied_cell_mechanisms() -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
    CELL_MECHANISMS
        .into_iter()
        .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
        .collect()
}

fn require_style(
    node: roxmltree::Node<'_, '_>,
    property: &str,
    expected: &str,
    node_id: &str,
) -> C6ProofResult<()> {
    let actual = style_value(node, property).ok_or_else(|| {
        C6ProofError::new(
            "flowchart-svg-node",
            format!("Flowchart node {node_id} lacks {property}"),
        )
    })?;
    c6_ensure!(
        "flowchart-svg-node",
        actual == expected,
        "Flowchart node {node_id} {property} is {actual}, expected {expected}"
    );
    Ok(())
}

fn fill_region(rect: [f64; 4], radius: f64, stroke_width: f64) -> C6ProofResult<[f64; 4]> {
    let [left, top, width, height] = rect;
    let inset = radius + stroke_width / 2.0 + 3.0;
    let available_width = width - inset * 2.0;
    let available_height = height - inset * 2.0;
    c6_ensure!(
        "flowchart-png-node-fill",
        available_width >= 8.0 && available_height >= 8.0,
        "Flowchart node is too small for a stable fill ROI"
    );
    Ok([
        left + inset,
        top + inset,
        available_width.min(12.0),
        available_height.min(10.0),
    ])
}

fn stroke_region(rect: [f64; 4], radius: f64, stroke_width: f64) -> C6ProofResult<[f64; 4]> {
    let [left, top, width, _] = rect;
    let clearance = radius + stroke_width + 2.0;
    let span = width - clearance * 2.0;
    c6_ensure!(
        "flowchart-png-node-stroke",
        span >= 12.0,
        "Flowchart node is too small for a stable stroke ROI"
    );
    Ok([
        left + clearance,
        top - stroke_width / 2.0 - 1.0,
        span.min(24.0),
        stroke_width + 2.0,
    ])
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
        .ok_or_else(|| C6ProofError::new(stage, "invalid Flowchart PNG proof region"))?;
    c6_ensure!(
        stage,
        count >= minimum_pixels,
        "Flowchart PNG ROI retained {count} matching pixels, expected at least {minimum_pixels}"
    );
    Ok(())
}

fn numeric_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> C6ProofResult<f64> {
    let value = node
        .attribute(name)
        .ok_or_else(|| C6ProofError::new("flowchart-svg-geometry", format!("missing {name}")))?
        .parse::<f64>()
        .map_err(|error| C6ProofError::new("flowchart-svg-geometry", error.to_string()))?;
    c6_ensure!(
        "flowchart-svg-geometry",
        value.is_finite(),
        "Flowchart SVG {name} is not finite"
    );
    Ok(value)
}

fn css_px(raw: &str) -> C6ProofResult<f64> {
    raw.trim()
        .strip_suffix("px")
        .unwrap_or(raw.trim())
        .parse::<f64>()
        .map_err(|error| C6ProofError::new("flowchart-svg-node", error.to_string()))
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-6
}
