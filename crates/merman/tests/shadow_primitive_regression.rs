//! Native export of a layered Paper Cut elevation.

#![cfg(all(feature = "png", feature = "diagram-flowchart"))]

use merman::svg::export::RasterOptions;
use merman::theme::{BuiltinPreset, ThemeSelection};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

#[test]
fn paper_cut_elevation_fits_the_default_png_filter_budget() {
    let request = SvgRequest::default()
        .with_native_theme(ThemeSelection::preset(BuiltinPreset::PaperCut, None));
    let RenderOutput::Document(Some(document)) = Renderer::new()
        .render(RenderRequest::document(
            "flowchart LR\nA[Paper] --> B[Cut]",
            OperationControl::new(),
            request,
        ))
        .unwrap()
    else {
        panic!("expected a rendered Paper Cut document");
    };
    let svg = roxmltree::Document::parse(document.svg()).unwrap();
    let layers: Vec<_> = svg
        .descendants()
        .filter(|node| node.attribute("data-merman-theme-depth") == Some("node"))
        .collect();
    assert_eq!(layers.len(), 6, "three depth layers per node");
    for layer in layers.as_chunks::<3>().0 {
        let colors: Vec<_> = layer
            .iter()
            .map(|group| {
                group
                    .children()
                    .find(|node| node.has_tag_name("path"))
                    .and_then(|node| node.attribute("fill"))
            })
            .collect();
        assert_eq!(colors, [Some("#243E4A"), Some("#7EB4AA"), Some("#EB9C88")]);
        assert_eq!(
            layer
                .iter()
                .filter(|node| node
                    .descendants()
                    .any(|child| child.has_tag_name("feGaussianBlur")))
                .count(),
            1
        );
    }

    let png = document
        .export_png(&RasterOptions::default(), OperationControl::new())
        .unwrap();
    assert!(png.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(png.theme_outcome(), document.theme_outcome());
    assert_eq!(png.plan(), png.export_report().raster());
    assert!(png.export_report().conversion().filter_primitives >= 2);
}
