use super::super::*;
use crate::info::{INFO_BASELINE_HEIGHT_PX, InfoClassicStyle, InfoNativeStyle};

pub(crate) fn render_native_info_svg(
    layout: &InfoDiagramLayout,
    style: &InfoNativeStyle,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let version = &style.version;
    let diagram_id = options.diagram_id_or("merman");
    let bounds = layout
        .bounds
        .as_ref()
        .ok_or_else(|| crate::Error::InvalidModel {
            message: "Info layout is missing its prepared viewport bounds".to_owned(),
        })?;
    let viewport = root_svg::RootViewportSpec::responsive(root_svg::DiagramBounds::from_extents(
        bounds.min_x,
        bounds.min_y,
        bounds.max_x,
        bounds.max_y,
        0.0,
    ));
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root = options
        .root_viewport(crate::DiagramFamilyId::INFO, diagram_id)
        .write_open(
            &mut out,
            viewport,
            root_svg::RootChrome::new(diagram_id, "info"),
        )?;
    let _ = write!(
        &mut out,
        r#"<g><text x="{}" y="{}" class="version" font-size="{}" font-family="{}" "#,
        fmt(version.x),
        fmt(version.y),
        fmt(version.font_size_px),
        escape_attr(&style.font_family),
    );
    let _ = crate::theme::canvas_svg::write_color(&mut out, "fill", style.text_color, 1.0);
    let _ = write!(
        &mut out,
        r#" text-anchor="middle">{}</text></g></svg>"#,
        escape_xml(&layout.version)
    );
    root.complete(out.finish()?)
}

pub(crate) fn render_info_diagram_svg(
    layout: &InfoDiagramLayout,
    style: &InfoClassicStyle,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let bounds = layout
        .bounds
        .as_ref()
        .ok_or_else(|| crate::Error::InvalidModel {
            message: "Info layout is missing its prepared viewport bounds".to_string(),
        })?;
    let mut root_spec = root_svg::RootViewportSpec::responsive_without_view_box(
        (bounds.max_x - bounds.min_x).max(0.0),
    );
    let height = (bounds.max_y - bounds.min_y).max(0.0);
    if height > INFO_BASELINE_HEIGHT_PX {
        root_spec = root_spec.with_mermaid_responsive_height(true, height);
    }
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "info");
    root_chrome.dom.trailing_newline = false;
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::INFO, diagram_id)
        .write_open(&mut out, root_spec, root_chrome)?;
    out.push_str("<style>");
    let selector = super::super::util::css_selector_diagram_id(diagram_id.semantic_str());
    let fragment = super::super::css::FragmentDiagramId(diagram_id.semantic_str());
    {
        let mut css = super::super::output::XmlTextOutput::new(&mut out);
        style.css.write_prefix(&mut css, selector)?;
        style.css.write_root(&mut css, selector, fragment)?;
    }
    out.push_str("</style>");
    out.checkpoint()?;
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;
    let version = &style.version;
    let _ = write!(
        &mut out,
        r#"<g><text x="{}" y="{}" class="version" font-size="{}" style="text-anchor: middle;">{}</text></g>"#,
        fmt(version.x),
        fmt(version.y),
        fmt(version.font_size_px),
        escape_xml(&layout.version)
    );
    out.checkpoint()?;
    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{
        RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
    };

    fn render_info_with_policy(policy: RenderResourcePolicy) -> crate::Result<root_svg::RootedSvg> {
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .expect("render session");
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let effective_config = merman_model::presentation::PresentationConfig::default();
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let (layout, style) = InfoClassicStyle::resolve(&effective_config, &measurer);
        let execution = SvgExecution::unthemed_for_test(
            &request,
            &debug,
            &session,
            crate::DiagramFamilyId::INFO,
        )
        .expect("SVG execution");
        render_info_diagram_svg(&layout, &style, &execution)
    }

    #[test]
    fn info_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
        let baseline = render_info_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .expect("render unbounded Info SVG");
        let exact_bytes = baseline.len();
        assert!(exact_bytes > 1);

        let exact = render_info_with_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
                .expect("valid exact SVG ceiling"),
        )
        .expect("exact Info SVG ceiling must succeed");
        assert_eq!(exact.as_bytes(), baseline.as_bytes());

        let below_exact = exact_bytes - 1;
        let error = render_info_with_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
                .expect("valid short SVG ceiling"),
        )
        .expect_err("one byte below the Info SVG size must fail");
        let crate::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected Info MaxSvgBytes rejection, got {error}");
        };
        assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
        assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
        assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
        assert_eq!(limit.max, below_exact);
        assert!(limit.actual > limit.max);
    }
}
