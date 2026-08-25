use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use indexmap::IndexMap;
use merman_core::diagrams::block::BlockClassDefRenderModel;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    ResolvedThemeStyle, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectPaintTerminalLedger, DirectStaticSelectorDomain,
    FamilyThemeEvidence, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::BlockDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::BlockShapeBoundary;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NodeExpectation {
    fill: Option<DirectPaintExpectation>,
    stroke: Option<DirectPaintExpectation>,
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

/// Block node shell paint resolved once and shared by the SVG writer and terminal evidence.
#[derive(Debug)]
pub(crate) struct BlockNodePaintThemePlan {
    node_indices: BTreeMap<String, usize>,
    expectations: Arc<[NodeExpectation]>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeMap<ResolvedStyleProperty, ThemeCapability>>,
    terminal_receipt: OnceLock<BlockNodePaintThemeReceipt>,
}

impl BlockNodePaintThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        layout: &BlockDiagramLayout,
        source_owned_fill: &[bool],
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let (node_indices, mut expectations) = terminal_domain(layout);
        let Some(theme) = theme else {
            return Ok(Self::baseline_from_domain(node_indices, expectations));
        };

        let node_count = expectations.len();
        let mermaid_owns_fill = mermaid_owns_node_fill(effective_config);
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

        if has_ordinal_node_rules {
            for (node_index, expectation) in expectations.iter_mut().enumerate() {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Default,
                    Some(node_index + 1),
                    work_meter,
                )?;
                observe_node_style(
                    theme,
                    &style,
                    mermaid_owns_fill,
                    mermaid_owns_stroke,
                    expectation,
                    &mut winner_properties,
                    &mut expected_capabilities,
                );
            }
        } else if let Some(style) = static_style.as_ref() {
            let mut static_expectation = NodeExpectation::default();
            observe_node_style(
                theme,
                style,
                mermaid_owns_fill,
                mermaid_owns_stroke,
                &mut static_expectation,
                &mut winner_properties,
                &mut expected_capabilities,
            );
            for expectation in &mut expectations {
                expectation.fill = static_expectation.fill.clone();
                expectation.stroke = static_expectation.stroke.clone();
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
                    if (mermaid_owns_fill && matches!(facet, FamilyThemeRuleFacet::Fill(_)))
                        || (mermaid_owns_stroke && matches!(facet, FamilyThemeRuleFacet::Stroke(_)))
                    {
                        continue;
                    }

                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static {
                                variant: None | Some(ThemeVariant::Default),
                            },
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            )
                            | FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some(capability) =
                                expected_capabilities.get(&(rule_index, property))
                            {
                                observation.capabilities.insert(property, *capability);
                            } else {
                                observation.incomplete = true;
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
                } => {}
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Node,
                    ..
                } => {}
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Node,
                TerminalVariantDomain::uniform(node_count, ThemeVariant::Default),
            )
            .with_source_owned_fill(source_owned_fill)],
            work_meter,
        )?;

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
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            node_indices,
            expectations: expectations.into(),
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
            expectations: expectations.into(),
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
        self.typed_paint(
            node_index,
            source_owns_stroke,
            ResolvedStyleProperty::Stroke,
        )
    }

    pub(crate) fn typed_fill(
        &self,
        node_index: usize,
        source_owns_fill: bool,
    ) -> Option<(usize, &str)> {
        self.typed_paint(node_index, source_owns_fill, ResolvedStyleProperty::Fill)
    }

    fn typed_paint(
        &self,
        node_index: usize,
        source_owns: bool,
        property: ResolvedStyleProperty,
    ) -> Option<(usize, &str)> {
        (!source_owns)
            .then(|| self.expectations.get(node_index)?.paint(property))
            .flatten()
            .map(|expected| (expected.rule_index(), expected.css()))
    }

    pub(crate) fn begin_terminal_receipt(&self) -> BlockNodePaintThemeReceipt {
        BlockNodePaintThemeReceipt::new(Arc::clone(&self.expectations))
    }

    pub(crate) fn record_terminal(&self, receipt: BlockNodePaintThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        for (key, properties) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            let capabilities = properties
                .iter()
                .filter(|(property, _)| receipt.proves_property(rule_index, **property))
                .map(|(_, capability)| *capability)
                .collect::<BTreeSet<_>>();
            if !capabilities.is_empty() {
                evidence.mark_applied_with_capabilities(key.clone(), capabilities);
            } else if receipt.proves_complete() && !receipt.has_effective_rule(rule_index) {
                evidence.mark_not_applicable(key.clone());
            }
        }
        evidence
    }
}

impl NodeExpectation {
    fn paint(&self, property: ResolvedStyleProperty) -> Option<&DirectPaintExpectation> {
        match property {
            ResolvedStyleProperty::Fill => self.fill.as_ref(),
            ResolvedStyleProperty::Stroke => self.stroke.as_ref(),
            _ => None,
        }
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
            fill: None,
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
    mermaid_owns_fill: bool,
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
    if let Some(expected) = typed_fill_expectation(theme, style, mermaid_owns_fill) {
        expected_capabilities.insert(
            (expected.rule_index(), ResolvedStyleProperty::Fill),
            expected.capability(),
        );
        expectation.fill = Some(expected);
    }
    if let Some(expected) = typed_stroke_expectation(theme, style, mermaid_owns_stroke) {
        expected_capabilities.insert(
            (expected.rule_index(), ResolvedStyleProperty::Stroke),
            expected.capability(),
        );
        expectation.stroke = Some(expected);
    }
}

fn typed_stroke_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_stroke: bool,
) -> Option<DirectPaintExpectation> {
    if mermaid_owns_stroke {
        return None;
    }
    resolve_direct_static_stroke(
        theme,
        style,
        &[ThemeTarget::Node],
        DirectStaticSelectorDomain::Default,
    )
    .map(DirectPaintExpectation::from_paint)
}

fn typed_fill_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_fill: bool,
) -> Option<DirectPaintExpectation> {
    if mermaid_owns_fill {
        return None;
    }
    resolve_direct_static_fill(
        theme,
        style,
        &[ThemeTarget::Node],
        DirectStaticSelectorDomain::Default,
    )
    .map(DirectPaintExpectation::from_paint)
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

fn mermaid_owns_node_fill(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.mainBkg")
}

#[derive(Debug, Default)]
struct NodeRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeMap<ResolvedStyleProperty, ThemeCapability>,
}

/// Precomputed Mermaid source-style owners for Block node shell paint.
pub(crate) struct BlockNodePaintSourceOwnership {
    fill_classes: BTreeSet<String>,
    stroke_classes: BTreeSet<String>,
}

impl BlockNodePaintSourceOwnership {
    pub(crate) fn new(class_defs: &IndexMap<String, BlockClassDefRenderModel>) -> Self {
        let (fill_classes, stroke_classes) = classes_owning_node_paint(class_defs);
        Self {
            fill_classes,
            stroke_classes,
        }
    }

    pub(crate) fn owns_fill(&self, inline_owns_fill: bool, assigned_classes: &[String]) -> bool {
        source_owns_property(inline_owns_fill, assigned_classes, &self.fill_classes)
    }

    pub(crate) fn owns_stroke(
        &self,
        inline_owns_stroke: bool,
        assigned_classes: &[String],
    ) -> bool {
        source_owns_property(inline_owns_stroke, assigned_classes, &self.stroke_classes)
    }
}

fn classes_owning_node_paint(
    class_defs: &IndexMap<String, BlockClassDefRenderModel>,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut fill_classes = BTreeSet::new();
    let mut stroke_classes = BTreeSet::new();
    for class_def in class_defs.values() {
        for declaration in &class_def.styles {
            let Some(parsed) = crate::mermaid_style::parse_style_declaration(declaration) else {
                continue;
            };
            match parsed.property() {
                "fill" => {
                    fill_classes.insert(class_def.id.clone());
                }
                "stroke" => {
                    stroke_classes.insert(class_def.id.clone());
                }
                _ => {}
            }
        }
    }
    (fill_classes, stroke_classes)
}

fn source_owns_property(
    inline_owns_property: bool,
    assigned_classes: &[String],
    owning_classes: &BTreeSet<String>,
) -> bool {
    inline_owns_property
        || owning_classes.contains("default")
        || assigned_classes
            .iter()
            .any(|assigned| owning_classes.contains(assigned))
}

impl BlockNodePaintSourceOwnership {
    /// Computes source-owned fill for the same visible node order used by the Block writer.
    pub(crate) fn source_owned_fill_mask(
        &self,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        mermaid_owns_fill: bool,
    ) -> Vec<bool> {
        let mut sources = BTreeMap::<String, BlockNodeSourceProperties>::new();
        for node in &model.blocks_flat {
            let source = sources.entry(node.id.clone()).or_default();
            if !node.classes.is_empty() {
                source.classes = node.classes.clone();
            }
            if !node.styles.is_empty() {
                source.inline_owns_fill = node.styles.iter().any(|raw| {
                    crate::mermaid_style::parse_style_declaration(raw)
                        .is_some_and(|declaration| declaration.property() == "fill")
                });
            }
        }

        layout
            .nodes
            .iter()
            .map(|node| {
                let Some(source) = sources.get(&node.id) else {
                    return mermaid_owns_fill;
                };
                mermaid_owns_fill || self.owns_fill(source.inline_owns_fill, &source.classes)
            })
            .collect()
    }
}

#[derive(Default)]
struct BlockNodeSourceProperties {
    classes: Vec<String>,
    inline_owns_fill: bool,
}

/// Writer-owned proof that every semantic Block node reached every canonical SVG shell.
#[derive(Debug, Clone)]
pub(crate) struct BlockNodePaintThemeReceipt {
    expectations: Arc<[NodeExpectation]>,
    checkpointed_nodes: Vec<bool>,
    attributes_match: bool,
    paint_ledger: DirectPaintTerminalLedger,
}

impl BlockNodePaintThemeReceipt {
    fn new(expectations: impl Into<Arc<[NodeExpectation]>>) -> Self {
        let expectations = expectations.into();
        Self {
            checkpointed_nodes: vec![false; expectations.len()],
            expectations,
            attributes_match: true,
            paint_ledger: DirectPaintTerminalLedger::default(),
        }
    }

    pub(crate) fn record_checkpointed_node<'a>(
        &mut self,
        node_index: usize,
        source_owns_fill: bool,
        emitted_fill: Option<(usize, &str)>,
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

        let Some(expectation) = self.expectations.get(node_index) else {
            self.attributes_match = false;
            return;
        };
        let mut actual_shells = shells.into_iter();
        let mut shell_match = true;
        for expected_kind in expectation.shells.iter().copied() {
            let Some((actual_kind, style)) = actual_shells.next() else {
                shell_match = false;
                break;
            };
            shell_match &= expected_kind == actual_kind;
            let (actual_fill, actual_stroke) = terminal_paints(style);
            if let Some(expected) = expectation.fill.as_ref()
                && !source_owns_fill
            {
                shell_match &= actual_fill == Some(expected.css());
            }
            if let Some(expected) = expectation.stroke.as_ref()
                && !source_owns_stroke
            {
                shell_match &= actual_stroke == Some(expected.css());
            }
        }
        shell_match &= actual_shells.next().is_none();
        self.attributes_match &= shell_match;
        self.attributes_match &= self.paint_ledger.record(
            expectation.fill.as_ref(),
            source_owns_fill,
            emitted_fill,
            ResolvedStyleProperty::Fill,
        );
        self.attributes_match &= self.paint_ledger.record(
            expectation.stroke.as_ref(),
            source_owns_stroke,
            emitted_stroke,
            ResolvedStyleProperty::Stroke,
        );
    }

    fn proves_complete(&self) -> bool {
        self.attributes_match && self.checkpointed_nodes.iter().all(|entry| *entry)
    }

    fn has_effective_rule(&self, rule_index: usize) -> bool {
        self.paint_ledger.has_effective_rule(rule_index)
    }

    #[cfg(test)]
    fn proves_rule(&self, rule_index: usize) -> bool {
        self.paint_ledger
            .proves_rule(self.proves_complete(), rule_index)
    }

    fn proves_property(&self, rule_index: usize, property: ResolvedStyleProperty) -> bool {
        self.paint_ledger
            .proves_property(self.proves_complete(), rule_index, property)
    }
}

fn terminal_paints(style: &str) -> (Option<&str>, Option<&str>) {
    let mut fill = None;
    let mut stroke = None;
    for declaration in style
        .split(';')
        .filter_map(crate::mermaid_style::parse_style_declaration)
    {
        match declaration.property() {
            "fill" => fill = Some(declaration.value()),
            "stroke" => stroke = Some(declaration.value()),
            _ => {}
        }
    }
    (fill, stroke)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint(rule_index: usize, css: &str) -> DirectPaintExpectation {
        DirectPaintExpectation::new(
            rule_index,
            css,
            if css == "transparent" {
                ThemeCapability::TransparentPaint
            } else {
                ThemeCapability::SolidPaint
            },
        )
    }

    fn paint_expectation(shells: &[BlockNodeShellKind]) -> NodeExpectation {
        NodeExpectation {
            fill: Some(paint(3, "#654321")),
            stroke: Some(paint(3, "#123456")),
            shells: shells.into(),
        }
    }

    #[test]
    fn receipt_requires_every_real_shell_with_the_final_paints() {
        let mut rect =
            BlockNodePaintThemeReceipt::new(vec![paint_expectation(&[BlockNodeShellKind::Rect])]);
        rect.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Rect, "fill:#654321;stroke:#123456;")],
        );
        assert!(rect.proves_complete());
        assert!(rect.proves_rule(3));
        assert!(rect.proves_property(3, ResolvedStyleProperty::Fill));
        assert!(rect.proves_property(3, ResolvedStyleProperty::Stroke));

        let mut double_circle = BlockNodePaintThemeReceipt::new(vec![paint_expectation(&[
            BlockNodeShellKind::Circle,
            BlockNodeShellKind::Circle,
        ])]);
        double_circle.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
            ],
        );
        assert!(double_circle.proves_complete());
        assert!(double_circle.proves_rule(3));
    }

    #[test]
    fn receipt_rejects_missing_wrong_duplicate_and_unwritten_shells() {
        let expected = vec![paint_expectation(&[
            BlockNodeShellKind::Circle,
            BlockNodeShellKind::Circle,
        ])];

        let mut missing = BlockNodePaintThemeReceipt::new(expected.clone());
        missing.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;")],
        );
        assert!(!missing.proves_complete());

        let mut wrong = BlockNodePaintThemeReceipt::new(expected.clone());
        wrong.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Path, "fill:#654321;stroke:#123456;"),
                (BlockNodeShellKind::Circle, "fill:#abcdef;stroke:#123456;"),
            ],
        );
        assert!(!wrong.proves_complete());

        let mut duplicate = BlockNodePaintThemeReceipt::new(expected);
        duplicate.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
            ],
        );
        duplicate.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
            ],
        );
        assert!(!duplicate.proves_complete());

        let unwritten =
            BlockNodePaintThemeReceipt::new(vec![paint_expectation(&[BlockNodeShellKind::Rect])]);
        assert!(!unwritten.proves_complete());
    }

    #[test]
    fn receipt_rejects_missing_or_wrong_fill_emission_even_when_terminal_style_matches() {
        let expectation = vec![paint_expectation(&[BlockNodeShellKind::Polygon])];
        let mut missing = BlockNodePaintThemeReceipt::new(expectation.clone());
        missing.record_checkpointed_node(
            0,
            false,
            None,
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Polygon, "fill:#654321;stroke:#123456;")],
        );
        assert!(!missing.proves_complete());

        let mut wrong = BlockNodePaintThemeReceipt::new(expectation);
        wrong.record_checkpointed_node(
            0,
            false,
            Some((3, "#abcdef")),
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Polygon, "fill:#654321;stroke:#123456;")],
        );
        assert!(!wrong.proves_complete());
    }

    #[test]
    fn source_owned_fill_is_independent_from_effective_typed_stroke() {
        let mut receipt = BlockNodePaintThemeReceipt::new(vec![paint_expectation(&[
            BlockNodeShellKind::Polygon,
        ])]);
        receipt.record_checkpointed_node(
            0,
            true,
            None,
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Polygon, "fill:#abcdef;stroke:#123456;")],
        );

        assert!(receipt.proves_complete());
        assert!(receipt.has_effective_rule(3));
        assert!(receipt.proves_rule(3));
        assert!(!receipt.proves_property(3, ResolvedStyleProperty::Fill));
        assert!(receipt.proves_property(3, ResolvedStyleProperty::Stroke));
    }

    #[test]
    fn source_owned_stroke_is_independent_from_effective_typed_fill() {
        let mut receipt =
            BlockNodePaintThemeReceipt::new(vec![paint_expectation(&[BlockNodeShellKind::Rect])]);
        receipt.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            true,
            None,
            [(BlockNodeShellKind::Rect, "fill:#654321;stroke:#abcdef;")],
        );

        assert!(receipt.proves_complete());
        assert!(receipt.has_effective_rule(3));
        assert!(receipt.proves_rule(3));
        assert!(receipt.proves_property(3, ResolvedStyleProperty::Fill));
        assert!(!receipt.proves_property(3, ResolvedStyleProperty::Stroke));
    }

    #[test]
    fn source_ownership_tracks_fill_and_stroke_independently() {
        let mut class_defs = IndexMap::new();
        let mut accent = BlockClassDefRenderModel::default();
        accent.id = "accent".to_string();
        accent.styles = vec!["fill:#abcdef".to_string()];
        class_defs.insert("accent".to_string(), accent);

        let ownership = BlockNodePaintSourceOwnership::new(&class_defs);
        assert!(ownership.owns_fill(true, &[]));
        assert!(ownership.owns_stroke(true, &[]));
        assert!(ownership.owns_fill(false, &["accent".to_string()]));
        assert!(!ownership.owns_stroke(false, &["accent".to_string()]));
        assert!(!ownership.owns_fill(false, &["other".to_string()]));

        let mut default_class = BlockClassDefRenderModel::default();
        default_class.id = "default".to_string();
        default_class.styles = vec!["FILL:#fedcba".to_string(), "STROKE:#abcdef".to_string()];
        class_defs.insert("default".to_string(), default_class);
        let ownership = BlockNodePaintSourceOwnership::new(&class_defs);
        assert!(ownership.owns_fill(false, &[]));
        assert!(ownership.owns_stroke(false, &[]));
        assert!(ownership.owns_fill(false, &["other".to_string()]));
        assert!(ownership.owns_stroke(false, &["other".to_string()]));
    }
}
