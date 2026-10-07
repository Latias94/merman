use merman::__theme_acceptance::TargetArtifactView;
use merman::{DiagramFamilyId, RasterOutput, RenderedDocument};

use super::state_proof::verify_host_admission;
use crate::observation::C6TargetArtifact;
use crate::runner::{
    C6ProofError, C6ProofResult, C6RasterImage, artifact_observation::sealed_svg_receipt,
    decode_bounded_png_artifact,
};

#[path = "preset_cyberpunk_proof/scene_semantics.rs"]
mod scene_semantics;

pub(super) fn verify(
    family: DiagramFamilyId,
    document: &RenderedDocument,
    png: &RasterOutput,
) -> C6ProofResult<()> {
    verify_host_admission(document, png)?;
    c6_ensure!(
        "cyberpunk-scale",
        png.plan().effective_scale == 1.0,
        "complete scenes require 1x PNG"
    );
    let artifact = C6TargetArtifact::new(TargetArtifactView::from_rendered_document(document));
    let receipt = sealed_svg_receipt(&artifact)?;
    scene_semantics::verify(family, receipt)?;
    let raster = decode_bounded_png_artifact(png.bytes(), png.plan())?;
    verify_pixels(family, document.svg(), &raster)
}

fn inventory(family: DiagramFamilyId) -> C6ProofResult<(&'static [&'static str], usize, usize)> {
    match family {
        DiagramFamilyId::FLOWCHART => Ok((
            &[
                "Browse Products",
                "Item in Stock?",
                "Add to Cart",
                "Out of Stock",
                "Checkout",
                "Yes",
                "No",
            ],
            13,
            3,
        )),
        DiagramFamilyId::SEQUENCE => Ok((
            &[
                "Client",
                "Client",
                "API",
                "API",
                "Request data",
                "Return data",
                "Verify token",
                "loop",
                "[Each request]",
            ],
            21,
            2,
        )),
        DiagramFamilyId::XY_CHART => Ok((
            &[
                "Request Volume",
                "Window",
                "Requests",
                "A",
                "B",
                "C",
                "D",
                "0",
                "1",
                "2",
                "3",
                "4",
                "5",
                "6",
                "7",
                "8",
                "9",
                "10",
            ],
            28,
            0,
        )),
        _ => Err(C6ProofError::new("cyberpunk-scope", "undeclared family")),
    }
}

fn rasterize(svg: &str) -> C6ProofResult<C6RasterImage> {
    let failure =
        |error: &dyn std::fmt::Display| C6ProofError::new("cyberpunk-raster", error.to_string());
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .map_err(|error| failure(&error))?;
    let compatible =
        merman_render::svg::finalize_resvg_svg(svg, &session).map_err(|error| failure(&error))?;
    let (bytes, plan) = merman_export::svg_to_png_with_plan(&compatible, &Default::default())
        .map_err(|error| failure(&error))?;
    decode_bounded_png_artifact(&bytes, plan)
}

fn override_paint(svg: &str, node: roxmltree::Node<'_, '_>, declaration: &str) -> String {
    let mut changed = svg.to_owned();
    if declaration == "marker-end:none" {
        changed.replace_range(
            node.attribute_node("marker-end").unwrap().range_value(),
            "none",
        );
    } else if let Some(style) = node.attribute_node("style") {
        changed.insert_str(
            style.range_value().end,
            &format!(";{declaration}!important"),
        );
    } else {
        let offset = node.range().start + 1 + node.tag_name().name().len();
        changed.insert_str(offset, &format!(" style=\"{declaration}!important\""));
    }
    changed
}

fn require_contribution(
    original: &C6RasterImage,
    svg: &str,
    node: roxmltree::Node<'_, '_>,
    declaration: &str,
    label: &str,
) -> C6ProofResult<()> {
    let changed = rasterize(&override_paint(svg, node, declaration))?;
    c6_ensure!(
        "cyberpunk-pixels",
        original.dimensions() == changed.dimensions() && original != &changed,
        "{label} must contribute actual PNG pixels without changing output dimensions"
    );
    Ok(())
}

fn verify_pixels(
    family: DiagramFamilyId,
    svg: &str,
    original: &C6RasterImage,
) -> C6ProofResult<()> {
    let (expected_labels, effects, arrows) = inventory(family)?;
    c6_ensure!(
        "cyberpunk-raster-replay",
        *original == rasterize(svg)?,
        "mutation path must reproduce the actual facade PNG"
    );
    let xml = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("cyberpunk-xml", error.to_string()))?;
    let filtered = xml
        .descendants()
        .filter(|node| node.has_attribute("filter"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "cyberpunk-effects",
        filtered.len() == effects,
        "wrong effect count"
    );
    let mut glyph_svg = svg.to_owned();
    for terminal in filtered.into_iter().rev() {
        glyph_svg = override_paint(&glyph_svg, terminal, "filter:none");
    }
    let glyph_pixels = rasterize(&glyph_svg)?;
    let glyph_xml = roxmltree::Document::parse(&glyph_svg)
        .map_err(|error| C6ProofError::new("cyberpunk-xml", error.to_string()))?;
    let labels = glyph_xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| {
            let text = node
                .descendants()
                .filter(|child| child.is_text())
                .filter_map(|child| child.text())
                .collect::<String>();
            (node, text.trim().to_owned())
        })
        .filter(|(_, text)| !text.is_empty())
        .collect::<Vec<_>>();
    let mut actual = labels
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>();
    let mut expected = expected_labels.to_vec();
    actual.sort_unstable();
    expected.sort_unstable();
    c6_ensure!(
        "cyberpunk-labels",
        actual == expected,
        "wrong labels: {actual:?}"
    );
    for (node, text) in labels {
        require_contribution(&glyph_pixels, &glyph_svg, node, "visibility:hidden", &text)?;
    }
    for (attribute, declaration, count) in [
        ("filter", "filter:none", effects),
        ("marker-end", "marker-end:none", arrows),
        ("data-merman-theme-canvas-layer", "visibility:hidden", 3),
    ] {
        let terminals = xml
            .descendants()
            .filter(|node| node.has_attribute(attribute))
            .collect::<Vec<_>>();
        c6_ensure!(
            "cyberpunk-terminals",
            terminals.len() == count,
            "wrong {attribute} inventory"
        );
        for (index, terminal) in terminals.into_iter().enumerate() {
            require_contribution(
                original,
                svg,
                terminal,
                declaration,
                &format!("{attribute} terminal {index}"),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman::svg::{DiagramThemeCompiler, ThemePreset};
    use merman::{Engine, OperationControl, RenderOutput, RenderRequest, Renderer};
    use merman_render::__private::SvgArtifactReceipt;
    use sha2::{Digest as _, Sha256};

    fn document(source: &str) -> RenderedDocument {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::Cyberpunk)
            .unwrap();
        let renderer = Renderer::new().with_engine(
            Engine::new().with_site_config(super::super::preset_qualification_config()),
        );
        let RenderOutput::Document(Some(document)) = renderer
            .render(
                RenderRequest::document(source, OperationControl::new(), Default::default())
                    .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("missing document")
        };
        document
    }

    fn observe(svg: &str) -> SvgArtifactReceipt {
        SvgArtifactReceipt::observe_svg_for_test(svg, Sha256::digest(svg.as_bytes()).into())
            .unwrap()
    }

    #[test]
    fn complete_scene_semantics_reject_changed_canvas_geometry_typography_and_effects() {
        for spec in super::super::CYBERPUNK_SPECS {
            let document = document(spec.source);
            let svg = document.svg();
            scene_semantics::verify(spec.family, &observe(svg)).unwrap();
            scene_semantics::verify(
                spec.family,
                &observe(&svg.replace("merman-shadow-", "renamed-shadow-")),
            )
            .unwrap();
            let mut mutations = vec![
                ("mix-blend-mode:screen", "mix-blend-mode:normal"),
                ("width=\"40\"", "width=\"39\""),
                ("rgba(0, 242, 255, 0.05)", "rgba(0, 242, 255, 0)"),
                (
                    "gradientUnits=\"userSpaceOnUse\"",
                    "gradientUnits=\"objectBoundingBox\"",
                ),
            ];
            match spec.family {
                DiagramFamilyId::FLOWCHART => mutations.extend([
                    ("stroke-width:3px !important", "stroke-width:1px !important"),
                    ("rx:10px", "rx:0px"),
                    ("font-weight:600", "font-weight:400"),
                    ("M 0 0 L 10 5 L 0 10 z", "M 0 0"),
                    (
                        "opacity:1;background-color:#051423",
                        "opacity:0.5;background-color:#051423",
                    ),
                    ("data-id=\"Cart\"", "data-id=\"Missing\""),
                    ("in=\"merman-shadow-0-result\"", "in=\"SourceGraphic\""),
                ]),
                DiagramFamilyId::SEQUENCE => mutations.extend([
                    (
                        ".actor{stroke:#00f2ff;fill:#051423",
                        ".actor{stroke:#00f2ff;fill:#ffffff",
                    ),
                    ("font-weight:400", "font-weight:600"),
                    ("stroke-dasharray:3, 3", "stroke-dasharray:none"),
                    ("rgba(0, 242, 255, 0.1)", "rgba(0, 242, 255, 0.9)"),
                    ("stdDeviation=\"16\"", "stdDeviation=\"8\""),
                    ("M -1 0 L 10 5 L 0 10 z", "M 0 0"),
                ]),
                DiagramFamilyId::XY_CHART => mutations.extend([
                    ("fill-opacity=\"0.2\"", "fill-opacity=\"1\""),
                    ("#6cc6cb", "#ff0000"),
                    ("stroke-width=\"3\"", "stroke-width=\"2\""),
                    ("opacity=\"0.3\"", "opacity=\"1\""),
                    ("font-size=\"14\"", "font-size=\"10\""),
                    ("font-weight=\"600\"", "font-weight=\"400\""),
                ]),
                _ => unreachable!(),
            }
            for (before, after) in mutations {
                assert!(
                    svg.contains(before),
                    "{}: missing mutation {before}",
                    spec.family
                );
                let changed = svg.replace(before, after);
                assert!(
                    scene_semantics::verify(spec.family, &observe(&changed)).is_err(),
                    "{} accepted {before} => {after}",
                    spec.family
                );
            }
        }
    }

    #[test]
    fn complete_scene_pixels_reject_blank_artifacts_and_missing_glyphs() {
        for spec in super::super::CYBERPUNK_SPECS {
            let document = document(spec.source);
            let original = rasterize(document.svg()).unwrap();
            let (width, height) = original.dimensions();
            let blank = C6RasterImage::solid_for_test(width, height, [5, 20, 35]);
            assert!(verify_pixels(spec.family, document.svg(), &blank).is_err());
            let xml = roxmltree::Document::parse(document.svg()).unwrap();
            let label = xml
                .descendants()
                .find(|node| {
                    node.has_tag_name("text")
                        && node.descendants().any(|child| {
                            child.is_text()
                                && child.text().is_some_and(|text| !text.trim().is_empty())
                        })
                })
                .unwrap();
            let hidden = override_paint(document.svg(), label, "visibility:hidden");
            let error =
                verify_pixels(spec.family, &hidden, &rasterize(&hidden).unwrap()).unwrap_err();
            assert!(error.to_string().contains("cyberpunk-pixels"), "{error}");
        }
    }
}
