use super::*;
use std::fmt::Write;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ErTextVisual {
    pub(crate) typed_style: String,
    pub(crate) wrapper_attr: String,
    pub(crate) inline_attr: String,
}

impl ErTextVisual {
    fn byte_len(&self) -> usize {
        self.typed_style
            .len()
            .saturating_add(self.wrapper_attr.len())
            .saturating_add(self.inline_attr.len())
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ErRowVisual {
    pub(crate) fill: String,
    pub(crate) has_fill: bool,
    pub(crate) fill_style_attr: String,
    pub(crate) border_style_attr: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ErSourceVisual {
    pub(crate) rect_style_attr: String,
    pub(crate) source_text_attr: String,
    pub(crate) svg_source_text_attr: String,
    pub(crate) subgraph_text_style: String,
    pub(crate) label_div_color_prefix: String,
    pub(crate) span_style_attr: String,
    pub(crate) box_fill: String,
    pub(crate) box_stroke: String,
    pub(crate) stroke_width_attr: String,
    pub(crate) group_style_attr: String,
    pub(crate) override_style_attr: String,
    pub(crate) base_fill_style_attr: String,
    pub(crate) rows: Vec<ErRowVisual>,
}

impl ErSourceVisual {
    fn byte_len(&self) -> usize {
        let bytes = [
            &self.rect_style_attr,
            &self.source_text_attr,
            &self.svg_source_text_attr,
            &self.subgraph_text_style,
            &self.label_div_color_prefix,
            &self.span_style_attr,
            &self.box_fill,
            &self.box_stroke,
            &self.stroke_width_attr,
            &self.group_style_attr,
            &self.override_style_attr,
            &self.base_fill_style_attr,
        ]
        .into_iter()
        .fold(0usize, |bytes, value| bytes.saturating_add(value.len()));
        self.rows.iter().fold(bytes, |bytes, row| {
            bytes
                .saturating_add(row.fill.len())
                .saturating_add(row.fill_style_attr.len())
                .saturating_add(row.border_style_attr.len())
        })
    }
}

pub(super) fn declarations(
    decls: &[ErStyleDeclaration],
    force_important: bool,
    join: &str,
) -> String {
    declarations_for_label_mode(decls, force_important, join, false)
}

fn declarations_for_label_mode(
    decls: &[ErStyleDeclaration],
    force_important: bool,
    join: &str,
    svg_text: bool,
) -> String {
    let mut out = String::new();
    for declaration in decls {
        if !out.is_empty() {
            out.push_str(join);
        }
        write!(
            out,
            "{}:{}",
            if svg_text && declaration.property() == "color" {
                "fill"
            } else {
                declaration.property_css()
            },
            declaration.value()
        )
        .unwrap();
        if force_important || declaration.important() {
            out.push_str(" !important");
        }
    }
    out
}

fn attr(style: &str) -> String {
    format!(r#"style="{}""#, crate::svg::escape_attr(style))
}

fn path_attr(style: &str) -> String {
    if style.is_empty() {
        String::new()
    } else {
        format!(" {}", attr(style))
    }
}

pub(super) fn text_visual(paint: Option<&str>) -> ErTextVisual {
    let typed_style = paint
        .map(|css| format!("color:{css};fill:{css}"))
        .unwrap_or_default();
    ErTextVisual {
        inline_attr: path_attr(&typed_style),
        typed_style,
        wrapper_attr: paint
            .map(|css| {
                format!(
                    r#" style="fill:{} !important""#,
                    crate::svg::escape_attr(css)
                )
            })
            .unwrap_or_default(),
    }
}

pub(super) fn label_style(source: &ErEntitySourceStyle, html_labels: bool) -> String {
    crate::svg::escape_attr(&declarations_for_label_mode(
        &source.text_declarations,
        true,
        "; ",
        !html_labels,
    ))
}

fn source_text_visual(source: &ErEntitySourceStyle) -> ErSourceVisual {
    let text = declarations(&source.text_declarations, true, "; ");
    ErSourceVisual {
        label_div_color_prefix: source
            .text_value("color")
            .map(|value| {
                format!(
                    "color: {} !important; ",
                    crate::svg::cssom_color_value(value)
                )
            })
            .unwrap_or_default(),
        span_style_attr: if text.is_empty() {
            String::new()
        } else {
            format!(r#" style="{}""#, crate::svg::escape_xml(&text))
        },
        ..Default::default()
    }
}

pub(super) fn source_visual(source: &ErEntitySourceStyle) -> ErSourceVisual {
    let rect = declarations(&source.rect_declarations, true, "; ");
    let text = declarations(&source.text_declarations, true, "; ");
    ErSourceVisual {
        rect_style_attr: format!(r#"style="{}""#, crate::svg::escape_xml(&rect)),
        source_text_attr: format!(r#"style="{}""#, crate::svg::escape_xml(&text)),
        svg_source_text_attr: attr(&declarations_for_label_mode(
            &source.text_declarations,
            true,
            "; ",
            true,
        )),
        ..Default::default()
    }
}

impl ErEntityThemePlan {
    pub(super) fn prepare_visuals(
        mut self,
        model: &merman_model::families::er::ErDiagramModel,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        for terminal in self.text_terminals.values_mut() {
            terminal.visual = text_visual(terminal.paint.as_ref().map(|paint| paint.css.as_str()));
            work.charge(1usize.saturating_add(terminal.visual.byte_len()))?;
        }
        for (index, entity) in model.entities.values().enumerate() {
            work.charge(1usize.saturating_add(entity.attributes.len()))?;
            let source = &self.entity_source_styles[index];
            let fill = self.typed_fill(index);
            let stroke = self.typed_stroke(index);
            let mut output = source_text_visual(source);
            let mut rect = Vec::new();
            if !source.rect_declarations.is_empty() {
                rect.push(declarations(&source.rect_declarations, true, "; "));
            }
            if let Some((_, css)) = fill {
                rect.push(format!("fill:{css}"));
            }
            if let Some((_, css)) = stroke {
                rect.push(format!("stroke:{css}"));
            }
            output.rect_style_attr = attr(&rect.join("; "));
            output.box_fill = source
                .fill()
                .or_else(|| fill.map(|(_, css)| css))
                .unwrap_or(&self.css_binding.main_bkg)
                .to_owned();
            output.box_stroke = source
                .stroke()
                .or_else(|| stroke.map(|(_, css)| css))
                .unwrap_or(&self.css_binding.node_border)
                .to_owned();
            let width = source
                .rect_value("stroke-width")
                .and_then(|v| {
                    let raw = v
                        .trim()
                        .trim_end_matches(';')
                        .trim()
                        .trim_end_matches("px")
                        .trim();
                    if raw.is_empty() {
                        None
                    } else {
                        raw.parse::<f64>().ok()
                    }
                })
                .unwrap_or(1.3)
                .max(0.0);
            output.stroke_width_attr = crate::number_format::canonical_number(width).to_string();
            let group: Vec<_> = source
                .rect_declarations
                .iter()
                .filter(|d| ["fill", "stroke", "stroke-width"].contains(&d.property()))
                .cloned()
                .collect();
            output.group_style_attr = format!(
                r#"style="{}""#,
                crate::svg::escape_xml(&declarations(&group, false, ";"))
            );
            let redux = self.css_binding.source_fill_owns_odd_rows();
            let mut overrides: Vec<_> = source
                .rect_declarations
                .iter()
                .filter(|d| redux || d.property().contains("stroke"))
                .map(|d| declarations(std::slice::from_ref(d), true, "; "))
                .collect();
            if source.stroke().is_none()
                && let Some((_, css)) = stroke
            {
                overrides.push(format!("stroke:{css}"));
            }
            output.override_style_attr = path_attr(&overrides.join("; "));
            let mut base = overrides.clone();
            if let Some((_, css)) = fill {
                base.push(format!("fill:{css}"));
            }
            output.base_fill_style_attr = path_attr(&base.join("; "));
            let even_override = if source.rect_declarations.is_empty() {
                output.override_style_attr.clone()
            } else {
                let mut parts = vec![declarations(&source.rect_declarations, true, "; ")];
                if source.stroke().is_none()
                    && let Some((_, css)) = stroke
                {
                    parts.push(format!("stroke:{css}"));
                }
                path_attr(&parts.join("; "))
            };
            for row_index in 0..entity.attributes.len() {
                let baseline = if row_index.is_multiple_of(2) {
                    self.css_binding.row_odd.as_deref()
                } else {
                    self.css_binding.row_even.as_deref()
                };
                let typed = self.typed_table_row(&entity.id, row_index);
                let border_style_attr = if row_index.is_multiple_of(2) {
                    output.override_style_attr.clone()
                } else {
                    even_override.clone()
                };
                let typed_style = typed
                    .map(|(_, css)| format!("fill:{css}"))
                    .unwrap_or_default();
                let fill_style_attr = if typed.is_some() {
                    let mut parts = vec![typed_style.clone()];
                    parts.extend(overrides.iter().cloned());
                    path_attr(&parts.join("; "))
                } else {
                    border_style_attr.clone()
                };
                output.rows.push(ErRowVisual {
                    fill: baseline.unwrap_or("none").to_owned(),
                    has_fill: baseline.is_some_and(|v| !v.is_empty() && v != "none")
                        || typed.is_some(),
                    fill_style_attr,
                    border_style_attr,
                });
            }
            work.charge(output.byte_len())?;
            self.entity_source_styles[index].visual = output;
        }
        for (id, source) in &mut self.subgraph_source_styles {
            let mut output = source_visual(source);
            let terminal = self
                .text_terminals
                .get(&ErTextTerminalId::SubgraphLabel(id.clone().into()));
            output.subgraph_text_style = source
                .text_value("color")
                .map(|css| format!("fill:{css} !important"))
                .unwrap_or_else(|| {
                    terminal
                        .map(|terminal| terminal.visual.typed_style.clone())
                        .unwrap_or_default()
                });
            work.charge(output.byte_len())?;
            source.visual = output;
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn er_source_important_declarations_are_serialized_once() {
        let entity = merman_model::families::er::ErEntity {
            css_styles: vec![
                "fill:#123456 !important".into(),
                "stroke:#654321 !important".into(),
            ],
            ..Default::default()
        };
        let source = compile_er_entity_source_style(&entity, &indexmap::IndexMap::new());
        let visual = source_visual(&source);
        assert_eq!(source.fill(), Some("#123456"));
        assert_eq!(visual.rect_style_attr.matches("!important").count(), 2);
        assert!(!visual.rect_style_attr.contains("!important !important"));
        assert_eq!(
            visual.rect_style_attr,
            "style=\"fill:#123456 !important; stroke:#654321 !important\""
        );
    }
}
