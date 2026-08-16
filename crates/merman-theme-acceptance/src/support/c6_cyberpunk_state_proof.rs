use std::collections::BTreeMap;

use merman_export::RasterPlan;
use merman_render::__private::NativeSvgFilterReceipt;
use merman_theme_fixtures::ReferenceThemeMechanism;

use crate::observation::C6ObservedMechanismDisposition;

use super::{
    C6BoundTargetProof, C6ProofError, C6ProofResult, C6TargetArtifact, class_contains,
    decode_bounded_png_artifact, parse_c6_hex_rgb, parse_c6_svg_view_box, state_rect_belongs_to,
    style_value, transformed_c6_svg_rect,
};

pub(crate) const CYBERPUNK_STATE_EFFECT_ID: &str = "c6-cyberpunk-state-glow";

const STATE_IDS: [&str; 4] = ["Boot", "Scan", "Breach", "Escape"];
const CELL_MECHANISMS: [ReferenceThemeMechanism; 6] = [
    ReferenceThemeMechanism::CanvasSolid,
    ReferenceThemeMechanism::CssFilter,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::RoundedCorners,
    ReferenceThemeMechanism::StrokeStyling,
    ReferenceThemeMechanism::ThemeVariables,
];
const MIN_GLOW_DIFFERENCE: u8 = 8;
const MIN_GLOW_COVERAGE: f64 = 0.35;

#[derive(Clone, Copy)]
pub(crate) struct CyberpunkStateProofContract<'a> {
    pub(crate) canvas: &'a str,
    pub(crate) surface: &'a str,
    pub(crate) primary: &'a str,
    pub(crate) text: &'a str,
    pub(crate) border_width: f64,
    pub(crate) radius: f64,
    pub(crate) glow_blur: f64,
    pub(crate) font_family: &'a str,
}

#[derive(Clone, Debug)]
pub(crate) struct CyberpunkStateSvgProof {
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    target_proof: C6BoundTargetProof,
    view_box: [f64; 4],
    glow_region: [f64; 4],
}

impl CyberpunkStateSvgProof {
    pub(crate) fn mechanisms(
        &self,
    ) -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
        self.mechanisms.clone()
    }

    pub(crate) fn target_proof(&self) -> C6BoundTargetProof {
        self.target_proof.clone()
    }
}

pub(crate) fn prove_cyberpunk_state_svg(
    contract: CyberpunkStateProofContract<'_>,
    artifact: C6TargetArtifact<'_>,
    native_filter_receipt: NativeSvgFilterReceipt,
) -> C6ProofResult<CyberpunkStateSvgProof> {
    let mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> =
        CELL_MECHANISMS
            .into_iter()
            .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
            .collect();
    let ((view_box, glow_region), target_proof) = artifact.check_with(
        "cyberpunk-state-standalone-svg-v1",
        mechanisms.clone(),
        |bytes| check_cyberpunk_state_svg(contract, native_filter_receipt, bytes),
    )?;
    Ok(CyberpunkStateSvgProof {
        mechanisms,
        target_proof,
        view_box,
        glow_region,
    })
}

type CyberpunkStateSvgGeometry = ([f64; 4], [f64; 4]);

fn check_cyberpunk_state_svg(
    contract: CyberpunkStateProofContract<'_>,
    native_filter_receipt: NativeSvgFilterReceipt,
    bytes: &[u8],
) -> C6ProofResult<CyberpunkStateSvgGeometry> {
    let svg = std::str::from_utf8(bytes)
        .map_err(|error| C6ProofError::new("cyberpunk-state-svg-utf8", error.to_string()))?;
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("cyberpunk-state-svg-parse", error.to_string()))?;
    let root = document.root_element();
    let view_box = parse_c6_svg_view_box(root.attribute("viewBox").ok_or_else(|| {
        C6ProofError::new("cyberpunk-state-svg-root", "State SVG root lacks a viewBox")
    })?)?;

    let canvas = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("data-merman-theme-canvas") == Some("base")
        })
        .ok_or_else(|| C6ProofError::new("cyberpunk-state-svg-canvas", "missing typed canvas"))?;
    c6_ensure!(
        "cyberpunk-state-svg-canvas",
        canvas.attribute("fill") == Some(contract.canvas),
        "Cyberpunk State canvas differs from the contract"
    );

    let state_rects = STATE_IDS
        .into_iter()
        .map(|state_id| {
            let rect = find_state_rect(&document, state_id)?;
            prove_node_style(rect, state_id, contract)?;
            Ok(rect)
        })
        .collect::<C6ProofResult<Vec<_>>>()?;
    prove_filter_references(&document, &state_rects, native_filter_receipt)?;
    let first_node = transformed_c6_svg_rect(state_rects[0])?;
    let glow_region = glow_visibility_region(view_box, first_node, contract)?;
    prove_native_text(&document, svg, contract)?;

    Ok((view_box, glow_region))
}

fn prove_filter_references<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    state_rects: &[roxmltree::Node<'document, 'input>],
    native_filter_receipt: NativeSvgFilterReceipt,
) -> C6ProofResult<()> {
    prove_local_filter_references(document, state_rects)?;
    let expected_count = u32::try_from(state_rects.len())
        .map_err(|error| C6ProofError::new("cyberpunk-state-native-filter", error.to_string()))?;
    c6_ensure!(
        "cyberpunk-state-native-filter",
        native_filter_receipt.drop_shadow_count() == expected_count
            && native_filter_receipt.reference_count() == expected_count
            && native_filter_receipt
                .identity_digest()
                .iter()
                .any(|byte| *byte != 0),
        "production State native-filter receipt lacks the exact opaque filter identity"
    );
    Ok(())
}

fn prove_local_filter_references<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    state_rects: &[roxmltree::Node<'document, 'input>],
) -> C6ProofResult<()> {
    for state_rect in state_rects {
        prove_local_filter_reference(document, *state_rect)?;
    }
    Ok(())
}

fn prove_local_filter_reference<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    state_rect: roxmltree::Node<'document, 'input>,
) -> C6ProofResult<()> {
    let wrapper_id = state_rect
        .parent()
        .and_then(|node| node.attribute("id"))
        .ok_or_else(|| {
            C6ProofError::new(
                "cyberpunk-state-svg-filter-reference",
                "State node lacks its owning wrapper id",
            )
        })?;
    let expected_id = format!("{wrapper_id}-theme-effect-{CYBERPUNK_STATE_EFFECT_ID}");
    let referenced_id = state_rect
        .attribute("filter")
        .and_then(local_fragment_id)
        .ok_or_else(|| {
            C6ProofError::new(
                "cyberpunk-state-svg-filter-reference",
                "State node does not reference one local glow filter",
            )
        })?;
    let definitions = document
        .descendants()
        .filter(|node| node.attribute("id") == Some(referenced_id))
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-state-svg-filter-reference",
        referenced_id == expected_id.as_str()
            && definitions.len() == 1
            && definitions[0].has_tag_name("filter"),
        "State node glow reference `{referenced_id}` does not resolve to `{expected_id}`"
    );
    Ok(())
}

pub(crate) fn prove_cyberpunk_state_png(
    contract: CyberpunkStateProofContract<'_>,
    svg_proof: &CyberpunkStateSvgProof,
    artifact: C6TargetArtifact<'_>,
    plan: RasterPlan,
) -> C6ProofResult<C6BoundTargetProof> {
    let (_, target_proof) =
        artifact.check_with("cyberpunk-state-png-v1", svg_proof.mechanisms(), |bytes| {
            let raster = decode_bounded_png_artifact(bytes, plan)?;
            raster.prove_opaque_color_difference_coverage_in_svg_rect(
                svg_proof.view_box,
                svg_proof.glow_region,
                parse_c6_hex_rgb(contract.canvas)?,
                MIN_GLOW_DIFFERENCE,
                MIN_GLOW_COVERAGE,
                "cyberpunk-state-png-glow",
                "Cyberpunk State unclipped exterior glow",
            )?;
            Ok(())
        })?;
    Ok(target_proof)
}

fn find_state_rect<'document, 'input>(
    document: &'document roxmltree::Document<'input>,
    state_id: &str,
) -> C6ProofResult<roxmltree::Node<'document, 'input>> {
    let mut rects = document.descendants().filter(|node| {
        node.has_tag_name("rect")
            && class_contains(*node, "basic")
            && class_contains(*node, "label-container")
            && state_rect_belongs_to(*node, state_id)
    });
    let rect = rects.next().ok_or_else(|| {
        C6ProofError::new(
            "cyberpunk-state-svg-node",
            format!("missing State node `{state_id}`"),
        )
    })?;
    c6_ensure!(
        "cyberpunk-state-svg-node",
        rects.next().is_none(),
        "State node `{state_id}` was emitted more than once"
    );
    Ok(rect)
}

fn prove_node_style(
    rect: roxmltree::Node<'_, '_>,
    state_id: &str,
    contract: CyberpunkStateProofContract<'_>,
) -> C6ProofResult<()> {
    let style_number = |property| {
        style_value(rect, property)
            .map(|value| value.strip_suffix("px").unwrap_or(value))
            .and_then(|value| value.parse::<f64>().ok())
    };
    let attr_number = |name| {
        rect.attribute(name)
            .and_then(|value| value.parse::<f64>().ok())
    };
    c6_ensure!(
        "cyberpunk-state-svg-node",
        style_value(rect, "fill") == Some(contract.surface)
            && style_value(rect, "stroke") == Some(contract.primary)
            && style_number("stroke-width") == Some(contract.border_width)
            && attr_number("rx") == Some(contract.radius)
            && attr_number("ry") == Some(contract.radius),
        "State `{state_id}` differs from the Cyberpunk style contract"
    );
    Ok(())
}

fn prove_native_text(
    document: &roxmltree::Document<'_>,
    svg: &str,
    contract: CyberpunkStateProofContract<'_>,
) -> C6ProofResult<()> {
    let mut has_text = false;
    let mut has_painted_text = false;
    for node in document.descendants() {
        c6_ensure!(
            "cyberpunk-state-svg-text",
            !node.has_tag_name("foreignObject"),
            "Cyberpunk State retained foreignObject text"
        );
        has_text |= node.has_tag_name("text");
        has_painted_text |= matches!(node.tag_name().name(), "text" | "tspan")
            && style_value(node, "fill") == Some(contract.text)
            && style_value(node, "font-family")
                .is_some_and(|value| value.contains(contract.font_family));
    }
    c6_ensure!(
        "cyberpunk-state-svg-text",
        has_text && has_painted_text && !svg.contains("merman-prepared-"),
        "Cyberpunk State did not retain native terminal text and authored text paint"
    );
    Ok(())
}

fn local_fragment_id(value: &str) -> Option<&str> {
    value.strip_prefix("url(#")?.strip_suffix(')')
}

fn glow_visibility_region(
    view_box: [f64; 4],
    rect: [f64; 4],
    contract: CyberpunkStateProofContract<'_>,
) -> C6ProofResult<[f64; 4]> {
    let roi_width = (contract.glow_blur * 0.5).clamp(3.0, 5.0);
    let roi_height = (rect[3] * 0.12).clamp(4.0, 8.0);
    let gap = (contract.glow_blur * 0.35).clamp(2.0, 4.0);
    let right = rect[0] - contract.border_width / 2.0 - gap;
    let region = [
        right - roi_width,
        rect[1] + (rect[3] - roi_height) / 2.0,
        roi_width,
        roi_height,
    ];
    let view_right = view_box[0] + view_box[2];
    let view_bottom = view_box[1] + view_box[3];
    c6_ensure!(
        "cyberpunk-state-png-glow",
        region[0] >= view_box[0]
            && region[1] >= view_box[1]
            && region[0] + region[2] <= view_right
            && region[1] + region[3] <= view_bottom
            && region[0] + region[2] < rect[0] - contract.border_width / 2.0,
        "Cyberpunk State viewBox leaves no stable node-exterior glow ROI"
    );
    Ok(region)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_state_filter_url_must_resolve_to_its_own_definition() {
        const BOOT_FILTER_ID: &str = "fixture-state-Boot-0-theme-effect-c6-cyberpunk-state-glow";
        const SCAN_FILTER_ID: &str = "fixture-state-Scan-1-theme-effect-c6-cyberpunk-state-glow";
        let valid = format!(
            r##"<svg><g id="fixture-state-Boot-0"><rect filter="url(#{BOOT_FILTER_ID})"/></g><g id="fixture-state-Scan-1"><rect filter="url(#{SCAN_FILTER_ID})"/></g><defs><filter id="{BOOT_FILTER_ID}"/><filter id="{SCAN_FILTER_ID}"/></defs></svg>"##
        );
        let document = roxmltree::Document::parse(&valid).unwrap();
        let rects = document
            .descendants()
            .filter(|node| node.has_tag_name("rect"))
            .collect::<Vec<_>>();
        assert!(prove_local_filter_references(&document, &rects).is_ok());

        let mutations = [
            valid.replace(
                &format!("url(#{SCAN_FILTER_ID})"),
                "url(#fixture-state-Scan-1-theme-effect-wrong)",
            ),
            valid.replace(&format!("<filter id=\"{SCAN_FILTER_ID}\"/>"), ""),
        ];
        for mutated in mutations {
            let document = roxmltree::Document::parse(&mutated).unwrap();
            let rects = document
                .descendants()
                .filter(|node| node.has_tag_name("rect"))
                .collect::<Vec<_>>();
            assert!(
                prove_local_filter_references(&document, &rects).is_err(),
                "mutation unexpectedly retained the filter reference closure: {mutated}"
            );
        }
    }
}
