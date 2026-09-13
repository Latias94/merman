//! Bounded terminal checks for the catalog's native Flowchart host profile.

use merman::__theme_acceptance::TargetArtifactView;
use merman::svg::ThemePreset;
use merman::{RasterOutput, RenderedDocument};
use merman_render::__private::{SvgArtifactReceipt, SvgElementObservation};

use super::state_proof::verify_host_admission;
use crate::observation::C6TargetArtifact;
use crate::runner::{
    C6ProofError, C6ProofResult, C6RasterImage,
    artifact_observation::{local_fragment_id, sealed_svg_receipt},
    c6_sequence_proof::require_exact_writer_declaration,
    decode_bounded_png_artifact, parse_c6_hex_rgb,
};

#[derive(Clone, Copy)]
struct Palette {
    canvas: &'static str,
    node: &'static str,
    node_border: &'static str,
    text: &'static str,
    cluster: &'static str,
    cluster_border: &'static str,
    edge: &'static str,
    background: &'static str,
}

impl Palette {
    fn for_preset(preset: ThemePreset) -> C6ProofResult<Self> {
        Ok(match preset {
            ThemePreset::Brutalist => Self {
                canvas: "#f4f0e6",
                node: "#fffdf5",
                node_border: "#111111",
                text: "#111111",
                cluster: "#ffe88a",
                cluster_border: "#111111",
                edge: "#111111",
                background: "#fffdf5",
            },
            ThemePreset::Spotless => Self {
                canvas: "#f7f5ef",
                node: "#ffffff",
                node_border: "#b8b2a7",
                text: "#1b1b1b",
                cluster: "#f0ece2",
                cluster_border: "#8c867b",
                edge: "#2c2416",
                background: "#f7f5ef",
            },
            ThemePreset::Cyberpunk => Self {
                canvas: "#020617",
                node: "#0f172a",
                node_border: "#22d3ee",
                text: "#e0f2fe",
                cluster: "#111827",
                cluster_border: "#22d3ee",
                edge: "#22d3ee",
                background: "#020617",
            },
            _ => {
                return Err(C6ProofError::new(
                    "preset-flowchart-scope",
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
        "preset-flowchart-scale",
        png.plan().effective_scale == 4.0,
        "Flowchart profile requires 4x PNG"
    );
    let palette = Palette::for_preset(preset)?;
    let artifact = C6TargetArtifact::new(TargetArtifactView::from_rendered_document(document));
    let receipt = sealed_svg_receipt(&artifact)?;
    let geometry = check_svg(receipt, palette)?;
    let raster = decode_bounded_png_artifact(png.bytes(), png.plan())?;
    check_pixels(&raster, &geometry, palette)
}

struct Geometry {
    view: [f64; 4],
    cluster: [f64; 4],
    nodes: [[f64; 4]; 2],
    background: [f64; 4],
}

fn check_svg(receipt: &SvgArtifactReceipt, palette: Palette) -> C6ProofResult<Geometry> {
    c6_ensure!(
        "preset-flowchart-svg",
        receipt.has_native_text()
            && !receipt.has_foreign_object()
            && !receipt.has_prepared_tokens(),
        "Flowchart requires native text"
    );

    let base = receipt
        .elements()
        .iter()
        .find(|e| e.tag_name() == "rect" && e.attribute("data-merman-theme-canvas") == Some("base"))
        .ok_or_else(|| C6ProofError::new("preset-flowchart-canvas", "missing canvas"))?;
    c6_ensure!(
        "preset-flowchart-canvas",
        base.attribute("fill") == Some(palette.canvas),
        "wrong canvas color"
    );
    let cluster = receipt
        .elements()
        .iter()
        .find(|e| {
            e.tag_name() == "g"
                && e.attribute("data-et") == Some("cluster")
                && e.attribute("data-id") == Some("phase")
        })
        .ok_or_else(|| C6ProofError::new("preset-flowchart-cluster", "missing phase cluster"))?;
    let cluster_rect = receipt
        .descendants_of(cluster.index())
        .find(|e| e.tag_name() == "rect" && e.attribute("class").is_none() && e.bounds().is_some())
        .ok_or_else(|| C6ProofError::new("preset-flowchart-cluster", "missing cluster surface"))?;
    let cluster_bounds = cluster_rect.bounds().unwrap();
    c6_ensure!(
        "preset-flowchart-cluster",
        cluster_rect.style_value("fill") == Some(palette.cluster)
            && cluster_rect.style_value("stroke") == Some(palette.cluster_border),
        "wrong cluster paint"
    );
    let cluster_title = receipt
        .descendants_of(cluster.index())
        .find(|e| e.tag_name() == "g" && e.has_class("cluster-label"))
        .ok_or_else(|| {
            C6ProofError::new("preset-flowchart-cluster-title", "missing cluster title")
        })?;
    require_text(receipt, cluster_title, "Review", palette.text)?;
    let mut node_bounds = Vec::with_capacity(2);
    for (id, label) in [("A", "Alpha"), ("B", "Beta")] {
        let nodes: Vec<_> = receipt
            .elements()
            .iter()
            .filter(|e| {
                e.tag_name() == "g"
                    && e.attribute("data-et") == Some("node")
                    && e.attribute("data-id") == Some(id)
            })
            .collect();
        c6_ensure!(
            "preset-flowchart-nodes",
            nodes.len() == 1,
            "expected one {id} node"
        );
        let node = nodes[0];
        let rect = receipt
            .descendants_of(node.index())
            .find(|e| {
                e.tag_name() == "rect" && e.has_class("basic") && e.has_class("label-container")
            })
            .ok_or_else(|| C6ProofError::new("preset-flowchart-node", "missing node surface"))?;
        c6_ensure!(
            "preset-flowchart-node",
            rect.style_value("fill") == Some(palette.node)
                && rect.style_value("stroke") == Some(palette.node_border),
            "wrong node paint"
        );
        require_text(receipt, node, label, palette.text)?;
        node_bounds.push(rect.bounds().ok_or_else(|| {
            C6ProofError::new("preset-flowchart-geometry", "missing node bounds")
        })?);
    }
    let edge = receipt
        .elements()
        .iter()
        .find(|e| {
            e.tag_name() == "path"
                && e.attribute("data-et") == Some("edge")
                && e.attribute("data-id") == Some("L_A_B_0")
        })
        .ok_or_else(|| C6ProofError::new("preset-flowchart-edge", "missing edge"))?;
    c6_ensure!(
        "preset-flowchart-edge",
        edge.style_value("stroke") == Some(palette.edge)
            && edge.attribute("d").is_some_and(|d| !d.trim().is_empty()),
        "wrong or empty edge"
    );
    let marker_id = edge
        .attribute("marker-end")
        .and_then(local_fragment_id)
        .ok_or_else(|| {
            C6ProofError::new("preset-flowchart-marker", "missing local direction marker")
        })?;
    let marker = receipt
        .elements()
        .iter()
        .find(|e| e.tag_name() == "marker" && e.id() == Some(marker_id))
        .ok_or_else(|| {
            C6ProofError::new("preset-flowchart-marker", "unresolved direction marker")
        })?;
    c6_ensure!(
        "preset-flowchart-marker",
        receipt
            .descendants_of(marker.index())
            .any(|e| e.tag_name() == "path"
                && e.has_class("arrowMarkerPath")
                && e.attribute("d") == Some("M 0 0 L 10 5 L 0 10 z")),
        "missing directed arrow shape"
    );
    let label = receipt
        .elements()
        .iter()
        .find(|e| {
            e.tag_name() == "g" && e.has_class("label") && e.attribute("data-id") == Some("L_A_B_0")
        })
        .ok_or_else(|| C6ProofError::new("preset-flowchart-edge-label", "missing Advance label"))?;
    c6_ensure!(
        "preset-flowchart-edge-label",
        label
            .parent_index()
            .and_then(|index| receipt.elements().get(index))
            .is_some_and(|parent| parent.tag_name() == "g" && parent.has_class("edgeLabel")),
        "missing edge label owner"
    );
    let bg = receipt
        .descendants_of(label.index())
        .find(|e| e.tag_name() == "rect" && e.has_class("background"))
        .ok_or_else(|| C6ProofError::new("preset-flowchart-background", "missing background"))?;
    let bg_bounds = bg.bounds().ok_or_else(|| {
        C6ProofError::new("preset-flowchart-background", "background has no area")
    })?;
    c6_ensure!(
        "preset-flowchart-background",
        bg_bounds[2] > 1.0 && bg_bounds[3] > 1.0,
        "background has zero area"
    );
    require_text(receipt, label, "Advance", palette.text)?;
    let title = receipt
        .elements()
        .iter()
        .find(|e| e.tag_name() == "text" && e.has_class("flowchartTitleText"))
        .ok_or_else(|| C6ProofError::new("preset-flowchart-title", "missing title"))?;
    require_text(receipt, title, "Approval", palette.text)?;
    let root = receipt
        .root_id()
        .ok_or_else(|| C6ProofError::new("preset-flowchart-root", "missing root"))?;
    // Check the producer's declarations, not a second CSS cascade implementation.
    for selector in [
        format!("#{root} .label text,#{root} span"),
        format!("#{root} .cluster-label text"),
        format!("#{root} .cluster text"),
        format!("#{root} .flowchartTitleText"),
        format!("#{root} .edgeLabel .label,#{root} .edgeLabel .label text,#{root} .edgeLabel span"),
    ] {
        require_exact_writer_declaration(receipt, &selector, "fill", palette.text)?;
    }
    require_exact_writer_declaration(
        receipt,
        &format!("#{root} .edgeLabel rect"),
        "fill",
        palette.background,
    )?;
    require_exact_writer_declaration(
        receipt,
        &format!("#{root} .edgeLabel rect"),
        "opacity",
        "0.5",
    )?;
    // The exact recipes retain the default marker; no explicit Marker rule is qualified here.
    require_exact_writer_declaration(receipt, &format!("#{root} .marker"), "fill", "#333333")?;
    Ok(Geometry {
        view: receipt.view_box(),
        cluster: cluster_bounds,
        nodes: [node_bounds[0], node_bounds[1]],
        background: bg_bounds,
    })
}

fn require_text(
    receipt: &SvgArtifactReceipt,
    owner: &SvgElementObservation,
    label: &str,
    color: &str,
) -> C6ProofResult<()> {
    let texts: Vec<_> = std::iter::once(owner)
        .chain(receipt.descendants_of(owner.index()))
        .filter(|e| e.tag_name() == "text")
        .collect();
    c6_ensure!(
        "preset-flowchart-label",
        texts.len() == 1 && texts[0].text() == label,
        "missing or changed {label} text"
    );
    c6_ensure!(
        "preset-flowchart-label",
        std::iter::once(owner)
            .chain(receipt.descendants_of(owner.index()))
            .filter(|e| matches!(e.tag_name(), "g" | "text" | "tspan"))
            .filter_map(|e| e.style_value("fill").or_else(|| e.attribute("fill")))
            .all(|fill| fill == color),
        "conflicting {label} fill"
    );
    Ok(())
}

fn check_pixels(
    raster: &C6RasterImage,
    geometry: &Geometry,
    palette: Palette,
) -> C6ProofResult<()> {
    let Geometry {
        view,
        cluster,
        nodes,
        background,
    } = *geometry;
    prove_surface(
        raster,
        view,
        [view[0], view[1], view[2], 2.0],
        palette.canvas,
        "canvas",
    )?;
    // Use the cluster's bottom margin, away from node, label and connection ink.
    prove_surface(
        raster,
        view,
        [
            cluster[0] + 6.0,
            cluster[1] + cluster[3] - 12.0,
            cluster[2] - 12.0,
            6.0,
        ],
        palette.cluster,
        "cluster",
    )?;
    prove_color(
        raster,
        view,
        [cluster[0] - 0.5, cluster[1] + cluster[3] / 2.0, 1.0, 8.0],
        palette.cluster_border,
        "cluster border",
    )?;
    for (bounds, label) in nodes.into_iter().zip(["Alpha", "Beta"]) {
        prove_surface(raster, view, inset(bounds, 6.0), palette.node, label)?;
        prove_color(
            raster,
            view,
            [bounds[0] - 0.5, bounds[1] + bounds[3] / 2.0, 1.0, 8.0],
            palette.node_border,
            "node border",
        )?;
        prove_ink(raster, view, inset(bounds, 6.0), palette.text, label)?;
    }
    prove_ink(
        raster,
        view,
        [cluster[0] + 6.0, cluster[1] + 6.0, cluster[2] - 12.0, 20.0],
        palette.text,
        "Review",
    )?;
    prove_ink(
        raster,
        view,
        [view[0] + 6.0, view[1] + 6.0, view[2] - 12.0, 22.0],
        palette.text,
        "Approval",
    )?;
    let [a, b] = nodes;
    let center = a[0] + a[2] / 2.0;
    // This fixed subgraph scenario has one vertical edge. Exclude its central strip from text
    // samples, even when the recipe uses exactly the same color for text and connection.
    c6_ensure!(
        "preset-flowchart-geometry",
        (center - (b[0] + b[2] / 2.0)).abs() < 0.01
            && (center - (background[0] + background[2] / 2.0)).abs() < 0.01
            && background[1] > a[1] + a[3] + 12.0
            && b[1] > background[1] + background[3] + 12.0,
        "expected separated nodes, label and vertical connection"
    );
    for region in [
        [
            background[0] + 4.0,
            background[1] + 4.0,
            background[2] / 2.0 - 8.0,
            background[3] - 8.0,
        ],
        [
            center + 4.0,
            background[1] + 4.0,
            background[2] / 2.0 - 8.0,
            background[3] - 8.0,
        ],
    ] {
        prove_ink(raster, view, region, palette.text, "Advance")?;
    }
    prove_color(
        raster,
        view,
        [center - 1.0, a[1] + a[3] + 4.0, 2.0, 8.0],
        palette.edge,
        "connection",
    )?;
    // The right wing excludes the central shaft and the target node's top border.
    prove_color(
        raster,
        view,
        [center + 1.5, b[1] - 7.0, 2.0, 4.0],
        "#333333",
        "direction marker",
    )?;
    let cluster_rgb = parse_c6_hex_rgb(palette.cluster)?;
    let bg_rgb = parse_c6_hex_rgb(palette.background)?;
    let composite =
        std::array::from_fn(|i| ((u16::from(bg_rgb[i]) + u16::from(cluster_rgb[i])) / 2) as u8);
    // Most of the rectangle must show its translucent paint. A few antialiased edge pixels
    // must not substitute for a missing background, especially for low-contrast recipes.
    raster.prove_opaque_color_coverage_in_svg_rect(
        view,
        inset(background, 1.0),
        composite,
        2,
        0.5,
        "preset-flowchart-background",
        "label background",
    )?;
    Ok(())
}

fn inset([x, y, width, height]: [f64; 4], amount: f64) -> [f64; 4] {
    [
        x + amount,
        y + amount,
        width - 2.0 * amount,
        height - 2.0 * amount,
    ]
}

fn prove_surface(
    raster: &C6RasterImage,
    view: [f64; 4],
    bounds: [f64; 4],
    color: &str,
    label: &str,
) -> C6ProofResult<()> {
    raster.prove_opaque_color_coverage_in_svg_rect(
        view,
        bounds,
        parse_c6_hex_rgb(color)?,
        0,
        0.5,
        "preset-flowchart-surface",
        label,
    )
}

fn prove_color(
    raster: &C6RasterImage,
    view: [f64; 4],
    bounds: [f64; 4],
    color: &str,
    label: &str,
) -> C6ProofResult<()> {
    c6_ensure!(
        "preset-flowchart-paint",
        raster
            .count_opaque_pixels_near_in_svg_rect(view, bounds, parse_c6_hex_rgb(color)?, 8)
            .is_some_and(|n| n >= 4),
        "{label} has no visible paint"
    );
    Ok(())
}

fn prove_ink(
    raster: &C6RasterImage,
    view: [f64; 4],
    bounds: [f64; 4],
    color: &str,
    label: &str,
) -> C6ProofResult<()> {
    c6_ensure!(
        "preset-flowchart-ink",
        bounds[2] > 0.0
            && bounds[3] > 0.0
            && raster
                .count_opaque_pixels_near_in_svg_rect(view, bounds, parse_c6_hex_rgb(color)?, 8)
                .is_some_and(|n| n >= 4),
        "{label} has no visible text ink"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman::svg::DiagramThemeCompiler;
    use merman::{Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer};
    use sha2::Digest as _;

    fn document(preset: ThemePreset) -> RenderedDocument {
        let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
        let renderer = Renderer::new().with_engine(Engine::new().with_site_config(
            MermaidConfig::from_value(serde_json::json!({"htmlLabels": false})),
        ));
        let RenderOutput::Document(Some(document)) = renderer
            .render(
                RenderRequest::document(
                    super::super::FLOWCHART_SPEC.source,
                    OperationControl::new(),
                    Default::default(),
                )
                .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("missing Flowchart document")
        };
        document
    }

    fn observe(svg: &str) -> SvgArtifactReceipt {
        SvgArtifactReceipt::observe_svg_for_test(svg, sha2::Sha256::digest(svg.as_bytes()).into())
            .unwrap()
    }

    #[test]
    fn unknown_preset_is_rejected() {
        assert!(Palette::for_preset(ThemePreset::EditorLight).is_err());
    }

    #[test]
    fn flowchart_contract_rejects_missing_or_changed_terminals() {
        for preset in [
            ThemePreset::Brutalist,
            ThemePreset::Spotless,
            ThemePreset::Cyberpunk,
        ] {
            assert_changed_terminals_rejected(preset);
        }
    }

    fn assert_changed_terminals_rejected(preset: ThemePreset) {
        let document = document(preset);
        let palette = Palette::for_preset(preset).unwrap();
        let receipt = observe(document.svg());
        assert!(check_svg(&receipt, palette).is_ok());
        let edge = receipt
            .elements()
            .iter()
            .find(|e| e.attribute("data-et") == Some("edge"))
            .unwrap();
        let reference = edge.attribute("marker-end").unwrap();
        let marker_id = local_fragment_id(reference).unwrap();
        for (old, new, stage) in [
            ("Review".to_owned(), "Changed".to_owned(), "label"),
            ("Review".to_owned(), String::new(), "label"),
            ("Approval".to_owned(), String::new(), "label"),
            ("Advance".to_owned(), String::new(), "label"),
            ("Alpha".to_owned(), String::new(), "label"),
            (
                "d=\"M 0 0 L 10 5 L 0 10 z\"".to_owned(),
                "d=\"M 0 0\"".to_owned(),
                "marker",
            ),
            (
                "data-id=\"B\"".to_owned(),
                "data-id=\"A\"".to_owned(),
                "nodes",
            ),
            (
                format!("marker-end=\"{reference}\""),
                "marker-end=\"none\"".to_owned(),
                "marker",
            ),
            (
                format!("id=\"{marker_id}\""),
                "id=\"missing-marker\"".to_owned(),
                "marker",
            ),
            (
                format!("d=\"{}\"", edge.attribute("d").unwrap()),
                "d=\"\"".to_owned(),
                "edge",
            ),
            (
                "class=\"background\"".to_owned(),
                "class=\"missing-background\"".to_owned(),
                "background",
            ),
            (
                "class=\"flowchartTitleText\"".to_owned(),
                "class=\"missing-title\"".to_owned(),
                "preset-flowchart-title",
            ),
            (
                "class=\"cluster-label\"".to_owned(),
                "class=\"missing-cluster-title\"".to_owned(),
                "cluster title",
            ),
            (
                "class=\"edgeLabel\"".to_owned(),
                "class=\"missing-edge-label\"".to_owned(),
                "edge label",
            ),
        ] {
            assert!(document.svg().contains(&old));
            let changed = observe(&document.svg().replace(&old, &new));
            let error = check_svg(&changed, palette)
                .err()
                .expect("bad terminal accepted");
            assert!(
                error.to_string().contains(stage),
                "{preset:?}/{old}: {error}"
            );
        }
    }

    fn paint_region(raster: &mut C6RasterImage, view: [f64; 4], region: [f64; 4], color: [u8; 3]) {
        let (width, height) = raster.dimensions();
        for y in 0..height {
            let sy = view[1] + (f64::from(y) + 0.5) * view[3] / f64::from(height);
            for x in 0..width {
                let sx = view[0] + (f64::from(x) + 0.5) * view[2] / f64::from(width);
                if sx >= region[0]
                    && sx <= region[0] + region[2]
                    && sy >= region[1]
                    && sy <= region[1] + region[3]
                {
                    raster.set_rgb_for_test(x, y, color);
                }
            }
        }
    }

    #[test]
    fn flowchart_pixels_reject_missing_surfaces_borders_connections_and_text() {
        // Brutalist deliberately shares its text, border and edge color.
        let document = document(ThemePreset::Brutalist);
        let palette = Palette::for_preset(ThemePreset::Brutalist).unwrap();
        let geometry = check_svg(&observe(document.svg()), palette).unwrap();
        let png = document
            .export_png(
                &merman_export::RasterOptions::default().with_scale(4.0),
                OperationControl::new(),
            )
            .unwrap();
        let raster = decode_bounded_png_artifact(png.bytes(), png.plan()).unwrap();
        check_pixels(&raster, &geometry, palette).unwrap();
        let Geometry {
            view,
            nodes: [a, b],
            cluster,
            background,
        } = geometry;
        let center = a[0] + a[2] / 2.0;
        for (region, replacement, stage) in [
            ([view[0], view[1], view[2], 3.0], palette.node, "surface"),
            (
                [view[0] + 6.0, view[1] + 6.0, view[2] - 12.0, 22.0],
                palette.canvas,
                "Approval has no visible text ink",
            ),
            (
                [cluster[0] + 6.0, cluster[1] + 6.0, cluster[2] - 12.0, 20.0],
                palette.cluster,
                "Review has no visible text ink",
            ),
            (inset(a, 5.0), palette.cluster, "surface"),
            (
                [a[0] - 2.0, a[1] + 8.0, 4.0, a[3] - 16.0],
                palette.cluster,
                "paint",
            ),
            (
                [
                    cluster[0] + 4.0,
                    cluster[1] + cluster[3] - 14.0,
                    cluster[2] - 8.0,
                    10.0,
                ],
                palette.canvas,
                "surface",
            ),
            (
                [
                    center - 3.0,
                    a[1] + a[3] + 2.0,
                    6.0,
                    background[1] - a[1] - a[3] - 4.0,
                ],
                palette.cluster,
                "connection",
            ),
            (
                [center - 5.0, b[1] - 10.0, 10.0, 9.0],
                palette.cluster,
                "direction marker",
            ),
        ] {
            let mut changed = raster.clone();
            paint_region(
                &mut changed,
                view,
                region,
                parse_c6_hex_rgb(replacement).unwrap(),
            );
            let error = check_pixels(&changed, &geometry, palette).unwrap_err();
            assert!(error.to_string().contains(stage), "{stage}: {error}");
        }
        let mut changed = raster.clone();
        paint_region(
            &mut changed,
            view,
            background,
            parse_c6_hex_rgb(palette.background).unwrap(),
        );
        paint_region(
            &mut changed,
            view,
            [center - 0.5, background[1], 1.0, background[3]],
            parse_c6_hex_rgb(palette.edge).unwrap(),
        );
        let error = check_pixels(&changed, &geometry, palette).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Advance has no visible text ink"),
            "{error}"
        );
    }

    #[test]
    fn flowchart_pixels_reject_missing_background_while_preserving_text() {
        let document = document(ThemePreset::Spotless);
        let palette = Palette::for_preset(ThemePreset::Spotless).unwrap();
        let geometry = check_svg(&observe(document.svg()), palette).unwrap();
        let png = document
            .export_png(
                &merman_export::RasterOptions::default().with_scale(4.0),
                OperationControl::new(),
            )
            .unwrap();
        let mut raster = decode_bounded_png_artifact(png.bytes(), png.plan()).unwrap();
        check_pixels(&raster, &geometry, palette).unwrap();
        // Erase only the pale background pixels; preserve all label ink and geometry.
        let mut reader = png::Decoder::new(std::io::Cursor::new(png.bytes()))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(frame.color_type, png::ColorType::Rgba);
        for (index, pixel) in pixels[..frame.buffer_size()].chunks_exact(4).enumerate() {
            let x = index as u32 % frame.width;
            let y = index as u32 / frame.width;
            if pixel[0].abs_diff(243) <= 2
                && pixel[1].abs_diff(240) <= 2
                && pixel[2].abs_diff(232) <= 2
            {
                raster.set_rgb_for_test(x, y, parse_c6_hex_rgb(palette.cluster).unwrap());
            }
        }
        let error = check_pixels(&raster, &geometry, palette).unwrap_err();
        assert!(
            error.to_string().contains("preset-flowchart-background"),
            "{error}"
        );
    }
}
