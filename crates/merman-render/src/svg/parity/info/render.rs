use super::super::*;
pub(crate) fn render_info_diagram_svg(
    layout: &InfoDiagramLayout,
    effective_config: &serde_json::Value,
    typography_theme: &crate::info::InfoTypographyThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let mut surface_receipt = typography_theme.begin_terminal_receipt();

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
    if typography_theme.requires_explicit_viewport_height() {
        root_spec =
            root_spec.with_mermaid_responsive_height(true, (bounds.max_y - bounds.min_y).max(0.0));
    }
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "info");
    root_chrome.dom.trailing_newline = false;
    let root_document = root_svg::RootViewportContext::new(
        crate::DiagramFamilyId::INFO,
        diagram_id,
    )
    .write_open(&mut out, root_spec, root_chrome)?;
    out.push_str("<style>");
    let css_write = write_info_css_with_font_family(
        &mut out,
        diagram_id,
        effective_config,
        typography_theme.font_family_css(),
    )?;
    out.push_str("</style>");
    out.checkpoint()?;
    surface_receipt.record_css_emission(
        css_write.root_font_family_css(),
        css_write.inherited_font_family_css(),
        css_write.root_variable_font_family_css(),
    );
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;
    let version = typography_theme.version_geometry();
    if layout.version != version.text() {
        return Err(crate::Error::InvalidModel {
            message: "Info layout version diverged from prepared terminal geometry".to_string(),
        });
    }
    let _ = write!(
        &mut out,
        r#"<g><text x="{}" y="{}" class="version" font-size="{}""#,
        fmt(version.x()),
        fmt(version.y()),
        fmt(version.font_size_px()),
    );
    let version_fill_css = typography_theme.version_fill_css();
    if let Some(fill_css) = version_fill_css {
        let _ = write!(&mut out, r#" fill="{}""#, escape_xml(fill_css));
    }
    let _ = write!(
        &mut out,
        r#" style="text-anchor: middle;">{}</text></g>"#,
        escape_xml(version.text())
    );
    out.checkpoint()?;
    surface_receipt.record_version_text(
        "version",
        version.text(),
        version.font_size_px(),
        version.x(),
        version.y(),
        version_fill_css,
    );
    out.push_str("</svg>\n");
    let rooted_svg = root_document.complete(out.finish()?)?;
    if !typography_theme.record_terminal(surface_receipt) {
        return Err(crate::Error::InvalidModel {
            message: "Info typography receipt was recorded more than once".to_string(),
        });
    }
    Ok(rooted_svg)
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
        let effective_config = merman_core::MermaidConfig::default();
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let typography_theme =
            crate::info::InfoTypographyThemePlan::resolve(None, &effective_config, &measurer);
        let execution = SvgExecution::unthemed_for_test(
            &request,
            &debug,
            &session,
            crate::DiagramFamilyId::INFO,
        )
        .expect("SVG execution");
        let layout = crate::info::layout_info_diagram_typed(
            &merman_core::diagrams::info::InfoDiagramRenderModel::default(),
            &typography_theme,
        )
        .expect("Info layout");
        render_info_diagram_svg(
            &layout,
            effective_config.as_value(),
            &typography_theme,
            &execution,
        )
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
