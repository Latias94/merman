use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

pub const MANIFEST_RELATIVE_PATH: &str = "manifest.json";
pub const SCHEMA_VERSION: u32 = 2;
pub const FIXTURE_EXPECTATION_VERSION: u32 = 2;
pub const THEME_INPUT_VERSION: u32 = 2;
pub const MODERN_MERMAID_REFERENCE_THEME_COUNT: usize = 24;
pub const MERMAID_STYLE_PRECEDENCE_ENTRY_COUNT: usize = 26;

pub const MERMAID_STYLE_PRECEDENCE_ENTRY_IDS: [&str; MERMAID_STYLE_PRECEDENCE_ENTRY_COUNT] = [
    "class-assignment-before-definition-copy",
    "class-definition-before-assignment-no-backfill",
    "class-node-inline",
    "class-typography-classdef-residual",
    "flow-edge-default",
    "flow-edge-specific",
    "flow-node-assigned",
    "flow-node-default",
    "flow-node-inline",
    "flow-node-named",
    "flow-node-theme",
    "global-config-frontmatter",
    "global-config-init",
    "global-css-classdef",
    "global-css-family",
    "global-css-family-typography-residual",
    "global-css-theme",
    "state-node-assigned",
    "state-node-inline",
    "state-node-theme",
    "er-node-default",
    "er-node-assigned",
    "er-node-inline",
    "er-typography-default-residual",
    "er-typography-assigned-residual",
    "er-typography-inline-residual",
];

pub const MODERN_MERMAID_REFERENCE_THEMES: [&str; MODERN_MERMAID_REFERENCE_THEME_COUNT] = [
    "aurora",
    "brutalist",
    "cyberpunk",
    "darkMinimal",
    "doodle",
    "geometricCollage",
    "ghibli",
    "glassmorphism",
    "grafana",
    "handDrawn",
    "hightech",
    "kawaii",
    "linearDark",
    "linearLight",
    "material",
    "memphis",
    "monochrome",
    "noir",
    "notion",
    "organic",
    "softPop",
    "spotless",
    "win95",
    "wireframe",
];

pub(crate) const MODERN_MERMAID_CANVAS_LAYER_COUNTS: [(&str, u8);
    MODERN_MERMAID_REFERENCE_THEME_COUNT] = [
    ("aurora", 3),
    ("brutalist", 0),
    ("cyberpunk", 3),
    ("darkMinimal", 0),
    ("doodle", 1),
    ("geometricCollage", 2),
    ("ghibli", 4),
    ("glassmorphism", 1),
    ("grafana", 2),
    ("handDrawn", 1),
    ("hightech", 1),
    ("kawaii", 1),
    ("linearDark", 1),
    ("linearLight", 1),
    ("material", 0),
    ("memphis", 6),
    ("monochrome", 0),
    ("noir", 3),
    ("notion", 0),
    ("organic", 1),
    ("softPop", 0),
    ("spotless", 2),
    ("win95", 4),
    ("wireframe", 2),
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceThemeMechanism {
    BackdropFilter,
    CanvasBlend,
    CanvasGradient,
    CanvasLayering,
    CanvasPattern,
    CanvasSolid,
    CssFilter,
    CssLetterSpacing,
    CssTextTransform,
    DashArray,
    ExternalSvgFilterReference,
    FontStack,
    HasSelector,
    NotSelector,
    NthChildSelector,
    RoundedCorners,
    StrokeStyling,
    ThemeVariables,
}

pub const REFERENCE_THEME_MECHANISMS: [ReferenceThemeMechanism; 18] = [
    ReferenceThemeMechanism::BackdropFilter,
    ReferenceThemeMechanism::CanvasBlend,
    ReferenceThemeMechanism::CanvasGradient,
    ReferenceThemeMechanism::CanvasLayering,
    ReferenceThemeMechanism::CanvasPattern,
    ReferenceThemeMechanism::CanvasSolid,
    ReferenceThemeMechanism::CssFilter,
    ReferenceThemeMechanism::CssLetterSpacing,
    ReferenceThemeMechanism::CssTextTransform,
    ReferenceThemeMechanism::DashArray,
    ReferenceThemeMechanism::ExternalSvgFilterReference,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::HasSelector,
    ReferenceThemeMechanism::NotSelector,
    ReferenceThemeMechanism::NthChildSelector,
    ReferenceThemeMechanism::RoundedCorners,
    ReferenceThemeMechanism::StrokeStyling,
    ReferenceThemeMechanism::ThemeVariables,
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ExpectedThemeCapability {
    BlendMode,
    BorderStyling,
    DashStyling,
    Displacement,
    GradientCanvas,
    LayeredCanvas,
    LetterSpacing,
    Noise,
    OrdinalPalette,
    PatternCanvas,
    RoundedGeometry,
    SemanticRules,
    SemanticTokens,
    Shadow,
    SolidCanvas,
    SvgFilter,
    TextTransform,
    Typography,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ExpectedOutputTarget {
    BrowserSvg,
    Jpeg,
    Pdf,
    Png,
    StandaloneSvg,
}

pub const EXPECTED_OUTPUT_TARGETS: [ExpectedOutputTarget; 5] = [
    ExpectedOutputTarget::BrowserSvg,
    ExpectedOutputTarget::Jpeg,
    ExpectedOutputTarget::Pdf,
    ExpectedOutputTarget::Png,
    ExpectedOutputTarget::StandaloneSvg,
];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExpectedPortabilityGrade {
    HostDependent,
    Portable,
    SvgOnly,
    Unverified,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AssetMediaType {
    FontWoff2,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceMechanismDisposition {
    Capability,
    Residual,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FixtureEvidenceKind {
    SourceCompatibility,
    TypedCapability,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum MermaidStyleFamily {
    ClassDiagram,
    ErDiagram,
    Flowchart,
    Global,
    StateDiagram,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum MermaidStyleOrigin {
    AssignedClass,
    BuiltIn,
    ClassDefDefault,
    ClassDefNamed,
    FamilyThemeCss,
    Frontmatter,
    GeneratedClassCss,
    InitDirective,
    InlineStyle,
    LinkStyleDefault,
    LinkStyleSpecific,
    SiteConfig,
    ThemeCss,
    ThemeVariables,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum MermaidStyleProperty {
    EdgePaint,
    EffectiveConfig,
    NodePaint,
    NodeTypography,
    StylesheetPaint,
    StylesheetTypography,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum MermaidStylePlane {
    Config,
    CssCascade,
    PaintOnly,
    PreLayout,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum MermaidStyleAdmission {
    PaintOnly,
    SourceCompatibility,
    Typed,
    UnverifiedResidual,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceFontBinding {
    FixtureAssets,
    VendoredDefault,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceTextTransform {
    Capitalize,
    Lowercase,
    Uppercase,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceBlendMode {
    Multiply,
    Overlay,
    Screen,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceGradientKind {
    Linear,
    Radial,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceGradientRepetition {
    None,
    Repeating,
    Tiled,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceDiagramFamily {
    ClassDiagram,
    ErDiagram,
    Flowchart,
    StateDiagram,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceSemanticTarget {
    Attribute,
    Cluster,
    Edge,
    Label,
    Node,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ReferenceThemeFacet {
    CanvasBlend {
        mode: ReferenceBlendMode,
    },
    CanvasGradient {
        gradient: ReferenceGradientKind,
        repetition: ReferenceGradientRepetition,
    },
    SemanticSelector {
        mechanism: ReferenceThemeMechanism,
        family: ReferenceDiagramFamily,
    },
}

impl ReferenceThemeFacet {
    pub const fn mechanism(self) -> ReferenceThemeMechanism {
        match self {
            Self::CanvasBlend { .. } => ReferenceThemeMechanism::CanvasBlend,
            Self::CanvasGradient { .. } => ReferenceThemeMechanism::CanvasGradient,
            Self::SemanticSelector { mechanism, .. } => mechanism,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceEvidenceRecord {
    pub(crate) source_path: String,
    pub(crate) sha256: String,
}

impl SourceEvidenceRecord {
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRecord {
    pub(crate) id: String,
    pub(crate) project: String,
    pub(crate) revision: String,
    pub(crate) upstream_license_path: String,
    pub(crate) license_path: String,
    pub(crate) license_sha256: String,
    pub(crate) snapshot_path: String,
    pub(crate) snapshot_sha256: String,
    pub(crate) evidence: Vec<SourceEvidenceRecord>,
}

impl SourceRecord {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn project(&self) -> &str {
        &self.project
    }

    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn upstream_license_path(&self) -> &str {
        &self.upstream_license_path
    }

    pub fn license_path(&self) -> &str {
        &self.license_path
    }

    pub fn license_sha256(&self) -> &str {
        &self.license_sha256
    }

    pub fn snapshot_path(&self) -> &str {
        &self.snapshot_path
    }

    pub fn snapshot_sha256(&self) -> &str {
        &self.snapshot_sha256
    }

    pub fn evidence(&self) -> &[SourceEvidenceRecord] {
        &self.evidence
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetRecord {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) media_type: AssetMediaType,
    pub(crate) canonical_family: String,
    pub(crate) family: String,
    pub(crate) weight: u16,
    pub(crate) style: String,
    pub(crate) unicode_range: String,
    pub(crate) source_id: String,
    pub(crate) source_path: String,
    pub(crate) source_sha256: String,
    pub(crate) license_path: String,
    pub(crate) license_sha256: String,
    pub(crate) notice_path: String,
    pub(crate) notice_sha256: String,
}

impl AssetRecord {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    pub fn media_type(&self) -> AssetMediaType {
        self.media_type
    }

    pub fn canonical_family(&self) -> &str {
        &self.canonical_family
    }

    pub fn family(&self) -> &str {
        &self.family
    }

    pub fn weight(&self) -> u16 {
        self.weight
    }

    pub fn style(&self) -> &str {
        &self.style
    }

    pub fn unicode_range(&self) -> &str {
        &self.unicode_range
    }

    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub fn license_path(&self) -> &str {
        &self.license_path
    }

    pub fn license_sha256(&self) -> &str {
        &self.license_sha256
    }

    pub fn notice_path(&self) -> &str {
        &self.notice_path
    }

    pub fn notice_sha256(&self) -> &str {
        &self.notice_sha256
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceThemeTokens {
    pub(crate) background: String,
    pub(crate) surface: String,
    pub(crate) primary: String,
    pub(crate) text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceFontStack {
    pub(crate) binding: ReferenceFontBinding,
    pub(crate) families: Vec<String>,
    pub(crate) asset_ids: BTreeSet<String>,
}

impl ReferenceFontStack {
    pub fn binding(&self) -> ReferenceFontBinding {
        self.binding
    }

    pub fn families(&self) -> &[String] {
        &self.families
    }

    pub fn asset_ids(&self) -> &BTreeSet<String> {
        &self.asset_ids
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceTypographyInput {
    pub(crate) font_stack: Option<ReferenceFontStack>,
    pub(crate) letter_spacing_milli_em: Option<i16>,
    pub(crate) text_transform: Option<ReferenceTextTransform>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceGradientStop {
    pub(crate) offset_percent: u8,
    pub(crate) color: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferenceCanvasLayer {
    LinearGradient {
        angle_degrees: u16,
        repetition: ReferenceGradientRepetition,
        tile_width_px: Option<u16>,
        tile_height_px: Option<u16>,
        stops: Vec<ReferenceGradientStop>,
    },
    RadialGradient {
        center_x_percent: u8,
        center_y_percent: u8,
        repetition: ReferenceGradientRepetition,
        tile_width_px: Option<u16>,
        tile_height_px: Option<u16>,
        stops: Vec<ReferenceGradientStop>,
    },
    Solid {
        color: String,
    },
}

impl ReferenceCanvasLayer {
    pub(crate) const fn facet(&self) -> Option<ReferenceThemeFacet> {
        match self {
            Self::LinearGradient { repetition, .. } => Some(ReferenceThemeFacet::CanvasGradient {
                gradient: ReferenceGradientKind::Linear,
                repetition: *repetition,
            }),
            Self::RadialGradient { repetition, .. } => Some(ReferenceThemeFacet::CanvasGradient {
                gradient: ReferenceGradientKind::Radial,
                repetition: *repetition,
            }),
            Self::Solid { .. } => None,
        }
    }

    pub(crate) const fn is_pattern(&self) -> bool {
        matches!(
            self,
            Self::LinearGradient {
                repetition: ReferenceGradientRepetition::Repeating
                    | ReferenceGradientRepetition::Tiled,
                ..
            } | Self::RadialGradient {
                repetition: ReferenceGradientRepetition::Repeating
                    | ReferenceGradientRepetition::Tiled,
                ..
            }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceBorderInput {
    pub(crate) color: String,
    pub(crate) width_px: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceShadowInput {
    pub(crate) offset_x_px: i16,
    pub(crate) offset_y_px: i16,
    pub(crate) blur_px: u16,
    pub(crate) spread_px: u16,
    pub(crate) color: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceNodeStyleInput {
    pub(crate) border: Option<ReferenceBorderInput>,
    pub(crate) dash_pattern: Vec<u16>,
    pub(crate) corner_radius_px: Option<u16>,
    pub(crate) shadow: Option<ReferenceShadowInput>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceSemanticStylePatch {
    pub(crate) fill: Option<String>,
    pub(crate) border: Option<ReferenceBorderInput>,
    pub(crate) corner_radius_px: Option<u16>,
    pub(crate) shadow: Option<ReferenceShadowInput>,
    pub(crate) text_color: Option<String>,
    pub(crate) font_weight: Option<u16>,
}

impl ReferenceSemanticStylePatch {
    pub(crate) fn is_empty(&self) -> bool {
        self.fill.is_none()
            && self.border.is_none()
            && self.corner_radius_px.is_none()
            && self.shadow.is_none()
            && self.text_color.is_none()
            && self.font_weight.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferenceSemanticRule {
    HasDescendant {
        family: ReferenceDiagramFamily,
        target: ReferenceSemanticTarget,
        descendant: ReferenceSemanticTarget,
        apply: ReferenceSemanticStylePatch,
    },
    NotClass {
        family: ReferenceDiagramFamily,
        target: ReferenceSemanticTarget,
        class_name: String,
        apply: ReferenceSemanticStylePatch,
    },
    OrdinalPalette {
        family: ReferenceDiagramFamily,
        target: ReferenceSemanticTarget,
        colors: Vec<String>,
    },
}

impl ReferenceSemanticRule {
    pub(crate) const fn mechanism(&self) -> ReferenceThemeMechanism {
        match self {
            Self::HasDescendant { .. } => ReferenceThemeMechanism::HasSelector,
            Self::NotClass { .. } => ReferenceThemeMechanism::NotSelector,
            Self::OrdinalPalette { .. } => ReferenceThemeMechanism::NthChildSelector,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferenceSvgEffect {
    TurbulenceDisplacement {
        base_frequency_milli: u16,
        octaves: u8,
        scale: u16,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceThemeInput {
    pub(crate) fixture_id: String,
    pub(crate) tokens: Option<ReferenceThemeTokens>,
    pub(crate) typography: Option<ReferenceTypographyInput>,
    pub(crate) canvas: Vec<ReferenceCanvasLayer>,
    pub(crate) blend_mode: Option<ReferenceBlendMode>,
    pub(crate) node_style: Option<ReferenceNodeStyleInput>,
    pub(crate) semantic_rules: Vec<ReferenceSemanticRule>,
    pub(crate) svg_effects: Vec<ReferenceSvgEffect>,
    pub(crate) residual_mechanisms: BTreeSet<ReferenceThemeMechanism>,
}

impl ReferenceThemeInput {
    pub fn fixture_id(&self) -> &str {
        &self.fixture_id
    }

    pub fn mechanisms(&self) -> BTreeSet<ReferenceThemeMechanism> {
        let mut mechanisms = BTreeSet::new();
        if self.tokens.is_some() {
            mechanisms.insert(ReferenceThemeMechanism::ThemeVariables);
        }
        if let Some(typography) = &self.typography {
            if typography.font_stack.is_some() {
                mechanisms.insert(ReferenceThemeMechanism::FontStack);
            }
            if typography.letter_spacing_milli_em.is_some() {
                mechanisms.insert(ReferenceThemeMechanism::CssLetterSpacing);
            }
            if typography.text_transform.is_some() {
                mechanisms.insert(ReferenceThemeMechanism::CssTextTransform);
            }
        }
        for layer in &self.canvas {
            match layer {
                ReferenceCanvasLayer::LinearGradient { .. }
                | ReferenceCanvasLayer::RadialGradient { .. } => {
                    mechanisms.insert(ReferenceThemeMechanism::CanvasGradient);
                    if layer.is_pattern() {
                        mechanisms.insert(ReferenceThemeMechanism::CanvasPattern);
                    }
                }
                ReferenceCanvasLayer::Solid { .. } => {
                    mechanisms.insert(ReferenceThemeMechanism::CanvasSolid);
                }
            }
        }
        let image_layer_count = self
            .canvas
            .iter()
            .filter(|layer| {
                matches!(
                    layer,
                    ReferenceCanvasLayer::LinearGradient { .. }
                        | ReferenceCanvasLayer::RadialGradient { .. }
                )
            })
            .count();
        if image_layer_count > 1 {
            mechanisms.insert(ReferenceThemeMechanism::CanvasLayering);
        }
        if self.blend_mode.is_some() && image_layer_count > 0 {
            mechanisms.insert(ReferenceThemeMechanism::CanvasBlend);
        }
        if let Some(style) = &self.node_style {
            if style.border.is_some() {
                mechanisms.insert(ReferenceThemeMechanism::StrokeStyling);
            }
            if !style.dash_pattern.is_empty() {
                mechanisms.insert(ReferenceThemeMechanism::DashArray);
            }
            if style.corner_radius_px.is_some() {
                mechanisms.insert(ReferenceThemeMechanism::RoundedCorners);
            }
            if style.shadow.is_some() {
                mechanisms.insert(ReferenceThemeMechanism::CssFilter);
            }
        }
        mechanisms.extend(
            self.semantic_rules
                .iter()
                .map(ReferenceSemanticRule::mechanism),
        );
        if !self.svg_effects.is_empty() {
            mechanisms.insert(ReferenceThemeMechanism::ExternalSvgFilterReference);
        }
        mechanisms.extend(self.residual_mechanisms.iter().copied());
        mechanisms
    }

    pub fn facets(&self) -> BTreeSet<ReferenceThemeFacet> {
        let mut facets = self
            .canvas
            .iter()
            .filter_map(ReferenceCanvasLayer::facet)
            .collect::<BTreeSet<_>>();
        if let Some(mode) = self.blend_mode {
            facets.insert(ReferenceThemeFacet::CanvasBlend { mode });
        }
        for rule in &self.semantic_rules {
            match rule {
                ReferenceSemanticRule::HasDescendant { family, .. } => {
                    facets.insert(ReferenceThemeFacet::SemanticSelector {
                        mechanism: ReferenceThemeMechanism::HasSelector,
                        family: *family,
                    });
                }
                ReferenceSemanticRule::NotClass { family, .. } => {
                    facets.insert(ReferenceThemeFacet::SemanticSelector {
                        mechanism: ReferenceThemeMechanism::NotSelector,
                        family: *family,
                    });
                }
                ReferenceSemanticRule::OrdinalPalette { .. } => {}
            }
        }
        facets
    }

    pub fn font_stack(&self) -> Option<&ReferenceFontStack> {
        self.typography
            .as_ref()
            .and_then(|typography| typography.font_stack.as_ref())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureExpectation {
    pub(crate) evidence_kind: FixtureEvidenceKind,
    pub(crate) intent: String,
    pub(crate) reference_mechanisms: BTreeSet<ReferenceThemeMechanism>,
    pub(crate) capabilities: BTreeSet<ExpectedThemeCapability>,
    pub(crate) outputs: BTreeSet<ExpectedOutputTarget>,
    pub(crate) target_capabilities:
        BTreeMap<ExpectedOutputTarget, BTreeSet<ExpectedThemeCapability>>,
    pub(crate) visible_text: Vec<String>,
    pub(crate) style_evidence_ids: BTreeSet<String>,
}

impl FixtureExpectation {
    pub fn evidence_kind(&self) -> FixtureEvidenceKind {
        self.evidence_kind
    }

    pub fn intent(&self) -> &str {
        &self.intent
    }

    pub fn reference_mechanisms(&self) -> &BTreeSet<ReferenceThemeMechanism> {
        &self.reference_mechanisms
    }

    pub fn capabilities(&self) -> &BTreeSet<ExpectedThemeCapability> {
        &self.capabilities
    }

    pub fn outputs(&self) -> &BTreeSet<ExpectedOutputTarget> {
        &self.outputs
    }

    pub fn capabilities_for(
        &self,
        target: ExpectedOutputTarget,
    ) -> Option<&BTreeSet<ExpectedThemeCapability>> {
        self.target_capabilities.get(&target)
    }

    pub fn visible_text(&self) -> &[String] {
        &self.visible_text
    }

    pub fn text_samples(&self) -> &[String] {
        self.visible_text()
    }

    pub fn style_evidence_ids(&self) -> &BTreeSet<String> {
        &self.style_evidence_ids
    }
}

#[derive(Clone, Debug)]
pub struct FixtureRecord {
    pub(crate) id: String,
    pub(crate) source_path: String,
    pub(crate) source_sha256: String,
    pub(crate) expectation_sha256: String,
    pub(crate) theme_input_sha256: Option<String>,
    pub(crate) evidence_source_ids: BTreeSet<String>,
    pub(crate) asset_ids: BTreeSet<String>,
    pub(crate) source_family: ReferenceDiagramFamily,
    pub(crate) source_reference_mechanisms: BTreeSet<ReferenceThemeMechanism>,
    pub(crate) source_style_evidence_ids: BTreeSet<String>,
    pub(crate) expectation: FixtureExpectation,
    pub(crate) theme_input: Option<ReferenceThemeInput>,
}

impl FixtureRecord {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub fn expectation_sha256(&self) -> &str {
        &self.expectation_sha256
    }

    pub fn theme_input_sha256(&self) -> Option<&str> {
        self.theme_input_sha256.as_deref()
    }

    pub fn evidence_source_ids(&self) -> &BTreeSet<String> {
        &self.evidence_source_ids
    }

    pub fn asset_ids(&self) -> &BTreeSet<String> {
        &self.asset_ids
    }

    pub fn source_family(&self) -> ReferenceDiagramFamily {
        self.source_family
    }

    pub fn source_reference_mechanisms(&self) -> &BTreeSet<ReferenceThemeMechanism> {
        &self.source_reference_mechanisms
    }

    pub fn source_style_evidence_ids(&self) -> &BTreeSet<String> {
        &self.source_style_evidence_ids
    }

    pub fn expectation(&self) -> &FixtureExpectation {
        &self.expectation
    }

    pub fn theme_input(&self) -> Option<&ReferenceThemeInput> {
        self.theme_input.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetTranslation {
    pub(crate) disposition: ReferenceMechanismDisposition,
    pub(crate) capabilities: BTreeSet<ExpectedThemeCapability>,
}

impl TargetTranslation {
    pub fn disposition(&self) -> ReferenceMechanismDisposition {
        self.disposition
    }

    pub fn capabilities(&self) -> &BTreeSet<ExpectedThemeCapability> {
        &self.capabilities
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceTranslation {
    pub(crate) source: ReferenceThemeMechanism,
    pub(crate) disposition: ReferenceMechanismDisposition,
    pub(crate) capabilities: BTreeSet<ExpectedThemeCapability>,
    pub(crate) target_overrides: BTreeMap<ExpectedOutputTarget, TargetTranslation>,
}

impl ReferenceTranslation {
    pub fn source(&self) -> ReferenceThemeMechanism {
        self.source
    }

    pub fn disposition(&self) -> ReferenceMechanismDisposition {
        self.disposition
    }

    pub fn capabilities(&self) -> &BTreeSet<ExpectedThemeCapability> {
        &self.capabilities
    }

    pub fn target_overrides(&self) -> &BTreeMap<ExpectedOutputTarget, TargetTranslation> {
        &self.target_overrides
    }

    pub fn effective_for(
        &self,
        target: ExpectedOutputTarget,
    ) -> (
        ReferenceMechanismDisposition,
        &BTreeSet<ExpectedThemeCapability>,
    ) {
        self.target_overrides
            .get(&target)
            .map_or((self.disposition, &self.capabilities), |translation| {
                (translation.disposition, &translation.capabilities)
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidualRecord {
    pub(crate) id: String,
    pub(crate) source_mechanism: ReferenceThemeMechanism,
    pub(crate) reason: String,
}

impl ResidualRecord {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn source_mechanism(&self) -> ReferenceThemeMechanism {
        self.source_mechanism
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetExpectation {
    pub(crate) target: ExpectedOutputTarget,
    pub(crate) grade: ExpectedPortabilityGrade,
    pub(crate) capabilities: BTreeSet<ExpectedThemeCapability>,
    pub(crate) residuals: Vec<ResidualRecord>,
}

impl TargetExpectation {
    pub fn target(&self) -> ExpectedOutputTarget {
        self.target
    }

    pub fn grade(&self) -> ExpectedPortabilityGrade {
        self.grade
    }

    pub fn capabilities(&self) -> &BTreeSet<ExpectedThemeCapability> {
        &self.capabilities
    }

    pub fn residuals(&self) -> &[ResidualRecord] {
        &self.residuals
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceThemeRecord {
    pub(crate) id: String,
    pub(crate) reference_name: String,
    pub(crate) source_id: String,
    pub(crate) source_line: u32,
    pub(crate) canvas_layer_count: u8,
    pub(crate) source_mechanisms: BTreeSet<ReferenceThemeMechanism>,
    pub(crate) source_facets: BTreeSet<ReferenceThemeFacet>,
    pub(crate) fixture_ids: BTreeSet<String>,
    pub(crate) targets: Vec<TargetExpectation>,
}

impl ReferenceThemeRecord {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn reference_name(&self) -> &str {
        &self.reference_name
    }

    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    pub fn source_line(&self) -> u32 {
        self.source_line
    }

    pub fn canvas_layer_count(&self) -> u8 {
        self.canvas_layer_count
    }

    pub fn source_mechanisms(&self) -> &BTreeSet<ReferenceThemeMechanism> {
        &self.source_mechanisms
    }

    pub fn source_facets(&self) -> &BTreeSet<ReferenceThemeFacet> {
        &self.source_facets
    }

    pub fn fixture_ids(&self) -> &BTreeSet<String> {
        &self.fixture_ids
    }

    pub fn targets(&self) -> &[TargetExpectation] {
        &self.targets
    }

    pub fn target(&self, target: ExpectedOutputTarget) -> Option<&TargetExpectation> {
        self.targets
            .iter()
            .find(|expectation| expectation.target == target)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StylePrecedenceEvidence {
    pub(crate) id: String,
    pub(crate) family: MermaidStyleFamily,
    pub(crate) origin: MermaidStyleOrigin,
    pub(crate) property: MermaidStyleProperty,
    pub(crate) plane: MermaidStylePlane,
    pub(crate) admission: MermaidStyleAdmission,
    pub(crate) rank: Option<u16>,
    pub(crate) encounter_order: bool,
    pub(crate) evidence: Vec<String>,
    pub(crate) note: String,
}

impl StylePrecedenceEvidence {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn family(&self) -> MermaidStyleFamily {
        self.family
    }

    pub fn origin(&self) -> MermaidStyleOrigin {
        self.origin
    }

    pub fn property(&self) -> MermaidStyleProperty {
        self.property
    }

    pub fn plane(&self) -> MermaidStylePlane {
        self.plane
    }

    pub fn admission(&self) -> MermaidStyleAdmission {
        self.admission
    }

    pub fn rank(&self) -> Option<u16> {
        self.rank
    }

    pub fn encounter_order(&self) -> bool {
        self.encounter_order
    }

    pub fn is_encounter_ordered(&self) -> bool {
        self.encounter_order
    }

    pub fn evidence(&self) -> &[String] {
        &self.evidence
    }

    pub fn note(&self) -> &str {
        &self.note
    }
}
