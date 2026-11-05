//! Usecase role colors and opt-in participant palettes from Mermaid 12 styles.ts.

use super::*;
use crate::usecase::UsecaseCssBinding;

pub(super) struct UsecaseBodyShadow<'a> {
    effect: &'a crate::diagram_theme::SvgShadowEffect,
    materialized: crate::diagram_theme::MaterializedShadowEffect,
    filter_id: String,
}

pub(super) fn prepare_body_shadows<'a>(
    prepared: &'a UsecasePreparedArtifact,
    geometry: &HashMap<&str, &LayoutNode>,
    bounds: &mut Bounds,
    diagram_id: SvgDiagramId<'_>,
    options: &SvgExecution<'_>,
) -> Result<HashMap<&'a str, UsecaseBodyShadow<'a>>> {
    let mut shadows = HashMap::new();
    for (index, plan) in prepared.nodes.iter().enumerate() {
        let Some(style) = prepared.theme_plan.shell(&plan.id) else {
            continue;
        };
        let Some(effect) = style.shadow.as_ref() else {
            continue;
        };
        options.checkpoint_emit()?;
        let source_width = plan
            .source_style
            .split(';')
            .filter_map(crate::mermaid_style::parse_style_declaration)
            .filter(|declaration| declaration.property() == "stroke-width")
            .last();
        let width = if let Some(declaration) = source_width {
            // A source width must have a finite SVG length before bounding the filter.
            declaration.svg_number_or_px()
        } else {
            style.stroke_width.or(Some(2.0))
        };
        let Some(width) = width else { continue };
        let node = geometry[plan.id.as_str()];
        options
            .work_meter()
            .charge(effect.stages().len().saturating_mul(3))?;
        let Some(materialized) = effect.materialize_rect(
            &options.theme_resource_policy(),
            node.width,
            node.height,
            width,
        )?
        else {
            continue;
        };
        let outsets = materialized.outsets();
        bounds.min_x = bounds.min_x.min(node.x - node.width / 2.0 - outsets.left);
        bounds.min_y = bounds.min_y.min(node.y - node.height / 2.0 - outsets.top);
        bounds.max_x = bounds.max_x.max(node.x + node.width / 2.0 + outsets.right);
        bounds.max_y = bounds
            .max_y
            .max(node.y + node.height / 2.0 + outsets.bottom);
        shadows.insert(
            plan.id.as_str(),
            UsecaseBodyShadow {
                effect,
                materialized,
                filter_id: format!(
                    "{}-usecase-body-{index}-theme-effect-{}",
                    diagram_id.semantic_str(),
                    effect.id()
                ),
            },
        );
    }
    Ok(shadows)
}

pub(super) fn write_body_shadow_open(out: &mut String, shadow: &UsecaseBodyShadow<'_>) {
    let reference = super::super::shadow::write_theme_shadow_application(
        out,
        &shadow.filter_id,
        shadow.effect,
        shadow.materialized.region(),
    );
    let _ = write!(
        out,
        r#"<g class="usecase-theme-body-effect" filter="{}">"#,
        escape_attr(&reference)
    );
}

pub(super) fn record_body_shadow(
    shadow: &UsecaseBodyShadow<'_>,
    evidence: &crate::diagram_theme::SvgShadowEvidenceRecorder,
) {
    evidence.record_application(
        shadow.effect,
        &shadow.filter_id,
        shadow.materialized.region(),
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "Marker instances preserve upstream geometry while sharing the edge's theme receipt."
)]
pub(super) fn write_marker(
    out: &mut String,
    diagram_id: SvgDiagramId<'_>,
    marker: &str,
    start: bool,
    margin: &str,
    edge_index: usize,
    style: Option<&UsecaseThemeStyle>,
    receipt: &UsecaseThemeReceipt,
) -> String {
    let name = marker.strip_prefix("arrow_").unwrap_or(marker);
    let end = if start { "Start" } else { "End" };
    let base = format!("{name}{end}{margin}");
    let css = style.map(|style| style.css(false)).unwrap_or_default();
    if css.is_empty() {
        if let Some(style) = style {
            style.record_emitted(receipt);
        }
        return format!("url(#{diagram_id}_usecase-{base})");
    }
    let suffix = format!("theme-{edge_index}-{base}");
    out.push_str("<defs>");
    if marker == "extension" {
        let _ = write!(
            out,
            r#"<marker id="{diagram_id}_usecase-{suffix}" class="marker extension usecase""#
        );
        if margin.is_empty() {
            let _ = write!(
                out,
                r#" refX="{}" refY="7" markerWidth="20" markerHeight="28" markerUnits="userSpaceOnUse" orient="auto"><path d="{}" style="{}"/></marker>"#,
                if start { "18" } else { "1" },
                if start {
                    "M 1,7 L18,13 V 1 Z"
                } else {
                    "M 1,1 V 13 L18,7 Z"
                },
                escape_attr(&css)
            );
        } else {
            let _ = write!(
                out,
                r#" refX="{}" refY="7" markerWidth="20" markerHeight="28" markerUnits="userSpaceOnUse" orient="auto" viewBox="0 0 20 14"><polygon points="{}" style="stroke-width:2;stroke-dasharray:0;{}"/></marker>"#,
                if start { "18" } else { "9" },
                if start {
                    "10,7 18,13 18,1"
                } else {
                    "10,1 10,13 18,7"
                },
                escape_attr(&css)
            );
        }
    } else if let Some(kind) = markers::BaseEdgeMarkerKind::ALL
        .into_iter()
        .find(|kind| kind.suffix() == base)
    {
        markers::push_base_edge_marker(out, diagram_id, "usecase", kind, &suffix, &css);
    }
    out.push_str("</defs>");
    if let Some(style) = style {
        style.record_emitted(receipt);
    }
    format!("url(#{diagram_id}_usecase-{suffix})")
}

pub(super) fn appearance_attributes(
    binding: &UsecaseCssBinding,
    color_index: Option<usize>,
) -> String {
    let mut attributes = format!(r#" data-look="{}""#, binding.look);
    if let Some(index) = color_index.filter(|_| !binding.palette.is_empty()) {
        let _ = write!(
            attributes,
            r#" data-color-id="color-{}""#,
            index % binding.palette.len()
        );
    }
    attributes
}

pub(super) fn write_css(
    out: &mut String,
    id: SvgDiagramId<'_>,
    binding: &UsecaseCssBinding,
    themed_background: bool,
) -> Result<()> {
    let UsecaseCssBinding {
        main,
        node_border,
        body,
        border,
        actor,
        actor_border,
        boundary,
        boundary_border,
        line,
        include,
        extend,
        text,
        root_text,
        actor_text,
        title,
        note,
        note_border,
        note_text,
        edge_background,
        look,
        ..
    } = binding;
    let mut css = String::new();
    // HTML label metrics exclude the browser's default paragraph margins, as does
    // Mermaid's shared styles.ts reset. Apply it before the family-specific styles.
    let _ = super::super::css::write_mermaid_paragraph_css_to(&mut css, id);
    let font = &binding.root_font;
    let _ = write!(
        css,
        "#{id}{{font-family:{};font-size:{}px;fill:{root_text}}}",
        font.font_family.as_deref().unwrap_or("sans-serif"),
        fmt(font.font_size)
    );
    for (index, color) in binding.palette.iter().enumerate() {
        let fill = if binding.backgrounds.is_empty() {
            String::new()
        } else {
            format!(
                "fill:{};",
                binding.backgrounds[index % binding.backgrounds.len()]
            )
        };
        let slot = format!(r##"#{id} [data-look="{look}"][data-color-id="color-{index}"]"##);
        let _ = write!(
            css,
            "{slot}.system-boundary rect.boundary-body,{slot}.system-boundary rect.boundary-tab,{slot}.system-boundary .boundary-body path,{slot}.system-boundary .boundary-tab path{{stroke:{color};{fill}}}"
        );
        if binding.rotate {
            let _ = write!(
                css,
                "{slot}.usecase-element ellipse,{slot}.usecase-element rect{{stroke:{color};{fill}}}{slot}.usecase-element .usecase-business-marker{{stroke:{color}}}{slot}.usecase-actor .usecase-actor-glyph{{stroke:{color};{fill}}}"
            );
            if look != "handDrawn" {
                let _ = write!(
                    css,
                    "{slot}.usecase-actor .usecase-actor-glyph path,{slot}.usecase-actor .usecase-actor-glyph circle{{stroke:{color}}}"
                );
            }
        }
    }
    let _ = write!(
        css,
        r#"
#{id} text,#{id} .nodeLabel{{fill:{text};color:{text}}}
#{id} .usecase-element ellipse,#{id} .usecase-element rect{{fill:{body};stroke:{border};stroke-width:2px}}
#{id} .usecase-actor-shape,#{id} .usecase-actor-hollow,#{id} .usecase-actor-awesome,#{id} .usecase-actor-icon{{fill:{actor};stroke:{actor_border};stroke-width:2px}}
#{id} .usecase-actor .nodeLabel,#{id} .actor-label{{color:{actor_text};fill:{actor_text}}}
#{id} .usecase-actor .nodeLabel,#{id} .actor-label{{font-family:var(--mermaid-usecase-actor-font-family);font-size:var(--mermaid-usecase-actor-font-size);font-weight:var(--mermaid-usecase-actor-font-weight)}}
#{id} .usecase-element .nodeLabel,#{id} .usecase-label{{font-family:var(--mermaid-usecase-font-family);font-size:var(--mermaid-usecase-font-size);font-weight:var(--mermaid-usecase-font-weight)}}
#{id} .system-boundary rect.boundary-body,#{id} .system-boundary rect.boundary-tab,#{id} .system-boundary-package-tab{{fill:{boundary};stroke:{boundary_border};stroke-width:1px}}
#{id} .system-boundary-title text,#{id} .system-boundary-title span{{fill:{title};color:{title}}}
#{id} .relationship{{fill:none;stroke:{line};stroke-width:1px}}
#{id} .relationship-include{{stroke:{include};stroke-dasharray:3}}
#{id} .relationship-extend{{stroke:{extend};stroke-dasharray:3}}
#{id} .relationship-note{{stroke-dasharray:3}}
#{id} .marker{{fill:{line};stroke:{line}}}#{id} .marker.extension{{fill:{main};stroke:{line}}}
#{id} .usecase-note{{fill:{note};stroke:{note_border};color:{note_text}}}
#{id} .usecase-note .nodeLabel{{color:{note_text};fill:{note_text}}}
#{id} .usecase-json-table,#{id} .usecase-json-table rect,#{id} .usecase-json-cell{{fill:{main};stroke:{node_border}}}
#{id} .usecase-stereotype,#{id} .usecase-business-marker{{stroke:{border};fill:{text};color:{text}}}
#{id} .usecase-json-title,#{id} .usecase-json-key,#{id} .usecase-json-value{{fill:{text};color:{text}}}
#{id} .edgeLabel,#{id} .edgeLabel p{{background-color:{edge_background}}}
#{id} .edgeLabel .label rect{{fill:{edge_background}}}
@keyframes dash{{to{{stroke-dashoffset:0}}}}
#{id} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round}}
#{id} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round}}
"#
    );
    // Mermaid appends shared neo paint after family CSS. The role selectors must
    // name the actual glyph/body children to outrank direct neo path/rect rules.
    if look != "handDrawn" {
        let _ = write!(
            css,
            r#"#{id} .node.usecase-actor .usecase-actor-glyph path,#{id} .node.usecase-actor .usecase-actor-glyph circle{{stroke:{actor_border}}}
#{id} [data-look="{look}"].node.usecase-element ellipse,#{id} [data-look="{look}"].node.usecase-element rect{{fill:{body};stroke:{border}}}
#{id} [data-look="{look}"].node.usecase-element .usecase-business-marker{{stroke:{border}}}"#
        );
    }
    binding.neo.write(&mut css, id)?;
    if themed_background {
        let _ = write!(
            css,
            "#{id} .usecase-themed-edge-background,#{id} .usecase-themed-edge-background .edgeLabel,#{id} .usecase-themed-edge-background p{{background-color:transparent !important;}}"
        );
    }
    // CSS is XML character data. Preserve declarations while preventing a token from closing style.
    out.push_str("<style>");
    super::super::output::XmlTextOutput::new(out).push_str(&css);
    out.push_str("</style>");
    Ok(())
}
