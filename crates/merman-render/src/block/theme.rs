use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use indexmap::IndexMap;
use merman_core::diagrams::block::BlockClassDefRenderModel;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    ResolvedThemeStyle, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::BlockDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::BlockShapeBoundary;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedStroke {
    rule_index: usize,
    css: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NodeExpectation {
    stroke: Option<ExpectedStroke>,
    shells: Box<[BlockNodeShellKind]>,
}

/// Concrete SVG shell owned by one semantic Block node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockNodeShellKind {
    Rect,
    Circle,
    Path,
    Polygon,
}

/// Block node stroke resolved once and shared by the SVG writer and terminal evidence.
#[derive(Debug)]
pub(crate) struct BlockNodeStrokeThemePlan {
    node_indices: BTreeMap<String, usize>,
    expectations: Vec<NodeExpectation>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    terminal_receipt: OnceLock<BlockNodeStrokeThemeReceipt>,
}

impl BlockNodeStrokeThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        layout: &BlockDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let (node_indices, mut expectations) = terminal_domain(layout);
        let Some(theme) = theme else {
            return Ok(Self::baseline_from_domain(node_indices, expectations));
        };

        let node_count = expectations.len();
        let mermaid_owns_stroke = mermaid_owns_node_stroke(effective_config);
        let has_ordinal_node_rules = theme.family_rules().any(|(_, rule)| {
            rule.target() == ThemeTarget::Node
                && matches!(rule.variant(), None | Some(ThemeVariant::Default))
                && rule.ordinal().is_some()
        });
        let static_style = if node_count == 0 || has_ordinal_node_rules {
            None
        } else {
            Some(theme.style_with_work_meter(
                ThemeTarget::Node,
                ThemeVariant::Default,
                None,
                work_meter,
            )?)
        };
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut expected_capabilities =
            BTreeMap::<(usize, ResolvedStyleProperty), ThemeCapability>::new();

        for (node_index, expectation) in expectations.iter_mut().enumerate() {
            if has_ordinal_node_rules {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Default,
                    Some(node_index + 1),
                    work_meter,
                )?;
                observe_node_style(
                    theme,
                    &style,
                    mermaid_owns_stroke,
                    expectation,
                    &mut winner_properties,
                    &mut expected_capabilities,
                );
            } else if let Some(style) = static_style.as_ref() {
                observe_node_style(
                    theme,
                    style,
                    mermaid_owns_stroke,
                    expectation,
                    &mut winner_properties,
                    &mut expected_capabilities,
                );
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, NodeRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Node,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector_matches_any_node(selector, node_count) {
                        continue;
                    }
                    let property = resolved_style_property_for_facet(facet);
                    if !winner_properties.contains(&(rule_index, property)) {
                        continue;
                    }

                    observation.applicable = true;
                    if mermaid_owns_stroke && matches!(facet, FamilyThemeRuleFacet::Stroke(_)) {
                        observation.suppressed = true;
                        continue;
                    }

                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some(capability) = expected_capabilities
                                .get(&(rule_index, ResolvedStyleProperty::Stroke))
                            {
                                observation.capabilities.insert(*capability);
                            } else {
                                observation.suppressed = true;
                            }
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Node,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if node_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Node,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if node_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() != FamilyThemeDisposition::LegacyCompatibility {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let mut pending = BTreeMap::new();
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Node,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A mixed rule remains fail-closed until every winning facet has a terminal owner.
            } else if !observation.capabilities.is_empty() {
                pending.insert(key, observation.capabilities);
            } else if observation.suppressed {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            node_indices,
            expectations,
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(layout: &BlockDiagramLayout) -> Self {
        let (node_indices, expectations) = terminal_domain(layout);
        Self::baseline_from_domain(node_indices, expectations)
    }

    fn baseline_from_domain(
        node_indices: BTreeMap<String, usize>,
        expectations: Vec<NodeExpectation>,
    ) -> Self {
        Self {
            node_indices,
            expectations,
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn index_for_node_id(&self, node_id: &str) -> Option<usize> {
        self.node_indices.get(node_id).copied()
    }

    pub(crate) fn typed_stroke(
        &self,
        node_index: usize,
        source_owns_stroke: bool,
    ) -> Option<(usize, &str)> {
        (!source_owns_stroke)
            .then(|| self.expectations.get(node_index)?.stroke.as_ref())
            .flatten()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn begin_terminal_receipt(&self) -> BlockNodeStrokeThemeReceipt {
        BlockNodeStrokeThemeReceipt::new(self.expectations.clone())
    }

    pub(crate) fn record_terminal(&self, receipt: BlockNodeStrokeThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        for (key, capabilities) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            if receipt.proves_rule(rule_index) {
                evidence.mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
            } else if receipt.proves_complete() && !receipt.has_effective_rule(rule_index) {
                evidence.mark_not_applicable(key.clone());
            }
        }
        evidence
    }
}

fn terminal_domain(layout: &BlockDiagramLayout) -> (BTreeMap<String, usize>, Vec<NodeExpectation>) {
    let geometries = layout
        .shape_geometries
        .iter()
        .map(|geometry| (geometry.id.as_str(), &geometry.boundary))
        .collect::<BTreeMap<_, _>>();
    let node_indices = layout
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.clone(), index))
        .collect();
    let expectations = layout
        .nodes
        .iter()
        .map(|node| NodeExpectation {
            stroke: None,
            shells: geometries
                .get(node.id.as_str())
                .map(|boundary| expected_node_shell_kinds(boundary).into())
                .unwrap_or_default(),
        })
        .collect();
    (node_indices, expectations)
}

fn expected_node_shell_kinds(boundary: &BlockShapeBoundary) -> &'static [BlockNodeShellKind] {
    match boundary {
        BlockShapeBoundary::Rectangle { .. } | BlockShapeBoundary::Stadium { .. } => {
            &[BlockNodeShellKind::Rect]
        }
        BlockShapeBoundary::Circle { .. } => &[BlockNodeShellKind::Circle],
        BlockShapeBoundary::DoubleCircle { .. } => {
            &[BlockNodeShellKind::Circle, BlockNodeShellKind::Circle]
        }
        BlockShapeBoundary::Cylinder { .. } => &[BlockNodeShellKind::Path],
        BlockShapeBoundary::Polygon { .. } => &[BlockNodeShellKind::Polygon],
    }
}

fn observe_node_style(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_stroke: bool,
    expectation: &mut NodeExpectation,
    winner_properties: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
    expected_capabilities: &mut BTreeMap<(usize, ResolvedStyleProperty), ThemeCapability>,
) {
    winner_properties.extend(
        style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (origin.rule_index(), property)),
    );
    if let Some(expected) = typed_stroke_expectation(theme, style, mermaid_owns_stroke) {
        expected_capabilities.insert(
            (expected.rule_index, ResolvedStyleProperty::Stroke),
            paint_capability_from_css(&expected.css),
        );
        expectation.stroke = Some(expected);
    }
}

fn typed_stroke_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_stroke: bool,
) -> Option<ExpectedStroke> {
    if mermaid_owns_stroke {
        return None;
    }
    let origin = style.stroke_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?;
    if !has_direct_static_stroke_route(theme, origin.rule_index(), facet) {
        return None;
    }
    let css = match style.stroke_resolution().specified() {
        Specified::Value(crate::diagram_theme::CanvasPaint::Transparent) => "transparent".into(),
        Specified::Value(crate::diagram_theme::CanvasPaint::Solid(color)) => color.as_css(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(crate::diagram_theme::CanvasPaint::LinearGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::RadialGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::Pattern(_)) => return None,
    };
    Some(ExpectedStroke {
        rule_index: origin.rule_index(),
        css,
    })
}

fn has_direct_static_stroke_route(
    theme: &ResolvedDiagramTheme,
    expected_rule_index: usize,
    expected_facet: FamilyThemeRuleFacet,
) -> bool {
    theme
        .family_mechanism_routes()
        .iter()
        .copied()
        .any(|route| {
            route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        rule_index,
                        target: ThemeTarget::Node,
                        selector: FamilyThemeSelectorShape::Static { variant: None },
                        facet,
                    } if rule_index == expected_rule_index && facet == expected_facet
                )
        })
}

fn selector_matches_any_node(selector: FamilyThemeSelectorShape, node_count: usize) -> bool {
    match selector {
        FamilyThemeSelectorShape::Static {
            variant: None | Some(ThemeVariant::Default),
        } => node_count != 0,
        FamilyThemeSelectorShape::Ordinal {
            variant: None | Some(ThemeVariant::Default),
            ..
        } => selector.ordinal_domain_intersects_occurrence_count(node_count),
        FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => false,
    }
}

fn mermaid_owns_node_stroke(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.nodeBorder")
}

fn paint_capability_from_css(css: &str) -> ThemeCapability {
    if css == "transparent" {
        ThemeCapability::TransparentPaint
    } else {
        ThemeCapability::SolidPaint
    }
}

#[derive(Debug, Default)]
struct NodeRuleObservation {
    applicable: bool,
    incomplete: bool,
    suppressed: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

/// Precomputed Mermaid source-style owners for Block node stroke.
pub(crate) struct BlockNodeStrokeSourceOwnership {
    stroke_classes: BTreeSet<String>,
}

impl BlockNodeStrokeSourceOwnership {
    pub(crate) fn new(class_defs: &IndexMap<String, BlockClassDefRenderModel>) -> Self {
        let stroke_classes = class_defs
            .values()
            .filter(|class_def| {
                class_def.styles.iter().any(|declaration| {
                    crate::mermaid_style::parse_style_declaration(declaration)
                        .is_some_and(|parsed| parsed.property() == "stroke")
                })
            })
            .map(|class_def| class_def.id.clone())
            .collect();
        Self { stroke_classes }
    }

    pub(crate) fn owns(&self, inline_styles: &[String], assigned_classes: &[String]) -> bool {
        inline_styles.iter().any(|declaration| {
            crate::mermaid_style::parse_safe_style_decl(declaration)
                .is_some_and(|(property, _)| property == "stroke")
        }) || self.stroke_classes.contains("default")
            || assigned_classes
                .iter()
                .any(|assigned| self.stroke_classes.contains(assigned))
    }
}

/// Writer-owned proof that every semantic Block node reached every canonical SVG shell.
#[derive(Debug, Clone)]
pub(crate) struct BlockNodeStrokeThemeReceipt {
    expectations: Vec<NodeExpectation>,
    checkpointed_nodes: Vec<bool>,
    attributes_match: bool,
    effective_by_rule: BTreeMap<usize, usize>,
    emitted_by_rule: BTreeMap<usize, usize>,
}

impl BlockNodeStrokeThemeReceipt {
    fn new(expectations: Vec<NodeExpectation>) -> Self {
        Self {
            checkpointed_nodes: vec![false; expectations.len()],
            expectations,
            attributes_match: true,
            effective_by_rule: BTreeMap::new(),
            emitted_by_rule: BTreeMap::new(),
        }
    }

    pub(crate) fn record_checkpointed_node<'a>(
        &mut self,
        node_index: usize,
        source_owns_stroke: bool,
        emitted_stroke: Option<(usize, &str)>,
        shells: impl IntoIterator<Item = (BlockNodeShellKind, &'a str)>,
    ) {
        let Some(checkpointed) = self.checkpointed_nodes.get_mut(node_index) else {
            self.attributes_match = false;
            return;
        };
        if *checkpointed {
            self.attributes_match = false;
            return;
        }
        *checkpointed = true;

        let expectation = self
            .expectations
            .get(node_index)
            .cloned()
            .unwrap_or_default();
        let mut actual_shells = shells.into_iter();
        let mut shell_match = true;
        for expected_kind in expectation.shells.iter().copied() {
            let Some((actual_kind, style)) = actual_shells.next() else {
                shell_match = false;
                break;
            };
            shell_match &= expected_kind == actual_kind;
            if let Some(expected) = expectation.stroke.as_ref()
                && !source_owns_stroke
            {
                shell_match &= terminal_stroke(style) == Some(expected.css.as_str());
            }
        }
        shell_match &= actual_shells.next().is_none();
        self.attributes_match &= shell_match;
        self.attributes_match &= record_stroke_checkpoint(
            expectation.stroke.as_ref(),
            source_owns_stroke,
            emitted_stroke,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
    }

    fn proves_complete(&self) -> bool {
        self.attributes_match && self.checkpointed_nodes.iter().all(|entry| *entry)
    }

    fn has_effective_rule(&self, rule_index: usize) -> bool {
        self.effective_by_rule
            .get(&rule_index)
            .copied()
            .unwrap_or(0)
            != 0
    }

    fn proves_rule(&self, rule_index: usize) -> bool {
        self.proves_complete() && self.emitted_by_rule.get(&rule_index).copied().unwrap_or(0) != 0
    }
}

fn terminal_stroke(style: &str) -> Option<&str> {
    style
        .split(';')
        .filter_map(crate::mermaid_style::parse_style_declaration)
        .filter(|declaration| declaration.property() == "stroke")
        .map(|declaration| declaration.value())
        .next_back()
}

fn record_stroke_checkpoint(
    expected: Option<&ExpectedStroke>,
    source_owns_stroke: bool,
    emitted_stroke: Option<(usize, &str)>,
    effective_by_rule: &mut BTreeMap<usize, usize>,
    emitted_by_rule: &mut BTreeMap<usize, usize>,
) -> bool {
    if source_owns_stroke {
        return emitted_stroke.is_none();
    }
    match (expected, emitted_stroke) {
        (None, None) => true,
        (Some(expected), Some((rule_index, css))) => {
            *effective_by_rule.entry(expected.rule_index).or_default() += 1;
            let matches = expected.rule_index == rule_index && expected.css == css;
            if matches {
                *emitted_by_rule.entry(rule_index).or_default() += 1;
            }
            matches
        }
        (Some(expected), None) => {
            *effective_by_rule.entry(expected.rule_index).or_default() += 1;
            false
        }
        (None, Some(_)) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expectation(shells: &[BlockNodeShellKind]) -> NodeExpectation {
        NodeExpectation {
            stroke: Some(ExpectedStroke {
                rule_index: 3,
                css: "#123456".to_string(),
            }),
            shells: shells.into(),
        }
    }

    #[test]
    fn receipt_requires_every_real_shell_with_the_final_stroke() {
        let mut rect =
            BlockNodeStrokeThemeReceipt::new(vec![expectation(&[BlockNodeShellKind::Rect])]);
        rect.record_checkpointed_node(
            0,
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Rect, "fill:#fff;stroke:#123456;")],
        );
        assert!(rect.proves_complete());
        assert!(rect.proves_rule(3));

        let mut double_circle = BlockNodeStrokeThemeReceipt::new(vec![expectation(&[
            BlockNodeShellKind::Circle,
            BlockNodeShellKind::Circle,
        ])]);
        double_circle.record_checkpointed_node(
            0,
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Circle, "stroke:#123456;"),
                (BlockNodeShellKind::Circle, "stroke:#123456;"),
            ],
        );
        assert!(double_circle.proves_complete());
        assert!(double_circle.proves_rule(3));
    }

    #[test]
    fn receipt_rejects_missing_wrong_duplicate_and_unwritten_shells() {
        let expected = vec![expectation(&[
            BlockNodeShellKind::Circle,
            BlockNodeShellKind::Circle,
        ])];

        let mut missing = BlockNodeStrokeThemeReceipt::new(expected.clone());
        missing.record_checkpointed_node(
            0,
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Circle, "stroke:#123456;")],
        );
        assert!(!missing.proves_complete());

        let mut wrong = BlockNodeStrokeThemeReceipt::new(expected.clone());
        wrong.record_checkpointed_node(
            0,
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Path, "stroke:#123456;"),
                (BlockNodeShellKind::Circle, "stroke:#abcdef;"),
            ],
        );
        assert!(!wrong.proves_complete());

        let mut duplicate = BlockNodeStrokeThemeReceipt::new(expected);
        duplicate.record_checkpointed_node(
            0,
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Circle, "stroke:#123456;"),
                (BlockNodeShellKind::Circle, "stroke:#123456;"),
            ],
        );
        duplicate.record_checkpointed_node(
            0,
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Circle, "stroke:#123456;"),
                (BlockNodeShellKind::Circle, "stroke:#123456;"),
            ],
        );
        assert!(!duplicate.proves_complete());
    }

    #[test]
    fn source_owned_stroke_is_complete_but_not_effective_typed_work() {
        let mut receipt =
            BlockNodeStrokeThemeReceipt::new(vec![expectation(&[BlockNodeShellKind::Polygon])]);
        receipt.record_checkpointed_node(
            0,
            true,
            None,
            [(BlockNodeShellKind::Polygon, "stroke:#abcdef;")],
        );

        assert!(receipt.proves_complete());
        assert!(!receipt.has_effective_rule(3));
        assert!(!receipt.proves_rule(3));
    }

    #[test]
    fn source_ownership_includes_inline_assigned_and_default_class_strokes() {
        let mut class_defs = IndexMap::new();
        let mut accent = BlockClassDefRenderModel::default();
        accent.id = "accent".to_string();
        accent.styles = vec!["stroke:#abcdef".to_string()];
        class_defs.insert("accent".to_string(), accent);

        let ownership = BlockNodeStrokeSourceOwnership::new(&class_defs);
        assert!(ownership.owns(&["stroke:#112233".to_string()], &[]));
        assert!(ownership.owns(&[], &["accent".to_string()]));
        assert!(!ownership.owns(&[], &["other".to_string()]));

        let mut default_class = BlockClassDefRenderModel::default();
        default_class.id = "default".to_string();
        default_class.styles = vec!["STROKE:#fedcba".to_string()];
        class_defs.insert("default".to_string(), default_class);
        assert!(BlockNodeStrokeSourceOwnership::new(&class_defs).owns(&[], &[]));
    }
}
