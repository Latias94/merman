use merman::__theme_acceptance::TargetArtifactView;
use merman::model::family::DiagramFamilyId;
use merman::{RasterOutput, RenderedDocument};

use super::state_proof::verify_host_admission;
use crate::observation::C6TargetArtifact;
use crate::runner::{
    C6ProofError, C6ProofResult, C6RasterImage, artifact_observation::observed_svg,
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
    let receipt = observed_svg(&artifact)?;
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
            7,
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
            13,
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
            10,
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
    if declaration == "filter:none" {
        let filter = node.attribute_node("filter").unwrap();
        let mut replacements = vec![(filter.range_value(), "none".to_owned())];
        if let Some(style) = node.attribute_node("style") {
            replacements.push((
                style.range_value(),
                style.value().replace(filter.value(), "none"),
            ));
        }
        replacements.sort_unstable_by_key(|(range, _)| std::cmp::Reverse(range.start));
        for (range, value) in replacements {
            changed.replace_range(range, &value);
        }
    } else if declaration == "marker-end:none" {
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
    use crate::svg_observation::SvgArtifactObservation;
    use merman::svg::{DiagramThemeCompiler, ThemePreset};
    use merman::{MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer};

    fn document(source: &str) -> RenderedDocument {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::Cyberpunk)
            .unwrap();
        let renderer = Renderer::new().with_presentation_engine(
            MermaidEngine::new().with_site_config(super::super::preset_qualification_config()),
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

    fn observe(svg: &str) -> SvgArtifactObservation {
        SvgArtifactObservation::observe(svg).unwrap()
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
                ("rgba(243, 230, 0, 0.04)", "rgba(243, 230, 0, 0)"),
                ("rgba(41, 243, 236, 0.035)", "rgba(41, 243, 236, 0)"),
                ("in=\"SourceGraphic\"", "in=\"SourceAlpha\""),
                (
                    "gradientUnits=\"userSpaceOnUse\"",
                    "gradientUnits=\"objectBoundingBox\"",
                ),
            ];
            match spec.family {
                DiagramFamilyId::FLOWCHART => mutations.extend([
                    (
                        "stroke-width:1.5px !important",
                        "stroke-width:1px !important",
                    ),
                    ("rx:0px", "rx:4px"),
                    ("font-weight:700", "font-weight:400"),
                    ("font-weight:500", "font-weight:400"),
                    ("fill:#161F25", "fill:#F3E600"),
                    ("stroke:#F3E600", "stroke:#080D11"),
                    ("fill:#F3E600", "fill:#080D11"),
                    ("M 0 0 L 10 5 L 0 10 z", "M 0 0"),
                    (
                        "opacity:1;background-color:#080D11",
                        "opacity:0.5;background-color:#080D11",
                    ),
                    ("data-id=\"Cart\"", "data-id=\"Missing\""),
                    ("in=\"SourceAlpha\"", "in=\"SourceGraphic\""),
                    ("dx=\"-3\"", "dx=\"-2\""),
                    ("dy=\"4\"", "dy=\"0\""),
                ]),
                DiagramFamilyId::SEQUENCE => mutations.extend([
                    (
                        ".actor{stroke:#F3E600;fill:#161F25",
                        ".actor{stroke:#F3E600;fill:#ffffff",
                    ),
                    (".actor{stroke:#F3E600", ".actor{stroke:#080D11"),
                    ("fill:#F3E600", "fill:#080D11"),
                    ("font-weight:700", "font-weight:400"),
                    ("font-weight:500", "font-weight:400"),
                    ("fill:#253139", "fill:#ffffff"),
                    ("fill:#351C29", "fill:#ffffff"),
                    ("fill:#FFE9EF", "fill:#ffffff"),
                    ("rgba(41, 243, 236, 0.25)", "rgba(41, 243, 236, 0.9)"),
                    ("stdDeviation=\"1.2\"", "stdDeviation=\"1.1\""),
                    ("dx=\"-3\"", "dx=\"-2\""),
                    ("dy=\"4\"", "dy=\"0\""),
                    (".actor-line{stroke:#29F3EC", ".actor-line{stroke:#080D11"),
                    ("M -1 0 L 10 5 L 0 10 z", "M 0 0"),
                ]),
                DiagramFamilyId::XY_CHART => mutations.extend([
                    ("fill-opacity=\"0.85\"", "fill-opacity=\"1\""),
                    ("#29F3EC", "#ff0000"),
                    ("stroke-width=\"1.25\"", "stroke-width=\"2\""),
                    ("opacity=\"0.35\"", "opacity=\"1\""),
                    ("font-size=\"14\"", "font-size=\"10\""),
                    ("font-weight=\"700\"", "font-weight=\"400\""),
                    ("stdDeviation=\"1\"", "stdDeviation=\"2\""),
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
            let xml = roxmltree::Document::parse(svg).unwrap();
            let label = xml
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .unwrap();
            for node in [label, label.parent().unwrap()] {
                for declaration in ["filter:blur(1px)", "text-shadow:0 0 1px #29F3EC"] {
                    let changed = override_paint(svg, node, declaration);
                    assert!(
                        scene_semantics::verify(spec.family, &observe(&changed)).is_err(),
                        "{} accepted {declaration} on a label or ancestor",
                        spec.family
                    );
                }
            }
        }
    }

    #[test]
    fn complete_sequence_semantics_reject_missing_roles_and_inline_paint() {
        let spec = super::super::CYBERPUNK_SPECS
            .iter()
            .find(|spec| spec.family == DiagramFamilyId::SEQUENCE)
            .unwrap();
        let document = document(spec.source);
        let svg = document.svg();
        scene_semantics::verify(spec.family, &observe(svg)).unwrap();
        let xml = roxmltree::Document::parse(svg).unwrap();
        for class in ["actor", "messageText", "noteText", "loopText"] {
            let text = xml
                .descendants()
                .find(|node| {
                    node.has_tag_name("text")
                        && node.attribute("class").is_some_and(|classes| {
                            classes.split_ascii_whitespace().any(|value| value == class)
                        })
                })
                .unwrap();
            let mut missing = svg.to_owned();
            missing.replace_range(text.range(), "");
            let conflicting = override_paint(svg, text, "fill:#000000");
            for changed in [missing, conflicting] {
                assert!(
                    scene_semantics::verify(spec.family, &observe(&changed)).is_err(),
                    "Sequence accepted missing or conflicting {class}"
                );
            }
        }
        let returned_message = xml
            .descendants()
            .find(|node| {
                node.has_tag_name("line") && node.attribute("class") == Some("messageLine1")
            })
            .unwrap();
        let changed = override_paint(svg, returned_message, "stroke-dasharray:none");
        assert!(
            scene_semantics::verify(spec.family, &observe(&changed)).is_err(),
            "Sequence accepted a solid return message"
        );
    }

    #[test]
    fn body_depth_rejects_missing_moved_mismatched_or_source_painting_layers() {
        for spec in super::super::CYBERPUNK_SPECS.iter().filter(|spec| {
            matches!(
                spec.family,
                DiagramFamilyId::FLOWCHART | DiagramFamilyId::SEQUENCE
            )
        }) {
            let document = document(spec.source);
            let svg = document.svg();
            scene_semantics::verify(spec.family, &observe(svg)).unwrap();
            let xml = roxmltree::Document::parse(svg).unwrap();
            let owner = xml
                .descendants()
                .find(|node| {
                    matches!(
                        node.attribute("data-merman-theme-depth"),
                        Some("node" | "actor")
                    )
                })
                .unwrap();
            let body = owner
                .children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            let filter_id = body
                .attribute("filter")
                .and_then(crate::runner::artifact_observation::local_fragment_id)
                .unwrap();
            let filter = xml
                .descendants()
                .find(|node| node.attribute("id") == Some(filter_id))
                .unwrap();
            let final_input = filter
                .descendants()
                .rfind(|node| node.has_tag_name("feMergeNode"))
                .unwrap();
            let mut missing = svg.to_owned();
            missing.replace_range(owner.range(), "");
            let mut moved = missing.clone();
            moved.insert_str(moved.rfind("</svg>").unwrap(), &svg[owner.range()]);
            let mut mismatched = svg.to_owned();
            mismatched.replace_range(body.attribute_node("width").unwrap().range_value(), "1");
            let mut repainting = svg.to_owned();
            repainting.replace_range(
                final_input.attribute_node("in").unwrap().range_value(),
                "SourceGraphic",
            );
            let offsets = filter
                .children()
                .filter(|node| node.has_tag_name("feOffset"))
                .collect::<Vec<_>>();
            assert_eq!(offsets.len(), 2);
            let first_shadow = filter
                .children()
                .find(|node| node.has_tag_name("feComposite"))
                .unwrap()
                .attribute("result")
                .unwrap();
            let mut cascaded = svg.to_owned();
            cascaded.replace_range(
                offsets[1].attribute_node("in").unwrap().range_value(),
                first_shadow,
            );
            for (label, changed) in [
                ("missing depth", missing),
                ("depth over connections", moved),
                ("mismatched silhouette", mismatched),
                ("source in depth merge", repainting),
                ("cascaded depth instead of independent shadows", cascaded),
            ] {
                assert!(
                    scene_semantics::verify(spec.family, &observe(&changed)).is_err(),
                    "{} accepted {label}",
                    spec.family
                );
            }
            if spec.family == DiagramFamilyId::FLOWCHART {
                let diamond = xml
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("polygon")
                            && node.parent().is_some_and(|parent| {
                                parent.attribute("data-merman-theme-depth") == Some("node")
                            })
                    })
                    .unwrap();
                let transform = diamond.attribute_node("transform").unwrap();
                assert_ne!(transform.value(), "translate(0,0)");
                let mut changed = svg.to_owned();
                changed.replace_range(transform.range_value(), "translate(0,0)");
                assert!(scene_semantics::verify(spec.family, &observe(&changed)).is_err());
            }
        }
    }

    #[test]
    fn complete_scene_pixels_reject_blank_artifacts_and_missing_glyphs() {
        for spec in super::super::CYBERPUNK_SPECS {
            let document = document(spec.source);
            let original = rasterize(document.svg()).unwrap();
            let (width, height) = original.dimensions();
            let blank = C6RasterImage::solid_for_test(width, height, [8, 13, 17]);
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
