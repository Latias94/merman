use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::model::SankeyDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::SankeyConfigView;

const TABLEAU10: [&str; 10] = [
    "#4e79a7", "#f28e2c", "#e15759", "#76b7b2", "#59a14f", "#edc949", "#af7aa1", "#ff9da7",
    "#9c755f", "#bab0ab",
];

#[derive(Debug)]
struct SankeyNodePaint {
    id: String,
    ordinal: usize,
    fill_css: String,
    source_owned: bool,
    typed_capability: Option<ThemeCapability>,
}

/// Resolves Sankey node paint once for terminal rectangles, derived link colors, and evidence.
#[derive(Debug)]
pub(crate) struct SankeyNodePalettePlan {
    node_indices: HashMap<String, usize>,
    paints: Vec<SankeyNodePaint>,
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    terminal_capabilities: OnceLock<BTreeSet<ThemeCapability>>,
}

impl SankeyNodePalettePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &SankeyDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(effective_config.as_value(), layout);
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let Some(disposition) = theme.ordinal_palette_disposition(ThemeTarget::Node) else {
            return Ok(plan);
        };
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Node,
        };

        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                plan.palette_key = Some(key.clone());
                let mut missing_typed_color = false;
                for paint in &mut plan.paints {
                    if paint.source_owned {
                        continue;
                    }
                    let style = theme.style_with_work_meter(
                        ThemeTarget::Node,
                        ThemeVariant::Default,
                        Some(paint.ordinal),
                        work_meter,
                    )?;
                    if style.fill_resolution().winner().is_some() {
                        continue;
                    }
                    let Some(color) = theme.series_color(ThemeTarget::Node, paint.ordinal) else {
                        missing_typed_color = true;
                        continue;
                    };
                    paint.fill_css = color.as_css();
                    paint.typed_capability = Some(if color.is_transparent() {
                        ThemeCapability::TransparentPaint
                    } else {
                        ThemeCapability::SolidPaint
                    });
                }
                if missing_typed_color {
                    plan.evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
                    plan.palette_key = None;
                }
            }
            FamilyThemeDisposition::Unsupported => plan
                .evidence
                .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette),
            FamilyThemeDisposition::LegacyCompatibility => {}
        }

        Ok(plan)
    }

    fn baseline(effective_config: &serde_json::Value, layout: &SankeyDiagramLayout) -> Self {
        let render_settings = SankeyConfigView::new(effective_config).render_settings();
        let node_colors = render_settings.node_colors;
        let mut node_indices = HashMap::with_capacity(layout.nodes.len());
        let mut paints = Vec::with_capacity(layout.nodes.len());

        for (node_index, node) in layout.nodes.iter().enumerate() {
            let explicit_fill = node_colors
                .and_then(|colors| colors.get(&node.id))
                .and_then(serde_json::Value::as_str);
            node_indices.insert(node.id.clone(), node_index);
            paints.push(SankeyNodePaint {
                id: node.id.clone(),
                ordinal: node_index + 1,
                fill_css: explicit_fill
                    .unwrap_or(TABLEAU10[node_index % TABLEAU10.len()])
                    .to_string(),
                source_owned: explicit_fill.is_some(),
                typed_capability: None,
            });
        }

        Self {
            node_indices,
            paints,
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            terminal_capabilities: OnceLock::new(),
        }
    }

    pub(crate) fn fill_for(&self, node_index: usize, node_id: &str) -> Option<&str> {
        self.paints
            .get(node_index)
            .filter(|paint| paint.id == node_id)
            .map(|paint| paint.fill_css.as_str())
    }

    pub(crate) fn fill_for_id(&self, node_id: &str) -> Option<&str> {
        let index = *self.node_indices.get(node_id)?;
        self.paints.get(index).map(|paint| paint.fill_css.as_str())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<SankeyNodePaletteReceipt> {
        self.palette_key
            .as_ref()
            .map(|_| SankeyNodePaletteReceipt::new(self.paints.len()))
    }

    pub(crate) fn record_terminal(&self, receipt: SankeyNodePaletteReceipt) -> bool {
        if self.palette_key.is_none() || !receipt.proves_complete(&self.paints) {
            return false;
        }
        self.terminal_capabilities
            .set(receipt.applied_capabilities)
            .is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(key) = self.palette_key.clone() else {
            return evidence;
        };
        match self.terminal_capabilities.get() {
            Some(capabilities) if capabilities.is_empty() => evidence.mark_not_applicable(key),
            Some(capabilities) => {
                evidence.mark_applied_with_capabilities(key, capabilities.iter().copied())
            }
            None => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette)
            }
        }
        evidence
    }
}

/// Writer-owned proof that every semantic Sankey node emitted its planned terminal rectangle fill.
#[derive(Debug)]
pub(crate) struct SankeyNodePaletteReceipt {
    expected_count: usize,
    next_index: usize,
    values_match: bool,
    applied_capabilities: BTreeSet<ThemeCapability>,
}

impl SankeyNodePaletteReceipt {
    fn new(expected_count: usize) -> Self {
        Self {
            expected_count,
            next_index: 0,
            values_match: true,
            applied_capabilities: BTreeSet::new(),
        }
    }

    pub(crate) fn record_node(
        &mut self,
        plan: &SankeyNodePalettePlan,
        emitted_index: usize,
        emitted_id: &str,
        emitted_ordinal: usize,
        emitted_fill: Option<&str>,
    ) {
        if emitted_index != self.next_index {
            self.values_match = false;
            return;
        }
        self.next_index = self.next_index.saturating_add(1);
        let Some(paint) = plan.paints.get(emitted_index) else {
            self.values_match = false;
            return;
        };
        let matches = paint.id == emitted_id
            && paint.ordinal == emitted_ordinal
            && emitted_fill.is_some_and(|fill| fill == paint.fill_css);
        self.values_match &= matches;
        if matches {
            self.applied_capabilities.extend(paint.typed_capability);
        }
    }

    fn proves_complete(&self, paints: &[SankeyNodePaint]) -> bool {
        self.values_match
            && self.expected_count == paints.len()
            && self.next_index == self.expected_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue, ThemeRuleSet,
    };
    use crate::model::SankeyNodeLayout;
    use crate::resources::RenderResourcePolicy;
    use serde_json::json;

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

    fn resolved_node_palette(colors: &[&str]) -> ResolvedDiagramTheme {
        let palette = OrdinalPalette::new(
            colors
                .iter()
                .map(|color| ThemeColorValue::parse(*color).expect("valid Sankey test color")),
        )
        .expect("non-empty Sankey test palette");
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette),
            ))
            .expect("compile Sankey test palette")
            .resolve(DiagramFamilyId::SANKEY)
    }

    fn palette_plan(config: serde_json::Value) -> SankeyNodePalettePlan {
        let layout = layout(&["A", "B"]);
        let config = MermaidConfig::from_value(config);
        let theme = resolved_node_palette(&["#123456", "#abcdef"]);
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        SankeyNodePalettePlan::resolve(Some(&theme), &config, &layout, &work_meter)
            .expect("resolve Sankey test palette")
    }

    #[test]
    fn explicit_node_color_owns_only_its_node() {
        let plan = palette_plan(json!({
            "sankey": { "nodeColors": { "A": "#fedcba" } }
        }));

        assert_eq!(plan.fill_for(0, "A"), Some("#fedcba"));
        assert_eq!(plan.fill_for(1, "B"), Some("#abcdef"));
    }

    #[test]
    fn terminal_receipt_requires_every_node_in_order_with_exact_fill() {
        let plan = palette_plan(json!({}));
        let mut receipt = plan
            .begin_terminal_receipt()
            .expect("direct Sankey palette receipt");
        receipt.record_node(&plan, 0, "A", 1, Some("#123456"));
        receipt.record_node(&plan, 1, "B", 2, Some("#abcdef"));

        assert!(plan.record_terminal(receipt));
        let evidence = plan.finish_evidence();
        assert_eq!(evidence.applied().len(), 1);
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn incomplete_or_mismatched_terminal_receipt_fails_closed() {
        for record_second in [false, true] {
            let plan = palette_plan(json!({}));
            let mut receipt = plan
                .begin_terminal_receipt()
                .expect("direct Sankey palette receipt");
            receipt.record_node(&plan, 0, "A", 1, Some("#123456"));
            if record_second {
                receipt.record_node(&plan, 1, "B", 2, Some("#badbad"));
            }

            assert!(!plan.record_terminal(receipt));
            let evidence = plan.finish_evidence();
            assert!(evidence.applied().is_empty());
            assert_eq!(evidence.residuals().len(), 1);
        }
    }
}
