//! Common layout options used by SVG compare commands.

pub(crate) fn svg_compare_layout_opts() -> merman_render::LayoutOptions {
    merman_render::LayoutOptions {
        text_measurer: std::sync::Arc::new(
            merman_render::text::VendoredFontMetricsTextMeasurer::default(),
        ),
        use_manatee_layout: true,
        ..Default::default()
    }
}
