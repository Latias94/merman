use super::ErCssBinding;
use merman_model::families::er::ErDiagramModel;
use merman_render_core::Result;
use merman_render_core::svg_css::{
    CssSelectorDiagramId, FragmentDiagramId, SvgDiagramIdValue, escape_css_identifier,
};
use merman_render_core::svg_output::{SvgOutput, XmlTextOutput};
use std::collections::HashMap;
use std::fmt::Write;

pub fn er_css_with_resolved_typography<I>(diagram_id: I, values: &ErCssBinding) -> Result<String>
where
    I: SvgDiagramIdValue,
{
    // Mirrors pinned Mermaid ER unified renderer stylesheet ordering (see `diagrams/er/styles.ts`
    // and shared base stylesheet).
    // Keep `:root` last (matches upstream fixtures).
    let id = CssSelectorDiagramId(diagram_id);
    let fragment_id = FragmentDiagramId(diagram_id);
    let font = &values.font_family;
    let text_color = &values.text_color;
    let line_color = &values.line_color;
    let main_bkg = &values.main_bkg;
    let node_border = &values.node_border;
    let cluster_bkg = &values.cluster_bkg;
    let cluster_border = &values.cluster_border;
    let title_color = &values.title_color;
    let node_text_color = &values.node_text_color;
    let tertiary_color = &values.tertiary_color;
    let label_background = values.label_background.clone()?;
    let edge_label_background = &values.edge_label_background;
    let stroke_width = &values.stroke_width;
    let mut out = String::new();
    values.common.write_prefix(&mut out, id)?;
    let _ = write!(
        &mut out,
        r#"#{} .entityBox{{fill:{};stroke:{};}}"#,
        id, main_bkg, node_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLabelBox{{fill:{};opacity:0.7;background-color:{};}}"#,
        id, tertiary_color, tertiary_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLabelBox rect{{opacity:0.5;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelBkg{{background-color:{};}}"#,
        id, label_background
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel{{background-color:{};}}#{} .edgeLabel .label rect{{fill:{};}}#{} .edgeLabel .label text{{fill:{};}}#{} .edgeLabel .label{{fill:{};font-size:14px;}}"#,
        id, edge_label_background, id, edge_label_background, id, text_color, id, node_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .label{{font-family:{};color:{};}}"#,
        id, font, node_text_color
    );
    // Mermaid duplicates `.edge-pattern-dashed` (base rule earlier sets dasharray:3).
    let _ = write!(
        &mut out,
        r#"#{} .edge-pattern-dashed{{stroke-dasharray:8,8;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon{{fill:{};stroke:{};stroke-width:{};}}"#,
        id, id, id, id, main_bkg, node_border, stroke_width
    );
    let _ = write!(
        &mut out,
        r#"#{} .relationshipLine{{stroke:{};stroke-width:{};fill:none;}}"#,
        id, line_color, stroke_width
    );
    // Mermaid duplicates `.marker` (base rule earlier sets fill/stroke from the line color).
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:none!important;stroke:{}!important;stroke-width:1;}}"#,
        id, line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} [data-look=neo].labelBkg{{background-color:{};}}"#,
        id,
        values.neo_label_background.clone()?
    );
    let _ = write!(
        &mut out,
        r#"#{id} .cluster rect{{fill:{cluster_bkg};stroke:{cluster_border};stroke-width:1px;}}#{id} .cluster text{{fill:{title_color};}}#{id} .cluster-label text{{fill:{title_color};}}"#,
    );
    values.common.write_root(&mut out, id, fragment_id)?;
    Ok(out)
}

pub fn er_redux_color_id(border_colors: &[String], color_index: usize) -> Option<String> {
    (!border_colors.is_empty()).then(|| format!("color-{}", color_index % border_colors.len()))
}

pub fn er_color_indices(model: &ErDiagramModel) -> HashMap<String, usize> {
    fn insertion_index(id: &str) -> Option<usize> {
        id.rsplit_once('-')?.1.parse().ok()
    }

    let mut entities: Vec<_> = model.entities.values().collect();
    entities.sort_by(|left, right| {
        (insertion_index(&left.id), left.id.as_str())
            .cmp(&(insertion_index(&right.id), right.id.as_str()))
    });
    entities
        .into_iter()
        .enumerate()
        .map(|(index, entity)| (entity.id.clone(), index))
        .collect()
}

fn write_er_redux_color_css(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    data_look: &str,
    border_colors: &[String],
    background_colors: &[String],
) -> Result<()> {
    let diagram_id = escape_css_identifier(diagram_id);
    for (index, border_color) in border_colors.iter().enumerate() {
        let border_color = border_color.trim();
        let fill = if background_colors.is_empty() {
            String::new()
        } else {
            format!(
                "fill:{};",
                background_colors[index % background_colors.len()].trim()
            )
        };
        for shape in ["path", "rect"] {
            let _ = write!(out, "#{diagram_id} [data-look=")
                .and_then(|()| cssparser::serialize_string(data_look, out))
                .and_then(|()| {
                    write!(
                        out,
                        r#"][data-color-id="color-{index}"].node {shape}{{stroke:{border_color};{fill}}}"#,
                    )
                });
            out.checkpoint()?;
        }
    }
    Ok(())
}

fn er_redux_color_css_insertion_point(css: &str, diagram_id: &str) -> usize {
    let escaped_id = escape_css_identifier(diagram_id);
    let family_rule = format!("#{escaped_id} .entityBox");
    let root_rule = format!("#{escaped_id} :root");
    css.find(&family_rule)
        .or_else(|| css.find(&root_rule))
        .unwrap_or(css.len())
}

/// Emits one complete SVG style element using the semantic diagram ID.
pub fn write_er_stylesheet(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    data_look: &str,
    binding: &ErCssBinding,
) -> Result<()> {
    write_er_stylesheet_with_cluster_titles(out, diagram_id, data_look, binding, false)
}

pub(super) fn write_er_stylesheet_with_cluster_titles(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    data_look: &str,
    binding: &ErCssBinding,
    html_cluster_titles: bool,
) -> Result<()> {
    let css = er_css_with_resolved_typography(diagram_id, binding)?;
    let insertion_point = er_redux_color_css_insertion_point(&css, diagram_id);

    out.push_str("<style>");
    {
        let mut css_out = XmlTextOutput::new(out);
        css_out.push_str(&css[..insertion_point]);
        css_out.checkpoint()?;
        write_er_redux_color_css(
            &mut css_out,
            diagram_id,
            data_look,
            &binding.redux_border_colors,
            &binding.redux_background_colors,
        )?;
        css_out.push_str(&css[insertion_point..]);
        if html_cluster_titles {
            let id = escape_css_identifier(diagram_id);
            let title = &binding.title_color;
            let _ = write!(
                &mut css_out,
                "#{id} .cluster-label .nodeLabel{{color:{title};}}"
            );
        }
        if let Some(native) = &binding.native {
            let id = escape_css_identifier(diagram_id);
            let code_family = &native.code_family;
            let title_family = native
                .title_family
                .as_deref()
                .unwrap_or(&binding.font_family);
            let title_weight = &native.title_weight;
            let text = &binding.node_text_color;
            let title = &binding.title_color;
            let border = native.border_width;
            let heading_ink = &native.heading_ink;
            let _ = write!(
                &mut css_out,
                "#{id} .node .label{{fill:{text};}}\
                 #{id} .label.attribute-type{{font-family:{code_family};}}\
                 #{id} .erDiagramTitleText{{font-family:{title_family};font-weight:{title_weight};fill:{title};}}\
                 #{id} .node > .basic.label-container{{stroke-width:{border};}}\
                 #{id} .node[data-native-er-heading] > .label.name,#{id} .node[data-native-er-heading] > .basic + .label{{fill:{heading_ink};color:{heading_ink};}}"
            );
        }
    }
    out.push_str("</style>");
    out.checkpoint()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render_core::Error;
    use serde_json::json;
    use std::{fmt, ops::Range};

    #[derive(Default)]
    struct RejectAfterFirstWrite {
        write_attempts: usize,
        rejected: bool,
        retained: String,
    }

    impl RejectAfterFirstWrite {
        fn record_write(&mut self, value: &str) -> fmt::Result {
            self.write_attempts += 1;
            if self.write_attempts == 1 {
                self.rejected = true;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl fmt::Write for RejectAfterFirstWrite {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.record_write(value)
        }
    }

    impl super::SvgOutput for RejectAfterFirstWrite {
        fn push_str(&mut self, value: &str) {
            let _ = self.record_write(value);
        }

        fn push(&mut self, value: char) {
            let mut encoded = [0u8; 4];
            let _ = self.record_write(value.encode_utf8(&mut encoded));
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            self.retained.as_str()
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> Result<()> {
            if self.rejected {
                Err(Error::InvalidModel {
                    message: "test SVG sink rejected the first write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn er_redux_color_css_stops_after_the_first_svg_sink_failure() {
        let border_colors = vec!["#e879f9".to_string(), "#2dd4bf".to_string()];
        let background_colors = vec!["#fdf4ff".to_string(), "#f0fdfa".to_string()];
        let mut out = RejectAfterFirstWrite::default();

        let error = super::write_er_redux_color_css(
            &mut out,
            "er",
            "classic",
            &border_colors,
            &background_colors,
        )
        .expect_err("the rejecting sink must stop ER Redux color CSS emission");

        assert!(matches!(error, Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "ER Redux color CSS emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn er_redux_color_css_serializes_the_look_as_a_css_string_inside_xml() {
        let mut svg = String::from("<style>");
        super::write_er_redux_color_css(
            &mut super::XmlTextOutput::new(&mut svg),
            "er",
            "neo\"\\\n&<]]>",
            &["#123456".into()],
            &[],
        )
        .unwrap();
        svg.push_str("</style>");

        let parsed = roxmltree::Document::parse(&svg).unwrap();
        let css = parsed.root_element().text().unwrap();
        assert_eq!(css.matches(r#"[data-look="neo\"\\\a &<]]>"]"#).count(), 2);
    }

    #[test]
    fn er_redux_color_theme_emits_per_entity_color_rules() {
        let config = json!({
            "theme": "redux-color",
            "themeVariables": {
                "borderColorArray": ["#e879f9", "#2dd4bf"],
                "bkgColorArray": ["#fdf4ff", "#f0fdfa"],
                "THEME_COLOR_LIMIT": 2
            }
        });
        let theme = ErCssBinding::resolve(&config);
        let borders = &theme.redux_border_colors;
        let backgrounds = &theme.redux_background_colors;

        assert_eq!(
            super::er_redux_color_id(borders, 0).as_deref(),
            Some("color-0")
        );
        assert_eq!(
            super::er_redux_color_id(borders, 3).as_deref(),
            Some("color-1")
        );
        assert_eq!(
            super::er_redux_color_id(&[], 0),
            None,
            "an empty color palette must not produce a modulo-by-zero color id"
        );

        let mut css = String::new();
        super::write_er_redux_color_css(&mut css, "er", "classic", borders, backgrounds).unwrap();
        assert!(css.contains(
            r##"#er [data-look="classic"][data-color-id="color-0"].node path{stroke:#e879f9;fill:#fdf4ff;}"##
        ));
        assert!(css.contains(
            r##"#er [data-look="classic"][data-color-id="color-1"].node rect{stroke:#2dd4bf;fill:#f0fdfa;}"##
        ));

        let mut dark_css = String::new();
        super::write_er_redux_color_css(&mut dark_css, "er", "classic", borders, &[]).unwrap();
        assert!(dark_css.contains(
            r##"#er [data-look="classic"][data-color-id="color-0"].node path{stroke:#e879f9;}"##
        ));
        assert!(!dark_css.contains("fill:"), "{dark_css}");

        let mut merged = String::new();
        super::write_er_stylesheet(&mut merged, "er", "classic", &theme).unwrap();
        let colors = merged.find("[data-color-id=").unwrap();
        let family = merged.find("#er .entityBox").unwrap();
        let root = merged.find("#er :root").unwrap();
        assert!(colors < family && family < root, "{merged}");
    }
}
