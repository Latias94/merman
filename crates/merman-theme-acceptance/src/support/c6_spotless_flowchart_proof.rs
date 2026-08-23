use std::collections::BTreeMap;

use merman_export::RasterPlan;
use merman_theme_fixtures::ReferenceThemeMechanism;

use crate::observation::C6ObservedMechanismDisposition;

use super::{
    C6BoundTargetProof, C6ProofError, C6ProofResult, C6TargetArtifact, decode_bounded_png_artifact,
    parse_c6_hex_rgb,
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
// The PNG proof observes only the repeating two-color canvas paint. Typography, node geometry,
// dash, and stroke mechanisms remain SVG-only evidence for this cell.
const PNG_MECHANISMS: [ReferenceThemeMechanism; 2] = [
    ReferenceThemeMechanism::CanvasGradient,
    ReferenceThemeMechanism::CanvasPattern,
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
    let renderer_receipt = artifact.flowchart_c6_svg_receipt().ok_or_else(|| {
        C6ProofError::new(
            "spotless-flowchart-svg-observation",
            "renderer did not seal a Flowchart SVG receipt",
        )
    })?;
    let artifact_digest = artifact.receipt().artifact_digest();
    c6_ensure!(
        "spotless-flowchart-svg-observation",
        renderer_receipt.proves_artifact(artifact_digest),
        "renderer Flowchart SVG receipt is not bound to the target artifact"
    );
    c6_ensure!(
        "spotless-flowchart-svg-contract",
        renderer_receipt.proves_spotless_flowchart_contract(
            &NODE_IDS,
            &contract.stripe_colors,
            &contract.stop_offsets,
            contract.period_px,
            contract.stroke,
            contract.stroke_width,
            &contract.dasharray(),
            contract.radius,
            contract.font_family,
        ),
        "renderer Flowchart SVG receipt does not satisfy the Spotless contract"
    );
    let view_box = renderer_receipt.view_box();
    let nodes = renderer_receipt.node_regions();
    let canvas_region = canvas_gap_region(nodes[0].region(), nodes[1].region())?;
    let renderer_receipt_digest = renderer_receipt.digest();
    let ((view_box, canvas_region), mut target_proof) = artifact.check_with(
        "spotless-flowchart-standalone-svg-v1",
        mechanisms.clone(),
        |_| Ok((view_box, canvas_region)),
    )?;
    target_proof.attach_renderer_receipt(renderer_receipt_digest);
    Ok(SpotlessFlowchartSvgProof {
        target_proof,
        mechanisms,
        view_box,
        canvas_region,
    })
}

pub(crate) fn prove_spotless_flowchart_png(
    contract: SpotlessFlowchartProofContract<'_>,
    svg_proof: &SpotlessFlowchartSvgProof,
    artifact: C6TargetArtifact<'_>,
    plan: RasterPlan,
) -> C6ProofResult<C6BoundTargetProof> {
    let mechanisms = spotless_flowchart_png_mechanisms(&svg_proof.mechanisms)?;
    let (_, target_proof) =
        artifact.check_with("spotless-flowchart-png-v1", mechanisms, |bytes| {
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
        })?;
    Ok(target_proof)
}

fn spotless_flowchart_png_mechanisms(
    svg_mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> C6ProofResult<BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>> {
    PNG_MECHANISMS
        .into_iter()
        .map(|mechanism| {
            let disposition = svg_mechanisms.get(&mechanism).copied().ok_or_else(|| {
                C6ProofError::new(
                    "spotless-flowchart-png-mechanisms",
                    format!(
                        "Spotless Flowchart PNG stripe proof lacks SVG mechanism `{}`",
                        mechanism.id()
                    ),
                )
            })?;
            c6_ensure!(
                "spotless-flowchart-png-mechanisms",
                disposition == C6ObservedMechanismDisposition::Applied,
                "Spotless Flowchart PNG cannot report `{}` from a non-Applied SVG mechanism",
                mechanism.id()
            );
            Ok((mechanism, disposition))
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spotless_flowchart_png_scope_excludes_svg_only_mechanisms() {
        let svg_mechanisms = CELL_MECHANISMS
            .into_iter()
            .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            spotless_flowchart_png_mechanisms(&svg_mechanisms).unwrap(),
            PNG_MECHANISMS
                .into_iter()
                .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
                .collect()
        );

        for mechanism in PNG_MECHANISMS {
            let mut missing = svg_mechanisms.clone();
            missing.remove(&mechanism);
            assert!(spotless_flowchart_png_mechanisms(&missing).is_err());

            let mut non_applied = svg_mechanisms.clone();
            non_applied.insert(mechanism, C6ObservedMechanismDisposition::Residual);
            assert!(spotless_flowchart_png_mechanisms(&non_applied).is_err());
        }
    }
}
