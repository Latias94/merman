use std::collections::BTreeMap;

use merman_export::RasterPlan;
use merman_render::__private::FlowchartSvgArtifactReceipt;
use merman_theme_fixtures::ReferenceThemeMechanism;

use crate::observation::C6ObservedMechanismDisposition;

use super::{
    BrutalistFlowchartFixtureContract, C6BoundTargetProof, C6ProofError, C6ProofResult,
    C6RasterImage, C6TargetArtifact, decode_bounded_png_artifact, parse_c6_hex_rgb,
};

const NODE_IDS: [&str; 6] = ["A", "B", "C", "D", "E", "F"];
const CELL_MECHANISMS: [ReferenceThemeMechanism; 3] = [
    ReferenceThemeMechanism::NthChildSelector,
    ReferenceThemeMechanism::RoundedCorners,
    ReferenceThemeMechanism::StrokeStyling,
];
// Every listed mechanism is independently covered by the raster checks below: all ordinal node
// fills, one rounded corner, and the first node's terminal stroke color and width.
const PNG_MECHANISMS: [ReferenceThemeMechanism; 3] = [
    ReferenceThemeMechanism::NthChildSelector,
    ReferenceThemeMechanism::RoundedCorners,
    ReferenceThemeMechanism::StrokeStyling,
];
const MIN_FILL_COVERAGE: f64 = 0.86;
const MIN_CORNER_COVERAGE: f64 = 0.75;
const PNG_COLOR_TOLERANCE: u8 = 10;
const PNG_STROKE_WIDTH_ERROR_PX: usize = 1;

#[derive(Clone, Debug)]
pub(crate) struct BrutalistFlowchartSvgProof {
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    target_proof: C6BoundTargetProof,
    view_box: [f64; 4],
    nodes: Vec<NodeGeometry>,
}

impl BrutalistFlowchartSvgProof {
    pub(crate) fn target_proof(&self) -> C6BoundTargetProof {
        self.target_proof.clone()
    }
}

#[derive(Clone, Copy, Debug)]
struct NodeGeometry {
    rect: [f64; 4],
}

pub(crate) fn prove_brutalist_flowchart_svg(
    contract: &BrutalistFlowchartFixtureContract<'_>,
    artifact: C6TargetArtifact<'_>,
) -> C6ProofResult<BrutalistFlowchartSvgProof> {
    let mechanisms = applied_cell_mechanisms();
    let renderer_receipt = artifact.flowchart_svg_artifact_receipt().ok_or_else(|| {
        C6ProofError::new(
            "flowchart-svg-observation",
            "renderer did not seal a Flowchart SVG receipt",
        )
    })?;
    let artifact_digest = artifact.receipt().artifact_digest();
    c6_ensure!(
        "flowchart-svg-observation",
        renderer_receipt.proves_artifact(artifact_digest),
        "renderer Flowchart SVG receipt is not bound to the target artifact"
    );
    let palette = contract
        .palette_colors
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    c6_ensure!(
        "flowchart-svg-contract",
        proves_brutalist_flowchart_contract(
            renderer_receipt,
            &palette,
            contract.border.color(),
            f64::from(contract.border.width_px()),
            f64::from(contract.radius_px),
        ),
        "renderer Flowchart SVG receipt does not satisfy the Brutalist contract"
    );
    let view_box = renderer_receipt.view_box();
    let nodes = renderer_receipt
        .node_observations()
        .iter()
        .map(|node| NodeGeometry {
            rect: node.region(),
        })
        .collect::<Vec<_>>();
    let ((view_box, nodes), target_proof) = artifact.check_with(
        "brutalist-flowchart-standalone-svg-v1",
        mechanisms.clone(),
        |_| Ok((view_box, nodes)),
    )?;
    Ok(BrutalistFlowchartSvgProof {
        mechanisms,
        target_proof,
        view_box,
        nodes,
    })
}

fn proves_brutalist_flowchart_contract(
    receipt: &FlowchartSvgArtifactReceipt,
    palette: &[&str],
    border: &str,
    stroke_width: f64,
    radius: f64,
) -> bool {
    receipt.has_native_text()
        && !receipt.has_foreign_object()
        && !receipt.has_prepared_tokens()
        && receipt.node_observations().len() == NODE_IDS.len()
        && palette.len() == 3
        && receipt
            .node_observations()
            .iter()
            .enumerate()
            .all(|(ordinal, node)| {
                node.id() == NODE_IDS[ordinal]
                    && node.fill() == Some(palette[ordinal % palette.len()])
                    && node.stroke() == border
                    && approx_eq(node.stroke_width(), stroke_width)
                    && approx_eq(node.radius_x(), radius)
                    && approx_eq(node.radius_y(), radius)
            })
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-6
}

pub(crate) fn prove_brutalist_flowchart_png(
    contract: &BrutalistFlowchartFixtureContract<'_>,
    svg_proof: &BrutalistFlowchartSvgProof,
    artifact: C6TargetArtifact<'_>,
    plan: RasterPlan,
) -> C6ProofResult<C6BoundTargetProof> {
    let mechanisms = brutalist_flowchart_png_mechanisms(&svg_proof.mechanisms)?;
    let (_, target_proof) =
        artifact.check_with("brutalist-flowchart-png-v1", mechanisms, |bytes| {
            let raster = decode_bounded_png_artifact(bytes, plan)?;
            for (ordinal, node) in svg_proof.nodes.iter().enumerate() {
                raster.prove_opaque_color_coverage_in_svg_rect(
                    svg_proof.view_box,
                    fill_region(
                        node.rect,
                        f64::from(contract.radius_px),
                        f64::from(contract.border.width_px()),
                    )?,
                    parse_c6_hex_rgb(
                        &contract.palette_colors[ordinal % contract.palette_colors.len()],
                    )?,
                    PNG_COLOR_TOLERANCE,
                    MIN_FILL_COVERAGE,
                    "flowchart-png-node-fill",
                    "Flowchart node fill",
                )?;
            }

            let first = svg_proof.nodes[0].rect;
            prove_stroke_width(
                &raster,
                svg_proof.view_box,
                first,
                f64::from(contract.radius_px),
                f64::from(contract.border.width_px()),
                parse_c6_hex_rgb(contract.border.color())?,
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
        })?;
    Ok(target_proof)
}

fn brutalist_flowchart_png_mechanisms(
    svg_mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> C6ProofResult<BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>> {
    PNG_MECHANISMS
        .into_iter()
        .map(|mechanism| {
            let disposition = svg_mechanisms.get(&mechanism).copied().ok_or_else(|| {
                C6ProofError::new(
                    "brutalist-flowchart-png-mechanisms",
                    format!(
                        "Brutalist Flowchart PNG proof lacks SVG mechanism `{}`",
                        mechanism.id()
                    ),
                )
            })?;
            c6_ensure!(
                "brutalist-flowchart-png-mechanisms",
                disposition == C6ObservedMechanismDisposition::Applied,
                "Brutalist Flowchart PNG cannot report `{}` from a non-Applied SVG mechanism",
                mechanism.id()
            );
            Ok((mechanism, disposition))
        })
        .collect()
}

fn applied_cell_mechanisms() -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
    CELL_MECHANISMS
        .into_iter()
        .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
        .collect()
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

fn prove_stroke_width(
    raster: &C6RasterImage,
    view_box: [f64; 4],
    rect: [f64; 4],
    radius: f64,
    stroke_width: f64,
    color: [u8; 3],
) -> C6ProofResult<()> {
    let [left, top, width, _] = rect;
    let clearance = radius + stroke_width + 2.0;
    let span = width - clearance * 2.0;
    c6_ensure!(
        "flowchart-png-node-stroke",
        span >= 12.0,
        "Flowchart node is too small for a stable stroke ROI"
    );
    c6_ensure!(
        "flowchart-png-node-stroke",
        view_box[3].is_finite() && view_box[3] > 0.0,
        "Flowchart PNG viewBox height must be positive"
    );
    let raster_scale = f64::from(raster.dimensions().1) / view_box[3];
    let expected = (stroke_width * raster_scale).round();
    c6_ensure!(
        "flowchart-png-node-stroke",
        expected >= 1.0 && expected <= usize::MAX as f64,
        "Flowchart PNG expected stroke width is invalid: {expected}"
    );
    let expected = expected as usize;
    let scan_radius = expected
        .checked_add(PNG_STROKE_WIDTH_ERROR_PX + 2)
        .ok_or_else(|| {
            C6ProofError::new(
                "flowchart-png-node-stroke",
                "Flowchart PNG stroke scan radius overflowed",
            )
        })?;
    for position in [0.25, 0.5, 0.75] {
        let x = left + clearance + span * position;
        let actual = raster.vertical_opaque_color_run_at_svg_point(
            view_box,
            [x, top],
            color,
            PNG_COLOR_TOLERANCE,
            scan_radius,
            "flowchart-png-node-stroke",
            "Flowchart",
        )?;
        c6_ensure!(
            "flowchart-png-node-stroke",
            actual.abs_diff(expected) <= PNG_STROKE_WIDTH_ERROR_PX,
            "Flowchart stroke width must remain within {PNG_STROKE_WIDTH_ERROR_PX}px of {expected}px, found {actual}px at x={x:.3}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_VIEW_BOX: [f64; 4] = [0.0, 0.0, 30.0, 20.0];
    const TEST_WIDTH: u32 = 60;
    const TEST_HEIGHT: u32 = 40;
    const TEST_COLOR: [u8; 3] = [0x11, 0x11, 0x11];

    #[test]
    fn brutalist_flowchart_png_scope_requires_each_raster_observed_mechanism() {
        let complete = applied_cell_mechanisms();
        assert_eq!(
            brutalist_flowchart_png_mechanisms(&complete).unwrap(),
            complete
        );

        for mechanism in PNG_MECHANISMS {
            let mut missing = complete.clone();
            missing.remove(&mechanism);
            assert!(brutalist_flowchart_png_mechanisms(&missing).is_err());

            let mut non_applied = complete.clone();
            non_applied.insert(mechanism, C6ObservedMechanismDisposition::Residual);
            assert!(brutalist_flowchart_png_mechanisms(&non_applied).is_err());
        }
    }

    #[test]
    fn partial_fill_cannot_satisfy_the_old_fixed_pixel_floor() {
        let mut raster = C6RasterImage::solid_for_test(TEST_WIDTH, TEST_HEIGHT, [0xff, 0xff, 0xff]);
        for x in 4..28 {
            raster.set_rgb_for_test(x, 4, TEST_COLOR);
        }

        let result = raster.prove_opaque_color_coverage_in_svg_rect(
            TEST_VIEW_BOX,
            [2.0, 2.0, 12.0, 10.0],
            TEST_COLOR,
            PNG_COLOR_TOLERANCE,
            MIN_FILL_COVERAGE,
            "flowchart-png-node-fill",
            "Flowchart node fill",
        );

        assert!(
            result.is_err(),
            "24 isolated fill pixels must not prove a 12x10 SVG fill region"
        );
    }

    #[test]
    fn thin_strokes_cannot_prove_a_three_pixel_contract_at_two_x() {
        for raster_stroke_width in [2, 4] {
            let mut raster =
                C6RasterImage::solid_for_test(TEST_WIDTH, TEST_HEIGHT, [0xff, 0xff, 0xff]);
            let top = 20 - raster_stroke_width / 2;
            for y in top..top + raster_stroke_width {
                for x in 4..56 {
                    raster.set_rgb_for_test(x, y, TEST_COLOR);
                }
            }

            let result = prove_stroke_width(
                &raster,
                TEST_VIEW_BOX,
                [2.0, 10.0, 26.0, 8.0],
                0.0,
                3.0,
                TEST_COLOR,
            );

            assert!(
                result.is_err(),
                "a {raster_stroke_width}-raster-pixel line is thinner than 3 SVG pixels at 2x"
            );
        }
    }
}
