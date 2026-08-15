use super::super::*;
pub(crate) fn render_info_diagram_svg(
    layout: &InfoDiagramLayout,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_spec = root_svg::RootViewportSpec::responsive_without_view_box(400.0);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "info");
    root_chrome.dom.trailing_newline = false;
    let root_document = root_svg::RootViewportContext::new(
        crate::DiagramFamilyId::INFO,
        diagram_id,
    )
    .write_open(&mut out, root_spec, root_chrome)?;
    let css = info_css_with_config(diagram_id, effective_config);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    drop(css);
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;
    let _ = write!(
        &mut out,
        r#"<g><text x="100" y="40" class="version" font-size="32" style="text-anchor: middle;">{}</text></g>"#,
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
        let execution = SvgExecution::unthemed_for_test(
            &request,
            &debug,
            &session,
            crate::DiagramFamilyId::INFO,
        )
        .expect("SVG execution");
        render_info_diagram_svg(
            &InfoDiagramLayout {
                bounds: None,
                version: "v11.15.0".to_string(),
            },
            &serde_json::json!({}),
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
