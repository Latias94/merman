use crate::DiagramFamilyId;

use super::ThemeCompileValidationError;
use super::canvas::{CanvasPaint, InsetsPx, ThemeColorValue};
use super::typography::{Specified, TextStylePatch};

/// Upper bound for semantic rules in one compiled theme recipe.
pub(crate) const MAX_THEME_RULES: usize = 512;
/// Upper bound for ordinal palettes in one compiled theme recipe.
pub(crate) const MAX_THEME_ORDINAL_PALETTES: usize = 64;
/// Upper bound for colors in one ordinal palette.
pub(crate) const MAX_THEME_PALETTE_COLORS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ThemeTarget {
    Canvas,
    Node,
    NodeLabel,
    Edge,
    EdgeLabel,
    EdgeLabelBackground,
    Cluster,
    ClusterLabel,
    Marker,
    Title,
    Text,
    Axis,
    Legend,
    Table,
    Task,
    State,
    StateLabel,
    Transition,
    TransitionMarker,
    TransitionLabel,
    TransitionLabelBackground,
    Composite,
    CompositeHeader,
    CompositeLabel,
    SpecialState,
    SpecialStateInner,
    Actor,
    ActorLabel,
    Lifeline,
    Message,
    MessageLabel,
    Loop,
    LoopLabel,
    Note,
    NoteLabel,
    Activation,
    Requirement,
    Entity,
    Relation,
    PieSlice,
    ChartSeries,
    TimelineEvent,
    JourneyTask,
}

impl ThemeTarget {
    /// Stable enumeration view for catalogs and bindings.
    ///
    /// Keep the collection behind a slice so adding a semantic target does not expose the
    /// collection length as part of the Rust type contract.
    pub const ALL: &'static [Self] = &[
        Self::Canvas,
        Self::Node,
        Self::NodeLabel,
        Self::Edge,
        Self::EdgeLabel,
        Self::EdgeLabelBackground,
        Self::Cluster,
        Self::ClusterLabel,
        Self::Marker,
        Self::Title,
        Self::Text,
        Self::Axis,
        Self::Legend,
        Self::Table,
        Self::Task,
        Self::State,
        Self::StateLabel,
        Self::Transition,
        Self::TransitionMarker,
        Self::TransitionLabel,
        Self::TransitionLabelBackground,
        Self::Composite,
        Self::CompositeHeader,
        Self::CompositeLabel,
        Self::SpecialState,
        Self::SpecialStateInner,
        Self::Actor,
        Self::ActorLabel,
        Self::Lifeline,
        Self::Message,
        Self::MessageLabel,
        Self::Loop,
        Self::LoopLabel,
        Self::Note,
        Self::NoteLabel,
        Self::Activation,
        Self::Requirement,
        Self::Entity,
        Self::Relation,
        Self::PieSlice,
        Self::ChartSeries,
        Self::TimelineEvent,
        Self::JourneyTask,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Canvas => "canvas",
            Self::Node => "node",
            Self::NodeLabel => "node-label",
            Self::Edge => "edge",
            Self::EdgeLabel => "edge-label",
            Self::EdgeLabelBackground => "edge-label-background",
            Self::Cluster => "cluster",
            Self::ClusterLabel => "cluster-label",
            Self::Marker => "marker",
            Self::Title => "title",
            Self::Text => "text",
            Self::Axis => "axis",
            Self::Legend => "legend",
            Self::Table => "table",
            Self::Task => "task",
            Self::State => "state",
            Self::StateLabel => "state-label",
            Self::Transition => "transition",
            Self::TransitionMarker => "transition-marker",
            Self::TransitionLabel => "transition-label",
            Self::TransitionLabelBackground => "transition-label-background",
            Self::Composite => "composite",
            Self::CompositeHeader => "composite-header",
            Self::CompositeLabel => "composite-label",
            Self::SpecialState => "special-state",
            Self::SpecialStateInner => "special-state-inner",
            Self::Actor => "actor",
            Self::ActorLabel => "actor-label",
            Self::Lifeline => "lifeline",
            Self::Message => "message",
            Self::MessageLabel => "message-label",
            Self::Loop => "loop",
            Self::LoopLabel => "loop-label",
            Self::Note => "note",
            Self::NoteLabel => "note-label",
            Self::Activation => "activation",
            Self::Requirement => "requirement",
            Self::Entity => "entity",
            Self::Relation => "relation",
            Self::PieSlice => "pie-slice",
            Self::ChartSeries => "chart-series",
            Self::TimelineEvent => "timeline-event",
            Self::JourneyTask => "journey-task",
        }
    }

    /// Parses one known semantic target identifier.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|target| target.id() == id)
    }

    pub(crate) fn valid_for(self, family: DiagramFamilyId) -> bool {
        match self {
            Self::State
            | Self::StateLabel
            | Self::Transition
            | Self::TransitionMarker
            | Self::TransitionLabel
            | Self::TransitionLabelBackground
            | Self::Composite
            | Self::CompositeHeader
            | Self::CompositeLabel
            | Self::SpecialState
            | Self::SpecialStateInner => {
                matches!(family, DiagramFamilyId::STATE)
            }
            Self::Actor
            | Self::ActorLabel
            | Self::Lifeline
            | Self::Message
            | Self::MessageLabel
            | Self::Loop
            | Self::LoopLabel
            | Self::Activation => {
                matches!(family, DiagramFamilyId::SEQUENCE)
            }
            Self::Requirement => matches!(family, DiagramFamilyId::REQUIREMENT),
            Self::Entity => matches!(family, DiagramFamilyId::ER),
            Self::Relation => matches!(family, DiagramFamilyId::REQUIREMENT | DiagramFamilyId::ER),
            Self::PieSlice => matches!(family, DiagramFamilyId::PIE),
            Self::ChartSeries | Self::Axis | Self::Legend => matches!(
                family,
                DiagramFamilyId::XY_CHART
                    | DiagramFamilyId::QUADRANT_CHART
                    | DiagramFamilyId::RADAR
            ),
            Self::TimelineEvent => matches!(family, DiagramFamilyId::TIMELINE),
            Self::JourneyTask => matches!(family, DiagramFamilyId::JOURNEY),
            Self::Task => matches!(family, DiagramFamilyId::GANTT | DiagramFamilyId::KANBAN),
            Self::Table => matches!(
                family,
                DiagramFamilyId::CLASS
                    | DiagramFamilyId::ER
                    | DiagramFamilyId::REQUIREMENT
                    | DiagramFamilyId::KANBAN
            ),
            Self::Node
            | Self::NodeLabel
            | Self::Edge
            | Self::EdgeLabel
            | Self::EdgeLabelBackground
            | Self::ClusterLabel
            | Self::Marker => {
                matches!(
                    family,
                    DiagramFamilyId::FLOWCHART
                        | DiagramFamilyId::SWIMLANE
                        | DiagramFamilyId::CLASS
                        | DiagramFamilyId::MINDMAP
                        | DiagramFamilyId::TREE_VIEW
                        | DiagramFamilyId::BLOCK
                        | DiagramFamilyId::GIT_GRAPH
                )
            }
            Self::Cluster => matches!(
                family,
                DiagramFamilyId::FLOWCHART
                    | DiagramFamilyId::SWIMLANE
                    | DiagramFamilyId::ARCHITECTURE
                    | DiagramFamilyId::CLASS
                    | DiagramFamilyId::MINDMAP
                    | DiagramFamilyId::TREE_VIEW
                    | DiagramFamilyId::BLOCK
                    | DiagramFamilyId::GIT_GRAPH
            ),
            Self::Canvas | Self::Title | Self::Text => true,
            Self::Note | Self::NoteLabel => {
                matches!(family, DiagramFamilyId::SEQUENCE | DiagramFamilyId::STATE)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[non_exhaustive]
pub enum ThemeVariant {
    #[default]
    Default,
    Primary,
    Secondary,
    Tertiary,
    Active,
    Selected,
    Odd,
    Even,
    Start,
    End,
    Special,
    Error,
    Warning,
    Success,
}

impl ThemeVariant {
    pub const ALL: &'static [Self] = &[
        Self::Default,
        Self::Primary,
        Self::Secondary,
        Self::Tertiary,
        Self::Active,
        Self::Selected,
        Self::Odd,
        Self::Even,
        Self::Start,
        Self::End,
        Self::Special,
        Self::Error,
        Self::Warning,
        Self::Success,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Tertiary => "tertiary",
            Self::Active => "active",
            Self::Selected => "selected",
            Self::Odd => "odd",
            Self::Even => "even",
            Self::Start => "start",
            Self::End => "end",
            Self::Special => "special",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Success => "success",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OrdinalSelector {
    Exact(usize),
    Cycle { period: usize, offset: usize },
}

impl OrdinalSelector {
    pub fn exact(index: usize) -> Result<Self, ThemeCompileValidationError> {
        if index == 0 {
            return Err(ThemeCompileValidationError::InvalidNumber {
                field: "style.ordinal.index",
            });
        }
        Ok(Self::Exact(index))
    }

    pub fn cycle(period: usize, offset: usize) -> Result<Self, ThemeCompileValidationError> {
        if period == 0 || offset >= period {
            return Err(ThemeCompileValidationError::InvalidNumber {
                field: "style.ordinal",
            });
        }
        Ok(Self::Cycle { period, offset })
    }

    pub const fn matches(self, one_based_index: usize) -> bool {
        match self {
            Self::Exact(index) => one_based_index == index,
            Self::Cycle { period, offset } => {
                one_based_index > 0 && (one_based_index - 1) % period == offset
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemePaintPatch {
    pub fill: Specified<CanvasPaint>,
    pub opacity: Specified<f32>,
    pub fill_opacity: Specified<f32>,
}

impl Default for ThemePaintPatch {
    fn default() -> Self {
        Self {
            fill: Specified::Unspecified,
            opacity: Specified::Unspecified,
            fill_opacity: Specified::Unspecified,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeStrokePatch {
    pub paint: Specified<CanvasPaint>,
    pub width: Specified<f32>,
    pub dasharray: Specified<Vec<f32>>,
    pub linecap: Specified<StrokeLineCap>,
    pub linejoin: Specified<StrokeLineJoin>,
    pub stroke_opacity: Specified<f32>,
}

impl Default for ThemeStrokePatch {
    fn default() -> Self {
        Self {
            paint: Specified::Unspecified,
            width: Specified::Unspecified,
            dasharray: Specified::Unspecified,
            linecap: Specified::Unspecified,
            linejoin: Specified::Unspecified,
            stroke_opacity: Specified::Unspecified,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeGeometryPatch {
    pub radius: Specified<f32>,
}

impl Default for ThemeGeometryPatch {
    fn default() -> Self {
        Self {
            radius: Specified::Unspecified,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSpacingPatch {
    pub padding: Specified<InsetsPx>,
}

impl Default for ThemeSpacingPatch {
    fn default() -> Self {
        Self {
            padding: Specified::Unspecified,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeEffectPatch {
    pub effect: Specified<String>,
}

impl Default for ThemeEffectPatch {
    fn default() -> Self {
        Self {
            effect: Specified::Unspecified,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeStylePatch {
    pub paint: ThemePaintPatch,
    pub stroke: ThemeStrokePatch,
    pub geometry: ThemeGeometryPatch,
    pub spacing: ThemeSpacingPatch,
    pub typography: TextStylePatch,
    pub effects: ThemeEffectPatch,
}

impl ThemeStylePatch {
    pub fn with_fill(mut self, fill: CanvasPaint) -> Self {
        self.paint.fill = Specified::Value(fill);
        self
    }

    pub fn with_stroke(mut self, stroke: CanvasPaint) -> Self {
        self.stroke.paint = Specified::Value(stroke);
        self
    }

    pub fn with_stroke_width(mut self, value: f32) -> Result<Self, ThemeCompileValidationError> {
        validate_nonnegative(value, "style.stroke_width")?;
        self.stroke.width = Specified::Value(value);
        Ok(self)
    }

    pub fn with_stroke_dasharray(
        mut self,
        values: impl IntoIterator<Item = f32>,
    ) -> Result<Self, ThemeCompileValidationError> {
        let values = values.into_iter().collect::<Vec<_>>();
        if values.is_empty()
            || values
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(ThemeCompileValidationError::InvalidCollection {
                field: "style.stroke_dasharray",
            });
        }
        self.stroke.dasharray = Specified::Value(values);
        Ok(self)
    }

    pub fn with_padding(mut self, padding: InsetsPx) -> Self {
        self.spacing.padding = Specified::Value(padding);
        self
    }

    pub fn with_effect(
        mut self,
        effect: impl Into<String>,
    ) -> Result<Self, ThemeCompileValidationError> {
        let effect = effect.into();
        super::effects::validate_effect_id(&effect)?;
        self.effects.effect = Specified::Value(effect);
        Ok(self)
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        if let Specified::Value(paint) = &self.paint.fill {
            paint.validate("style.fill")?;
        }
        if let Specified::Value(paint) = &self.stroke.paint {
            paint.validate("style.stroke")?;
        }
        validate_optional_nonnegative(&self.stroke.width, "style.stroke_width")?;
        validate_optional_unit(&self.paint.opacity, "style.opacity")?;
        validate_optional_unit(&self.paint.fill_opacity, "style.fill_opacity")?;
        validate_optional_unit(&self.stroke.stroke_opacity, "style.stroke_opacity")?;
        validate_optional_nonnegative(&self.geometry.radius, "style.radius")?;
        if let Specified::Value(padding) = &self.spacing.padding {
            padding.validate("style.padding")?;
        }
        if let Specified::Value(dashes) = &self.stroke.dasharray
            && (dashes.is_empty()
                || dashes
                    .iter()
                    .any(|value| !value.is_finite() || *value < 0.0))
        {
            return Err(ThemeCompileValidationError::InvalidCollection {
                field: "style.stroke_dasharray",
            });
        }
        self.typography.validate()
    }

    pub(crate) fn is_empty(&self) -> bool {
        matches!(self.paint.fill, Specified::Unspecified)
            && matches!(self.stroke.paint, Specified::Unspecified)
            && matches!(self.stroke.width, Specified::Unspecified)
            && matches!(self.stroke.dasharray, Specified::Unspecified)
            && matches!(self.stroke.linecap, Specified::Unspecified)
            && matches!(self.stroke.linejoin, Specified::Unspecified)
            && matches!(self.paint.opacity, Specified::Unspecified)
            && matches!(self.paint.fill_opacity, Specified::Unspecified)
            && matches!(self.stroke.stroke_opacity, Specified::Unspecified)
            && matches!(self.geometry.radius, Specified::Unspecified)
            && matches!(self.spacing.padding, Specified::Unspecified)
            && matches!(self.effects.effect, Specified::Unspecified)
            && self.typography == TextStylePatch::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StrokeLineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StrokeLineJoin {
    Miter,
    Round,
    Bevel,
}

impl StrokeLineCap {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Butt => "butt",
            Self::Round => "round",
            Self::Square => "square",
        }
    }
}

impl StrokeLineJoin {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Miter => "miter",
            Self::Round => "round",
            Self::Bevel => "bevel",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeRule {
    target: ThemeTarget,
    family: Option<DiagramFamilyId>,
    variant: Option<ThemeVariant>,
    ordinal: Option<OrdinalSelector>,
    style: ThemeStylePatch,
}

impl ThemeRule {
    /// Creates a base rule that applies to every variant of the target.
    ///
    /// Use [`Self::with_variant`] to restrict the rule to one named variant.
    pub fn new(target: ThemeTarget, style: ThemeStylePatch) -> Self {
        Self {
            target,
            family: None,
            variant: None,
            ordinal: None,
            style,
        }
    }

    pub fn for_family(mut self, family: DiagramFamilyId) -> Self {
        self.family = Some(family);
        self
    }

    /// Restricts this rule to one named semantic variant.
    pub fn with_variant(mut self, variant: ThemeVariant) -> Self {
        self.variant = Some(variant);
        self
    }

    pub fn with_ordinal(mut self, ordinal: OrdinalSelector) -> Self {
        self.ordinal = Some(ordinal);
        self
    }

    pub const fn target(&self) -> ThemeTarget {
        self.target
    }

    pub const fn family(&self) -> Option<DiagramFamilyId> {
        self.family
    }

    pub const fn variant(&self) -> Option<ThemeVariant> {
        self.variant
    }

    pub const fn ordinal(&self) -> Option<OrdinalSelector> {
        self.ordinal
    }

    pub const fn style(&self) -> &ThemeStylePatch {
        &self.style
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        if self.style.is_empty() {
            return Err(ThemeCompileValidationError::EmptyValue {
                field: "styles.rule",
            });
        }
        if self.target == ThemeTarget::Canvas
            && let Some(family) = self.family
        {
            return Err(ThemeCompileValidationError::InvalidCanvasFamilyScope {
                family: family.as_str(),
            });
        }
        if let Some(family) = self.family
            && !self.target.valid_for(family)
        {
            return Err(ThemeCompileValidationError::InvalidTargetFamily {
                target: self.target.id(),
                family: family.as_str(),
            });
        }
        self.style.validate()
    }

    #[cfg(test)]
    pub(crate) fn applies_to(
        &self,
        family: DiagramFamilyId,
        variant: ThemeVariant,
        ordinal: Option<usize>,
    ) -> bool {
        self.family.is_none_or(|expected| expected == family)
            && self.variant.is_none_or(|expected| expected == variant)
            && self
                .ordinal
                .is_none_or(|selector| ordinal.is_some_and(|index| selector.matches(index)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrdinalPalette {
    colors: Vec<ThemeColorValue>,
}

impl OrdinalPalette {
    pub fn new(
        colors: impl IntoIterator<Item = ThemeColorValue>,
    ) -> Result<Self, ThemeCompileValidationError> {
        let colors = colors.into_iter().collect::<Vec<_>>();
        if colors.is_empty() {
            return Err(ThemeCompileValidationError::InvalidCollection {
                field: "styles.ordinal_palette",
            });
        }
        if colors.len() > MAX_THEME_PALETTE_COLORS {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "styles.ordinal_palette.colors",
            });
        }
        Ok(Self { colors })
    }

    pub fn colors(&self) -> &[ThemeColorValue] {
        &self.colors
    }

    pub fn color_for(&self, one_based_index: usize) -> Option<&ThemeColorValue> {
        let index = one_based_index.checked_sub(1)?;
        let palette_len = self.colors.len();
        (palette_len != 0).then(|| &self.colors[index % palette_len])
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeRuleSet {
    rules: Vec<ThemeRule>,
    ordinal_palettes: Vec<(ThemeTarget, OrdinalPalette)>,
}

impl ThemeRuleSet {
    pub fn with_rule(mut self, rule: ThemeRule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn with_ordinal_palette(mut self, target: ThemeTarget, palette: OrdinalPalette) -> Self {
        self.ordinal_palettes.push((target, palette));
        self
    }

    pub fn rules(&self) -> &[ThemeRule] {
        &self.rules
    }

    pub fn ordinal_palettes(&self) -> &[(ThemeTarget, OrdinalPalette)] {
        &self.ordinal_palettes
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        if self.rules.len() > MAX_THEME_RULES {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "styles.rules",
            });
        }
        if self.ordinal_palettes.len() > MAX_THEME_ORDINAL_PALETTES {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "styles.ordinal_palettes",
            });
        }
        for rule in &self.rules {
            rule.validate()?;
        }
        let mut targets = std::collections::BTreeSet::new();
        for (target, palette) in &self.ordinal_palettes {
            if !targets.insert(*target) {
                return Err(ThemeCompileValidationError::DuplicateId {
                    field: "styles.ordinal_palette.target",
                });
            }
            if palette.colors.is_empty() {
                return Err(ThemeCompileValidationError::InvalidCollection {
                    field: "styles.ordinal_palette",
                });
            }
            if palette.colors.len() > MAX_THEME_PALETTE_COLORS {
                return Err(ThemeCompileValidationError::LimitExceeded {
                    field: "styles.ordinal_palette.colors",
                });
            }
        }
        Ok(())
    }
}

fn validate_nonnegative(
    value: f32,
    field: &'static str,
) -> Result<(), ThemeCompileValidationError> {
    if !value.is_finite() || value < 0.0 {
        return Err(ThemeCompileValidationError::InvalidNumber { field });
    }
    Ok(())
}

fn validate_optional_nonnegative(
    value: &Specified<f32>,
    field: &'static str,
) -> Result<(), ThemeCompileValidationError> {
    if let Specified::Value(value) = value {
        validate_nonnegative(*value, field)?;
    }
    Ok(())
}

fn validate_optional_unit(
    value: &Specified<f32>,
    field: &'static str,
) -> Result<(), ThemeCompileValidationError> {
    if let Specified::Value(value) = value
        && (!value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(ThemeCompileValidationError::InvalidNumber { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_rule() -> ThemeRule {
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#2563eb").expect("valid test paint")),
        )
    }

    fn palette_color() -> ThemeColorValue {
        ThemeColorValue::parse("#2563eb").expect("valid test color")
    }

    #[test]
    fn semantic_rule_limit_accepts_the_boundary_and_rejects_the_next_rule() {
        let boundary = ThemeRuleSet {
            rules: vec![solid_rule(); MAX_THEME_RULES],
            ordinal_palettes: Vec::new(),
        };
        boundary.validate().expect("rule boundary should validate");

        let exceeded = ThemeRuleSet {
            rules: vec![solid_rule(); MAX_THEME_RULES + 1],
            ordinal_palettes: Vec::new(),
        };
        assert_eq!(
            exceeded.validate(),
            Err(ThemeCompileValidationError::LimitExceeded {
                field: "styles.rules",
            })
        );
    }

    #[test]
    fn ordinal_palette_color_limit_accepts_the_boundary_and_rejects_the_next_color() {
        OrdinalPalette::new(vec![palette_color(); MAX_THEME_PALETTE_COLORS])
            .expect("palette boundary should validate");

        assert_eq!(
            OrdinalPalette::new(vec![palette_color(); MAX_THEME_PALETTE_COLORS + 1]),
            Err(ThemeCompileValidationError::LimitExceeded {
                field: "styles.ordinal_palette.colors",
            })
        );
    }

    #[test]
    fn ordinal_palette_count_is_bounded_before_duplicate_target_validation() {
        let palette = OrdinalPalette::new([palette_color()]).expect("valid test palette");
        let exceeded = ThemeRuleSet {
            rules: Vec::new(),
            ordinal_palettes: vec![
                (ThemeTarget::ChartSeries, palette);
                MAX_THEME_ORDINAL_PALETTES + 1
            ],
        };

        assert_eq!(
            exceeded.validate(),
            Err(ThemeCompileValidationError::LimitExceeded {
                field: "styles.ordinal_palettes",
            })
        );
    }

    #[test]
    fn architecture_admits_only_cluster_from_the_shared_node_family_targets() {
        assert!(ThemeTarget::Cluster.valid_for(DiagramFamilyId::ARCHITECTURE));
        for target in [
            ThemeTarget::Node,
            ThemeTarget::NodeLabel,
            ThemeTarget::Edge,
            ThemeTarget::EdgeLabel,
            ThemeTarget::EdgeLabelBackground,
            ThemeTarget::ClusterLabel,
            ThemeTarget::Marker,
        ] {
            assert!(!target.valid_for(DiagramFamilyId::ARCHITECTURE));
        }
    }

    #[test]
    fn er_owns_entity_while_requirement_keeps_its_family_identity() {
        assert!(ThemeTarget::Entity.valid_for(DiagramFamilyId::ER));
        assert!(!ThemeTarget::Entity.valid_for(DiagramFamilyId::REQUIREMENT));
        assert!(ThemeTarget::Requirement.valid_for(DiagramFamilyId::REQUIREMENT));
        assert!(!ThemeTarget::Requirement.valid_for(DiagramFamilyId::ER));
        assert!(ThemeTarget::Relation.valid_for(DiagramFamilyId::ER));
        assert!(ThemeTarget::Relation.valid_for(DiagramFamilyId::REQUIREMENT));
    }
}
