use super::*;
use merman_render_core::theme_svg::cssom_color_value;

#[derive(Debug, Clone, Default)]
pub(crate) struct TreemapNodeVisual {
    pub(crate) fill: String,
    pub(crate) stroke: String,
    pub(crate) rect_style: String,
    pub(crate) native_category: Option<usize>,
    pub(crate) label_fill: String,
    pub(crate) value_fill: String,
    pub(crate) fitted_label_fill: String,
    pub(crate) fitted_value_fill: String,
    pub(crate) label_styles_suffix: String,
    pub(crate) label_styles_without_font_size_suffix: String,
}

impl TreemapNodeVisual {
    fn retained_bytes(&self) -> Option<usize> {
        [
            &self.fill,
            &self.stroke,
            &self.rect_style,
            &self.label_fill,
            &self.value_fill,
            &self.fitted_label_fill,
            &self.fitted_value_fill,
            &self.label_styles_suffix,
            &self.label_styles_without_font_size_suffix,
        ]
        .into_iter()
        .try_fold(0usize, |bytes, value| bytes.checked_add(value.len()))
    }
}

#[derive(Default)]
struct OrdinalScale {
    range: Vec<String>,
    domain: std::collections::HashMap<String, usize>,
}

impl OrdinalScale {
    fn get(&mut self, key: &str) -> String {
        let idx = if let Some(idx) = self.domain.get(key).copied() {
            idx
        } else {
            let idx = self.domain.len();
            self.domain.insert(key.to_string(), idx);
            idx
        };
        if self.range.is_empty() {
            return String::new();
        }
        self.range[idx % self.range.len()].clone()
    }
}

fn replace_first(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let Some(idx) = haystack.find(needle) else {
        return haystack.to_string();
    };
    let mut out = String::with_capacity(haystack.len() - needle.len() + replacement.len());
    out.push_str(&haystack[..idx]);
    out.push_str(replacement);
    out.push_str(&haystack[idx + needle.len()..]);
    out
}

#[derive(Default)]
struct OrderedMap {
    order: Vec<(String, String)>,
    idx: std::collections::HashMap<String, usize>,
}

impl OrderedMap {
    fn set(&mut self, k: &str, v: &str) {
        if k.is_empty() {
            return;
        }
        if let Some(&i) = self.idx.get(k) {
            self.order[i].1 = v.to_string();
            return;
        }
        self.idx.insert(k.to_string(), self.order.len());
        self.order.push((k.to_string(), v.to_string()));
    }
}

fn treemap_is_label_style(key: &str) -> bool {
    matches!(
        key.trim(),
        "color"
            | "font-size"
            | "font-family"
            | "font-weight"
            | "font-style"
            | "text-decoration"
            | "text-align"
            | "text-transform"
            | "line-height"
            | "letter-spacing"
            | "word-spacing"
            | "text-shadow"
            | "text-overflow"
            | "white-space"
            | "word-wrap"
            | "word-break"
            | "overflow-wrap"
            | "hyphens"
    )
}

#[derive(Debug, Clone, Default)]
struct TreemapCompiledStyles {
    label_styles: String,
    label_styles_without_font_size: String,
    node_styles: String,
    border_styles: Vec<String>,
}

fn treemap_styles2_string(
    css_compiled_styles: &[String],
    work_meter: &OperationWorkMeter,
) -> Result<TreemapCompiledStyles> {
    // Ported from Mermaid `handDrawnShapeStyles.compileStyles()` / `styles2String()`:
    // - preserve insertion order of the first occurrence of a key
    // - later occurrences override values, without changing order
    // - tolerate tokens without `:` (JS `split(':')` yields `value = undefined`)
    let mut m = OrderedMap::default();

    for entry in css_compiled_styles {
        work_meter.charge(1usize.saturating_add(entry.len().div_ceil(64)))?;
        let mut checkpoint = || work_meter.checkpoint(OperationPhase::Layout);
        mermaid_style::visit_style_declaration_boundaries_with_checkpoints(
            entry,
            &mut checkpoint,
            |boundary| {
                work_meter.charge(1)?;
                let raw = boundary.raw();
                let s = raw
                    .trim()
                    .strip_suffix(';')
                    .unwrap_or_else(|| raw.trim())
                    .trim();
                if s.is_empty() {
                    return Ok(true);
                }
                let (k, v) = if let Some(declaration) = mermaid_style::parse_style_declaration(raw)
                {
                    (declaration.property_source(), declaration.source_value())
                } else if let Some((k, v)) = s.split_once(':') {
                    (k.trim(), v.trim())
                } else {
                    (s.trim(), "")
                };
                m.set(k, v);
                Ok(true)
            },
        )?;
    }

    let mut label_styles: Vec<String> = Vec::new();
    let mut label_styles_without_font_size: Vec<String> = Vec::new();
    let mut node_styles: Vec<String> = Vec::new();
    let mut border_styles: Vec<String> = Vec::new();
    for (k, v) in &m.order {
        if v.is_empty() {
            continue;
        }
        work_meter.charge(
            1usize
                .saturating_add(k.len().div_ceil(64))
                .saturating_add(v.len().div_ceil(64)),
        )?;
        let decl = format!("{k}:{v}");
        let decl_imp = format!("{decl} !important");
        if treemap_is_label_style(k) {
            label_styles.push(decl_imp.clone());
            if k.trim() != "font-size" {
                label_styles_without_font_size.push(decl_imp);
            }
        } else {
            node_styles.push(decl_imp.clone());
            if k.contains("stroke") {
                border_styles.push(decl_imp);
            }
        }
    }

    Ok(TreemapCompiledStyles {
        label_styles: label_styles.join(";"),
        label_styles_without_font_size: label_styles_without_font_size.join(";"),
        node_styles: node_styles.join(";"),
        border_styles,
    })
}

impl TreemapStyle {
    pub(super) fn prepare_node_visuals(
        mut self,
        layout: &TreemapDiagramLayout,
        work: &Arc<OperationWorkMeter>,
        native: Option<&FamilyTheme>,
        background: Option<Color>,
    ) -> Result<Self> {
        let mut colors = OrdinalScale::default();
        colors.range.push("transparent".to_owned());
        colors.range.extend(self.css.color_scale.iter().cloned());
        let mut peers = OrdinalScale::default();
        peers.range.push("transparent".to_owned());
        peers
            .range
            .extend(self.css.color_scale_peer.iter().cloned());
        let mut labels = OrdinalScale {
            range: self.css.color_scale_label.clone(),
            ..Default::default()
        };
        let native_section = native.map(|theme| {
            (
                theme.roles.surface_container.first_color().to_string(),
                theme.roles.stroke_border.first_color().to_string(),
                theme.roles.text_primary.to_string(),
            )
        });
        let native_leaf_stroke = native.map(|theme| {
            background
                .unwrap_or_else(|| theme.roles.canvas.first_color())
                .to_string()
        });

        // Keep Mermaid's traversal and three independent first-seen color domains.
        for (section, style) in layout.sections.iter().zip(&mut self.section_source_styles) {
            work.charge(1)?;
            let (fill, stroke) = match &native_section {
                Some((fill, stroke, _)) => (fill.clone(), stroke.clone()),
                None => (colors.get(&section.name), peers.get(&section.name)),
            };
            let source = treemap_styles2_string(
                section.css_compiled_styles.as_deref().unwrap_or_default(),
                work,
            )?;
            let rect_style = if section.depth == 0 {
                "display: none;".to_owned()
            } else {
                format!("{};{}", source.node_styles, source.border_styles.join(";"))
            };
            let label_fill = match &native_section {
                Some((_, _, ink)) => ink.clone(),
                None if section.depth == 0 => String::new(),
                None => labels.get(&section.name),
            };
            style.visual = TreemapNodeVisual {
                fill,
                stroke,
                rect_style,
                value_fill: label_fill.clone(),
                label_fill,
                label_styles_suffix: replace_first(&source.label_styles, "color:", "fill:"),
                ..Default::default()
            };
        }

        let mut categories = rustc_hash::FxHashMap::default();
        for (leaf, style) in layout.leaves.iter().zip(&mut self.leaf_source_styles) {
            work.charge(1)?;
            let category = leaf.parent_name.as_deref().unwrap_or(&leaf.name);
            let source = treemap_styles2_string(
                leaf.css_compiled_styles.as_deref().unwrap_or_default(),
                work,
            )?;
            let mut visual = TreemapNodeVisual {
                rect_style: source.node_styles,
                label_styles_suffix: replace_first(&source.label_styles, "color:", "fill:"),
                label_styles_without_font_size_suffix: replace_first(
                    &source.label_styles_without_font_size,
                    "color:",
                    "fill:",
                ),
                ..Default::default()
            };
            if let Some((theme, leaf_stroke)) = native.zip(native_leaf_stroke.as_deref()) {
                let next = categories.len();
                let index = *categories.entry(category).or_insert(next);
                visual.fill = theme.data.fill(index).to_string();
                visual.stroke = leaf_stroke.to_owned();
                let ink = theme.data.fill_ink(index).to_string();
                visual.label_fill.clone_from(&ink);
                visual.value_fill.clone_from(&ink);
                visual.fitted_label_fill.clone_from(&ink);
                visual.fitted_value_fill = ink;
                visual.native_category = Some(index);
            } else {
                visual.fill = colors.get(category);
                visual.stroke.clone_from(&visual.fill);
                let label_fill = self.css.readable_leaf_label_fill(
                    &visual.fill,
                    &visual.rect_style,
                    labels.get(&leaf.name),
                );
                visual.fitted_label_fill = cssom_color_value(&label_fill);
                visual
                    .fitted_value_fill
                    .clone_from(&visual.fitted_label_fill);
                visual.value_fill.clone_from(&label_fill);
                visual.label_fill = label_fill;
            }
            style.visual = visual;
        }

        let retained = self
            .section_source_styles
            .iter()
            .chain(&self.leaf_source_styles)
            .try_fold(0usize, |bytes, style| {
                bytes.checked_add(style.visual.retained_bytes()?)
            })
            .ok_or_else(|| Error::from(work.arithmetic_overflow()))?;
        work.charge(retained)?;
        self._visual_retained_reservation =
            Some(work.reserve_prepared_text_retained_bytes(retained)?);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::treemap::test_support::{FALLBACK_C_SCALE, presentation_config};
    use merman_render_core::resources::{RenderResourcePolicy, ResourceLimitId};

    fn layout() -> TreemapDiagramLayout {
        serde_json::from_value(serde_json::json!({
            "title_height": 0, "width": 100, "height": 100, "use_max_width": true,
            "diagram_padding": 8, "show_values": true, "value_format": ",",
            "sections": [
                {"name":"root","depth":0,"value":10,"x0":0,"y0":0,"x1":100,"y1":100},
                {"name":"branch","depth":1,"value":10,"x0":0,"y0":0,"x1":100,"y1":100}
            ],
            "leaves": [
                {"name":"first","parent_name":"branch","value":5,"x0":0,"y0":0,"x1":50,"y1":100,
                 "css_compiled_styles":["color:#123456;font-size:20px;fill:#aabbcc", "color:#654321"]},
                {"name":"second","parent_name":"branch","value":5,"x0":50,"y0":0,"x1":100,"y1":100}
            ]
        })).unwrap()
    }

    #[test]
    fn visual_reservation_accepts_exact_retained_bytes_and_rejects_one_less() {
        let mut layout = layout();
        layout.sections.clear();
        layout.leaves.truncate(1);
        layout.leaves[0].css_compiled_styles = None;
        let config = PresentationConfig::empty_object();
        let meter = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let baseline =
            TreemapStyle::resolve(&config, &layout, None, None, &FALLBACK_C_SCALE, &meter).unwrap();
        let exact_bytes = meter.prepared_text_retained_bytes();
        drop(baseline);
        for (budget, succeeds) in [(exact_bytes, true), (exact_bytes - 1, false)] {
            let meter = std::sync::Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, budget)
                    .unwrap(),
            ));
            let result =
                TreemapStyle::resolve(&config, &layout, None, None, &FALLBACK_C_SCALE, &meter);
            if succeeds {
                let plan = result.expect("exact final retained-byte budget fits");
                assert_eq!(meter.prepared_text_retained_bytes(), exact_bytes);
                drop(plan);
            } else {
                assert!(matches!(result, Err(Error::ResourceLimitExceeded(_))));
            }
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }
    }

    #[test]
    fn visual_reservation_counts_folded_fields_and_ignores_unused_palette_slots() {
        let mut layout = layout();
        layout.sections.clear();
        layout.leaves.truncate(1);
        layout.leaves[0].parent_name = None;
        layout.leaves[0].css_compiled_styles = Some(vec!["fill:red;".repeat(1000)]);
        let config = presentation_config(serde_json::json!({
            "theme": "custom", "themeVariables": {"cScale11": "x".repeat(70_000)}
        }));
        let meter = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 65_536)
                .unwrap(),
        ));
        let plan = TreemapStyle::resolve(&config, &layout, None, None, &FALLBACK_C_SCALE, &meter)
            .expect("actual visual fits retained budget");
        let visual = plan.leaf_visual(0).unwrap();
        assert_eq!(visual.rect_style, "fill:red !important");
        let expected = plan.font_family_css().len()
            + plan.leaf_source_styles[0].retained_bytes().unwrap()
            + visual.retained_bytes().unwrap();
        assert_eq!(meter.prepared_text_retained_bytes(), expected);
        assert!(meter.prepared_text_retained_bytes_peak() < 65_536);
        drop(plan);
        assert_eq!(
            meter.prepared_text_retained_bytes(),
            0,
            "artifact drop releases both reservations"
        );
    }

    #[test]
    fn prepared_visuals_keep_independent_ordinal_domains_and_fitted_source_variants() {
        let config = presentation_config(serde_json::json!({
            "theme": "custom",
            "themeVariables": {"cScale0":"#111111", "cScale1":"#222222", "cScaleLabel0":"#333333", "cScaleLabel1":"#444444", "cScaleLabel2":"#555555"}
        }));
        let layout = layout();
        let plan = TreemapStyle::resolve(
            &config,
            &layout,
            None,
            None,
            &FALLBACK_C_SCALE,
            &Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            )),
        )
        .unwrap();
        let root = plan.section_visual(0).unwrap();
        let branch = plan.section_visual(1).unwrap();
        let first = plan.leaf_visual(0).unwrap();
        let second = plan.leaf_visual(1).unwrap();
        assert_eq!(root.fill, "transparent");
        assert!(root.label_fill.is_empty());
        assert_eq!(branch.fill, "#111111");
        assert_eq!(branch.label_fill, "#333333");
        assert_eq!(first.fill, branch.fill);
        assert_eq!(second.fill, branch.fill);
        assert_eq!(first.label_fill, "#444444");
        assert_eq!(second.label_fill, "#555555");
        assert_eq!(
            first.label_styles_suffix,
            "fill:#654321 !important;font-size:20px !important"
        );
        assert_eq!(
            first.label_styles_without_font_size_suffix,
            "fill:#654321 !important"
        );
        assert_eq!(first.rect_style, "fill:#aabbcc !important");
    }

    #[test]
    fn prepared_default_theme_keeps_explicit_fill_and_label_scales() {
        let config = presentation_config(serde_json::json!({
            "themeVariables": {"cScale0":"#111111", "cScaleLabel0":"#333333"}
        }));
        let layout = layout();
        let plan = TreemapStyle::resolve(
            &config,
            &layout,
            None,
            None,
            &FALLBACK_C_SCALE,
            &Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            )),
        )
        .unwrap();
        let branch = plan.section_visual(1).unwrap();
        assert_eq!(branch.fill, "#111111");
        assert_eq!(branch.label_fill, "#333333");
        assert_eq!(plan.leaf_visual(0).unwrap().fill, branch.fill);
    }

    #[test]
    fn prepared_visuals_preserve_transparent_leaf_readability_fallback() {
        let mut layout = layout();
        layout.sections.clear();
        layout.leaves.truncate(1);
        layout.leaves[0].parent_name = None;
        layout.leaves[0].css_compiled_styles = None;
        let config = presentation_config(
            serde_json::json!({"themeVariables":{"cScaleLabel0":"#ffffff", "textColor":"#123456"}}),
        );
        let plan = TreemapStyle::resolve(
            &config,
            &layout,
            None,
            None,
            &FALLBACK_C_SCALE,
            &Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            )),
        )
        .unwrap();
        let leaf = plan.leaf_visual(0).unwrap();
        assert_eq!(leaf.fill, "transparent");
        assert_eq!(leaf.label_fill, "#123456");
        assert_eq!(leaf.value_fill, "#123456");
    }
}
