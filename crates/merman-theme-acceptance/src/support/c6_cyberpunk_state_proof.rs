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
const TRANSITION_LABELS: [(&str, &str); 3] = [
    ("edge1", "acquire target"),
    ("edge2", "bypass firewall"),
    ("edge3", "exfiltrate"),
];
const CELL_MECHANISMS: [ReferenceThemeMechanism; 6] = [
    ReferenceThemeMechanism::CanvasSolid,
    ReferenceThemeMechanism::CssFilter,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::RoundedCorners,
    ReferenceThemeMechanism::StrokeStyling,
    ReferenceThemeMechanism::ThemeVariables,
];
// The raster differential independently proves only the unclipped glow/filter effect.
const PNG_MECHANISMS: [ReferenceThemeMechanism; 1] = [ReferenceThemeMechanism::CssFilter];
const MIN_GLOW_DIFFERENCE: u8 = 8;
const MIN_GLOW_COVERAGE: f64 = 0.35;

#[derive(Clone, Copy)]
pub(crate) struct CyberpunkStateProofContract<'a> {
    pub(crate) canvas: &'a str,
    pub(crate) surface: &'a str,
    pub(crate) primary: &'a str,
    pub(crate) text: &'a str,
    pub(crate) transition_surface: &'a str,
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
    let mechanisms = cyberpunk_state_png_mechanisms(&svg_proof.mechanisms)?;
    let (_, target_proof) = artifact.check_with("cyberpunk-state-png-v1", mechanisms, |bytes| {
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

fn cyberpunk_state_png_mechanisms(
    svg_mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> C6ProofResult<BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>> {
    PNG_MECHANISMS
        .into_iter()
        .map(|mechanism| {
            let disposition = svg_mechanisms.get(&mechanism).copied().ok_or_else(|| {
                C6ProofError::new(
                    "cyberpunk-state-png-mechanisms",
                    format!(
                        "Cyberpunk State PNG glow proof lacks SVG mechanism `{}`",
                        mechanism.id()
                    ),
                )
            })?;
            c6_ensure!(
                "cyberpunk-state-png-mechanisms",
                disposition == C6ObservedMechanismDisposition::Applied,
                "Cyberpunk State PNG cannot report `{}` from a non-Applied SVG mechanism",
                mechanism.id()
            );
            Ok((mechanism, disposition))
        })
        .collect()
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
    c6_ensure!(
        "cyberpunk-state-svg-text",
        document.descendants().any(|node| node.has_tag_name("text"))
            && !document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject"))
            && !svg.contains("merman-prepared-"),
        "Cyberpunk State did not retain sealed native terminal text"
    );
    prove_role_contrast("state-label", contract.text, contract.surface)?;
    prove_role_contrast(
        "transition-label",
        contract.text,
        contract.transition_surface,
    )?;

    for state_id in STATE_IDS {
        prove_state_label_occurrence(document, state_id, contract)?;
    }
    prove_transition_label_occurrences(document, contract)?;
    Ok(())
}

fn prove_state_label_occurrence(
    document: &roxmltree::Document<'_>,
    state_id: &str,
    contract: CyberpunkStateProofContract<'_>,
) -> C6ProofResult<()> {
    let surface = find_state_rect(document, state_id)?;
    c6_ensure!(
        "cyberpunk-state-svg-text-surface",
        style_value(surface, "fill") == Some(contract.surface),
        "State `{state_id}` label is not bound to its Cyberpunk surface"
    );
    let wrapper = surface.parent().ok_or_else(|| {
        C6ProofError::new(
            "cyberpunk-state-svg-text-surface",
            format!("State `{state_id}` surface lacks its terminal wrapper"),
        )
    })?;
    let labels = wrapper
        .descendants()
        .filter(|node| node.has_tag_name("g") && class_contains(*node, "label"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-state-svg-text-role",
        labels.len() == 1,
        "State `{state_id}` requires exactly one terminal label occurrence"
    );
    prove_exact_text_occurrence(
        labels[0],
        "state-label",
        state_id,
        state_id,
        contract.text,
        contract.font_family,
    )
}

fn prove_transition_label_occurrences(
    document: &roxmltree::Document<'_>,
    contract: CyberpunkStateProofContract<'_>,
) -> C6ProofResult<()> {
    let visible_edge_labels = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && class_contains(*node, "edgeLabel"))
        .filter(|node| {
            node.descendants()
                .any(|descendant| descendant.has_tag_name("text"))
        })
        .count();
    c6_ensure!(
        "cyberpunk-state-svg-text-role",
        visible_edge_labels == TRANSITION_LABELS.len(),
        "Cyberpunk State requires exactly {} visible transition-label occurrences; got {visible_edge_labels}",
        TRANSITION_LABELS.len()
    );

    for (edge_id, expected_text) in TRANSITION_LABELS {
        let labels = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("g")
                    && class_contains(*node, "label")
                    && node.attribute("data-id") == Some(edge_id)
            })
            .collect::<Vec<_>>();
        c6_ensure!(
            "cyberpunk-state-svg-text-role",
            labels.len() == 1,
            "Transition `{edge_id}` requires exactly one terminal label occurrence"
        );
        let backgrounds = labels[0]
            .descendants()
            .filter(|node| node.has_tag_name("rect") && class_contains(*node, "background"))
            .collect::<Vec<_>>();
        c6_ensure!(
            "cyberpunk-state-svg-text-surface",
            backgrounds.len() == 1
                && style_value(backgrounds[0], "fill") == Some(contract.transition_surface)
                && style_property_is_important(backgrounds[0], "fill"),
            "Transition `{edge_id}` lacks its exact terminal Cyberpunk label background"
        );
        prove_exact_text_occurrence(
            labels[0],
            "transition-label",
            edge_id,
            expected_text,
            contract.text,
            contract.font_family,
        )?;
    }
    Ok(())
}

fn prove_exact_text_occurrence(
    owner: roxmltree::Node<'_, '_>,
    role: &str,
    occurrence_id: &str,
    expected_text: &str,
    expected_fill: &str,
    expected_font_family: &str,
) -> C6ProofResult<()> {
    let texts = owner
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-state-svg-text-role",
        texts.len() == 1 && descendant_text(texts[0]) == expected_text,
        "terminal `{role}/{occurrence_id}` text differs from `{expected_text}`"
    );
    let text = texts[0];
    c6_ensure!(
        "cyberpunk-state-svg-text-paint",
        style_value(text, "fill") == Some(expected_fill)
            && style_property_is_important(text, "fill"),
        "terminal `{role}/{occurrence_id}` lacks the final typed text-paint owner"
    );
    c6_ensure!(
        "cyberpunk-state-svg-text-font",
        style_value(text, "font-family")
            .is_some_and(|value| css_family_matches(value, expected_font_family)),
        "terminal `{role}/{occurrence_id}` lacks `{expected_font_family}`"
    );
    c6_ensure!(
        "cyberpunk-state-svg-text-paint",
        text.descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .all(|node| {
                if let Some(value) = style_value(node, "fill") {
                    value == expected_fill && style_property_is_important(node, "fill")
                } else {
                    node.attribute("fill").is_none()
                }
            }),
        "terminal `{role}/{occurrence_id}` contains a conflicting tspan paint owner"
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

fn style_property_is_important(node: roxmltree::Node<'_, '_>, property: &str) -> bool {
    node.attribute("style").is_some_and(|style| {
        style.split(';').any(|declaration| {
            let Some((name, value)) = declaration.split_once(':') else {
                return false;
            };
            name.trim() == property && value.trim().ends_with("!important")
        })
    })
}

fn css_family_matches(actual: &str, expected: &str) -> bool {
    actual
        .split(',')
        .next()
        .map(str::trim)
        .map(|family| family.trim_matches(['\'', '"']))
        == Some(expected)
}

fn prove_role_contrast(role: &str, foreground: &str, background: &str) -> C6ProofResult<()> {
    let foreground = parse_c6_hex_rgb(foreground)?;
    let background = parse_c6_hex_rgb(background)?;
    let contrast = contrast_ratio(foreground, background);
    c6_ensure!(
        "cyberpunk-state-svg-text-contrast",
        contrast >= 4.5,
        "Cyberpunk State `{role}` foreground/background contrast is {contrast:.2}:1, expected at least 4.5:1"
    );
    Ok(())
}

fn contrast_ratio(first: [u8; 3], second: [u8; 3]) -> f64 {
    let first = relative_luminance(first);
    let second = relative_luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

fn relative_luminance(rgb: [u8; 3]) -> f64 {
    let [red, green, blue] = rgb.map(|channel| {
        let channel = f64::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * red + 0.7152 * green + 0.0722 * blue
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

    const TEXT_CONTRACT: CyberpunkStateProofContract<'static> = CyberpunkStateProofContract {
        canvas: "#020617",
        surface: "#0f172a",
        primary: "#22d3ee",
        text: "#e0f2fe",
        transition_surface: "#0f172a",
        border_width: 2.0,
        radius: 10.0,
        glow_blur: 8.0,
        font_family: "Excalifont",
    };

    #[test]
    fn cyberpunk_state_png_scope_excludes_canvas_font_geometry_stroke_and_foreground_claims() {
        let svg_mechanisms = CELL_MECHANISMS
            .into_iter()
            .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
            .collect();

        assert_eq!(
            cyberpunk_state_png_mechanisms(&svg_mechanisms).unwrap(),
            BTreeMap::from([(
                ReferenceThemeMechanism::CssFilter,
                C6ObservedMechanismDisposition::Applied,
            )])
        );
    }

    #[test]
    fn cyberpunk_state_png_scope_rejects_a_missing_or_non_applied_filter_mechanism() {
        let without_filter = CELL_MECHANISMS
            .into_iter()
            .filter(|mechanism| *mechanism != ReferenceThemeMechanism::CssFilter)
            .map(|mechanism| (mechanism, C6ObservedMechanismDisposition::Applied))
            .collect();
        assert!(cyberpunk_state_png_mechanisms(&without_filter).is_err());

        let non_applied = BTreeMap::from([(
            ReferenceThemeMechanism::CssFilter,
            C6ObservedMechanismDisposition::Residual,
        )]);
        assert!(cyberpunk_state_png_mechanisms(&non_applied).is_err());
    }

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

    #[test]
    fn cyberpunk_state_text_contract_covers_every_state_and_transition_occurrence() {
        let svg = state_text_proof_svg("#e0f2fe", "#0f172a", true);
        let document = roxmltree::Document::parse(&svg).unwrap();

        assert!(prove_native_text(&document, &svg, TEXT_CONTRACT).is_ok());
    }

    #[test]
    fn cyberpunk_state_text_contract_rejects_one_wrong_transition_paint_or_surface() {
        let wrong_paint = state_text_proof_svg("#333333", "#0f172a", true);
        let document = roxmltree::Document::parse(&wrong_paint).unwrap();
        assert!(prove_native_text(&document, &wrong_paint, TEXT_CONTRACT).is_err());

        let wrong_surface = state_text_proof_svg("#e0f2fe", "#ececff", true);
        let document = roxmltree::Document::parse(&wrong_surface).unwrap();
        assert!(prove_native_text(&document, &wrong_surface, TEXT_CONTRACT).is_err());

        let child_override = state_text_proof_svg("#e0f2fe", "#0f172a", true).replacen(
            "<tspan>acquire target</tspan>",
            r##"<tspan fill="#333333">acquire target</tspan>"##,
            1,
        );
        let document = roxmltree::Document::parse(&child_override).unwrap();
        assert!(prove_native_text(&document, &child_override, TEXT_CONTRACT).is_err());
    }

    #[test]
    fn cyberpunk_state_text_contract_rejects_a_missing_transition_occurrence() {
        let svg = state_text_proof_svg("#e0f2fe", "#0f172a", false);
        let document = roxmltree::Document::parse(&svg).unwrap();

        assert!(prove_native_text(&document, &svg, TEXT_CONTRACT).is_err());
    }

    #[test]
    fn cyberpunk_state_text_contract_rejects_an_unreadable_foreground_surface_pair() {
        let svg = state_text_proof_svg("#0f172a", "#0f172a", true).replace("#e0f2fe", "#0f172a");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let low_contrast = CyberpunkStateProofContract {
            text: "#0f172a",
            ..TEXT_CONTRACT
        };

        assert!(prove_native_text(&document, &svg, low_contrast).is_err());
    }

    fn state_text_proof_svg(edge2_fill: &str, edge2_surface: &str, include_edge2: bool) -> String {
        let states = STATE_IDS
            .into_iter()
            .enumerate()
            .map(|(index, state_id)| {
                format!(
                    r#"<g class="node statediagram-state" id="fixture-state-{state_id}-{index}"><rect class="basic label-container" style="fill:#0f172a !important"/><g class="label"><text style="fill:#e0f2fe !important;font-family:Excalifont !important"><tspan>{state_id}</tspan></text></g></g>"#
                )
            })
            .collect::<String>();
        let transitions = TRANSITION_LABELS
            .into_iter()
            .filter(|(edge_id, _)| include_edge2 || *edge_id != "edge2")
            .map(|(edge_id, label)| {
                let fill = if edge_id == "edge2" {
                    edge2_fill
                } else {
                    "#e0f2fe"
                };
                let surface = if edge_id == "edge2" {
                    edge2_surface
                } else {
                    "#0f172a"
                };
                format!(
                    r#"<g class="edgeLabel"><g class="label" data-id="{edge_id}"><rect class="background" style="fill:{surface} !important"/><text style="fill:{fill} !important;font-family:Excalifont !important"><tspan>{label}</tspan></text></g></g>"#
                )
            })
            .collect::<String>();
        format!(r#"<svg>{states}{transitions}</svg>"#)
    }
}
