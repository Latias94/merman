use std::collections::BTreeMap;

use merman_model::families::c4::C4DiagramModel;
use merman_model::presentation::PresentationConfig;

use crate::model::C4DiagramLayout;

/// Source-owned colors and theme fallbacks for each prepared layout occurrence.
#[derive(Debug)]
pub(crate) struct C4TextPaintPlan {
    elements: Vec<C4ElementTerminalPaint>,
    relations: Vec<C4RelationTerminalPaint>,
    boundary_text: Vec<Option<String>>,
}

#[derive(Debug)]
pub(crate) struct C4ElementTerminalPaint {
    pub(crate) background: String,
    pub(crate) border: String,
    pub(crate) text: String,
}

#[derive(Debug)]
pub(crate) struct C4RelationTerminalPaint {
    pub(crate) text: String,
    pub(crate) line: String,
}

impl C4TextPaintPlan {
    pub(crate) fn prepare(
        model: &C4DiagramModel,
        layout: &C4DiagramLayout,
        typography: &mut super::C4TypographyThemePlan,
        config: &PresentationConfig,
    ) -> crate::Result<Self> {
        // The last source entry for an alias owns overrides. Defaults belong to each
        // layout occurrence's actual type, even when aliases repeat.
        let source_shapes: BTreeMap<_, _> = model
            .shapes
            .iter()
            .map(|shape| (shape.alias.as_str(), shape))
            .collect();
        let view = super::C4ConfigView::new(config.as_value());
        let native = typography.native_style().is_some();
        let config_color = |path: &str| {
            config
                .get_str(path)
                .filter(|_| !native || config.explicit_config_owns_path(path))
        };
        let mut elements = Vec::with_capacity(layout.shapes.len());
        for shape in &layout.shapes {
            let source = source_shapes.get(shape.alias.as_str()).ok_or_else(|| {
                crate::Error::InvalidModel {
                    message: format!("c4: missing model shape {}", shape.alias),
                }
            })?;
            let defaults = typography
                .prepare_element_colors(&shape.type_c4_shape, &view)
                .ok_or_else(|| crate::Error::InvalidModel {
                    message: format!("c4: missing prepared element style {}", shape.type_c4_shape),
                })?;
            let text = source
                .font_color
                .as_deref()
                .or_else(|| config_color(&format!("c4.{}_font_color", shape.type_c4_shape)))
                .unwrap_or(&defaults.text);
            elements.push(C4ElementTerminalPaint {
                background: source
                    .bg_color
                    .as_deref()
                    .unwrap_or(&defaults.background)
                    .to_owned(),
                border: source
                    .border_color
                    .as_deref()
                    .unwrap_or(&defaults.border)
                    .to_owned(),
                text: text.to_owned(),
            });
        }
        let uses_shoal = cfg!(feature = "layout-shoal")
            && crate::layout_backend::resolve_graph_layout(config.as_value()).backend
                == crate::layout_backend::GraphLayoutBackend::Shoal;
        let source_relations = (!uses_shoal).then(|| {
            model
                .rels
                .iter()
                .map(|relation| {
                    (
                        (relation.from_alias.as_str(), relation.to_alias.as_str()),
                        relation,
                    )
                })
                .collect::<BTreeMap<_, _>>()
        });
        let relations = layout
            .rels
            .iter()
            .enumerate()
            .map(|(index, relation)| {
                let source = if let Some(source_relations) = &source_relations {
                    source_relations
                        .get(&(relation.from.as_str(), relation.to.as_str()))
                        .copied()
                } else {
                    model.rels.get(index)
                };
                C4RelationTerminalPaint {
                    text: source
                        .and_then(|source| source.text_color.as_deref())
                        .or_else(|| config_color("c4.relationship_text_color"))
                        .unwrap_or(typography.relationship_text_color())
                        .to_owned(),
                    line: source
                        .and_then(|source| source.line_color.as_deref())
                        .or_else(|| config_color("c4.relationship_line_color"))
                        .unwrap_or(typography.relationship_line_color())
                        .to_owned(),
                }
            })
            .collect();
        let source_boundaries: BTreeMap<_, _> = model
            .boundaries
            .iter()
            .map(|boundary| (boundary.alias.as_str(), boundary))
            .collect();
        let boundary_text = layout
            .boundaries
            .iter()
            .map(|boundary| {
                (boundary.alias != "global").then(|| {
                    source_boundaries
                        .get(boundary.alias.as_str())
                        .and_then(|boundary| boundary.font_color.as_deref())
                        .or_else(|| config_color("c4.boundary_font_color"))
                        .unwrap_or_else(|| {
                            typography
                                .native_style()
                                .map_or("#444444", |native| native.text_ink.as_str())
                        })
                        .to_owned()
                })
            })
            .collect();
        Ok(Self {
            elements,
            relations,
            boundary_text,
        })
    }

    pub(crate) fn boundary_text_color(&self, index: usize) -> Option<&str> {
        self.boundary_text.get(index)?.as_deref()
    }

    pub(crate) fn element_paint(&self, index: usize) -> Option<&C4ElementTerminalPaint> {
        self.elements.get(index)
    }

    pub(crate) fn relation_paint(&self, index: usize) -> Option<&C4RelationTerminalPaint> {
        self.relations.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_terminal_binding_preserves_actual_layout_defaults_and_last_metadata() {
        let model = serde_json::from_value(serde_json::json!({
            "shapes": [
                {"alias": "duplicate", "typeC4Shape": "person", "bgColor": "red", "fontColor": "blue"},
                {"alias": "duplicate", "typeC4Shape": "custom", "bgColor": "", "fontColor": "var(--label)"}
            ],
            "rels": [
                {"from": "duplicate", "to": "duplicate", "type": "rel", "lineColor": "red"},
                {"from": "duplicate", "to": "duplicate", "type": "rel", "textColor": ""}
            ]
        })).expect("C4 source metadata fixture");
        let block =
            serde_json::json!({"text": "", "y": 0.0, "width": 0.0, "height": 0.0, "line_count": 0});
        let shape = serde_json::json!({
            "alias": "duplicate", "parent_boundary": "", "type_c4_shape": "person",
            "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0, "margin": 0.0,
            "image": {"width": 0.0, "height": 0.0, "y": 0.0}, "type_block": block, "label": block
        });
        let relation = serde_json::json!({
            "from": "duplicate", "to": "duplicate", "rel_type": "rel",
            "start_point": {"x": 0.0, "y": 0.0}, "end_point": {"x": 1.0, "y": 1.0}, "label": block
        });
        let mut external_shape = shape.clone();
        external_shape["type_c4_shape"] = serde_json::json!("external_person");
        let layout = serde_json::from_value(serde_json::json!({
            "bounds": null, "width": 10.0, "height": 10.0,
            "container_width": 100.0, "container_height": 100.0, "c4_type": "C4Context", "title": null,
            "boundaries": [], "shapes": [shape, external_shape], "rels": [relation, relation]
        })).expect("C4 actual layout fixture");
        let config = crate::test_support::presentation_config(serde_json::json!({"c4": {
            "person_border_color": "var(--actual-border)", "custom_border_color": "red"
        }}));
        let mut typography = super::super::C4TypographyThemePlan::resolve(None, &config, &model);
        let plan = C4TextPaintPlan::prepare(&model, &layout, &mut typography, &config)
            .expect("C4 source paint");
        let paint = plan.element_paint(0).expect("bound occurrence");
        assert_eq!(paint.background, "");
        assert_eq!(paint.border, "var(--actual-border)");
        assert_eq!(paint.text, "var(--label)");
        let external = plan.element_paint(1).expect("second bound occurrence");
        assert_eq!(external.background, "");
        assert_eq!(external.border, "#8A8A8A");
        assert_eq!(external.text, "var(--label)");
        let paint = plan
            .relation_paint(0)
            .expect("bound relationship occurrence");
        assert_eq!(paint.text, "");
        assert_eq!(paint.line, "#444444");
    }
}
