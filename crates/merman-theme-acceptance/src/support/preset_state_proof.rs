//! Bounded terminal checks for the catalog's native State host profile.

use merman::__theme_acceptance::TargetArtifactView;
use merman::svg::ThemePreset;
use merman::{
    RasterOutput, RenderedDocument, TargetAdmissionReason, TargetAdmissionStatus, TargetFontSource,
};

use crate::observation::C6TargetArtifact;
use crate::runner::{
    C6ProofError, C6ProofResult, C6RasterImage,
    artifact_observation::{sealed_svg_receipt, subtree_style_value},
    decode_bounded_png_artifact, parse_c6_hex_rgb,
};

pub(super) fn verify(
    preset: ThemePreset,
    document: &RenderedDocument,
    png: &RasterOutput,
) -> C6ProofResult<()> {
    // These expectations are independent of the recipe builder. A palette change must update
    // the declared profile and its qualification schema, not silently redefine the oracle.
    let (canvas, surface, text, border) = match preset {
        ThemePreset::Brutalist => ("#f6f3e9", "#ffffff", "#000000", "#000000"),
        ThemePreset::Spotless => ("#EDE8DC", "#F5F1E8", "#1a1a1a", "#2C2416"),
        _ => return Err(C6ProofError::new("preset-scope", "undeclared preset")),
    };
    verify_host_admission(document, png)?;
    let artifact = C6TargetArtifact::new(TargetArtifactView::from_rendered_document(document));
    let receipt = sealed_svg_receipt(&artifact)?;
    c6_ensure!(
        "preset-native-text",
        receipt.has_native_text()
            && !receipt.has_foreign_object()
            && !receipt.has_prepared_tokens(),
        "host profile requires terminal native text"
    );
    let bases = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "rect" && node.attribute("data-merman-theme-canvas") == Some("base")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "preset-canvas",
        bases.len() == 1 && bases[0].attribute("fill") == Some(canvas),
        "catalog canvas did not reach the terminal SVG"
    );
    let nodes = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "g" && node.id().is_some_and(|id| id.contains("-state-Active-"))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "preset-state",
        nodes.len() == 1,
        "expected one Active State node"
    );
    let node = nodes[0];
    let shapes = receipt
        .elements()
        .iter()
        .filter(|element| {
            element.parent_index() == Some(node.index())
                && element.tag_name() == "rect"
                && element.has_class("basic")
                && element.has_class("label-container")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "preset-state",
        shapes.len() == 1
            && shapes[0].style_value("fill") == Some(surface)
            && shapes[0].style_value("stroke") == Some(border),
        "catalog surface/border did not reach the State writer"
    );
    let label = receipt
        .descendants_of(node.index())
        .find(|element| element.tag_name() == "g" && element.has_class("label"))
        .ok_or_else(|| C6ProofError::new("preset-text", "missing State label"))?;
    c6_ensure!(
        "preset-text",
        subtree_style_value(receipt, label, "fill") == Some(text)
            && receipt
                .descendants_of(label.index())
                .any(|element| element.tag_name() == "text" && element.text() == "Active"),
        "catalog text did not reach the terminal State label"
    );
    let bounds = shapes[0]
        .bounds()
        .ok_or_else(|| C6ProofError::new("preset-geometry", "missing State bounds"))?;
    let raster = decode_bounded_png_artifact(png.bytes(), png.plan())?;
    verify_pixels(&raster, receipt.view_box(), bounds, [canvas, surface, text])
}

pub(super) fn verify_host_admission(
    document: &RenderedDocument,
    png: &RasterOutput,
) -> C6ProofResult<()> {
    for receipt in [document.standalone_svg_admission(), png.admission()] {
        c6_ensure!(
            "preset-admission",
            receipt.status() == TargetAdmissionStatus::HostDependent
                && !receipt.reasons().is_empty()
                && receipt.reasons().iter().all(|reason| matches!(
                    reason,
                    TargetAdmissionReason::HostDependentTextLayout
                        | TargetAdmissionReason::SvgFontsNotSelfContained
                        | TargetAdmissionReason::SystemOrHostFontDependency
                )),
            "host profile rejected unexpected admission: {:?} {:?}",
            receipt.status(),
            receipt.reasons()
        );
    }
    c6_ensure!(
        "preset-font-source",
        png.admission().font_source() == TargetFontSource::System,
        "host profile requires actual system font resolution"
    );
    Ok(())
}

fn verify_pixels(
    raster: &C6RasterImage,
    view: [f64; 4],
    bounds: [f64; 4],
    [canvas, surface, text]: [&str; 3],
) -> C6ProofResult<()> {
    let inner = [
        bounds[0] + 4.0,
        bounds[1] + 4.0,
        bounds[2] - 8.0,
        bounds[3] - 8.0,
    ];
    raster.prove_opaque_color_coverage_in_svg_rect(
        view,
        [view[0], view[1], view[2], 2.0],
        parse_c6_hex_rgb(canvas)?,
        0,
        0.95,
        "preset-png-canvas",
        "canvas margin",
    )?;
    raster.prove_opaque_color_coverage_in_svg_rect(
        view,
        inner,
        parse_c6_hex_rgb(surface)?,
        0,
        0.5,
        "preset-png-surface",
        "State interior",
    )?;
    // Exclude the border so a missing label cannot pass when text and border share a color.
    c6_ensure!(
        "preset-png-text",
        raster
            .count_opaque_pixels_near_in_svg_rect(view, inner, parse_c6_hex_rgb(text)?, 8)
            .is_some_and(|count| count >= 8),
        "State interior has no visible text ink"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_profile_rejects_blank_output_and_missing_label_ink() {
        let colors = ["#f6f3e9", "#ffffff", "#000000"];
        let view = [0.0, 0.0, 120.0, 80.0];
        let bounds = [20.0, 20.0, 80.0, 40.0];
        let mut raster =
            C6RasterImage::solid_for_test(120, 80, parse_c6_hex_rgb(colors[0]).unwrap());
        assert!(
            verify_pixels(&raster, view, bounds, colors)
                .unwrap_err()
                .to_string()
                .contains("preset-png-surface")
        );
        for y in 20..60 {
            for x in 20..100 {
                raster.set_rgb_for_test(x, y, parse_c6_hex_rgb(colors[1]).unwrap());
            }
        }
        // The surface is visible, but a missing label must still fail.
        assert!(
            verify_pixels(&raster, view, bounds, colors)
                .unwrap_err()
                .to_string()
                .contains("preset-png-text")
        );
        for x in 50..58 {
            raster.set_rgb_for_test(x, 40, parse_c6_hex_rgb(colors[2]).unwrap());
        }
        verify_pixels(&raster, view, bounds, colors).unwrap();
    }
}
