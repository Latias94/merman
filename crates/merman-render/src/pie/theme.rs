use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::pie::PieDiagramRenderModel;

use crate::config::config_string;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::resources::{OperationWorkError, OperationWorkMeter};
const MERMAID_PIE_PALETTE_SIZE: usize = 12;
const MERMAID_PIE_SLOT_KEYS: [&str; MERMAID_PIE_PALETTE_SIZE] = [
    "pie1", "pie2", "pie3", "pie4", "pie5", "pie6", "pie7", "pie8", "pie9", "pie10", "pie11",
    "pie12",
];
const MERMAID_PIE_SLOT_PATHS: [&str; MERMAID_PIE_PALETTE_SIZE] = [
    "themeVariables.pie1",
    "themeVariables.pie2",
    "themeVariables.pie3",
    "themeVariables.pie4",
    "themeVariables.pie5",
    "themeVariables.pie6",
    "themeVariables.pie7",
    "themeVariables.pie8",
    "themeVariables.pie9",
    "themeVariables.pie10",
    "themeVariables.pie11",
    "themeVariables.pie12",
];

#[derive(Debug)]
struct PieSlicePaint {
    fill: String,
    typed_capability: Option<ThemeCapability>,
}

/// Resolves Pie slice paint once for layout, SVG emission, and terminal evidence.
#[derive(Debug)]
pub(crate) struct PieSlicePaintPlan {
    label_indices: HashMap<String, usize>,
    section_paint_indices: Vec<usize>,
    paints: Vec<PieSlicePaint>,
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    terminal_capabilities: OnceLock<BTreeSet<ThemeCapability>>,
}

impl PieSlicePaintPlan {
    pub(crate) fn baseline(
        model: &PieDiagramRenderModel,
        effective_config: &serde_json::Value,
    ) -> Self {
        let mut plan = Self {
            label_indices: HashMap::new(),
            section_paint_indices: Vec::with_capacity(model.sections.len()),
            paints: Vec::new(),
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            terminal_capabilities: OnceLock::new(),
        };
        for section in &model.sections {
            let paint_index = plan.insert_mermaid_paint(&section.label, effective_config);
            plan.section_paint_indices.push(paint_index);
        }
        plan
    }

    pub(crate) fn resolve(
        model: &PieDiagramRenderModel,
        effective_config: &MermaidConfig,
        theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(model, effective_config.as_value());
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let Some(disposition) = theme.ordinal_palette_disposition(ThemeTarget::PieSlice) else {
            return Ok(plan);
        };
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::PieSlice,
        };
        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                plan.palette_key = Some(key.clone());
                let mut missing_typed_color = false;
                for (index, paint) in plan.paints.iter_mut().enumerate() {
                    if merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        MERMAID_PIE_SLOT_PATHS[index % MERMAID_PIE_PALETTE_SIZE],
                    ) {
                        continue;
                    }
                    let style = theme.style_with_work_meter(
                        ThemeTarget::PieSlice,
                        ThemeVariant::Default,
                        Some(index + 1),
                        work_meter,
                    )?;
                    if style.fill_resolution().winner().is_some() {
                        continue;
                    }
                    let Some(color) = theme.series_color(ThemeTarget::PieSlice, index + 1) else {
                        missing_typed_color = true;
                        continue;
                    };
                    paint.fill = color.as_css();
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
            FamilyThemeDisposition::Unsupported => {
                plan.evidence
                    .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
            }
            FamilyThemeDisposition::LegacyCompatibility => {}
        }
        Ok(plan)
    }

    fn insert_mermaid_paint(&mut self, label: &str, effective_config: &serde_json::Value) -> usize {
        if let Some(index) = self.label_indices.get(label).copied() {
            return index;
        }
        let index = self.paints.len();
        let slot = index % MERMAID_PIE_PALETTE_SIZE;
        let fill = config_string(
            effective_config,
            &["themeVariables", MERMAID_PIE_SLOT_KEYS[slot]],
        )
        .unwrap_or_else(|| default_pie_palette()[slot].to_string());
        self.label_indices.insert(label.to_string(), index);
        self.paints.push(PieSlicePaint {
            fill,
            typed_capability: None,
        });
        index
    }

    pub(crate) fn fill_for(&self, label: &str) -> Option<&str> {
        let index = *self.label_indices.get(label)?;
        self.paints.get(index).map(|paint| paint.fill.as_str())
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &self,
        visible_slice_labels: impl IntoIterator<Item = &'a str>,
    ) -> Option<PieSlicePaintReceipt> {
        self.palette_key
            .as_ref()
            .map(|_| PieSlicePaintReceipt::new(self, visible_slice_labels))
    }

    pub(crate) fn record_terminal(&self, receipt: PieSlicePaintReceipt) -> bool {
        if self.palette_key.is_none() || !receipt.proves_complete() {
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

/// Writer-owned proof that every visible Pie slice and semantic legend swatch emitted the
/// planned fill in canonical order.
#[derive(Debug)]
pub(crate) struct PieSlicePaintReceipt {
    expected_slice_paints: Vec<Option<usize>>,
    expected_legend_paints: Vec<Option<usize>>,
    next_slice_index: usize,
    next_legend_index: usize,
    values_match: bool,
    applied_capabilities: BTreeSet<ThemeCapability>,
}

impl PieSlicePaintReceipt {
    fn new<'a>(
        plan: &PieSlicePaintPlan,
        visible_slice_labels: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        let mut values_match = true;
        let expected_slice_paints = visible_slice_labels
            .into_iter()
            .map(|label| {
                let paint_index = plan.label_indices.get(label).copied();
                values_match &= paint_index.is_some();
                paint_index
            })
            .collect();
        Self {
            expected_slice_paints,
            expected_legend_paints: plan
                .section_paint_indices
                .iter()
                .copied()
                .map(Some)
                .collect(),
            next_slice_index: 0,
            next_legend_index: 0,
            values_match,
            applied_capabilities: BTreeSet::new(),
        }
    }

    pub(crate) fn record_slice(
        &mut self,
        plan: &PieSlicePaintPlan,
        slice_index: usize,
        emitted_label: &str,
        emitted_fill: Option<&str>,
    ) {
        let (matches, capability) = record_terminal_paint(
            plan,
            &self.expected_slice_paints,
            &mut self.next_slice_index,
            slice_index,
            emitted_label,
            emitted_fill,
        );
        self.values_match &= matches;
        if matches {
            self.applied_capabilities.extend(capability);
        }
    }

    pub(crate) fn record_legend(
        &mut self,
        plan: &PieSlicePaintPlan,
        legend_index: usize,
        emitted_label: &str,
        emitted_fill: Option<&str>,
    ) {
        let (matches, capability) = record_terminal_paint(
            plan,
            &self.expected_legend_paints,
            &mut self.next_legend_index,
            legend_index,
            emitted_label,
            emitted_fill,
        );
        self.values_match &= matches;
        if matches {
            self.applied_capabilities.extend(capability);
        }
    }

    fn proves_complete(&self) -> bool {
        self.values_match
            && self.next_slice_index == self.expected_slice_paints.len()
            && self.next_legend_index == self.expected_legend_paints.len()
    }
}

fn record_terminal_paint(
    plan: &PieSlicePaintPlan,
    expected_paints: &[Option<usize>],
    next_index: &mut usize,
    emitted_index: usize,
    emitted_label: &str,
    emitted_fill: Option<&str>,
) -> (bool, Option<ThemeCapability>) {
    if emitted_index != *next_index || emitted_index >= expected_paints.len() {
        return (false, None);
    }
    *next_index += 1;
    let Some(paint_index) = expected_paints[emitted_index] else {
        return (false, None);
    };
    let Some(paint) = plan.paints.get(paint_index) else {
        return (false, None);
    };
    let matches = plan.label_indices.get(emitted_label).copied() == Some(paint_index)
        && emitted_fill.is_some_and(|emitted_fill| paint.fill == emitted_fill);
    (matches, matches.then_some(paint.typed_capability).flatten())
}

fn default_pie_palette() -> [&'static str; MERMAID_PIE_PALETTE_SIZE] {
    [
        "#ECECFF",
        "#ffffde",
        "hsl(80, 100%, 56.2745098039%)",
        "hsl(240, 100%, 86.2745098039%)",
        "hsl(60, 100%, 63.5294117647%)",
        "hsl(80, 100%, 76.2745098039%)",
        "hsl(300, 100%, 76.2745098039%)",
        "hsl(180, 100%, 56.2745098039%)",
        "hsl(0, 100%, 56.2745098039%)",
        "hsl(300, 100%, 56.2745098039%)",
        "hsl(150, 100%, 56.2745098039%)",
        "hsl(0, 100%, 66.2745098039%)",
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue, ThemeRuleSet,
    };
    use crate::resources::RenderResourcePolicy;
    use merman_core::diagrams::pie::PieRenderSection;
    use serde_json::json;

    fn resolved_slice_palette(colors: &[&str]) -> ResolvedDiagramTheme {
        let palette = OrdinalPalette::new(
            colors
                .iter()
                .map(|color| ThemeColorValue::parse(*color).expect("valid Pie test color")),
        )
        .expect("non-empty Pie test palette");
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::PieSlice, palette),
            ))
            .expect("compile Pie test palette")
            .resolve(DiagramFamilyId::PIE)
    }

    fn palette_plan(colors: &[&str]) -> (PieSlicePaintPlan, Vec<String>) {
        let mut model = PieDiagramRenderModel::default();
        model.sections = colors
            .iter()
            .enumerate()
            .map(|(index, _)| PieRenderSection {
                label: format!("Slice {index}"),
                value: 1.0,
            })
            .collect();
        let config = MermaidConfig::from_value(json!({}));
        let theme = resolved_slice_palette(colors);
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let plan = PieSlicePaintPlan::resolve(&model, &config, Some(&theme), &work_meter)
            .expect("resolve Pie test palette");
        let labels = model
            .sections
            .iter()
            .map(|section| section.label.clone())
            .collect();
        (plan, labels)
    }

    fn finish_palette_evidence(colors: &[&str]) -> FamilyThemeEvidence {
        let (plan, labels) = palette_plan(colors);
        let mut receipt = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie palette receipt");
        for (index, label) in labels.iter().enumerate() {
            let fill = plan.fill_for(label).expect("planned Pie slice fill");
            receipt.record_slice(&plan, index, label, Some(fill));
            receipt.record_legend(&plan, index, label, Some(fill));
        }
        assert!(plan.record_terminal(receipt));
        plan.finish_evidence()
    }

    #[test]
    fn palette_receipt_rejects_missing_reordered_and_mismatched_terminal_paint() {
        let (plan, labels) = palette_plan(&["#ef4444", "#22c55e"]);

        let mut missing = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie palette receipt");
        for (index, label) in labels.iter().enumerate() {
            let fill = plan.fill_for(label).expect("planned Pie slice fill");
            missing.record_slice(&plan, index, label, Some(fill));
        }
        missing.record_legend(
            &plan,
            0,
            &labels[0],
            Some(plan.fill_for(&labels[0]).expect("first Pie legend fill")),
        );
        assert!(!missing.proves_complete());

        let mut reordered = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie palette receipt");
        reordered.record_slice(
            &plan,
            1,
            &labels[1],
            Some(plan.fill_for(&labels[1]).expect("second Pie slice fill")),
        );
        assert!(!reordered.proves_complete());

        let mut mismatched = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie palette receipt");
        mismatched.record_slice(&plan, 0, &labels[0], Some("#000000"));
        assert!(!mismatched.proves_complete());

        let mut missing_fill = plan
            .begin_terminal_receipt(labels.iter().map(String::as_str))
            .expect("typed Pie palette receipt");
        missing_fill.record_slice(&plan, 0, &labels[0], None);
        assert!(!missing_fill.proves_complete());
    }

    #[test]
    fn transparent_palette_reports_only_transparent_paint() {
        assert_eq!(
            finish_palette_evidence(&["transparent"]).applied_capabilities(),
            BTreeSet::from([ThemeCapability::TransparentPaint])
        );
    }

    #[test]
    fn mixed_palette_reports_transparent_and_solid_paint() {
        assert_eq!(
            finish_palette_evidence(&["transparent", "#22c55e"]).applied_capabilities(),
            BTreeSet::from([
                ThemeCapability::SolidPaint,
                ThemeCapability::TransparentPaint,
            ])
        );
    }
}
