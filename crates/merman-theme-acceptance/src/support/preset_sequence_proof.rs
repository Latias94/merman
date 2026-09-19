//! Exact catalog Sequence scenario, using the existing C6 observers and raster decoder.

use merman::__theme_acceptance::TargetArtifactView;
use merman::svg::ThemePreset;
use merman::{RasterOutput, RenderedDocument};
use merman_render::__private::SvgArtifactReceipt;

use super::state_proof::verify_host_admission;
use crate::observation::C6TargetArtifact;
use crate::runner::{
    C6ProofError, C6ProofResult,
    artifact_observation::sealed_svg_receipt,
    c6_sequence_proof::{
        SequenceLineProofStyle, SequenceProofContract, SequenceRectProofStyle,
        prove_sequence_png_with_raster, prove_sequence_svg, require_exact_writer_declaration,
    },
    parse_c6_hex_rgb,
};

#[derive(Clone, Copy)]
struct Palette {
    canvas: &'static str,
    actor: &'static str,
    border: &'static str,
    text: &'static str,
    note: &'static str,
    note_border: &'static str,
    note_text: &'static str,
    activation: &'static str,
    activation_border: &'static str,
    number: &'static str,
}

impl Palette {
    fn for_preset(preset: ThemePreset) -> C6ProofResult<Self> {
        // Independent expected values: catalog recipe changes require explicit requalification.
        Ok(match preset {
            ThemePreset::Brutalist => Self {
                canvas: "#f4f0e6",
                actor: "#fffdf5",
                border: "#111111",
                text: "#111111",
                note: "#ffd84d",
                note_border: "#111111",
                note_text: "#111111",
                activation: "#ff6b35",
                activation_border: "#111111",
                number: "#ffffff",
            },
            ThemePreset::Spotless => Self {
                canvas: "#f7f5ef",
                actor: "#ffffff",
                border: "#8c867b",
                text: "#1b1b1b",
                note: "#fff8e7",
                note_border: "#b89245",
                note_text: "#4a3712",
                activation: "#eeeae0",
                activation_border: "#8c867b",
                number: "#ffffff",
            },
            _ => {
                return Err(C6ProofError::new(
                    "preset-sequence-scope",
                    "undeclared preset",
                ));
            }
        })
    }
}

pub(super) fn verify(
    preset: ThemePreset,
    document: &RenderedDocument,
    png: &RasterOutput,
) -> C6ProofResult<()> {
    verify_host_admission(document, png)?;
    c6_ensure!(
        "preset-sequence-scale",
        png.plan().effective_scale == 4.0,
        "Sequence profile requires 4x PNG to observe the default 0.5px lifeline"
    );
    let palette = Palette::for_preset(preset)?;
    let line = match preset {
        ThemePreset::Spotless => "#2c2416",
        _ => palette.border,
    };
    let contract = SequenceProofContract {
        actor: SequenceRectProofStyle {
            fill: palette.actor,
            stroke: palette.border,
        },
        actor_rect_count: 4,
        require_actor_man: false,
        lifeline: Some(SequenceLineProofStyle {
            stroke: palette.border,
            width_px: None,
        }),
        message_stroke: line,
        message_line_counts: [1, 1],
        message_text_count: 2,
        note: Some(SequenceRectProofStyle {
            fill: palette.note,
            stroke: palette.note_border,
        }),
        activation: Some(SequenceRectProofStyle {
            fill: palette.activation,
            stroke: palette.activation_border,
        }),
        font_family: None,
    };
    let artifact = C6TargetArtifact::new(TargetArtifactView::from_rendered_document(document));
    let observed = sealed_svg_receipt(&artifact)?;
    check_labels(observed, palette)?;
    let proof = prove_sequence_svg(
        contract,
        &[merman_theme_fixtures::ReferenceThemeMechanism::ThemeVariables],
        "preset-sequence-svg-v1",
        artifact,
    )?;
    let (raster, _) = prove_sequence_png_with_raster(
        contract,
        &proof,
        "preset-sequence-png-v1",
        C6TargetArtifact::new(TargetArtifactView::from_raster_output(png)),
        png.plan(),
    )?;
    let view = observed.view_box();
    // Actor and note interiors exclude their border, so border ink cannot replace label ink.
    for (class, color) in [("actor", palette.text), ("note", palette.note_text)] {
        for rect in observed
            .elements()
            .iter()
            .filter(|node| node.tag_name() == "rect" && node.has_class(class))
        {
            let [x, y, width, height] = rect.bounds().ok_or_else(|| {
                C6ProofError::new("preset-sequence-geometry", "missing rectangle bounds")
            })?;
            c6_ensure!(
                "preset-sequence-ink",
                raster
                    .count_opaque_pixels_near_in_svg_rect(
                        view,
                        [x + 5.0, y + 5.0, width - 10.0, height - 10.0],
                        parse_c6_hex_rgb(color)?,
                        8
                    )
                    .is_some_and(|count| count >= 8),
                "Sequence {class} interior is missing label ink"
            );
        }
    }
    Ok(())
}

fn check_labels(receipt: &SvgArtifactReceipt, palette: Palette) -> C6ProofResult<()> {
    let root = receipt
        .root_id()
        .ok_or_else(|| C6ProofError::new("preset-sequence-root", "missing root ID"))?;
    let bases = receipt
        .elements()
        .iter()
        .filter(|node| node.attribute("data-merman-theme-canvas") == Some("base"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "preset-sequence-canvas",
        bases.len() == 1 && bases[0].attribute("fill") == Some(palette.canvas),
        "wrong catalog canvas"
    );
    for (selector, color) in [
        (
            format!(
                "#{root} text.actor,#{root} text.actor>tspan,#{root} text.text,#{root} text.text>tspan"
            ),
            palette.text,
        ),
        (
            format!("#{root} .messageText,#{root} .messageText>tspan"),
            palette.text,
        ),
        (
            format!("#{root} .noteText,#{root} .noteText>tspan"),
            palette.note_text,
        ),
        (
            format!(
                "#{root} .loopText,#{root} .loopText>tspan,#{root} .sectionTitle,#{root} .sectionTitle>tspan,#{root} .labelText,#{root} .labelText>tspan"
            ),
            palette.text,
        ),
        (
            format!("#{root} .sequenceNumber,#{root} .sequenceNumber>tspan"),
            palette.number,
        ),
    ] {
        require_exact_writer_declaration(receipt, &selector, "fill", color)?;
    }
    for (class, expected, color) in [
        ("actor", vec!["Alice", "Alice", "Bob", "Bob"], palette.text),
        ("messageText", vec!["Hello", "World"], palette.text),
        ("noteText", vec!["Remember"], palette.note_text),
        ("loopText", vec!["[Work]"], palette.text),
        ("labelText", vec!["loop"], palette.text),
        ("sequenceNumber", vec!["1", "2"], palette.number),
    ] {
        let nodes = receipt
            .elements()
            .iter()
            .filter(|node| node.tag_name() == "text" && node.has_class(class))
            .collect::<Vec<_>>();
        let mut labels = nodes.iter().map(|node| node.text()).collect::<Vec<_>>();
        labels.sort_unstable();
        c6_ensure!(
            "preset-sequence-label",
            labels == expected,
            "wrong {class} labels: {labels:?}"
        );
        for node in nodes {
            c6_ensure!(
                "preset-sequence-label",
                std::iter::once(node)
                    .chain(
                        receipt
                            .descendants_of(node.index())
                            .filter(|node| node.tag_name() == "tspan")
                    )
                    .filter_map(|node| node.style_value("fill").or_else(|| node.attribute("fill")))
                    .all(|fill| fill == color),
                "conflicting inline {class} fill"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman::svg::DiagramThemeCompiler;
    use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};
    use sha2::Digest as _;

    #[test]
    fn sequence_label_contract_rejects_changed_or_missing_text() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::Spotless)
            .unwrap();
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(
                    super::super::SEQUENCE_SPEC.source,
                    OperationControl::new(),
                    Default::default(),
                )
                .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("missing Sequence document")
        };
        let palette = Palette::for_preset(ThemePreset::Spotless).unwrap();
        for (old, new) in [("Hello", "Changed"), ("Remember", "")] {
            assert!(document.svg().contains(old));
            let changed = document.svg().replace(old, new);
            let receipt = SvgArtifactReceipt::observe_svg_for_test(
                &changed,
                sha2::Sha256::digest(changed.as_bytes()).into(),
            )
            .unwrap();
            let error = check_labels(&receipt, palette).unwrap_err();
            assert!(error.to_string().contains("preset-sequence-label"));
        }
    }
}
