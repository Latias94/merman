use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use merman_model::{OperationPhase, presentation::PresentationConfig};
use merman_render_core::{
    Error, Result, config,
    entities::decode_mermaid_entities_for_render_text,
    mermaid_style::font_family_css,
    resources::{OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation},
    svg_css::PreparedCommonCss,
    xml::strip_forbidden_xml_1_0_chars_cow_with_checkpoints,
};
use merman_theme_contract::v2::{Color, FamilyTheme};

use super::{
    SankeyDiagramLayout,
    config::{SankeyConfigView, SankeyRenderSettings},
};

const TABLEAU10: [&str; 10] = [
    "#4e79a7", "#f28e2c", "#e15759", "#76b7b2", "#59a14f", "#edc949", "#af7aa1", "#ff9da7",
    "#9c755f", "#bab0ab",
];

/// Final literal SVG text, independent of the original node and link identities.
#[derive(Debug)]
pub(crate) struct SankeyPreparedLabel {
    text: Box<str>,
}

impl SankeyPreparedLabel {
    fn new(
        text: Cow<'_, str>,
        work_meter: &OperationWorkMeter,
    ) -> std::result::Result<Self, OperationWorkError> {
        let decoded = match decode_mermaid_entities_for_render_text(&text) {
            Cow::Borrowed(_) => text,
            Cow::Owned(decoded) => Cow::Owned(decoded),
        };
        let text = strip_forbidden_xml_1_0_chars_cow_with_checkpoints(decoded, || {
            work_meter.checkpoint(OperationPhase::Layout)
        })?;
        Ok(Self {
            text: text.into_owned().into_boxed_str(),
        })
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug)]
pub struct SankeyStyle {
    pub(crate) common_css: PreparedCommonCss,
    pub(crate) label_background: String,
    pub(crate) outlined_labels: bool,
    pub(crate) label_size: f64,
    pub(crate) label_weight: Option<u16>,
    pub(crate) show_values: bool,
    pub(crate) use_max_width: bool,
    pub(crate) palette: SankeyNodePalette,
}

impl SankeyStyle {
    pub(super) fn resolve(
        config: &PresentationConfig,
        layout: &SankeyDiagramLayout,
        native: Option<&FamilyTheme>,
        canvas: Option<Color>,
        work: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        let config_view = SankeyConfigView::new(config.as_value());
        let render_settings = config_view.render_settings();
        let mut palette = SankeyNodePalette::prepare(&render_settings, layout, native, work)?;
        let link_color = config_view.link_color();
        let (common_css, label_background, label_size, label_weight) = if let Some(theme) = native {
            let typography = &theme.typography;
            let font = font_family_css(typography.stack(typography.body_family));
            (
                PreparedCommonCss::new(config.as_value(), Some(&font))
                    .with_text_color(&theme.roles.text_primary.to_string()),
                canvas
                    .unwrap_or_else(|| theme.roles.canvas.first_color())
                    .to_string(),
                typography.base_size_px,
                Some(typography.label_weight),
            )
        } else {
            let font_family = config::config_font_family_css(config);
            let label_background = config::config_string(config, &["themeVariables", "mainBkg"])
                .or_else(|| config::config_string(config, &["themeVariables", "background"]))
                .unwrap_or_else(|| "#fff".to_owned());
            (
                PreparedCommonCss::new(config.as_value(), Some(&font_family)),
                label_background,
                14.0,
                None,
            )
        };
        let link_color = if native.is_some() && matches!(link_color.as_str(), "source" | "target") {
            "gradient"
        } else {
            &link_color
        };
        palette.prepare_links(layout, link_color, work)?;
        Ok(Self {
            common_css,
            label_background,
            outlined_labels: config_view.outlined_labels(),
            label_size,
            label_weight,
            show_values: render_settings.show_values,
            use_max_width: render_settings.use_max_width,
            palette,
        })
    }
}

#[derive(Debug)]
struct SankeyNodePaint {
    id: String,
    fill_css: String,
}

#[derive(Debug, Clone, Copy)]
struct SankeyNodeLookup {
    first_layout_index: usize,
    last_paint_index: usize,
}

#[derive(Debug)]
struct SankeyPreparedLink {
    source_layout_index: usize,
    target_layout_index: usize,
    paint: SankeyLinkPaintSelection,
}

#[derive(Debug, Clone, Copy)]
enum SankeyLinkPaintSelection {
    NodeFill {
        paint_index: usize,
    },
    Gradient {
        source_paint_index: usize,
        target_paint_index: usize,
    },
    Literal,
}

pub(crate) enum SankeyLinkPaint<'a> {
    Solid(&'a str),
    Gradient { source: &'a str, target: &'a str },
}

pub(crate) struct SankeyLinkView<'a> {
    pub(crate) source_layout_index: usize,
    pub(crate) target_layout_index: usize,
    pub(crate) paint: SankeyLinkPaint<'a>,
}

/// Prepares node paint and literal labels for SVG output.
#[derive(Debug)]
pub(crate) struct SankeyNodePalette {
    node_indices: HashMap<String, SankeyNodeLookup>,
    links: Option<Vec<SankeyPreparedLink>>,
    literal_link_paint: Box<str>,
    #[cfg(test)]
    endpoint_lookups: usize,
    paints: Vec<SankeyNodePaint>,
    native_link_fills: Option<Vec<String>>,
    labels: Vec<SankeyPreparedLabel>,
    _retained_labels: PreparedTextRetainedReservation,
}

impl SankeyNodePalette {
    #[cfg(test)]
    pub(crate) fn prepare_labels_and_nodes(
        effective_config: &serde_json::Value,
        layout: &SankeyDiagramLayout,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> std::result::Result<Self, OperationWorkError> {
        let render_settings = SankeyConfigView::new(effective_config).render_settings();
        Self::prepare(&render_settings, layout, None, work_meter)
    }

    fn prepare(
        render_settings: &SankeyRenderSettings<'_>,
        layout: &SankeyDiagramLayout,
        native: Option<&FamilyTheme>,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> std::result::Result<Self, OperationWorkError> {
        let node_colors = render_settings.node_colors;
        let slot_bytes = std::mem::size_of::<SankeyPreparedLabel>();
        work_meter.preflight_prepared_text_retained_bytes(
            layout.nodes.len().saturating_mul(slot_bytes),
        )?;
        let mut labels = Vec::with_capacity(layout.nodes.len());
        let label_slots = labels.capacity().saturating_mul(slot_bytes);
        // A named entity can expand to two Unicode scalars. Allow their maximum UTF-8 bytes
        // per possible reference start, also covering shorthand restoration, without parsing it.
        const ENTITY_EXPANSION_BYTES: usize = 2 * char::MAX.len_utf8();
        let text_allowance = |text: &str| {
            text.len().saturating_add(
                text.bytes()
                    .filter(|byte| matches!(*byte, b'&' | b'#'))
                    .count()
                    .saturating_mul(ENTITY_EXPANSION_BYTES),
            )
        };
        let affix_bytes = if render_settings.show_values {
            1usize
                .saturating_add(text_allowance(&render_settings.prefix))
                .saturating_add(text_allowance(&render_settings.suffix))
        } else {
            0
        };
        let retained_upper_bound = layout.nodes.iter().try_fold(label_slots, |total, node| {
            work_meter.checkpoint(OperationPhase::Layout)?;
            let mut bytes = text_allowance(&node.id);
            if render_settings.show_values {
                let value = (node.value * 100.0).round() / 100.0;
                let mut number = ryu_js::Buffer::new();
                bytes = bytes
                    .saturating_add(affix_bytes)
                    .saturating_add(number.format(value).len());
            }
            Ok::<_, OperationWorkError>(total.saturating_add(bytes))
        })?;
        work_meter.preflight_prepared_text_retained_bytes(retained_upper_bound)?;
        let mut retained_labels =
            work_meter.reserve_prepared_text_retained_bytes(retained_upper_bound)?;
        let mut node_indices = HashMap::with_capacity(layout.nodes.len());
        let mut paints = Vec::with_capacity(layout.nodes.len());
        let mut default_color_domain = HashMap::new();
        let mut native_paints = if let Some(theme) = native {
            work_meter.charge(layout.nodes.len())?;
            Some((theme, Vec::with_capacity(layout.nodes.len())))
        } else {
            default_color_domain.reserve(layout.nodes.len());
            None
        };

        for (node_index, node) in layout.nodes.iter().enumerate() {
            work_meter.checkpoint(OperationPhase::Layout)?;
            let text = if render_settings.show_values {
                let value = (node.value * 100.0).round() / 100.0;
                let mut number = ryu_js::Buffer::new();
                Cow::Owned(format!(
                    "{}\n{}{}{}",
                    node.id,
                    render_settings.prefix,
                    number.format(value),
                    render_settings.suffix,
                ))
            } else {
                Cow::Borrowed(node.id.as_str())
            };
            labels.push(SankeyPreparedLabel::new(text, work_meter)?);
            let explicit_fill = node_colors
                .and_then(|colors| colors.get(&node.id))
                .and_then(serde_json::Value::as_str);
            node_indices
                .entry(node.id.clone())
                .and_modify(|lookup: &mut SankeyNodeLookup| lookup.last_paint_index = node_index)
                .or_insert(SankeyNodeLookup {
                    first_layout_index: node_index,
                    last_paint_index: node_index,
                });
            let fill_css = if let Some((theme, fills)) = &mut native_paints {
                fills.push(
                    explicit_fill
                        .map_or_else(|| theme.data.fill(node_index).to_string(), str::to_owned),
                );
                explicit_fill.map_or_else(|| theme.data.ink(node_index).to_string(), str::to_owned)
            } else {
                explicit_fill
                    .unwrap_or_else(|| {
                        let next_index = default_color_domain.len();
                        let palette_index = *default_color_domain
                            .entry(node.id.as_str())
                            .or_insert(next_index);
                        TABLEAU10[palette_index % TABLEAU10.len()]
                    })
                    .to_owned()
            };
            paints.push(SankeyNodePaint {
                id: node.id.clone(),
                fill_css,
            });
        }

        let retained_bytes = labels.iter().fold(label_slots, |bytes, label| {
            bytes.saturating_add(label.text().len())
        });
        retained_labels.reconcile_downward(retained_bytes);

        Ok(Self {
            node_indices,
            links: None,
            literal_link_paint: "".into(),
            #[cfg(test)]
            endpoint_lookups: 0,
            paints,
            native_link_fills: native_paints.map(|(_, fills)| fills),
            labels,
            _retained_labels: retained_labels,
        })
    }

    pub(crate) const fn link_stroke_opacity(&self) -> &str {
        if self.native_link_fills.is_some() {
            "0.35"
        } else {
            "0.5"
        }
    }

    pub(crate) fn labels(&self) -> &[SankeyPreparedLabel] {
        &self.labels
    }

    pub(crate) fn link_blend_mode_css(&self) -> &'static str {
        if self.links_before_nodes() {
            "normal"
        } else {
            "multiply"
        }
    }

    pub(crate) fn links_before_nodes(&self) -> bool {
        self.native_link_fills.is_some()
    }

    pub(crate) fn fill_for(&self, node_index: usize, node_id: &str) -> Option<&str> {
        self.paints
            .get(node_index)
            .filter(|paint| paint.id == node_id)
            .map(|paint| paint.fill_css.as_str())
    }

    /// Selects each link once from final node paints without allocating terminal SVG IDs.
    pub(crate) fn prepare_links(
        &mut self,
        layout: &SankeyDiagramLayout,
        link_color: &str,
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        assert!(
            self.links.is_none(),
            "Sankey links prepared once before emission"
        );
        enum Mode {
            Source,
            Target,
            Gradient,
            Literal,
        }
        let mode = match link_color {
            "source" => Mode::Source,
            "target" => Mode::Target,
            "gradient" => Mode::Gradient,
            _ => Mode::Literal,
        };
        let mut links = Vec::with_capacity(layout.links.len());
        for link in &layout.links {
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            #[cfg(test)]
            {
                self.endpoint_lookups += 1;
            }
            let source =
                self.node_indices
                    .get(&link.source)
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("missing source node {}", link.source),
                    })?;
            #[cfg(test)]
            {
                self.endpoint_lookups += 1;
            }
            let target =
                self.node_indices
                    .get(&link.target)
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("missing target node {}", link.target),
                    })?;
            let paint = match mode {
                Mode::Source => SankeyLinkPaintSelection::NodeFill {
                    paint_index: source.last_paint_index,
                },
                Mode::Target => SankeyLinkPaintSelection::NodeFill {
                    paint_index: target.last_paint_index,
                },
                Mode::Gradient => SankeyLinkPaintSelection::Gradient {
                    source_paint_index: source.last_paint_index,
                    target_paint_index: target.last_paint_index,
                },
                Mode::Literal => SankeyLinkPaintSelection::Literal,
            };
            links.push(SankeyPreparedLink {
                source_layout_index: source.first_layout_index,
                target_layout_index: target.first_layout_index,
                paint,
            });
        }
        self.literal_link_paint = link_color.into();
        self.links = Some(links);
        Ok(())
    }

    pub(crate) fn terminal_link(&self, link_index: usize) -> Option<SankeyLinkView<'_>> {
        let link = self.links.as_ref()?.get(link_index)?;
        let fill = |index: usize| match &self.native_link_fills {
            Some(fills) => fills.get(index).map(String::as_str),
            None => self.paints.get(index).map(|paint| paint.fill_css.as_str()),
        };
        let paint = match link.paint {
            SankeyLinkPaintSelection::NodeFill { paint_index } => {
                SankeyLinkPaint::Solid(fill(paint_index)?)
            }
            SankeyLinkPaintSelection::Gradient {
                source_paint_index,
                target_paint_index,
            } => SankeyLinkPaint::Gradient {
                source: fill(source_paint_index)?,
                target: fill(target_paint_index)?,
            },
            SankeyLinkPaintSelection::Literal => SankeyLinkPaint::Solid(&self.literal_link_paint),
        };
        Some(SankeyLinkView {
            source_layout_index: link.source_layout_index,
            target_layout_index: link.target_layout_index,
            paint,
        })
    }

    #[cfg(test)]
    pub(crate) fn link_lookup_stats(&self) -> (usize, usize) {
        (self.paints.len(), self.endpoint_lookups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sankey::{SankeyNodeLayout, test_support::presentation_config};
    use merman_render_core::resources::RenderResourcePolicy;
    use serde_json::json;

    #[test]
    fn unthemed_binding_keeps_label_background_and_link_paint_semantics() {
        let config = presentation_config(json!({
            "sankey": {"linkColor": "var(--link)", "labelStyle": "outlined"},
            "themeVariables": {"background": "fallback", "mainBkg": "",
                "textColor": "currentColor"}
        }));
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let plan = SankeyStyle::resolve(&config, &layout(&["A"]), None, None, &meter).unwrap();
        assert_eq!(plan.label_background, "");
        assert_eq!(plan.palette.literal_link_paint.as_ref(), "var(--link)");
        assert!(plan.outlined_labels);
        assert_eq!(plan.common_css.text_color(), "currentColor");

        let ignored = presentation_config(json!({"sankey": {
            "$ref": "schema", "linkColor": "source", "labelStyle": "outlined"
        }}));
        let fallback = SankeyStyle::resolve(&ignored, &layout(&["A"]), None, None, &meter).unwrap();
        assert_eq!(fallback.palette.literal_link_paint.as_ref(), "gradient");
        assert!(!fallback.outlined_labels);
        assert_eq!(fallback.label_background, "#fff");
    }

    fn layout(ids: &[&str]) -> SankeyDiagramLayout {
        SankeyDiagramLayout {
            bounds: None,
            width: 600.0,
            height: 400.0,
            node_width: 10.0,
            node_padding: 12.0,
            nodes: ids
                .iter()
                .enumerate()
                .map(|(index, id)| SankeyNodeLayout {
                    id: (*id).to_string(),
                    index,
                    depth: index,
                    height: ids.len().saturating_sub(index + 1),
                    layer: index,
                    value: 1.0,
                    x0: index as f64 * 20.0,
                    x1: index as f64 * 20.0 + 10.0,
                    y0: 0.0,
                    y1: 10.0,
                })
                .collect(),
            links: Vec::new(),
        }
    }

    #[test]
    fn prepared_labels_account_for_storage_and_release_the_shared_meter() {
        use merman_render_core::resources::ResourceLimitId;

        let config = presentation_config(json!({"sankey":{"showValues":false}}));
        let plain = layout(&["AB", "CD"]);
        let probe = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let plan =
            SankeyNodePalette::prepare_labels_and_nodes(config.as_value(), &plain, &probe).unwrap();
        let slots = plan.labels.capacity() * std::mem::size_of::<SankeyPreparedLabel>();
        let retained = slots + 4;
        assert_eq!(probe.prepared_text_retained_bytes(), retained);
        assert_eq!(probe.prepared_text_retained_bytes_peak(), retained);
        drop(plan);
        assert_eq!(probe.prepared_text_retained_bytes(), 0);

        for (ids, expected) in [
            (["#35;32;", "#1;"], ["#32;", ""]),
            (
                ["#&nLt;&nLt;", ""],
                ["#\u{226a}\u{20d2}\u{226a}\u{20d2}", ""],
            ),
        ] {
            let decoded = layout(&ids);
            let plan =
                SankeyNodePalette::prepare_labels_and_nodes(config.as_value(), &decoded, &probe)
                    .unwrap();
            for (label, expected) in plan.labels().iter().zip(expected) {
                assert_eq!(label.text(), expected);
            }
            assert_eq!(
                probe.prepared_text_retained_bytes(),
                plan.labels.capacity() * std::mem::size_of::<SankeyPreparedLabel>()
                    + expected.iter().map(|text| text.len()).sum::<usize>(),
                "decoded payload bytes replace the conservative preparation allowance"
            );
            drop(plan);
            assert_eq!(probe.prepared_text_retained_bytes(), 0);
        }

        let shared_bytes = 7;
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    ResourceLimitId::MaxPreparedTextRetainedBytes,
                    shared_bytes + retained,
                )
                .unwrap(),
        ));
        let other_owner = meter
            .reserve_prepared_text_retained_bytes(shared_bytes)
            .unwrap();
        let exact =
            SankeyNodePalette::prepare_labels_and_nodes(config.as_value(), &plain, &meter).unwrap();
        assert_eq!(
            meter.prepared_text_retained_bytes(),
            shared_bytes + retained
        );
        let error = SankeyNodePalette::prepare_labels_and_nodes(config.as_value(), &plain, &meter)
            .unwrap_err();
        let OperationWorkError::ResourceLimitExceeded(error) = error else {
            panic!("a second owner must share the prepared-text ceiling");
        };
        assert_eq!(error.limit, "max_prepared_text_retained_bytes");
        assert_eq!(
            meter.prepared_text_retained_bytes(),
            shared_bytes + retained
        );
        drop(exact);
        assert_eq!(meter.prepared_text_retained_bytes(), shared_bytes);
        drop(other_owner);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);

        let below = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, retained - 1)
                .unwrap(),
        ));
        let error = SankeyNodePalette::prepare_labels_and_nodes(config.as_value(), &plain, &below)
            .unwrap_err();
        let OperationWorkError::ResourceLimitExceeded(error) = error else {
            panic!("one byte below the prepared-text boundary must fail");
        };
        assert_eq!(error.limit, "max_prepared_text_retained_bytes");
        assert_eq!(error.max, retained - 1);
        assert_eq!(error.actual, retained);
        assert_eq!(below.prepared_text_retained_bytes(), 0);
        assert_eq!(below.prepared_text_retained_bytes_peak(), 0);
    }
}
