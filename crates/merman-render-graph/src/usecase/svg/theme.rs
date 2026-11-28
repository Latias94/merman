//! Usecase role colors and opt-in participant palettes from Mermaid 12 styles.ts.

use super::super::UsecaseCssBinding;
use super::*;
use merman_render_core::svg_output::XmlTextOutput;

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
    out: &mut impl SvgOutput,
    id: SvgId<'_>,
    binding: &UsecaseCssBinding,
    config: &merman_model::presentation::PresentationConfig,
    canonical: bool,
    mermaid_grid_label_font: bool,
) -> Result<()> {
    let UsecaseCssBinding {
        main,
        node_border,
        body,
        border,
        actor,
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
    let actor_border = binding.actor_outline(config, canonical);
    let hand_drawn_shapes = canonical && look == "handDrawn";
    out.push_str("<style>");
    let mut css = XmlTextOutput::new(out);
    // HTML label metrics exclude the browser's default paragraph margins, as does
    // Mermaid's shared styles.ts reset. Apply it before the family-specific styles.
    let _ = write!(css, "#{id} p{{margin:0;}}");
    css.checkpoint()?;
    let font = &binding.root_font;
    let connector_width = fmt(binding.connector_width);
    let _ = write!(
        css,
        "#{id}{{font-family:{};font-size:{}px;fill:{root_text}}}",
        font.font_family.as_deref().unwrap_or("sans-serif"),
        fmt(font.font_size)
    );
    if canonical {
        let _ = write!(
            css,
            "#{id} .usecase-json-title,#{id} .usecase-json-key,#{id} .usecase-json-value,#{id} .usecase-json-table text{{stroke:none}}"
        );
        let _ = write!(css, "#{id} .edgeLabel{{color:{text}}}");
        let _ = write!(
            css,
            "#{id} .edgeLabel.usecase-panel-caption,#{id} .usecase-panel-caption .labelBkg,#{id} .usecase-panel-caption .edgeLabel,#{id} .usecase-panel-caption p{{background-color:transparent}}"
        );
    }
    #[cfg(feature = "layout-grid")]
    if mermaid_grid_label_font {
        let _ = write!(
            css,
            "#{id} .nodes>.label.edgeLabel{{font-size:{}px;font-weight:normal}}",
            super::super::grid::MERMAID_EDGE_LABEL_FONT_SIZE
        );
    }
    #[cfg(not(feature = "layout-grid"))]
    let _ = mermaid_grid_label_font;
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
        if hand_drawn_shapes {
            let _ = write!(
                css,
                "{slot}.system-boundary .usecase-sketch-shape{{stroke:{color};{fill}}}"
            );
        } else {
            let _ = write!(
                css,
                "{slot}.system-boundary rect.boundary-body,{slot}.system-boundary rect.boundary-tab,{slot}.system-boundary .boundary-body path,{slot}.system-boundary .boundary-tab path{{stroke:{color};{fill}}}"
            );
        }
        if binding.rotate {
            let _ = write!(
                css,
                "{slot}.usecase-element ellipse,{slot}.usecase-element rect{{stroke:{color};{fill}}}{slot}.usecase-element .usecase-business-marker{{stroke:{color}}}{slot}.usecase-actor .usecase-actor-glyph{{stroke:{color};{fill}}}"
            );
            if hand_drawn_shapes {
                let _ = write!(
                    css,
                    "{slot}.usecase-element .usecase-sketch-shape{{stroke:{color};{fill}}}"
                );
            }
            if look != "handDrawn" {
                let _ = write!(
                    css,
                    "{slot}.usecase-actor .usecase-actor-glyph path,{slot}.usecase-actor .usecase-actor-glyph circle{{stroke:{color}}}"
                );
            }
        }
        css.checkpoint()?;
    }
    let _ = write!(
        css,
        r#"
#{id} text,#{id} .nodeLabel{{fill:{text};color:{text}}}
#{id} .usecase-element ellipse,#{id} .usecase-element rect{{fill:{body};stroke:{border};stroke-width:2px}}
#{id} .usecase-actor-glyph{{fill:{actor};stroke:{actor_border};stroke-width:2px}}
#{id} .usecase-actor .nodeLabel,#{id} .actor-label,#{id} .actor-label text{{color:{actor_text};fill:{actor_text};stroke:none}}
#{id} .usecase-actor .nodeLabel,#{id} .actor-label{{font-family:var(--mermaid-usecase-actor-font-family);font-size:var(--mermaid-usecase-actor-font-size);font-weight:var(--mermaid-usecase-actor-font-weight)}}
#{id} .usecase-element .nodeLabel,#{id} .usecase-label{{font-family:var(--mermaid-usecase-font-family);font-size:var(--mermaid-usecase-font-size);font-weight:var(--mermaid-usecase-font-weight)}}
#{id} .system-boundary rect.boundary-body,#{id} .system-boundary rect.boundary-tab,#{id} .system-boundary-package-tab{{fill:{boundary};stroke:{boundary_border};stroke-width:1px}}
#{id} .system-boundary-title text,#{id} .system-boundary-title span{{fill:{title};color:{title}}}
#{id} .relationship{{fill:none;stroke:{line};stroke-width:{connector_width}px}}
#{id} .relationship-include{{stroke:{include};stroke-dasharray:3}}
#{id} .relationship-extend{{stroke:{extend};stroke-dasharray:3}}
#{id} .relationship-note{{stroke-dasharray:3}}
#{id} .marker{{fill:{line};stroke:{line}}}#{id} .marker.extension{{fill:{main};stroke:{line}}}
#{id} .usecase-note{{fill:{note};stroke:{note_border};color:{note_text}}}
#{id} .usecase-note .nodeLabel,#{id} .usecase-note .noteLabel text{{color:{note_text};fill:{note_text};stroke:none}}
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
    if hand_drawn_shapes {
        let _ = write!(
            css,
            r#"#{id} .usecase-element .usecase-sketch-shape{{fill:{body};stroke:{border};stroke-width:2px}}
#{id} .system-boundary .usecase-sketch-shape{{fill:{boundary};stroke:{boundary_border};stroke-width:1px}}
#{id} .usecase-note .usecase-sketch-shape{{stroke-width:1.3px}}"#,
        );
    }
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
    if let Some(look) = &binding.mermaid_look {
        look.css.write(&mut css, id)?;
    }
    if let Some(font) = &binding.native_title_font {
        let _ = write!(
            css,
            "#{id} .usecaseDiagramTitleText{{font-family:{};font-size:{}px;font-weight:{};fill:{title};filter:none}}",
            font.font_family.as_deref().unwrap_or("sans-serif"),
            fmt(font.font_size),
            font.font_weight.as_deref().unwrap_or("normal"),
        );
    }
    css.checkpoint()?;
    out.push_str("</style>");
    out.checkpoint()
}
