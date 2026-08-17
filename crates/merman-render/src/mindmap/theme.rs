use std::collections::BTreeSet;
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanismKey, ResolvedDiagramTheme, Specified,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::resources::{OperationWorkError, OperationWorkMeter};

pub(crate) const MINDMAP_SECTION_COUNT: usize = 11;
const MINDMAP_COLOR_SCALE_PATHS: [&str; MINDMAP_SECTION_COUNT] = [
    "themeVariables.cScale1",
    "themeVariables.cScale2",
    "themeVariables.cScale3",
    "themeVariables.cScale4",
    "themeVariables.cScale5",
    "themeVariables.cScale6",
    "themeVariables.cScale7",
    "themeVariables.cScale8",
    "themeVariables.cScale9",
    "themeVariables.cScale10",
    "themeVariables.cScale11",
];

/// The Mermaid token that owns the final Mindmap node fill at the SVG writer seam.
///
/// The terminal renderer chooses this from its existing look/theme logic. Keeping that decision
/// out of the plan avoids maintaining a second Mindmap theme interpreter here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MindmapNodeFillSource {
    ColorScale,
    MainBackground,
}

/// Fixed-size ownership view for the Mermaid tokens that can win Mindmap node fill.
///
/// The renderer computes this once per document so the node loop does not repeatedly scan the
/// same configuration provenance sets.
#[derive(Debug)]
pub(crate) struct MindmapNodeFillOwnership {
    color_scale: [bool; MINDMAP_SECTION_COUNT],
    main_background: bool,
}

impl MindmapNodeFillOwnership {
    pub(crate) fn from_config(config: &MermaidConfig) -> Self {
        Self {
            color_scale: std::array::from_fn(|section| {
                merman_core::__private::config_path_overrides_typed_default(
                    config,
                    MINDMAP_COLOR_SCALE_PATHS[section],
                )
            }),
            main_background: merman_core::__private::config_path_overrides_typed_default(
                config,
                "themeVariables.mainBkg",
            ),
        }
    }

    fn owns(&self, fill_source: MindmapNodeFillSource, section: usize) -> bool {
        match fill_source {
            MindmapNodeFillSource::ColorScale => self.color_scale[section],
            MindmapNodeFillSource::MainBackground => self.main_background,
        }
    }
}

/// Converts Mermaid's zero-based branch section into the typed Node palette's one-based ordinal.
///
/// The root has no section and is intentionally outside this mapping. Mermaid assigns branch
/// sections in an eleven-slot ring, so defensive inputs above ten retain the renderer's wrap.
const fn node_palette_ordinal_for_section(section: i32) -> Option<usize> {
    if section < 0 {
        None
    } else {
        Some(section as usize % MINDMAP_SECTION_COUNT + 1)
    }
}

fn normalized_section(section: i32) -> Option<usize> {
    node_palette_ordinal_for_section(section).map(|ordinal| ordinal - 1)
}

/// Family-local projection of the direct Mindmap Node ordinal palette.
///
/// The plan stores only the bounded section-to-color mapping. The SVG writer still chooses the
/// effective Mermaid fill source for each concrete node and owns the terminal checkpoint.
#[derive(Debug)]
pub(crate) struct MindmapNodePalettePlan {
    fills_by_section: [Option<MindmapNodePaletteFill>; MINDMAP_SECTION_COUNT],
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    node_count: usize,
    terminal_receipt: OnceLock<MindmapNodePaletteReceipt>,
}

#[derive(Debug)]
struct MindmapNodePaletteFill {
    css: String,
    capability: ThemeCapability,
}

impl MindmapNodePalettePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        sections: impl IntoIterator<Item = Option<i32>>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut node_count = 0;
        let mut visited = [false; MINDMAP_SECTION_COUNT];
        for section in sections {
            node_count += 1;
            let Some(section) = section.and_then(normalized_section) else {
                continue;
            };
            visited[section] = true;
        }

        let Some(theme) = theme else {
            return Ok(Self::baseline(node_count));
        };

        let mut plan = Self::baseline(node_count);
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
                for (section, present) in visited.into_iter().enumerate() {
                    if !present {
                        continue;
                    }
                    let ordinal = section + 1;
                    let style = theme.style_with_work_meter(
                        ThemeTarget::Node,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work_meter,
                    )?;
                    if !matches!(style.fill_resolution().specified(), Specified::Unspecified) {
                        continue;
                    }
                    let Some(color) = theme.series_color(ThemeTarget::Node, ordinal) else {
                        plan.evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                        plan.palette_key = None;
                        plan.fills_by_section = std::array::from_fn(|_| None);
                        return Ok(plan);
                    };
                    plan.fills_by_section[section] = Some(MindmapNodePaletteFill {
                        css: color.as_css(),
                        capability: if color.is_transparent() {
                            ThemeCapability::TransparentPaint
                        } else {
                            ThemeCapability::SolidPaint
                        },
                    });
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

    fn baseline(node_count: usize) -> Self {
        Self {
            fills_by_section: std::array::from_fn(|_| None),
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            node_count,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn terminal_decision_for_node(
        &self,
        section: Option<i32>,
        fill_source: MindmapNodeFillSource,
        ownership: &MindmapNodeFillOwnership,
        generated_color_rule_limit: usize,
    ) -> MindmapNodePaletteTerminalDecision {
        let Some(section) = section.and_then(normalized_section) else {
            return MindmapNodePaletteTerminalDecision::NotApplicable;
        };
        let Some(fill) = self.fills_by_section[section].as_ref() else {
            return MindmapNodePaletteTerminalDecision::NotApplicable;
        };
        if section + 1 >= generated_color_rule_limit {
            return MindmapNodePaletteTerminalDecision::Unsupported;
        }
        if ownership.owns(fill_source, section) {
            return MindmapNodePaletteTerminalDecision::NotApplicable;
        }
        MindmapNodePaletteTerminalDecision::Applied {
            section,
            capability: fill.capability,
        }
    }

    pub(crate) fn css_fill_for_section(
        &self,
        section: usize,
        generated_color_rule_limit: usize,
    ) -> Option<&str> {
        if section >= MINDMAP_SECTION_COUNT || section + 1 >= generated_color_rule_limit {
            return None;
        }
        self.fills_by_section[section]
            .as_ref()
            .map(|fill| fill.css.as_str())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> MindmapNodePaletteReceipt {
        MindmapNodePaletteReceipt::new(self.node_count)
    }

    pub(crate) fn record_terminal(&self, receipt: MindmapNodePaletteReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(key) = self.palette_key.clone() else {
            return evidence;
        };
        match self.terminal_receipt.get() {
            Some(receipt) if receipt.unsupported_surface => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette)
            }
            Some(receipt) if !receipt.applied_capabilities.is_empty() => {
                evidence.mark_applied_with_capabilities(
                    key,
                    receipt.applied_capabilities.iter().copied(),
                );
            }
            Some(_) => evidence.mark_not_applicable(key),
            None => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette)
            }
        }
        evidence
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MindmapNodePaletteTerminalDecision {
    Applied {
        section: usize,
        capability: ThemeCapability,
    },
    NotApplicable,
    Unsupported,
}

/// Writer-owned proof that every semantic Mindmap node reached its canonical shape checkpoint.
#[derive(Debug)]
pub(crate) struct MindmapNodePaletteReceipt {
    expected_node_count: usize,
    checkpointed_node_count: usize,
    node_order_matches: bool,
    unsupported_surface: bool,
    applied_capabilities: BTreeSet<ThemeCapability>,
}

impl MindmapNodePaletteReceipt {
    fn new(node_count: usize) -> Self {
        Self {
            expected_node_count: node_count,
            checkpointed_node_count: 0,
            node_order_matches: true,
            unsupported_surface: false,
            applied_capabilities: BTreeSet::new(),
        }
    }

    pub(crate) fn record_checkpointed_node(
        &mut self,
        node_index: usize,
        terminal_decision: MindmapNodePaletteTerminalDecision,
    ) {
        if node_index != self.checkpointed_node_count || node_index >= self.expected_node_count {
            self.node_order_matches = false;
            return;
        }
        self.checkpointed_node_count += 1;
        match terminal_decision {
            MindmapNodePaletteTerminalDecision::Applied { capability, .. } => {
                self.applied_capabilities.insert(capability);
            }
            MindmapNodePaletteTerminalDecision::Unsupported => self.unsupported_surface = true,
            MindmapNodePaletteTerminalDecision::NotApplicable => {}
        }
    }

    fn proves_complete(&self) -> bool {
        self.node_order_matches && self.checkpointed_node_count == self.expected_node_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, OrdinalSelector,
        ThemeColorValue, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    };
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use serde_json::{Value, json};

    fn effective_config(site_config: Value) -> MermaidConfig {
        merman_core::Engine::new()
            .with_site_config(MermaidConfig::from_value(site_config))
            .parse_metadata_sync("mindmap\n  root\n    child\n")
            .expect("parse Mindmap config fixture")
            .effective_config
    }

    fn work_meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    fn resolved_node_palette(colors: &[&str], static_fill: Option<&str>) -> ResolvedDiagramTheme {
        let palette = OrdinalPalette::new(
            colors
                .iter()
                .map(|color| ThemeColorValue::parse(*color).expect("valid test color")),
        )
        .expect("non-empty test palette");
        let mut styles = ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette);
        if let Some(fill) = static_fill {
            styles = styles.with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(
                        CanvasPaint::solid(fill).expect("valid static Mindmap Node fill"),
                    ),
                )
                .for_family(DiagramFamilyId::MINDMAP),
            );
        }
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile Mindmap Node palette")
            .resolve(DiagramFamilyId::MINDMAP)
    }

    #[test]
    fn palette_plan_skips_root_and_maps_branch_sections() {
        let theme = resolved_node_palette(&["#ef4444", "#2563eb"], None);
        let config = effective_config(json!({}));
        let plan = MindmapNodePalettePlan::resolve(
            Some(&theme),
            [None, Some(-1), Some(0), Some(1), Some(10), Some(11)],
            &work_meter(),
        )
        .expect("resolve Mindmap palette plan");
        let ownership = MindmapNodeFillOwnership::from_config(&config);

        assert_eq!(
            plan.terminal_decision_for_node(
                Some(-1),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::NotApplicable
        );
        assert_eq!(
            plan.terminal_decision_for_node(
                Some(0),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::Applied {
                section: 0,
                capability: ThemeCapability::SolidPaint,
            }
        );
        assert_eq!(
            plan.terminal_decision_for_node(
                Some(1),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::Applied {
                section: 1,
                capability: ThemeCapability::SolidPaint,
            }
        );
        assert_eq!(
            plan.terminal_decision_for_node(
                Some(10),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::Applied {
                section: 10,
                capability: ThemeCapability::SolidPaint,
            }
        );
        assert_eq!(
            plan.terminal_decision_for_node(
                Some(11),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::Applied {
                section: 0,
                capability: ThemeCapability::SolidPaint,
            }
        );
        assert_eq!(
            plan.css_fill_for_section(0, MINDMAP_SECTION_COUNT + 1),
            Some("#ef4444")
        );
        assert_eq!(
            plan.css_fill_for_section(1, MINDMAP_SECTION_COUNT + 1),
            Some("#2563eb")
        );
    }

    #[test]
    fn static_node_fill_winner_blocks_palette_fallback() {
        let theme = resolved_node_palette(&["#ef4444"], Some("#111827"));
        let config = effective_config(json!({}));
        let plan = MindmapNodePalettePlan::resolve(Some(&theme), [Some(0), Some(1)], &work_meter())
            .expect("resolve Mindmap palette plan");
        let ownership = MindmapNodeFillOwnership::from_config(&config);

        assert_eq!(
            plan.terminal_decision_for_node(
                Some(0),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::NotApplicable
        );
        assert_eq!(
            plan.terminal_decision_for_node(
                Some(1),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::NotApplicable
        );
    }

    #[test]
    fn transparent_palette_color_reports_transparent_paint() {
        let theme = resolved_node_palette(&["transparent"], None);
        let config = effective_config(json!({}));
        let plan = MindmapNodePalettePlan::resolve(Some(&theme), [Some(0)], &work_meter())
            .expect("resolve Mindmap palette plan");
        let ownership = MindmapNodeFillOwnership::from_config(&config);

        assert_eq!(
            plan.terminal_decision_for_node(
                Some(0),
                MindmapNodeFillSource::ColorScale,
                &ownership,
                MINDMAP_SECTION_COUNT + 1,
            ),
            MindmapNodePaletteTerminalDecision::Applied {
                section: 0,
                capability: ThemeCapability::TransparentPaint,
            }
        );
    }

    #[test]
    fn palette_plan_has_an_exact_ordinal_work_boundary() {
        let palette = OrdinalPalette::new([ThemeColorValue::parse("#ef4444").unwrap()]).unwrap();
        let styles = ThemeRuleSet::default()
            .with_ordinal_palette(ThemeTarget::Node, palette)
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#111827").unwrap()),
                )
                .for_family(DiagramFamilyId::MINDMAP)
                .with_ordinal(OrdinalSelector::exact(99).unwrap()),
            );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile metered Mindmap palette")
            .resolve(DiagramFamilyId::MINDMAP);
        let sections = [Some(0), Some(1)];
        let unbounded = work_meter();

        MindmapNodePalettePlan::resolve(Some(&theme), sections, &unbounded)
            .expect("resolve unbounded Mindmap palette plan");
        let exact = unbounded.used();
        assert!(exact > 0);

        let exact_meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact)
                .unwrap(),
        );
        MindmapNodePalettePlan::resolve(Some(&theme), sections, &exact_meter)
            .expect("exact Mindmap ordinal work budget must succeed");
        assert_eq!(exact_meter.used(), exact);

        let short_meter = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
                .unwrap(),
        );
        assert!(MindmapNodePalettePlan::resolve(Some(&theme), sections, &short_meter).is_err());
        assert_eq!(short_meter.used(), exact - 1);
    }
}
