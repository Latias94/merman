use crate::MermaidConfig;
use crate::ThemeEvaluationLimitExceeded;
use crate::theme_color::{self, ColorAdjustment, ColorError};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

mod staged;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ThemeResolutionError {
    #[error(transparent)]
    Color(#[from] ColorError),
    #[error(transparent)]
    EvaluationLimit(#[from] ThemeEvaluationLimitExceeded),
}

impl From<ThemeResolutionError> for crate::Error {
    fn from(error: ThemeResolutionError) -> Self {
        match error {
            ThemeResolutionError::Color(error) => Self::ThemeColor(error),
            ThemeResolutionError::EvaluationLimit(error) => Self::ThemeEvaluationLimit(error),
        }
    }
}

// Source: Mermaid 11.16.1 `packages/mermaid/src/themes/index.js`.
macro_rules! define_mermaid_theme_ids {
    ($(($variant:ident, $name:literal)),+ $(,)?) => {
        /// A theme identifier from the pinned Mermaid theme catalog.
        ///
        /// This type is deliberately separate from Merman's compiled theme presets. It models
        /// only the upstream `MermaidConfig.theme` contract and is therefore also the source of
        /// truth for bindings, CLI discovery, and compatibility projection. Mermaid's `"null"`
        /// configuration sentinel is not a registered theme and is deliberately excluded.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum MermaidThemeId {
            $( $variant, )+
        }

        impl MermaidThemeId {
            /// All theme identifiers supported by the pinned Mermaid baseline, in catalog order.
            pub const ALL: &'static [Self] = &[
                $(Self::$variant,)+
            ];

            /// The exact upstream spellings in the same order as [`Self::ALL`].
            pub const NAMES: &'static [&'static str] = &[
                $($name,)+
            ];

            /// Returns the exact upstream spelling used in Mermaid configuration and mmdc files.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            /// Parses an upstream theme identifier without applying a fallback.
            pub fn parse(value: &str) -> Result<Self, MermaidThemeIdParseError> {
                value.parse()
            }

            /// Returns whether the selected upstream theme uses a dark visual palette.
            ///
            /// This classification does not set or imply Mermaid's independent
            /// `themeVariables.darkMode` compatibility value.
            pub const fn is_dark(self) -> bool {
                matches!(
                    self,
                    Self::Dark | Self::NeoDark | Self::ReduxDark | Self::ReduxDarkColor
                )
            }
        }

        impl Default for MermaidThemeId {
            fn default() -> Self {
                Self::Default
            }
        }

        impl fmt::Display for MermaidThemeId {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for MermaidThemeId {
            type Err = MermaidThemeIdParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($name => Ok(Self::$variant),)+
                    _ => Err(MermaidThemeIdParseError::new(value)),
                }
            }
        }

    };
}

define_mermaid_theme_ids![
    (Default, "default"),
    (Base, "base"),
    (Dark, "dark"),
    (Forest, "forest"),
    (Neutral, "neutral"),
    (Neo, "neo"),
    (NeoDark, "neo-dark"),
    (Redux, "redux"),
    (ReduxDark, "redux-dark"),
    (ReduxColor, "redux-color"),
    (ReduxDarkColor, "redux-dark-color"),
];

/// Error returned when a string is not one of the pinned Mermaid theme identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MermaidThemeIdParseError {
    input: String,
}

impl MermaidThemeIdParseError {
    fn new(input: &str) -> Self {
        Self {
            input: input.to_string(),
        }
    }

    /// Returns the unsupported identifier supplied to the parser.
    pub fn input(&self) -> &str {
        &self.input
    }
}

impl fmt::Display for MermaidThemeIdParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported Mermaid theme `{}`", self.input)
    }
}

impl std::error::Error for MermaidThemeIdParseError {}

const THEME_ARTIFACT_SCHEMA_VERSION: u32 = 2;

// Generated from the content-pinned Mermaid runtime by `xtask gen-theme-snapshot`.
static GENERATED_THEME_ARTIFACT: OnceLock<GeneratedThemeArtifact> = OnceLock::new();

#[cfg(test)]
static GENERATED_THEME_ORACLES: OnceLock<GeneratedThemeOracles> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedThemeArtifact {
    schema_version: u32,
    provenance: GeneratedThemeProvenance,
    prepared_constructors: Map<String, Value>,
    resolved_without_overrides: Map<String, Value>,
    resolved_dark_mode_true: Map<String, Value>,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedThemeOracles {
    schema_version: u32,
    provenance: GeneratedThemeProvenance,
    oracle_cases: Vec<Value>,
    stage_oracle_cases: Vec<Value>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedThemeProvenance {
    generator: String,
    mermaid_version: String,
    mermaid_package_sha256: String,
    mermaid_source_tag: String,
    mermaid_source_commit: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeProgramKind {
    Default,
    Base,
    Staged(staged::StagedProgram),
    Extended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeDependencyGraph {
    None,
    Default,
    Base,
    DarkenedScale,
    DarkenedScaleAndGit,
    DynamicGit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeVariableDependencyScope {
    Default,
    Base,
    Extended,
    ExtendedLight,
    ExtendedDark,
    LegacyNonDefault,
    NeoFamily,
    NeoDark,
    BaseAndNeoFamily,
    ExtendedWithoutReduxDark,
    ExtendedWithoutReduxVariants,
    ExtendedWithoutReduxDarkVariants,
}

impl ThemeVariableDependencyScope {
    const fn includes(self, theme: MermaidThemeId) -> bool {
        match self {
            Self::Default => matches!(theme, MermaidThemeId::Default),
            Self::Base => matches!(theme, MermaidThemeId::Base),
            Self::Extended => matches!(
                theme,
                MermaidThemeId::Neo
                    | MermaidThemeId::NeoDark
                    | MermaidThemeId::Redux
                    | MermaidThemeId::ReduxDark
                    | MermaidThemeId::ReduxColor
                    | MermaidThemeId::ReduxDarkColor
            ),
            Self::ExtendedLight => matches!(
                theme,
                MermaidThemeId::Neo | MermaidThemeId::Redux | MermaidThemeId::ReduxColor
            ),
            Self::ExtendedDark => matches!(
                theme,
                MermaidThemeId::NeoDark
                    | MermaidThemeId::ReduxDark
                    | MermaidThemeId::ReduxDarkColor
            ),
            Self::LegacyNonDefault => matches!(
                theme,
                MermaidThemeId::Base
                    | MermaidThemeId::Neo
                    | MermaidThemeId::NeoDark
                    | MermaidThemeId::Redux
                    | MermaidThemeId::ReduxDark
                    | MermaidThemeId::ReduxColor
                    | MermaidThemeId::ReduxDarkColor
            ),
            Self::NeoFamily => matches!(theme, MermaidThemeId::Neo | MermaidThemeId::NeoDark),
            Self::NeoDark => matches!(theme, MermaidThemeId::NeoDark),
            Self::BaseAndNeoFamily => matches!(
                theme,
                MermaidThemeId::Base | MermaidThemeId::Neo | MermaidThemeId::NeoDark
            ),
            Self::ExtendedWithoutReduxDark => matches!(
                theme,
                MermaidThemeId::Neo
                    | MermaidThemeId::NeoDark
                    | MermaidThemeId::Redux
                    | MermaidThemeId::ReduxColor
                    | MermaidThemeId::ReduxDarkColor
            ),
            Self::ExtendedWithoutReduxVariants => matches!(
                theme,
                MermaidThemeId::Neo
                    | MermaidThemeId::NeoDark
                    | MermaidThemeId::ReduxColor
                    | MermaidThemeId::ReduxDarkColor
            ),
            Self::ExtendedWithoutReduxDarkVariants => matches!(
                theme,
                MermaidThemeId::Neo
                    | MermaidThemeId::NeoDark
                    | MermaidThemeId::Redux
                    | MermaidThemeId::ReduxColor
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ThemeVariableDependencyTransform {
    Copy,
    Invert,
    AdjustHsl {
        hue: f64,
        saturation: f64,
        lightness: f64,
    },
    Darken(f64),
    Lighten(f64),
    MkBorder,
    DarkenWhenDarkMode(f64),
}

macro_rules! copied_theme_dependency {
    ($scope:ident, $source:literal, $target:literal) => {
        ThemeVariableDependency::copied(ThemeVariableDependencyScope::$scope, $source, $target)
    };
}

macro_rules! assigned_theme_dependency {
    ($scope:ident, $source:literal, $target:literal) => {
        ThemeVariableDependency::assigned(ThemeVariableDependencyScope::$scope, $source, $target)
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeVariableDependencyAssignment {
    IfMissing,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeVariableDependencyCondition {
    Always,
    SourceExplicit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ThemeVariableDependency {
    scope: ThemeVariableDependencyScope,
    source: &'static str,
    target: &'static str,
    transform: ThemeVariableDependencyTransform,
    assignment: ThemeVariableDependencyAssignment,
    condition: ThemeVariableDependencyCondition,
}

impl ThemeVariableDependency {
    const fn copied(
        scope: ThemeVariableDependencyScope,
        source: &'static str,
        target: &'static str,
    ) -> Self {
        Self {
            scope,
            source,
            target,
            transform: ThemeVariableDependencyTransform::Copy,
            assignment: ThemeVariableDependencyAssignment::IfMissing,
            condition: ThemeVariableDependencyCondition::Always,
        }
    }

    const fn assigned(
        scope: ThemeVariableDependencyScope,
        source: &'static str,
        target: &'static str,
    ) -> Self {
        Self {
            scope,
            source,
            target,
            transform: ThemeVariableDependencyTransform::Copy,
            assignment: ThemeVariableDependencyAssignment::Always,
            condition: ThemeVariableDependencyCondition::Always,
        }
    }

    const fn copied_when_source_explicit(
        scope: ThemeVariableDependencyScope,
        source: &'static str,
        target: &'static str,
    ) -> Self {
        Self {
            scope,
            source,
            target,
            transform: ThemeVariableDependencyTransform::Copy,
            assignment: ThemeVariableDependencyAssignment::IfMissing,
            condition: ThemeVariableDependencyCondition::SourceExplicit,
        }
    }

    const fn transformed(
        scope: ThemeVariableDependencyScope,
        source: &'static str,
        target: &'static str,
        transform: ThemeVariableDependencyTransform,
    ) -> Self {
        Self {
            scope,
            source,
            target,
            transform,
            assignment: ThemeVariableDependencyAssignment::IfMissing,
            condition: ThemeVariableDependencyCondition::Always,
        }
    }
}

// Ordered dependencies from the pinned Mermaid theme programs. This ledger is consumed by both
// calculation and ownership propagation. Order and assignment mode are significant: Mermaid
// calculates through unconditional intermediate assignments before replaying explicit targets.
const THEME_VARIABLE_DEPENDENCIES: &[ThemeVariableDependency] = &[
    assigned_theme_dependency!(Default, "border1", "nodeBorder"),
    assigned_theme_dependency!(Default, "textColor", "titleColor"),
    assigned_theme_dependency!(Default, "labelBackground", "edgeLabelBackground"),
    assigned_theme_dependency!(Default, "border1", "actorBorder"),
    assigned_theme_dependency!(Default, "mainBkg", "actorBkg"),
    assigned_theme_dependency!(Default, "actorBorder", "actorLineColor"),
    assigned_theme_dependency!(Default, "actorBkg", "labelBoxBkgColor"),
    assigned_theme_dependency!(Default, "textColor", "signalColor"),
    assigned_theme_dependency!(Default, "textColor", "signalTextColor"),
    assigned_theme_dependency!(Default, "actorBorder", "labelBoxBorderColor"),
    assigned_theme_dependency!(Default, "actorTextColor", "labelTextColor"),
    assigned_theme_dependency!(Default, "actorTextColor", "loopTextColor"),
    assigned_theme_dependency!(Default, "border2", "noteBorderColor"),
    assigned_theme_dependency!(Default, "actorTextColor", "noteTextColor"),
    assigned_theme_dependency!(Default, "nodeBorder", "innerEndBackground"),
    assigned_theme_dependency!(Default, "lineColor", "specialStateColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Base,
        "background",
        "lineColor",
        ThemeVariableDependencyTransform::Invert,
    ),
    copied_theme_dependency!(Base, "primaryTextColor", "textColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Base,
        "primaryColor",
        "primaryBorderColor",
        ThemeVariableDependencyTransform::MkBorder,
    ),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Base,
        "secondaryColor",
        "secondaryBorderColor",
        ThemeVariableDependencyTransform::MkBorder,
    ),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Base,
        "tertiaryColor",
        "tertiaryBorderColor",
        ThemeVariableDependencyTransform::MkBorder,
    ),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Base,
        "tertiaryColor",
        "tertiaryTextColor",
        ThemeVariableDependencyTransform::Invert,
    ),
    assigned_theme_dependency!(Base, "primaryBorderColor", "gradientStart"),
    assigned_theme_dependency!(Base, "secondaryBorderColor", "gradientStop"),
    copied_theme_dependency!(Base, "primaryBorderColor", "nodeBorder"),
    copied_theme_dependency!(Base, "tertiaryTextColor", "titleColor"),
    copied_theme_dependency!(Base, "primaryColor", "taskBkgColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Base,
        "primaryColor",
        "activeTaskBkgColor",
        ThemeVariableDependencyTransform::Lighten(23.0),
    ),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Base,
        "secondaryColor",
        "edgeLabelBackground",
        ThemeVariableDependencyTransform::DarkenWhenDarkMode(30.0),
    ),
    copied_theme_dependency!(Base, "primaryBorderColor", "actorBorder"),
    copied_theme_dependency!(Base, "mainBkg", "actorBkg"),
    copied_theme_dependency!(Base, "primaryTextColor", "actorTextColor"),
    copied_theme_dependency!(Base, "actorBorder", "actorLineColor"),
    copied_theme_dependency!(Base, "actorBkg", "labelBoxBkgColor"),
    copied_theme_dependency!(Base, "textColor", "signalColor"),
    copied_theme_dependency!(Base, "textColor", "signalTextColor"),
    copied_theme_dependency!(Base, "actorBorder", "labelBoxBorderColor"),
    copied_theme_dependency!(Base, "actorTextColor", "labelTextColor"),
    copied_theme_dependency!(Base, "actorTextColor", "loopTextColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Base,
        "secondaryColor",
        "activationBorderColor",
        ThemeVariableDependencyTransform::Darken(10.0),
    ),
    copied_theme_dependency!(Base, "secondaryColor", "activationBkgColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Base,
        "lineColor",
        "sequenceNumberColor",
        ThemeVariableDependencyTransform::Invert,
    ),
    copied_theme_dependency!(Base, "lineColor", "transitionColor"),
    copied_theme_dependency!(Base, "textColor", "transitionLabelColor"),
    copied_theme_dependency!(Base, "mainBkg", "stateBkg"),
    copied_theme_dependency!(Base, "stateBkg", "labelBackgroundColor"),
    copied_theme_dependency!(Base, "background", "compositeBackground"),
    copied_theme_dependency!(Base, "mainBkg", "compositeTitleBackground"),
    assigned_theme_dependency!(Base, "nodeBorder", "innerEndBackground"),
    assigned_theme_dependency!(Base, "lineColor", "specialStateColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::BaseAndNeoFamily,
        "noteBkgColor",
        "noteBorderColor",
        ThemeVariableDependencyTransform::MkBorder,
    ),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::ExtendedLight,
        "primaryColor",
        "secondaryColor",
        ThemeVariableDependencyTransform::AdjustHsl {
            hue: -120.0,
            saturation: 0.0,
            lightness: 0.0,
        },
    ),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Extended,
        "background",
        "lineColor",
        ThemeVariableDependencyTransform::Invert,
    ),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::ExtendedLight,
        "background",
        "arrowheadColor",
        ThemeVariableDependencyTransform::Invert,
    ),
    copied_theme_dependency!(Extended, "primaryColor", "nodeBkg"),
    copied_theme_dependency!(Extended, "tertiaryTextColor", "titleColor"),
    copied_theme_dependency!(NeoDark, "border1", "nodeBorder"),
    copied_theme_dependency!(Extended, "lineColor", "defaultLinkColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Extended,
        "secondaryColor",
        "edgeLabelBackground",
        ThemeVariableDependencyTransform::DarkenWhenDarkMode(30.0),
    ),
    copied_theme_dependency!(Extended, "mainBkg", "actorBkg"),
    copied_theme_dependency!(NeoFamily, "primaryBorderColor", "actorBorder"),
    copied_theme_dependency!(Extended, "primaryTextColor", "actorTextColor"),
    copied_theme_dependency!(
        ExtendedWithoutReduxDarkVariants,
        "actorBorder",
        "actorLineColor"
    ),
    copied_theme_dependency!(Extended, "actorBkg", "labelBoxBkgColor"),
    copied_theme_dependency!(ExtendedWithoutReduxDarkVariants, "textColor", "signalColor"),
    copied_theme_dependency!(Extended, "textColor", "signalTextColor"),
    copied_theme_dependency!(
        ExtendedWithoutReduxDarkVariants,
        "actorBorder",
        "labelBoxBorderColor"
    ),
    copied_theme_dependency!(Extended, "actorTextColor", "labelTextColor"),
    copied_theme_dependency!(Extended, "actorTextColor", "loopTextColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Extended,
        "secondaryColor",
        "activationBorderColor",
        ThemeVariableDependencyTransform::Darken(10.0),
    ),
    copied_theme_dependency!(Extended, "secondaryColor", "activationBkgColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Extended,
        "lineColor",
        "sequenceNumberColor",
        ThemeVariableDependencyTransform::Invert,
    ),
    assigned_theme_dependency!(Extended, "lineColor", "archEdgeColor"),
    assigned_theme_dependency!(Extended, "lineColor", "archEdgeArrowColor"),
    copied_theme_dependency!(Extended, "mainBkg", "personBkg"),
    copied_theme_dependency!(Extended, "lineColor", "transitionColor"),
    copied_theme_dependency!(Extended, "textColor", "transitionLabelColor"),
    copied_theme_dependency!(Extended, "mainBkg", "stateBkg"),
    copied_theme_dependency!(Extended, "stateBkg", "labelBackgroundColor"),
    copied_theme_dependency!(
        ExtendedWithoutReduxDark,
        "background",
        "compositeBackground"
    ),
    copied_theme_dependency!(
        ExtendedWithoutReduxVariants,
        "mainBkg",
        "compositeTitleBackground"
    ),
    assigned_theme_dependency!(Extended, "nodeBorder", "innerEndBackground"),
    assigned_theme_dependency!(Extended, "lineColor", "specialStateColor"),
    copied_theme_dependency!(ExtendedDark, "primaryColor", "taskBkgColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::ExtendedDark,
        "primaryColor",
        "activeTaskBkgColor",
        ThemeVariableDependencyTransform::Lighten(23.0),
    ),
    copied_theme_dependency!(ExtendedDark, "primaryColor", "requirementBackground"),
    copied_theme_dependency!(Extended, "lineColor", "relationColor"),
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::Extended,
        "secondaryColor",
        "relationLabelBackground",
        ThemeVariableDependencyTransform::DarkenWhenDarkMode(30.0),
    ),
    copied_theme_dependency!(Extended, "primaryColor", "tagLabelBackground"),
    copied_theme_dependency!(Extended, "secondaryColor", "commitLabelBackground"),
    copied_theme_dependency!(ExtendedDark, "primaryColor", "pie1"),
    // Unlike theme-default, every other pinned theme creates its Venn variables during
    // calculate(). Keep Default's constructor snapshot stable while replaying the non-Default
    // nullish assignments and their ownership edges.
    copied_theme_dependency!(LegacyNonDefault, "titleColor", "vennTitleTextColor"),
    copied_theme_dependency!(LegacyNonDefault, "textColor", "vennSetTextColor"),
    copied_theme_dependency!(LegacyNonDefault, "primaryTextColor", "stateLabelColor"),
    ThemeVariableDependency::copied_when_source_explicit(
        ThemeVariableDependencyScope::LegacyNonDefault,
        "stateBkg",
        "stateLabelColor",
    ),
];

/// Pure-Rust execution contract for one pinned Mermaid theme class.
///
/// The generated artifact owns exact release snapshots; this descriptor owns the operations and
/// input dependencies still used by theme programs that have not entered staged execution.
#[derive(Debug, Clone, Copy)]
struct ThemeProgram {
    id: MermaidThemeId,
    kind: ThemeProgramKind,
    dependencies: ThemeDependencyGraph,
    evaluated_color_inputs: &'static [&'static str],
}

const DEFAULT_COLOR_INPUTS: &[&str] = &[
    "primaryColor",
    "secondaryColor",
    "cScale0",
    "cScale1",
    "git0",
    "git1",
    "quadrant1Fill",
];
const BASE_COLOR_INPUTS: &[&str] = &[
    "primaryColor",
    "secondaryColor",
    "tertiaryColor",
    "background",
    "cScale0",
    "cScale1",
    "git0",
    "git1",
    "quadrant1Fill",
];
const REDUX_COLOR_INPUTS: &[&str] = &[
    "primaryColor",
    "secondaryColor",
    "tertiaryColor",
    "background",
    "git0",
    "git1",
    "quadrant1Fill",
];
const THEME_PROGRAMS: &[ThemeProgram] = &[
    ThemeProgram::new(
        MermaidThemeId::Default,
        ThemeProgramKind::Default,
        ThemeDependencyGraph::Default,
        DEFAULT_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::Base,
        ThemeProgramKind::Base,
        ThemeDependencyGraph::Base,
        BASE_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::Dark,
        ThemeProgramKind::Staged(staged::StagedProgram::Dark),
        ThemeDependencyGraph::None,
        &[],
    ),
    ThemeProgram::new(
        MermaidThemeId::Forest,
        ThemeProgramKind::Staged(staged::StagedProgram::Forest),
        ThemeDependencyGraph::None,
        &[],
    ),
    ThemeProgram::new(
        MermaidThemeId::Neutral,
        ThemeProgramKind::Staged(staged::StagedProgram::Neutral),
        ThemeDependencyGraph::None,
        &[],
    ),
    ThemeProgram::new(
        MermaidThemeId::Neo,
        ThemeProgramKind::Extended,
        ThemeDependencyGraph::None,
        BASE_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::NeoDark,
        ThemeProgramKind::Extended,
        ThemeDependencyGraph::DarkenedScale,
        BASE_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::Redux,
        ThemeProgramKind::Extended,
        ThemeDependencyGraph::None,
        REDUX_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::ReduxDark,
        ThemeProgramKind::Extended,
        ThemeDependencyGraph::DarkenedScaleAndGit,
        BASE_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::ReduxColor,
        ThemeProgramKind::Extended,
        ThemeDependencyGraph::None,
        BASE_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::ReduxDarkColor,
        ThemeProgramKind::Extended,
        ThemeDependencyGraph::DynamicGit,
        BASE_COLOR_INPUTS,
    ),
];

impl ThemeProgram {
    const fn new(
        id: MermaidThemeId,
        kind: ThemeProgramKind,
        dependencies: ThemeDependencyGraph,
        evaluated_color_inputs: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            kind,
            dependencies,
            evaluated_color_inputs,
        }
    }

    fn resolve(id: MermaidThemeId) -> &'static Self {
        THEME_PROGRAMS
            .iter()
            .find(|program| program.id == id)
            .expect("every MermaidThemeId must have a theme program")
    }

    const fn staged_program(self) -> Option<staged::StagedProgram> {
        match self.kind {
            ThemeProgramKind::Staged(program) => Some(program),
            ThemeProgramKind::Default | ThemeProgramKind::Base | ThemeProgramKind::Extended => None,
        }
    }

    fn variable_dependencies(self) -> impl Iterator<Item = &'static ThemeVariableDependency> {
        THEME_VARIABLE_DEPENDENCIES
            .iter()
            .filter(move |dependency| dependency.scope.includes(self.id))
    }

    fn prepared_constructor(self) -> &'static Map<String, Value> {
        generated_theme_artifact()
            .prepared_constructors
            .get(self.id.as_str())
            .and_then(Value::as_object)
            .unwrap_or_else(|| {
                panic!(
                    "generated theme artifact is missing prepared constructor `{}`",
                    self.id
                )
            })
    }

    fn resolved_without_overrides(self) -> &'static Map<String, Value> {
        generated_theme_artifact()
            .resolved_without_overrides
            .get(self.id.as_str())
            .and_then(Value::as_object)
            .unwrap_or_else(|| {
                panic!(
                    "generated theme artifact is missing resolved no-override `{}`",
                    self.id
                )
            })
    }

    fn resolved_dark_mode_true(self) -> &'static Map<String, Value> {
        generated_theme_artifact()
            .resolved_dark_mode_true
            .get(self.id.as_str())
            .and_then(Value::as_object)
            .unwrap_or_else(|| {
                panic!(
                    "generated theme artifact is missing darkMode=true `{}`",
                    self.id
                )
            })
    }

    fn calculation_snapshot(self, explicit: &Map<String, Value>) -> &'static Map<String, Value> {
        if explicit.get("darkMode").is_some_and(is_js_truthy) {
            self.resolved_dark_mode_true()
        } else {
            self.resolved_without_overrides()
        }
    }

    fn exact_snapshot(self, explicit: &Map<String, Value>) -> Option<&'static Map<String, Value>> {
        if explicit
            .keys()
            .all(|key| matches!(key.as_str(), "darkMode" | "fontFamily" | "fontSize"))
        {
            Some(self.calculation_snapshot(explicit))
        } else {
            None
        }
    }

    fn normalize_overrides(self, raw: Map<String, Value>) -> Map<String, Value> {
        // Mermaid's public initialize/config path runs themeVariables through assignWithDepth
        // before Theme.calculate(). Object-valued source fields whose value is null are skipped
        // there, so they never participate in either explicit overlay or explicit replay.
        raw.into_iter()
            .filter(|(_, value)| !value.is_null())
            .collect()
    }

    fn validate_evaluated_inputs(self, explicit: &Map<String, Value>) -> Result<(), ColorError> {
        for key in self.evaluated_color_inputs {
            let Some(value) = explicit.get(*key) else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            let Value::String(color) = value else {
                return Err(ColorError::UnsupportedFormat {
                    input: value.to_string(),
                });
            };
            theme_color::ThemeColor::parse(color)?;
        }
        Ok(())
    }

    fn execute_legacy(self, config: &mut MermaidConfig) -> Result<(), ColorError> {
        match self.kind {
            ThemeProgramKind::Default => apply_default_theme_defaults(config),
            ThemeProgramKind::Base => apply_base_theme_defaults(config),
            ThemeProgramKind::Staged(_) => {
                unreachable!("staged themes do not enter legacy theme execution")
            }
            ThemeProgramKind::Extended => apply_snapshot_theme_defaults(config, self.id),
        }
    }

    fn propagate_derived_ownership(
        self,
        secondary_color_missing: bool,
        tertiary_color_missing: bool,
        explicit_dependency_variables: &[&str],
        config: &mut MermaidConfig,
    ) {
        match self.dependencies {
            ThemeDependencyGraph::Base => {
                if secondary_color_missing {
                    config.propagate_theme_variable_ownership("primaryColor", "secondaryColor");
                }
                if tertiary_color_missing {
                    config.propagate_theme_variable_ownership("primaryColor", "tertiaryColor");
                }
                propagate_standard_color_scale_ownership(config);
            }
            ThemeDependencyGraph::DarkenedScale | ThemeDependencyGraph::DarkenedScaleAndGit => {
                propagate_standard_color_scale_ownership(config);
            }
            ThemeDependencyGraph::None
            | ThemeDependencyGraph::Default
            | ThemeDependencyGraph::DynamicGit => {}
        }

        let dependencies = self.variable_dependencies().collect::<Vec<_>>();
        let mut calculated_origins = BTreeMap::new();
        for dependency in &dependencies {
            for variable in [dependency.source, dependency.target] {
                calculated_origins.entry(variable).or_insert_with(|| {
                    config
                        .config_path_overrides_typed_default(&format!("themeVariables.{variable}"))
                        .then_some(variable)
                });
            }
        }

        let mut winning_assignments = BTreeMap::new();
        for (index, dependency) in dependencies.iter().enumerate() {
            if dependency.condition == ThemeVariableDependencyCondition::SourceExplicit
                && !explicit_dependency_variables.contains(&dependency.source)
            {
                continue;
            }
            if dependency.assignment == ThemeVariableDependencyAssignment::IfMissing
                && explicit_dependency_variables.contains(&dependency.target)
            {
                continue;
            }
            let source_origin = calculated_origins.get(dependency.source).copied().flatten();
            calculated_origins.insert(dependency.target, source_origin);
            winning_assignments.insert(dependency.target, index);
        }

        for (index, dependency) in dependencies.iter().enumerate() {
            if explicit_dependency_variables.contains(&dependency.target)
                || winning_assignments.get(dependency.target) != Some(&index)
            {
                continue;
            }
            if let Some(source_origin) =
                calculated_origins.get(dependency.target).copied().flatten()
            {
                config.propagate_theme_variable_ownership(source_origin, dependency.target);
            }
        }
    }

    fn apply_dependency_graph(
        self,
        explicit: &Map<String, Value>,
        calculated: &mut Map<String, Value>,
    ) -> Result<(), ColorError> {
        match self.dependencies {
            ThemeDependencyGraph::None => Ok(()),
            ThemeDependencyGraph::Default => apply_default_theme_dependencies(explicit, calculated),
            ThemeDependencyGraph::Base => apply_base_theme_dependencies(explicit, calculated),
            ThemeDependencyGraph::DarkenedScale => {
                apply_darkened_scale_dependencies(explicit, calculated)
            }
            ThemeDependencyGraph::DarkenedScaleAndGit => {
                apply_darkened_scale_dependencies(explicit, calculated)?;
                apply_dynamic_git_dependencies(explicit, calculated)
            }
            ThemeDependencyGraph::DynamicGit => {
                apply_dynamic_git_dependencies(explicit, calculated)
            }
        }?;
        apply_theme_variable_dependencies(self, explicit, calculated)?;
        Ok(())
    }
}

fn apply_theme_variable_dependencies(
    program: ThemeProgram,
    explicit: &Map<String, Value>,
    calculated: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    for dependency in program.variable_dependencies() {
        if dependency.condition == ThemeVariableDependencyCondition::SourceExplicit
            && !explicit.contains_key(dependency.source)
        {
            continue;
        }
        if dependency.assignment == ThemeVariableDependencyAssignment::IfMissing
            && explicit.contains_key(dependency.target)
        {
            continue;
        }
        let Some(source) = calculated.get(dependency.source) else {
            continue;
        };
        let value = match dependency.transform {
            ThemeVariableDependencyTransform::Copy => source.clone(),
            ThemeVariableDependencyTransform::Invert => {
                let source = required_color(calculated, dependency.source)?;
                Value::String(theme_color::invert(&source)?)
            }
            ThemeVariableDependencyTransform::AdjustHsl {
                hue,
                saturation,
                lightness,
            } => {
                let source = required_color(calculated, dependency.source)?;
                Value::String(theme_color::adjust(
                    &source,
                    ColorAdjustment::hsl(hue, saturation, lightness),
                )?)
            }
            ThemeVariableDependencyTransform::Darken(amount) => {
                let source = required_color(calculated, dependency.source)?;
                Value::String(theme_color::darken(&source, amount)?)
            }
            ThemeVariableDependencyTransform::Lighten(amount) => {
                let source = required_color(calculated, dependency.source)?;
                Value::String(theme_color::lighten(&source, amount)?)
            }
            ThemeVariableDependencyTransform::MkBorder => {
                let source = required_color(calculated, dependency.source)?;
                Value::String(mk_border(
                    &source,
                    calculated.get("darkMode").is_some_and(is_js_truthy),
                )?)
            }
            ThemeVariableDependencyTransform::DarkenWhenDarkMode(amount) => {
                let source = required_color(calculated, dependency.source)?;
                if calculated.get("darkMode").is_some_and(is_js_truthy) {
                    Value::String(theme_color::darken(&source, amount)?)
                } else {
                    Value::String(source)
                }
            }
        };
        calculated.insert(dependency.target.to_string(), value);
    }
    Ok(())
}

fn propagate_theme_variable_to_scales(
    config: &mut MermaidConfig,
    source: &str,
    indices: impl IntoIterator<Item = usize>,
) {
    for index in indices {
        config.propagate_theme_variable_ownership(source, &format!("cScale{index}"));
    }
}

fn propagate_standard_color_scale_ownership(config: &mut MermaidConfig) {
    propagate_theme_variable_to_scales(config, "primaryColor", [0]);
    propagate_theme_variable_to_scales(config, "secondaryColor", [1]);
    propagate_theme_variable_to_scales(config, "tertiaryColor", [2]);
    propagate_theme_variable_to_scales(config, "primaryColor", 3..12);
}

fn get_truthy_string(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn is_js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn required_color(map: &Map<String, Value>, key: &str) -> Result<String, ColorError> {
    get_truthy_string(map, key).ok_or_else(|| ColorError::UnsupportedFormat {
        input: map
            .get(key)
            .map(Value::to_string)
            .unwrap_or_else(|| format!("missing theme color `{key}`")),
    })
}

fn value_is_missing(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).is_none_or(|value| !is_js_truthy(value))
}

fn set_if_missing(map: &mut Map<String, Value>, key: &str, value: Value) {
    if value_is_missing(map, key) {
        map.insert(key.to_string(), value);
    }
}

fn set_string_if_missing(map: &mut Map<String, Value>, key: &str, value: impl Into<String>) {
    set_if_missing(map, key, Value::String(value.into()));
}

fn set_finite_number_if_missing(map: &mut Map<String, Value>, key: &str, value: f64) {
    if let Some(number) = serde_json::Number::from_f64(value) {
        set_if_missing(map, key, Value::Number(number));
    }
}

fn set_derived_string_unless_explicit(
    map: &mut Map<String, Value>,
    explicit: &Map<String, Value>,
    key: &str,
    value: impl Into<String>,
) {
    if !explicit.contains_key(key) {
        map.insert(key.to_string(), Value::String(value.into()));
    }
}

fn theme_variables_map(config: &MermaidConfig) -> Map<String, Value> {
    match config.as_value().get("themeVariables") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    }
}

fn generated_theme_artifact() -> &'static GeneratedThemeArtifact {
    GENERATED_THEME_ARTIFACT.get_or_init(|| {
        let artifact: GeneratedThemeArtifact =
            serde_json::from_str(include_str!("generated/theme_variables_11_16_1.json"))
                .expect("generated Mermaid theme artifact JSON is valid");
        assert_generated_theme_provenance(artifact.schema_version, &artifact.provenance);
        for program in THEME_PROGRAMS {
            assert!(
                artifact
                    .prepared_constructors
                    .get(program.id.as_str())
                    .is_some_and(Value::is_object)
            );
            assert!(
                artifact
                    .resolved_without_overrides
                    .get(program.id.as_str())
                    .is_some_and(Value::is_object)
            );
            assert!(
                artifact
                    .resolved_dark_mode_true
                    .get(program.id.as_str())
                    .is_some_and(Value::is_object)
            );
        }
        artifact
    })
}

fn assert_generated_theme_provenance(schema_version: u32, provenance: &GeneratedThemeProvenance) {
    assert_eq!(schema_version, THEME_ARTIFACT_SCHEMA_VERSION);
    assert_eq!(
        provenance.mermaid_version,
        crate::baseline::PINNED_MERMAID_BASELINE_VERSION
    );
    assert_eq!(
        provenance.mermaid_source_tag,
        crate::baseline::PINNED_MERMAID_BASELINE_TAG
    );
    assert_eq!(
        provenance.generator,
        "cargo run -p xtask -- gen-theme-snapshot"
    );
    assert_eq!(provenance.mermaid_source_commit.len(), 40);
    assert_eq!(provenance.mermaid_package_sha256.len(), 64);
}

#[cfg(test)]
fn generated_theme_oracles() -> &'static GeneratedThemeOracles {
    GENERATED_THEME_ORACLES.get_or_init(|| {
        let artifact: GeneratedThemeOracles =
            serde_json::from_str(include_str!("generated/theme_oracles_11_16_1.json"))
                .expect("generated Mermaid theme oracle JSON is valid");
        assert_generated_theme_provenance(artifact.schema_version, &artifact.provenance);
        assert_eq!(
            &artifact.provenance,
            &generated_theme_artifact().provenance,
            "runtime and test-only theme artifacts must share one provenance"
        );
        assert_eq!(
            artifact.oracle_cases.len(),
            MermaidThemeId::ALL.len() * 5 + 12
        );
        assert!(!artifact.stage_oracle_cases.is_empty());
        artifact
    })
}

fn merge_theme_variable_defaults(target: &mut Map<String, Value>, defaults: &Map<String, Value>) {
    for (key, default_value) in defaults {
        match (target.get_mut(key), default_value) {
            (Some(Value::Object(target_map)), Value::Object(default_map)) => {
                merge_theme_variable_defaults(target_map, default_map);
            }
            (Some(Value::Null), _) => {
                target.insert(key.clone(), default_value.clone());
            }
            (Some(Value::String(current)), _) if current.trim().is_empty() => {
                target.insert(key.clone(), default_value.clone());
            }
            (None, _) => {
                target.insert(key.clone(), default_value.clone());
            }
            _ => {}
        }
    }
}

fn finish_theme_defaults(
    config: &mut MermaidConfig,
    theme: MermaidThemeId,
    tv: Map<String, Value>,
) -> Result<(), ColorError> {
    let explicit = theme_variables_map(config);
    let resolved = resolve_legacy_theme_variables(theme, explicit, tv)?;
    config.set_value_preserving_theme_compatibility("themeVariables", Value::Object(resolved));
    Ok(())
}

fn resolve_legacy_theme_variables(
    theme: MermaidThemeId,
    explicit: Map<String, Value>,
    mut calculated: Map<String, Value>,
) -> Result<Map<String, Value>, ColorError> {
    let program = ThemeProgram::resolve(theme);
    let has_user_theme_variables = !explicit.is_empty();

    if let Some(snapshot) = program.exact_snapshot(&explicit) {
        calculated = snapshot.clone();
    } else {
        merge_theme_variable_defaults(&mut calculated, program.calculation_snapshot(&explicit));
        program.apply_dependency_graph(&explicit, &mut calculated)?;
    }

    // `theme-default` constructs and updates its color scale before calculate() applies
    // overrides. A second update darkens the already-created cScale values, while peer and
    // inverse values retain their first-pass values. Restore the generated no-override palette
    // baseline before replaying explicit values; this is why a font-only override must not change
    // Radar/Kanban/Mindmap/Timeline colors.
    if has_user_theme_variables && theme == MermaidThemeId::Default {
        restore_default_baseline_palette(&mut calculated, program.resolved_without_overrides());
    }
    calculated.extend(explicit);
    Ok(calculated)
}

fn restore_default_baseline_palette(
    target: &mut Map<String, Value>,
    baseline: &Map<String, Value>,
) {
    for prefix in [
        "cScale",
        "cScalePeer",
        "cScaleInv",
        "cScaleLabel",
        "surface",
        "surfacePeer",
    ] {
        for index in 0..12 {
            let key = format!("{prefix}{index}");
            if let Some(value) = baseline.get(&key) {
                target.insert(key, value.clone());
            }
        }
    }
    for index in 1..=12 {
        let key = format!("pie{index}");
        if let Some(value) = baseline.get(&key) {
            target.insert(key, value.clone());
        }
    }
    if let Some(value) = baseline.get("scaleLabelColor") {
        target.insert("scaleLabelColor".to_string(), value.clone());
    }
}

fn mermaid_default_font_family() -> Value {
    Value::String("\"trebuchet ms\", verdana, arial, sans-serif".to_string())
}

fn mk_border(color: &str, dark_mode: bool) -> Result<String, ColorError> {
    theme_color::adjust(
        color,
        ColorAdjustment::hsl(0.0, -40.0, if dark_mode { 10.0 } else { -10.0 }),
    )
}

fn ensure_gradient_theme_defaults(tv: &mut Map<String, Value>) {
    let primary_border_color =
        get_truthy_string(tv, "primaryBorderColor").unwrap_or_else(|| "#9370DB".to_string());
    let secondary_border_color = get_truthy_string(tv, "secondaryBorderColor")
        .unwrap_or_else(|| primary_border_color.clone());

    set_if_missing(tv, "useGradient", Value::Bool(true));
    set_if_missing(tv, "gradientStart", Value::String(primary_border_color));
    set_if_missing(tv, "gradientStop", Value::String(secondary_border_color));
}

fn ensure_xychart_theme_defaults(tv: &mut Map<String, Value>, default_palette: &str) {
    let background = get_truthy_string(tv, "background").unwrap_or_else(|| "white".to_string());
    let primary_text = get_truthy_string(tv, "primaryTextColor")
        .or_else(|| get_truthy_string(tv, "textColor"))
        .unwrap_or_else(|| "#333".to_string());

    let mut xy = match tv.get("xyChart") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };

    set_if_missing(
        &mut xy,
        "backgroundColor",
        Value::String(background.clone()),
    );
    for key in [
        "titleColor",
        "dataLabelColor",
        "xAxisTitleColor",
        "xAxisLabelColor",
        "xAxisTickColor",
        "xAxisLineColor",
        "yAxisTitleColor",
        "yAxisLabelColor",
        "yAxisTickColor",
        "yAxisLineColor",
    ] {
        set_if_missing(&mut xy, key, Value::String(primary_text.clone()));
    }
    set_if_missing(
        &mut xy,
        "plotColorPalette",
        Value::String(default_palette.to_string()),
    );

    tv.insert("xyChart".to_string(), Value::Object(xy));
}

fn apply_quadrant_theme_defaults(
    tv: &mut Map<String, Value>,
    fill_base: &str,
    text_base: &str,
    border_base: &str,
) -> Result<(), ColorError> {
    set_string_if_missing(tv, "quadrant1Fill", fill_base);
    for (key, adjustment) in [
        ("quadrant2Fill", 5.0),
        ("quadrant3Fill", 10.0),
        ("quadrant4Fill", 15.0),
    ] {
        set_string_if_missing(
            tv,
            key,
            theme_color::adjust(
                fill_base,
                ColorAdjustment::rgb(adjustment, adjustment, adjustment),
            )?,
        );
    }

    set_string_if_missing(tv, "quadrant1TextFill", text_base);
    for (key, adjustment) in [
        ("quadrant2TextFill", -5.0),
        ("quadrant3TextFill", -10.0),
        ("quadrant4TextFill", -15.0),
    ] {
        set_string_if_missing(
            tv,
            key,
            theme_color::adjust(
                text_base,
                ColorAdjustment::rgb(adjustment, adjustment, adjustment),
            )?,
        );
    }

    // Upstream omits the amount argument. Khroma therefore preserves the hue and saturation but
    // serializes the lightness as NaN. Evaluate this even when the point color is explicit: the
    // JavaScript expression computes it during updateColors() before replaying explicit values.
    let quadrant1_fill = required_color(tv, "quadrant1Fill")?;
    let point_fill = if theme_color::is_dark(&quadrant1_fill)? {
        theme_color::lighten(&quadrant1_fill, f64::NAN)?
    } else {
        theme_color::darken(&quadrant1_fill, f64::NAN)?
    };
    set_string_if_missing(tv, "quadrantPointFill", point_fill);

    for key in [
        "quadrantPointTextFill",
        "quadrantXAxisTextFill",
        "quadrantYAxisTextFill",
        "quadrantTitleFill",
    ] {
        set_string_if_missing(tv, key, text_base);
    }
    for key in [
        "quadrantInternalBorderStrokeFill",
        "quadrantExternalBorderStrokeFill",
    ] {
        set_string_if_missing(tv, key, border_base);
    }
    Ok(())
}

fn apply_current_quadrant_theme_defaults(tv: &mut Map<String, Value>) -> Result<(), ColorError> {
    let fill_base = required_color(tv, "primaryColor")?;
    let text_base = required_color(tv, "primaryTextColor")?;
    let border_base = required_color(tv, "primaryBorderColor")?;
    apply_quadrant_theme_defaults(tv, &fill_base, &text_base, &border_base)
}

fn discard_non_explicit_quadrant_snapshot_values(
    tv: &mut Map<String, Value>,
    explicit: &Map<String, Value>,
) {
    for key in [
        "quadrant1Fill",
        "quadrant2Fill",
        "quadrant3Fill",
        "quadrant4Fill",
        "quadrant1TextFill",
        "quadrant2TextFill",
        "quadrant3TextFill",
        "quadrant4TextFill",
        "quadrantPointFill",
        "quadrantPointTextFill",
        "quadrantXAxisTextFill",
        "quadrantYAxisTextFill",
        "quadrantInternalBorderStrokeFill",
        "quadrantExternalBorderStrokeFill",
        "quadrantTitleFill",
    ] {
        if !explicit.contains_key(key) {
            tv.remove(key);
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ColorTransform {
    Darken(f64),
    Lighten(f64),
}

impl ColorTransform {
    fn apply(self, color: &str) -> Result<String, ColorError> {
        match self {
            Self::Darken(amount) => theme_color::darken(color, amount),
            Self::Lighten(amount) => theme_color::lighten(color, amount),
        }
    }
}

fn validate_explicit_git_transforms(
    tv: &Map<String, Value>,
    explicit: &Map<String, Value>,
    transform: ColorTransform,
) -> Result<(), ColorError> {
    for index in 0..8 {
        let key = format!("git{index}");
        if explicit.contains_key(&key) {
            transform.apply(&required_color(tv, &key)?)?;
        }
    }
    Ok(())
}

fn apply_single_pass_git_palette(
    tv: &mut Map<String, Value>,
    explicit: &Map<String, Value>,
    bases: [String; 8],
    transform: ColorTransform,
) -> Result<(), ColorError> {
    for (index, base) in bases.into_iter().enumerate() {
        let git_key = format!("git{index}");
        let source = if explicit.contains_key(&git_key) {
            required_color(tv, &git_key)?
        } else {
            base
        };
        let transformed = transform.apply(&source)?;
        if !explicit.contains_key(&git_key) {
            tv.insert(git_key, Value::String(transformed.clone()));
        }

        let inverse_key = format!("gitInv{index}");
        if !explicit.contains_key(&inverse_key) {
            tv.insert(
                inverse_key,
                Value::String(theme_color::invert(&transformed)?),
            );
        }
    }
    Ok(())
}

fn calculated_or_fallback_value(
    explicit: &Map<String, Value>,
    key: &str,
    fallback: Value,
) -> Value {
    explicit
        .get(key)
        .filter(|value| is_js_truthy(value))
        .cloned()
        .unwrap_or(fallback)
}

fn apply_scale_label_dependencies(
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
    fallback: Value,
) {
    let scale_label = calculated_or_fallback_value(explicit, "scaleLabelColor", fallback);
    tv.insert("scaleLabelColor".to_string(), scale_label.clone());
    for index in 0..12 {
        let key = format!("cScaleLabel{index}");
        if !explicit.contains_key(&key) {
            tv.insert(key, scale_label.clone());
        }
    }
}

fn apply_default_theme_dependencies(
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    let primary = required_color(tv, "primaryColor")?;
    set_derived_string_unless_explicit(
        tv,
        explicit,
        "rowOdd",
        theme_color::lighten(&primary, 75.0)?,
    );
    set_derived_string_unless_explicit(
        tv,
        explicit,
        "rowEven",
        theme_color::lighten(&primary, 1.0)?,
    );
    Ok(())
}

fn apply_base_theme_dependencies(
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    let dark_mode = tv.get("darkMode").is_some_and(is_js_truthy);
    let main_bkg = required_color(tv, "mainBkg")?;
    let (row_odd, row_even) = if dark_mode {
        (
            theme_color::darken(&main_bkg, 5.0)?,
            theme_color::darken(&main_bkg, 10.0)?,
        )
    } else {
        (
            theme_color::lighten(&main_bkg, 75.0)?,
            theme_color::lighten(&main_bkg, 5.0)?,
        )
    };
    set_derived_string_unless_explicit(tv, explicit, "rowOdd", row_odd);
    set_derived_string_unless_explicit(tv, explicit, "rowEven", row_even);

    let multiplier = if dark_mode { -4.0 } else { -1.0 };
    for index in 0..5 {
        set_derived_string_unless_explicit(
            tv,
            explicit,
            &format!("surface{index}"),
            theme_color::adjust(
                &main_bkg,
                ColorAdjustment::hsl(180.0, -15.0, multiplier * (5 + index * 3) as f64),
            )?,
        );
        set_derived_string_unless_explicit(
            tv,
            explicit,
            &format!("surfacePeer{index}"),
            theme_color::adjust(
                &main_bkg,
                ColorAdjustment::hsl(180.0, -15.0, multiplier * (8 + index * 3) as f64),
            )?,
        );
    }

    let label_text = tv
        .get("labelTextColor")
        .cloned()
        .unwrap_or_else(|| Value::String(if dark_mode { "#eee" } else { "#333" }.to_string()));
    apply_scale_label_dependencies(explicit, tv, label_text);
    Ok(())
}

fn git_palette_bases(tv: &Map<String, Value>) -> Result<[String; 8], ColorError> {
    let primary = required_color(tv, "primaryColor")?;
    Ok([
        primary.clone(),
        required_color(tv, "secondaryColor")?,
        required_color(tv, "tertiaryColor")?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(-30.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(-60.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(-90.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(60.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(120.0, 0.0, 0.0))?,
    ])
}

fn apply_dynamic_git_dependencies(
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    let transform = if tv.get("darkMode").is_some_and(is_js_truthy) {
        ColorTransform::Lighten(25.0)
    } else {
        ColorTransform::Darken(25.0)
    };
    apply_single_pass_git_palette(tv, explicit, git_palette_bases(tv)?, transform)
}

fn darkened_scale_bases(tv: &Map<String, Value>) -> Result<[String; 12], ColorError> {
    let primary = required_color(tv, "primaryColor")?;
    Ok([
        primary.clone(),
        required_color(tv, "secondaryColor")?,
        required_color(tv, "tertiaryColor")?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(30.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(60.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(90.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(120.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(150.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(210.0, 0.0, 150.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(270.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(300.0, 0.0, 0.0))?,
        theme_color::adjust(&primary, ColorAdjustment::hsl(330.0, 0.0, 0.0))?,
    ])
}

fn apply_darkened_scale_dependencies(
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    let dark_mode = tv.get("darkMode").is_some_and(is_js_truthy);
    let transform = ColorTransform::Darken(if dark_mode { 75.0 } else { 25.0 });
    let bases = darkened_scale_bases(tv)?;

    for (index, base) in bases.into_iter().enumerate() {
        let scale_key = format!("cScale{index}");
        let source = get_truthy_string(explicit, &scale_key).unwrap_or(base);
        let scale = transform.apply(&source)?;
        if !explicit.contains_key(&scale_key) {
            tv.insert(scale_key, Value::String(scale.clone()));
        }
        set_derived_string_unless_explicit(
            tv,
            explicit,
            &format!("cScaleInv{index}"),
            theme_color::invert(&scale)?,
        );
        let peer = if dark_mode {
            theme_color::lighten(&scale, 10.0)?
        } else {
            theme_color::darken(&scale, 10.0)?
        };
        set_derived_string_unless_explicit(tv, explicit, &format!("cScalePeer{index}"), peer);
    }

    let label_text = tv
        .get("labelTextColor")
        .cloned()
        .unwrap_or_else(|| Value::String("#e0dfdf".to_string()));
    apply_scale_label_dependencies(explicit, tv, label_text);
    Ok(())
}

fn validate_explicit_operation(
    tv: &Map<String, Value>,
    explicit: &Map<String, Value>,
    key: &str,
    operation: impl FnOnce(&str) -> Result<String, ColorError>,
) -> Result<(), ColorError> {
    if explicit.contains_key(key) {
        operation(&required_color(tv, key)?)?;
    }
    Ok(())
}

fn replay_extended_theme_khroma_operations(
    theme: MermaidThemeId,
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    let dark_mode = tv.get("darkMode").is_some_and(is_js_truthy);
    match theme {
        MermaidThemeId::Neo | MermaidThemeId::Redux | MermaidThemeId::ReduxColor => {
            for key in ["secondaryColor", "tertiaryColor"] {
                validate_explicit_operation(tv, explicit, key, |color| {
                    mk_border(color, dark_mode)
                })?;
            }
        }
        MermaidThemeId::NeoDark => {
            validate_explicit_operation(tv, explicit, "secondaryColor", |color| {
                theme_color::darken(color, 10.0)
            })?;
            validate_explicit_operation(tv, explicit, "tertiaryColor", |color| {
                theme_color::darken(color, if dark_mode { 75.0 } else { 25.0 })
            })?;
        }
        _ => {}
    }

    let scale_policy = match theme {
        MermaidThemeId::Neo | MermaidThemeId::NeoDark | MermaidThemeId::ReduxDark => {
            Some(Some(ColorTransform::Darken(if dark_mode {
                75.0
            } else {
                25.0
            })))
        }
        MermaidThemeId::ReduxColor | MermaidThemeId::ReduxDarkColor => Some(None),
        _ => None,
    };
    if let Some(transform) = scale_policy {
        for index in 0..2 {
            let scale_key = format!("cScale{index}");
            if !explicit.contains_key(&scale_key) {
                continue;
            }
            let source = required_color(tv, &scale_key)?;
            let calculated = match transform {
                Some(transform) => transform.apply(&source)?,
                None => source,
            };

            let inverse_key = format!("cScaleInv{index}");
            if !explicit.contains_key(&inverse_key) {
                tv.insert(
                    inverse_key,
                    Value::String(theme_color::invert(&calculated)?),
                );
            }
            let peer_key = format!("cScalePeer{index}");
            if !explicit.contains_key(&peer_key) {
                let peer = if dark_mode {
                    theme_color::lighten(&calculated, 10.0)?
                } else {
                    theme_color::darken(&calculated, 10.0)?
                };
                tv.insert(peer_key, Value::String(peer));
            }
            if theme == MermaidThemeId::ReduxDarkColor {
                let label_key = format!("cScaleLabel{index}");
                if !explicit.contains_key(&label_key) {
                    tv.insert(
                        label_key,
                        Value::String(theme_color::darken(&calculated, 75.0)?),
                    );
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn apply_theme_defaults(config: &mut MermaidConfig) -> Result<(), ThemeResolutionError> {
    let requested = config.get_str("theme").unwrap_or("default");
    // Mermaid's raw configuration runtime falls back to `default` for unknown strings. Typed
    // Merman theme APIs use `MermaidThemeId::parse` and never enter this compatibility branch.
    let theme = MermaidThemeId::parse(requested).unwrap_or_default();
    let program = *ThemeProgram::resolve(theme);
    let raw = theme_variables_map(config);
    let explicit = program.normalize_overrides(raw);

    if let Some(staged_program) = program.staged_program() {
        let resolution =
            staged::Resolution::execute(staged_program, program.prepared_constructor(), &explicit)?;
        // Keep the staged path transactional: normalization affects compatibility ownership,
        // so defer that side effect until every evaluated assignment has succeeded.
        config.retain_normalized_theme_compatibility_variables(&explicit);
        resolution.materialize_into(config);
        return Ok(());
    }

    let mut resolved_config = config.clone();
    resolved_config.retain_normalized_theme_compatibility_variables(&explicit);
    program.validate_evaluated_inputs(&explicit)?;
    let secondary_color_missing = value_is_missing(&explicit, "secondaryColor");
    let tertiary_color_missing = value_is_missing(&explicit, "tertiaryColor");
    let explicit_dependency_variables = program
        .variable_dependencies()
        .flat_map(|dependency| [dependency.source, dependency.target])
        .filter(|variable| explicit.contains_key(*variable))
        .collect::<Vec<_>>();
    resolved_config
        .set_value_preserving_theme_compatibility("themeVariables", Value::Object(explicit));
    program.execute_legacy(&mut resolved_config)?;
    program.propagate_derived_ownership(
        secondary_color_missing,
        tertiary_color_missing,
        &explicit_dependency_variables,
        &mut resolved_config,
    );
    *config = resolved_config;
    Ok(())
}

pub(crate) fn materialize_source_selected_theme(
    site_config: &MermaidConfig,
    initialization_config: &MermaidConfig,
    source_config: &MermaidConfig,
) -> Result<Option<MermaidConfig>, ThemeResolutionError> {
    let Some(requested) = source_config.get_str("theme") else {
        return Ok(None);
    };
    if MermaidThemeId::parse(requested).is_err() {
        return Ok(None);
    }

    // Mermaid updates ordinary source config from the already materialized site config. A source
    // directive that selects a registered theme is the one exception: it rebuilds themeVariables
    // from the raw initialize inputs plus the source inputs, then materializes the selected theme.
    // Reconstruct from the unmaterialized site config so ownership is derived exactly once for the
    // newly selected theme instead of carrying derived paths from the previous site theme.
    let mut effective_config = site_config.clone();
    effective_config.deep_merge_explicit(source_config.as_value());

    let initial_theme_variables = initialization_config
        .as_value()
        .get("themeVariables")
        .filter(|value| is_js_truthy(value))
        .map(crate::config::clone_value_nonrecursive)
        .unwrap_or_else(|| Value::Object(Map::new()));
    let mut raw_theme_variables = MermaidConfig::from_value(initial_theme_variables);
    if let Some(source_theme_variables) = source_config.as_value().get("themeVariables") {
        raw_theme_variables.deep_merge(source_theme_variables);
    }
    effective_config.set_value_preserving_theme_compatibility(
        "themeVariables",
        crate::config::clone_value_nonrecursive(raw_theme_variables.as_value()),
    );
    apply_theme_defaults(&mut effective_config)?;
    Ok(Some(effective_config))
}

fn apply_snapshot_theme_defaults(
    config: &mut MermaidConfig,
    theme: MermaidThemeId,
) -> Result<(), ColorError> {
    let tv = theme_variables_map(config);
    if tv.is_empty() {
        return finish_theme_defaults(config, theme, tv);
    }

    let explicit = tv.clone();
    let mut resolved = tv;
    let program = ThemeProgram::resolve(theme);
    merge_theme_variable_defaults(&mut resolved, program.calculation_snapshot(&explicit));
    apply_extended_theme_visible_derivations(*program, &explicit, &mut resolved)?;
    finish_theme_defaults(config, theme, resolved)
}

fn apply_extended_theme_visible_derivations(
    program: ThemeProgram,
    explicit: &Map<String, Value>,
    tv: &mut Map<String, Value>,
) -> Result<(), ColorError> {
    let theme = program.id;
    if !matches!(
        theme,
        MermaidThemeId::Neo
            | MermaidThemeId::NeoDark
            | MermaidThemeId::Redux
            | MermaidThemeId::ReduxDark
            | MermaidThemeId::ReduxColor
            | MermaidThemeId::ReduxDarkColor
    ) {
        return Ok(());
    }

    replay_extended_theme_khroma_operations(theme, explicit, tv)?;

    // Mermaid's extended themes run `calculate(overrides)`: copy user base variables, update
    // derived colors, then re-apply explicit user keys. Keep generated snapshots as the default
    // source of truth, but recompute visible derived keys that current renderers consume.
    apply_theme_variable_dependencies(program, explicit, tv)?;

    for i in 0..8 {
        let git_key = format!("git{i}");
        let git_inv_key = format!("gitInv{i}");
        if explicit.contains_key(&git_key) && !explicit.contains_key(&git_inv_key) {
            let git = required_color(tv, &git_key)?;
            let inv = theme_color::invert(&git)?;
            tv.insert(git_inv_key, Value::String(inv));
        }
    }

    discard_non_explicit_quadrant_snapshot_values(tv, explicit);
    let quadrant_fill_base = if matches!(
        theme,
        MermaidThemeId::Neo | MermaidThemeId::Redux | MermaidThemeId::ReduxColor
    ) {
        "#ECECFE".to_string()
    } else {
        required_color(tv, "primaryColor")?
    };
    let quadrant_text_base = required_color(tv, "primaryTextColor")?;
    let quadrant_border_base = required_color(tv, "primaryBorderColor")?;
    apply_quadrant_theme_defaults(
        tv,
        &quadrant_fill_base,
        &quadrant_text_base,
        &quadrant_border_base,
    )?;
    Ok(())
}

fn apply_default_theme_defaults(config: &mut MermaidConfig) -> Result<(), ColorError> {
    let mut tv = theme_variables_map(config);
    let explicit_theme_variables = tv.clone();

    // Mermaid 11.16.1: `theme-default` constructor defaults and `updateColors()`.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-default.js`.
    let default_primary = "#ECECFF";
    let default_secondary = "#ffffde";
    let default_tertiary =
        theme_color::adjust(default_primary, ColorAdjustment::hsl(-160.0, 0.0, 0.0))?;
    let default_primary_border = mk_border(default_primary, false)?;
    let default_secondary_border = mk_border(default_secondary, false)?;

    set_if_missing(&mut tv, "background", Value::String("white".to_string()));
    set_if_missing(
        &mut tv,
        "primaryColor",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "secondaryColor",
        Value::String(default_secondary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "tertiaryColor",
        Value::String(default_tertiary.clone()),
    );

    set_if_missing(
        &mut tv,
        "primaryBorderColor",
        Value::String(default_primary_border.clone()),
    );
    set_if_missing(
        &mut tv,
        "secondaryBorderColor",
        Value::String(default_secondary_border.clone()),
    );
    set_if_missing(
        &mut tv,
        "tertiaryBorderColor",
        Value::String(mk_border(&default_tertiary, false)?),
    );

    set_if_missing(
        &mut tv,
        "primaryTextColor",
        Value::String("#131300".to_string()),
    );
    set_if_missing(
        &mut tv,
        "secondaryTextColor",
        Value::String("#000021".to_string()),
    );
    set_if_missing(
        &mut tv,
        "tertiaryTextColor",
        Value::String(theme_color::invert(&default_tertiary)?),
    );

    set_if_missing(
        &mut tv,
        "mainBkg",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "secondBkg",
        Value::String(default_secondary.to_string()),
    );
    set_if_missing(&mut tv, "lineColor", Value::String("#333333".to_string()));
    set_if_missing(&mut tv, "border1", Value::String("#9370DB".to_string()));
    set_if_missing(&mut tv, "border2", Value::String("#aaaa33".to_string()));
    set_if_missing(
        &mut tv,
        "arrowheadColor",
        Value::String("#333333".to_string()),
    );
    set_if_missing(&mut tv, "fontFamily", mermaid_default_font_family());
    set_if_missing(&mut tv, "fontSize", Value::String("16px".to_string()));
    set_if_missing(
        &mut tv,
        "labelBackground",
        Value::String("rgba(232,232,232, 0.8)".to_string()),
    );
    set_if_missing(&mut tv, "textColor", Value::String("#333".to_string()));
    set_if_missing(&mut tv, "THEME_COLOR_LIMIT", Value::Number(12.into()));
    set_if_missing(&mut tv, "radius", Value::Number(5.into()));
    set_if_missing(&mut tv, "strokeWidth", Value::Number(1.into()));

    let main_bkg = get_truthy_string(&tv, "mainBkg").unwrap_or_else(|| default_primary.to_string());
    let second_bkg =
        get_truthy_string(&tv, "secondBkg").unwrap_or_else(|| default_secondary.to_string());
    let line_color = get_truthy_string(&tv, "lineColor").unwrap_or_else(|| "#333333".to_string());
    let text_color = get_truthy_string(&tv, "textColor").unwrap_or_else(|| "#333".to_string());
    let border1 = get_truthy_string(&tv, "border1").unwrap_or_else(|| "#9370DB".to_string());
    let border2 = get_truthy_string(&tv, "border2").unwrap_or_else(|| "#aaaa33".to_string());
    let label_background = get_truthy_string(&tv, "labelBackground")
        .unwrap_or_else(|| "rgba(232,232,232, 0.8)".to_string());
    let primary_text_color =
        get_truthy_string(&tv, "primaryTextColor").unwrap_or_else(|| "#131300".to_string());

    // Flowchart and block/class surfaces.
    set_if_missing(&mut tv, "nodeBkg", Value::String(main_bkg.clone()));
    set_if_missing(&mut tv, "nodeBorder", Value::String(border1.clone()));
    set_if_missing(&mut tv, "clusterBkg", Value::String(second_bkg.clone()));
    set_if_missing(&mut tv, "clusterBorder", Value::String(border2.clone()));
    set_if_missing(
        &mut tv,
        "defaultLinkColor",
        Value::String(line_color.clone()),
    );
    set_if_missing(&mut tv, "titleColor", Value::String(text_color.clone()));
    set_if_missing(
        &mut tv,
        "edgeLabelBackground",
        Value::String(label_background.clone()),
    );
    set_if_missing(
        &mut tv,
        "nodeTextColor",
        Value::String(primary_text_color.clone()),
    );

    // Sequence diagram surfaces.
    set_if_missing(&mut tv, "actorBorder", Value::String(border1.clone()));
    set_if_missing(&mut tv, "actorBkg", Value::String(main_bkg.clone()));
    set_if_missing(
        &mut tv,
        "actorTextColor",
        Value::String("black".to_string()),
    );
    let actor_text_color =
        get_truthy_string(&tv, "actorTextColor").unwrap_or_else(|| "black".to_string());
    set_if_missing(&mut tv, "actorLineColor", Value::String(border1.clone()));
    set_if_missing(&mut tv, "labelBoxBkgColor", Value::String(main_bkg.clone()));
    set_if_missing(&mut tv, "signalColor", Value::String(text_color.clone()));
    set_if_missing(
        &mut tv,
        "signalTextColor",
        Value::String(text_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "labelBoxBorderColor",
        Value::String(border1.clone()),
    );
    set_if_missing(
        &mut tv,
        "labelTextColor",
        Value::String(actor_text_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "loopTextColor",
        Value::String(actor_text_color.clone()),
    );
    set_if_missing(&mut tv, "noteBorderColor", Value::String(border2.clone()));
    set_if_missing(
        &mut tv,
        "noteBkgColor",
        Value::String("#fff5ad".to_string()),
    );
    set_if_missing(
        &mut tv,
        "noteTextColor",
        Value::String(actor_text_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "activationBorderColor",
        Value::String("#666".to_string()),
    );
    set_if_missing(
        &mut tv,
        "activationBkgColor",
        Value::String("#f4f4f4".to_string()),
    );
    set_if_missing(
        &mut tv,
        "sequenceNumberColor",
        Value::String("white".to_string()),
    );
    set_if_missing(
        &mut tv,
        "rectBkgColor",
        Value::String(default_tertiary.clone()),
    );

    // Gantt chart surfaces.
    for (key, value) in [
        ("sectionBkgColor", "rgba(102, 102, 255, 0.49)"),
        ("altSectionBkgColor", "white"),
        ("sectionBkgColor2", "#fff400"),
        ("excludeBkgColor", "#eeeeee"),
        ("taskBorderColor", "#534fbc"),
        ("taskBkgColor", "#8a90dd"),
        ("taskTextLightColor", "white"),
        ("taskTextColor", "white"),
        ("taskTextDarkColor", "black"),
        ("taskTextOutsideColor", "black"),
        ("taskTextClickableColor", "#003163"),
        ("activeTaskBorderColor", "#534fbc"),
        ("activeTaskBkgColor", "#bfc7ff"),
        ("gridColor", "lightgrey"),
        ("doneTaskBkgColor", "lightgrey"),
        ("doneTaskBorderColor", "grey"),
        ("critBorderColor", "#ff8888"),
        ("critBkgColor", "red"),
        ("todayLineColor", "red"),
        ("vertLineColor", "navy"),
        ("noteFontWeight", "normal"),
        ("fontWeight", "normal"),
    ] {
        set_if_missing(&mut tv, key, Value::String(value.to_string()));
    }

    // C4 and architecture defaults.
    let primary_border_color = match get_truthy_string(&tv, "primaryBorderColor") {
        Some(color) => color,
        None => mk_border(default_primary, false)?,
    };
    set_if_missing(
        &mut tv,
        "personBorder",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(&mut tv, "personBkg", Value::String(main_bkg.clone()));
    set_if_missing(&mut tv, "archEdgeColor", Value::String(line_color.clone()));
    set_if_missing(
        &mut tv,
        "archEdgeArrowColor",
        Value::String(line_color.clone()),
    );
    set_if_missing(&mut tv, "archEdgeWidth", Value::String("3".to_string()));
    set_if_missing(
        &mut tv,
        "archGroupBorderColor",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "archGroupBorderWidth",
        Value::String("2px".to_string()),
    );

    // ER, state, class, and requirement surfaces.
    set_if_missing(
        &mut tv,
        "rowOdd",
        Value::String(theme_color::lighten(default_primary, 75.0)?),
    );
    set_if_missing(
        &mut tv,
        "rowEven",
        Value::String(theme_color::lighten(default_primary, 1.0)?),
    );
    set_if_missing(
        &mut tv,
        "attributeBackgroundColorOdd",
        Value::String("#ffffff".to_string()),
    );
    set_if_missing(
        &mut tv,
        "attributeBackgroundColorEven",
        Value::String("#f2f2f2".to_string()),
    );
    set_if_missing(&mut tv, "labelColor", Value::String("black".to_string()));
    set_if_missing(
        &mut tv,
        "errorBkgColor",
        Value::String("#552222".to_string()),
    );
    set_if_missing(
        &mut tv,
        "errorTextColor",
        Value::String("#552222".to_string()),
    );
    // `theme-default` materializes these `||` State fallbacks during construction, before
    // calculate() applies overrides. Only the unconditional specialStateColor assignment below
    // follows a replayed lineColor override during the second updateColors() pass.
    set_if_missing(
        &mut tv,
        "transitionColor",
        Value::String("#333333".to_string()),
    );
    set_if_missing(
        &mut tv,
        "transitionLabelColor",
        Value::String("#333".to_string()),
    );
    let state_label_color = get_truthy_string(&tv, "stateBkg")
        .map(Value::String)
        .unwrap_or_else(|| Value::String(primary_text_color.clone()));
    set_if_missing(&mut tv, "stateLabelColor", state_label_color);
    set_if_missing(
        &mut tv,
        "stateBkg",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "labelBackgroundColor",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "compositeBackground",
        Value::String("white".to_string()),
    );
    set_if_missing(
        &mut tv,
        "altBackground",
        Value::String("#f0f0f0".to_string()),
    );
    set_if_missing(
        &mut tv,
        "compositeTitleBackground",
        Value::String(main_bkg.clone()),
    );
    let node_border = get_truthy_string(&tv, "nodeBorder").unwrap_or_else(|| border1.clone());
    set_if_missing(
        &mut tv,
        "compositeBorder",
        Value::String(node_border.clone()),
    );
    set_if_missing(&mut tv, "innerEndBackground", Value::String(node_border));
    set_if_missing(
        &mut tv,
        "specialStateColor",
        Value::String(line_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "classText",
        Value::String(primary_text_color.clone()),
    );

    // Color scale.
    let primary_color =
        get_truthy_string(&tv, "primaryColor").unwrap_or_else(|| default_primary.to_string());
    let secondary_color =
        get_truthy_string(&tv, "secondaryColor").unwrap_or_else(|| default_secondary.to_string());
    // The constructor's first update already materializes every tertiary-derived fallback. The
    // calculate pass therefore replays an explicit tertiary token without evaluating it.
    let tertiary_color = default_tertiary.clone();
    let c_scale_bases = [
        primary_color.clone(),
        secondary_color.clone(),
        tertiary_color.clone(),
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(30.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(60.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(90.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(120.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(150.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(210.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(270.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(300.0, 0.0, 0.0))?,
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(330.0, 0.0, 0.0))?,
    ];
    let mut c_scales = Vec::with_capacity(c_scale_bases.len());
    for base in c_scale_bases {
        c_scales.push(theme_color::darken(&base, 10.0)?);
    }

    for (i, v) in c_scales.iter().enumerate() {
        set_if_missing(&mut tv, &format!("cScale{i}"), Value::String(v.clone()));
    }
    set_if_missing(
        &mut tv,
        "cScalePeer1",
        Value::String(theme_color::darken(&secondary_color, 45.0)?),
    );
    set_if_missing(
        &mut tv,
        "cScalePeer2",
        Value::String(theme_color::darken(&tertiary_color, 40.0)?),
    );
    for (i, fallback_color) in c_scales.iter().enumerate() {
        let color =
            get_truthy_string(&tv, &format!("cScale{i}")).unwrap_or_else(|| fallback_color.clone());
        set_if_missing(
            &mut tv,
            &format!("cScalePeer{i}"),
            Value::String(theme_color::darken(&color, 25.0)?),
        );
        set_if_missing(
            &mut tv,
            &format!("cScaleInv{i}"),
            Value::String(theme_color::adjust(
                &color,
                ColorAdjustment::hsl(180.0, 0.0, 0.0),
            )?),
        );
        if i == 0 || i == 3 {
            set_if_missing(
                &mut tv,
                &format!("cScaleLabel{i}"),
                Value::String("#ffffff".to_string()),
            );
        }
        set_if_missing(
            &mut tv,
            &format!("cScaleLabel{i}"),
            Value::String(actor_text_color.clone()),
        );
    }
    set_if_missing(
        &mut tv,
        "scaleLabelColor",
        Value::String(actor_text_color.clone()),
    );

    // Journey and pie color defaults.
    for (key, value) in [
        ("fillType0", primary_color.clone()),
        ("fillType1", secondary_color.clone()),
        (
            "fillType2",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(64.0, 0.0, 0.0))?,
        ),
        (
            "fillType3",
            theme_color::adjust(&secondary_color, ColorAdjustment::hsl(64.0, 0.0, 0.0))?,
        ),
        (
            "fillType4",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(-64.0, 0.0, 0.0))?,
        ),
        (
            "fillType5",
            theme_color::adjust(&secondary_color, ColorAdjustment::hsl(-64.0, 0.0, 0.0))?,
        ),
        (
            "fillType6",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(128.0, 0.0, 0.0))?,
        ),
        (
            "fillType7",
            theme_color::adjust(&secondary_color, ColorAdjustment::hsl(128.0, 0.0, 0.0))?,
        ),
        ("pie1", primary_color.clone()),
        ("pie2", secondary_color.clone()),
        ("pie3", theme_color::darken(&tertiary_color, 40.0)?),
        ("pie4", theme_color::darken(&primary_color, 10.0)?),
        ("pie5", theme_color::darken(&secondary_color, 30.0)?),
        ("pie6", theme_color::darken(&tertiary_color, 20.0)?),
        (
            "pie7",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(60.0, 0.0, -20.0))?,
        ),
        (
            "pie8",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(-60.0, 0.0, -40.0))?,
        ),
        (
            "pie9",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(120.0, 0.0, -40.0))?,
        ),
        (
            "pie10",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(60.0, 0.0, -40.0))?,
        ),
        (
            "pie11",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(-90.0, 0.0, -40.0))?,
        ),
        (
            "pie12",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(120.0, 0.0, -30.0))?,
        ),
    ] {
        set_if_missing(&mut tv, key, Value::String(value));
    }
    for (key, value) in [
        ("pieTitleTextSize", "25px"),
        ("pieTitleTextColor", "black"),
        ("pieSectionTextSize", "17px"),
        ("pieSectionTextColor", text_color.as_str()),
        ("pieLegendTextSize", "17px"),
        ("pieLegendTextColor", "black"),
        ("pieStrokeColor", "black"),
        ("pieStrokeWidth", "2px"),
        ("pieOuterStrokeWidth", "2px"),
        ("pieOuterStrokeColor", "black"),
        ("pieOpacity", "0.7"),
    ] {
        set_if_missing(&mut tv, key, Value::String(value.to_string()));
    }

    // Requirement and git surfaces consumed by current renderers.
    set_if_missing(
        &mut tv,
        "requirementBackground",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "requirementBorderColor",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "requirementBorderSize",
        Value::String("1".to_string()),
    );
    set_if_missing(
        &mut tv,
        "requirementTextColor",
        Value::String(primary_text_color.clone()),
    );
    set_if_missing(&mut tv, "relationColor", Value::String(line_color.clone()));
    set_if_missing(
        &mut tv,
        "relationLabelBackground",
        Value::String(label_background),
    );
    set_if_missing(
        &mut tv,
        "relationLabelColor",
        Value::String(actor_text_color.clone()),
    );

    set_if_missing(&mut tv, "tagLabelColor", Value::String(primary_text_color));
    set_if_missing(
        &mut tv,
        "tagLabelBackground",
        Value::String(default_primary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "tagLabelBorder",
        Value::String(primary_border_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "tagLabelFontSize",
        Value::String("10px".to_string()),
    );
    set_if_missing(
        &mut tv,
        "commitLabelColor",
        Value::String("#000021".to_string()),
    );
    set_if_missing(
        &mut tv,
        "commitLabelBackground",
        Value::String(default_secondary.to_string()),
    );
    set_if_missing(
        &mut tv,
        "commitLabelFontSize",
        Value::String("10px".to_string()),
    );

    set_if_missing(&mut tv, "useGradient", Value::Bool(false));
    set_if_missing(
        &mut tv,
        "gradientStart",
        Value::String(default_primary_border.clone()),
    );
    set_if_missing(
        &mut tv,
        "gradientStop",
        Value::String(default_secondary_border),
    );
    set_if_missing(
        &mut tv,
        "dropShadow",
        Value::String("drop-shadow(1px 2px 2px rgba(185, 185, 185, 1))".to_string()),
    );

    ensure_xychart_theme_defaults(
        &mut tv,
        "#ECECFF,#8493A6,#FFC3A0,#DCDDE1,#B8E994,#D1A36F,#C3CDE6,#FFB6C1,#496078,#F8F3E3",
    );

    apply_quadrant_theme_defaults(&mut tv, default_primary, "#131300", &default_primary_border)?;
    validate_explicit_git_transforms(&tv, &explicit_theme_variables, ColorTransform::Darken(25.0))?;

    finish_theme_defaults(config, MermaidThemeId::Default, tv)
}

fn apply_base_theme_defaults(config: &mut MermaidConfig) -> Result<(), ColorError> {
    let mut tv = theme_variables_map(config);
    let explicit_theme_variables = tv.clone();

    let dark_mode = tv.get("darkMode").is_some_and(is_js_truthy);
    let background = get_truthy_string(&tv, "background").unwrap_or_else(|| "#f4f4f4".to_string());
    let primary_color =
        get_truthy_string(&tv, "primaryColor").unwrap_or_else(|| "#fff4dd".to_string());

    // `theme-base` constructor defaults.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-base.js`.
    set_if_missing(&mut tv, "background", Value::String(background.clone()));
    set_if_missing(
        &mut tv,
        "primaryColor",
        Value::String(primary_color.clone()),
    );

    set_if_missing(
        &mut tv,
        "primaryTextColor",
        Value::String(if dark_mode { "#eee" } else { "#333" }.to_string()),
    );
    set_if_missing(&mut tv, "fontFamily", mermaid_default_font_family());
    set_if_missing(&mut tv, "fontSize", Value::String("16px".to_string()));

    let primary_text_color = get_truthy_string(&tv, "primaryTextColor")
        .unwrap_or_else(|| if dark_mode { "#eee" } else { "#333" }.to_string());

    let secondary_color = if let Some(color) = get_truthy_string(&tv, "secondaryColor") {
        color
    } else {
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(-120.0, 0.0, 0.0))?
    };
    set_if_missing(
        &mut tv,
        "secondaryColor",
        Value::String(secondary_color.clone()),
    );

    let tertiary_color = if let Some(color) = get_truthy_string(&tv, "tertiaryColor") {
        color
    } else {
        theme_color::adjust(&primary_color, ColorAdjustment::hsl(180.0, 0.0, 5.0))?
    };
    set_if_missing(
        &mut tv,
        "tertiaryColor",
        Value::String(tertiary_color.clone()),
    );

    let derives_primary_border_from_primary = value_is_missing(&tv, "primaryBorderColor");
    if derives_primary_border_from_primary {
        let color = mk_border(&primary_color, dark_mode)?;
        tv.insert("primaryBorderColor".to_string(), Value::String(color));
        config.propagate_theme_variable_ownership("primaryColor", "primaryBorderColor");
    }

    if get_truthy_string(&tv, "secondaryBorderColor").is_none() {
        let color = mk_border(&secondary_color, dark_mode)?;
        tv.insert("secondaryBorderColor".to_string(), Value::String(color));
    }

    if get_truthy_string(&tv, "tertiaryBorderColor").is_none() {
        let color = mk_border(&tertiary_color, dark_mode)?;
        tv.insert("tertiaryBorderColor".to_string(), Value::String(color));
    }

    if get_truthy_string(&tv, "lineColor").is_none() {
        tv.insert(
            "lineColor".to_string(),
            Value::String(theme_color::invert(&background)?),
        );
    }
    let line_color = get_truthy_string(&tv, "lineColor").unwrap_or_else(|| "#333333".to_string());
    set_if_missing(&mut tv, "arrowheadColor", Value::String(line_color));

    set_if_missing(
        &mut tv,
        "textColor",
        Value::String(primary_text_color.clone()),
    );

    let primary_border_color =
        get_truthy_string(&tv, "primaryBorderColor").unwrap_or_else(|| "#9370DB".to_string());
    let tertiary_border_color =
        get_truthy_string(&tv, "tertiaryBorderColor").unwrap_or_else(|| "#aaaa33".to_string());
    ensure_gradient_theme_defaults(&mut tv);

    set_if_missing(&mut tv, "nodeBkg", Value::String(primary_color.clone()));
    let derives_main_background_from_primary = value_is_missing(&tv, "mainBkg");
    set_if_missing(&mut tv, "mainBkg", Value::String(primary_color.clone()));
    if derives_main_background_from_primary {
        config.propagate_theme_variable_ownership("primaryColor", "mainBkg");
    }
    let derives_node_border_from_primary_border = value_is_missing(&tv, "nodeBorder");
    set_if_missing(&mut tv, "nodeBorder", Value::String(primary_border_color));
    if derives_node_border_from_primary_border {
        config.propagate_theme_variable_ownership("primaryBorderColor", "nodeBorder");
    }
    set_if_missing(&mut tv, "clusterBkg", Value::String(tertiary_color.clone()));
    set_if_missing(
        &mut tv,
        "clusterBorder",
        Value::String(tertiary_border_color),
    );
    set_if_missing(&mut tv, "nodeTextColor", Value::String(primary_text_color));

    if get_truthy_string(&tv, "tertiaryTextColor").is_none() {
        tv.insert(
            "tertiaryTextColor".to_string(),
            Value::String(theme_color::invert(&tertiary_color)?),
        );
    }
    let tertiary_text_color =
        get_truthy_string(&tv, "tertiaryTextColor").unwrap_or_else(|| "#333".to_string());
    set_if_missing(
        &mut tv,
        "titleColor",
        Value::String(tertiary_text_color.clone()),
    );

    if get_truthy_string(&tv, "edgeLabelBackground").is_none() {
        let color = if dark_mode {
            theme_color::darken(&secondary_color, 30.0)?
        } else {
            secondary_color.clone()
        };
        tv.insert("edgeLabelBackground".to_string(), Value::String(color));
    }

    set_if_missing(
        &mut tv,
        "errorBkgColor",
        Value::String(tertiary_color.clone()),
    );
    set_if_missing(
        &mut tv,
        "errorTextColor",
        Value::String(tertiary_text_color),
    );

    // Theme color scales (used across multiple diagrams, including radar's `cScale*` palette).
    // Mermaid's base theme derives these from `primaryColor` and then darkens them.
    let darken_amount = if dark_mode { 75.0 } else { 25.0 };
    for (key, base) in [
        ("cScale0", primary_color.clone()),
        ("cScale1", secondary_color.clone()),
        ("cScale2", tertiary_color.clone()),
        (
            "cScale3",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(30.0, 0.0, 0.0))?,
        ),
        (
            "cScale4",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(60.0, 0.0, 0.0))?,
        ),
        (
            "cScale5",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(90.0, 0.0, 0.0))?,
        ),
        (
            "cScale6",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(120.0, 0.0, 0.0))?,
        ),
        (
            "cScale7",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(150.0, 0.0, 0.0))?,
        ),
        (
            "cScale8",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(210.0, 0.0, 150.0))?,
        ),
        (
            "cScale9",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(270.0, 0.0, 0.0))?,
        ),
        (
            "cScale10",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(300.0, 0.0, 0.0))?,
        ),
        (
            "cScale11",
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(330.0, 0.0, 0.0))?,
        ),
    ] {
        let color = theme_color::darken(&base, darken_amount)?;
        set_if_missing(&mut tv, key, Value::String(color));
    }

    // Derived scale fields must use the value that survived the override stage. In particular,
    // an explicit cScale0 is the input to Mermaid's inverse/peer calculations before it is
    // replayed, rather than a reason to keep the default peer values.
    let scale_label_color = get_truthy_string(&tv, "labelTextColor")
        .unwrap_or_else(|| if dark_mode { "black" } else { "#333" }.to_string());
    for i in 0..12 {
        let key = format!("cScale{i}");
        let mut color = required_color(&tv, &key)?;
        if explicit_theme_variables.contains_key(&key) {
            color = theme_color::darken(&color, darken_amount)?;
        }
        let peer = if dark_mode {
            theme_color::lighten(&color, 10.0)?
        } else {
            theme_color::darken(&color, 10.0)?
        };
        set_if_missing(&mut tv, &format!("cScalePeer{i}"), Value::String(peer));
        set_if_missing(
            &mut tv,
            &format!("cScaleInv{i}"),
            Value::String(theme_color::invert(&color)?),
        );
        set_if_missing(
            &mut tv,
            &format!("cScaleLabel{i}"),
            Value::String(scale_label_color.clone()),
        );
    }

    // Diagram style defaults (themeVariables.radar.*).
    let mut radar = match tv.get("radar") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    let line_color = get_truthy_string(&tv, "lineColor").unwrap_or_else(|| "#333333".to_string());
    set_if_missing(&mut radar, "axisColor", Value::String(line_color));
    set_if_missing(&mut radar, "axisStrokeWidth", Value::Number(2.into()));
    set_if_missing(&mut radar, "axisLabelFontSize", Value::Number(12.into()));
    set_finite_number_if_missing(&mut radar, "curveOpacity", 0.5);
    set_if_missing(&mut radar, "curveStrokeWidth", Value::Number(2.into()));
    set_if_missing(
        &mut radar,
        "graticuleColor",
        Value::String("#DEDEDE".to_string()),
    );
    set_if_missing(&mut radar, "graticuleStrokeWidth", Value::Number(1.into()));
    set_finite_number_if_missing(&mut radar, "graticuleOpacity", 0.3);
    set_if_missing(&mut radar, "legendBoxSize", Value::Number(12.into()));
    set_if_missing(&mut radar, "legendFontSize", Value::Number(12.into()));
    tv.insert("radar".to_string(), Value::Object(radar));

    // `theme-base` xychart palette + colors.
    // Source: `repo-ref/mermaid/packages/mermaid/src/themes/theme-base.js`.
    ensure_xychart_theme_defaults(
        &mut tv,
        "#FFF4DD,#FFD8B1,#FFA07A,#ECEFF1,#D6DBDF,#C3E0A8,#FFB6A4,#FFD74D,#738FA7,#FFFFF0",
    );
    apply_single_pass_git_palette(
        &mut tv,
        &explicit_theme_variables,
        [
            primary_color.clone(),
            secondary_color.clone(),
            tertiary_color.clone(),
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(-30.0, 0.0, 0.0))?,
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(-60.0, 0.0, 0.0))?,
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(-90.0, 0.0, 0.0))?,
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(60.0, 0.0, 0.0))?,
            theme_color::adjust(&primary_color, ColorAdjustment::hsl(120.0, 0.0, 0.0))?,
        ],
        if dark_mode {
            ColorTransform::Lighten(25.0)
        } else {
            ColorTransform::Darken(25.0)
        },
    )?;
    apply_current_quadrant_theme_defaults(&mut tv)?;

    finish_theme_defaults(config, MermaidThemeId::Base, tv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn value_at_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
        path.split('.')
            .try_fold(root, |value, key| value.as_object()?.get(key))
    }

    #[test]
    fn supported_theme_names_match_core_expansion_surface() {
        assert_eq!(
            crate::supported_themes(),
            &[
                "default",
                "base",
                "dark",
                "forest",
                "neutral",
                "neo",
                "neo-dark",
                "redux",
                "redux-dark",
                "redux-color",
                "redux-dark-color"
            ]
        );
        assert_eq!(crate::supported_theme_ids(), MermaidThemeId::ALL);
        assert!(
            THEME_PROGRAMS
                .iter()
                .map(|program| program.id)
                .eq(MermaidThemeId::ALL.iter().copied()),
            "the executable theme programs must cover the typed catalog exactly"
        );
        assert_eq!(
            MermaidThemeId::ALL
                .iter()
                .copied()
                .map(MermaidThemeId::as_str)
                .collect::<Vec<_>>(),
            crate::supported_themes()
        );
    }

    #[test]
    fn typed_theme_ids_round_trip_without_fallback() {
        for &theme in MermaidThemeId::ALL {
            assert_eq!(MermaidThemeId::parse(theme.as_str()).unwrap(), theme);
            assert_eq!(theme.to_string(), theme.as_str());
        }

        for invalid in ["unknown", "null"] {
            let error = MermaidThemeId::parse(invalid).unwrap_err();
            assert_eq!(error.input(), invalid);
            assert_eq!(
                error.to_string(),
                format!("unsupported Mermaid theme `{invalid}`")
            );
        }
    }

    #[test]
    fn source_theme_materialization_merges_array_sources_without_object_coercion() {
        let source = MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": [
                {"primaryColor": "#123456"},
                {"secondaryColor": "#654321"}
            ]
        }));
        let materialized = materialize_source_selected_theme(
            &crate::generated::default_site_config(),
            &MermaidConfig::empty_object(),
            &source,
        )
        .expect("materialize source theme")
        .expect("registered source theme");

        assert_eq!(
            materialized.get_str("themeVariables.primaryColor"),
            Some("#123456")
        );
        assert_eq!(
            materialized.get_str("themeVariables.secondaryColor"),
            Some("#654321")
        );
    }

    #[test]
    fn supported_theme_defaults_match_upstream_snapshot() {
        for &theme in MermaidThemeId::NAMES {
            let mut cfg = MermaidConfig::from_value(json!({
                "theme": theme
            }));
            apply_theme_defaults(&mut cfg).unwrap();

            let actual = cfg
                .as_value()
                .get("themeVariables")
                .and_then(|v| v.as_object())
                .unwrap();
            let expected = ThemeProgram::resolve(MermaidThemeId::parse(theme).unwrap())
                .resolved_without_overrides();

            assert_eq!(actual, expected, "theme {theme}");
        }
    }

    #[test]
    fn theme_variable_derived_ownership_follows_base_primary_color_dependencies() {
        for (theme, derives_main_background, derives_node_border) in [
            (MermaidThemeId::Base, true, true),
            (MermaidThemeId::Default, false, false),
            (MermaidThemeId::Dark, false, false),
            (MermaidThemeId::Forest, false, false),
            (MermaidThemeId::Neutral, false, false),
            (MermaidThemeId::Neo, false, false),
            (MermaidThemeId::NeoDark, false, false),
            (MermaidThemeId::Redux, false, false),
            (MermaidThemeId::ReduxDark, false, false),
            (MermaidThemeId::ReduxColor, false, false),
            (MermaidThemeId::ReduxDarkColor, false, false),
        ] {
            let mut config = MermaidConfig::from_value(json!({ "theme": theme.as_str() }));
            config.deep_merge_explicit(&json!({
                "themeVariables": { "primaryColor": "#fff4dd" }
            }));

            apply_theme_defaults(&mut config).unwrap();

            assert_eq!(
                config.config_path_overrides_typed_default("themeVariables.mainBkg"),
                derives_main_background,
                "theme {theme}"
            );
            assert_eq!(
                config.config_path_overrides_typed_default("themeVariables.nodeBorder"),
                derives_node_border,
                "theme {theme}"
            );
            assert!(
                !config.config_path_overrides_typed_default("themeVariables.radius"),
                "theme {theme} must not generalize derived ownership to sibling variables"
            );
        }
    }

    #[test]
    fn theme_variable_derived_ownership_requires_owned_primary_color() {
        let mut config = MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": { "primaryColor": "#fff4dd" }
        }));

        apply_theme_defaults(&mut config).unwrap();

        assert!(!config.config_path_overrides_typed_default("themeVariables.mainBkg"));
    }

    #[test]
    fn theme_variable_derived_ownership_follows_color_scale_inputs() {
        for (theme, source, expected_owned) in [
            ("default", "secondaryColor", &[][..]),
            ("base", "secondaryColor", &[1][..]),
            (
                "base",
                "primaryColor",
                &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
            ),
            (
                "forest",
                "primaryColor",
                &[0, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
            ),
            (
                "neo-dark",
                "primaryColor",
                &[0, 3, 4, 5, 6, 7, 8, 9, 10, 11][..],
            ),
            ("neutral", "primaryColor", &[][..]),
        ] {
            let mut config = MermaidConfig::from_value(json!({ "theme": theme }));
            config.deep_merge_explicit(&json!({
                "themeVariables": { (source): "#336699" }
            }));

            apply_theme_defaults(&mut config).unwrap();

            for index in 0..12 {
                assert_eq!(
                    config.config_path_overrides_typed_default(&format!(
                        "themeVariables.cScale{index}"
                    )),
                    expected_owned.contains(&index),
                    "theme {theme} source {source} has incorrect cScale{index} ownership"
                );
            }
        }
    }

    #[test]
    fn signal_color_ownership_follows_themes_that_derive_it_from_text_color() {
        for (theme, text_color, derives_signal_color) in [
            (MermaidThemeId::Default, "#22c55e", true),
            (MermaidThemeId::Base, "#333", true),
            (MermaidThemeId::Dark, "lightgrey", false),
            (MermaidThemeId::Forest, "#333", false),
            (MermaidThemeId::Neutral, "#333", false),
            (MermaidThemeId::Neo, "#333", true),
            (MermaidThemeId::NeoDark, "#ccc", true),
            (MermaidThemeId::Redux, "#28253D", true),
            (MermaidThemeId::ReduxDark, "#FFFFFF", false),
            (MermaidThemeId::ReduxColor, "#28253D", true),
            (MermaidThemeId::ReduxDarkColor, "#FFFFFF", false),
        ] {
            let mut config = MermaidConfig::from_value(json!({ "theme": theme.as_str() }));
            config.deep_merge_explicit(&json!({
                "themeVariables": { "textColor": text_color }
            }));

            apply_theme_defaults(&mut config).unwrap();

            assert_eq!(
                config.get_str("themeVariables.signalColor"),
                Some(text_color),
                "theme {theme} intentionally uses an equal final value in this provenance probe"
            );
            assert_eq!(
                config.config_path_overrides_typed_default("themeVariables.signalColor"),
                derives_signal_color,
                "theme {theme}"
            );
        }
    }

    #[test]
    fn variable_dependency_ownership_follows_the_selected_theme_program() {
        let cases: &[(&str, &str, &str, &[&str])] = &[
            (
                "default",
                "textColor",
                "#22c55e",
                &["signalColor", "signalTextColor"],
            ),
            (
                "default",
                "actorTextColor",
                "#16a34a",
                &["labelTextColor", "loopTextColor", "noteTextColor"],
            ),
            (
                "base",
                "mainBkg",
                "#0f172a",
                &["actorBkg", "labelBoxBkgColor"],
            ),
            (
                "base",
                "primaryBorderColor",
                "#38bdf8",
                &["actorBorder", "actorLineColor", "labelBoxBorderColor"],
            ),
            (
                "dark",
                "mainContrastColor",
                "#f8fafc",
                &[
                    "actorTextColor",
                    "signalColor",
                    "signalTextColor",
                    "labelTextColor",
                    "loopTextColor",
                ],
            ),
            (
                "dark",
                "border1",
                "#a78bfa",
                &[
                    "actorBorder",
                    "actorLineColor",
                    "labelBoxBorderColor",
                    "activationBorderColor",
                ],
            ),
            (
                "neo",
                "primaryColor",
                "#0ea5e9",
                &["nodeBkg", "tagLabelBackground"],
            ),
            (
                "neo-dark",
                "primaryBorderColor",
                "#38bdf8",
                &["actorBorder", "actorLineColor", "labelBoxBorderColor"],
            ),
            (
                "redux",
                "mainBkg",
                "#0f172a",
                &["actorBkg", "labelBoxBkgColor", "personBkg", "stateBkg"],
            ),
            (
                "redux-color",
                "secondaryColor",
                "#22c55e",
                &[
                    "edgeLabelBackground",
                    "activationBkgColor",
                    "commitLabelBackground",
                    "relationLabelBackground",
                ],
            ),
            (
                "redux-dark",
                "primaryTextColor",
                "#f8fafc",
                &["actorTextColor", "labelTextColor", "loopTextColor"],
            ),
            (
                "redux-dark-color",
                "textColor",
                "#e2e8f0",
                &["signalTextColor", "transitionLabelColor"],
            ),
        ];

        for &(theme, source, value, targets) in cases {
            let mut config = MermaidConfig::from_value(json!({ "theme": theme }));
            config.deep_merge_explicit(&json!({
                "themeVariables": { (source): value }
            }));

            apply_theme_defaults(&mut config).unwrap();

            for target in targets {
                assert_eq!(
                    config.get_str(&format!("themeVariables.{target}")),
                    Some(value),
                    "theme {theme} source {source} has incorrect {target} value"
                );
                assert!(
                    config.config_path_overrides_typed_default(&format!("themeVariables.{target}")),
                    "theme {theme} source {source} must own derived {target}"
                );
            }
            assert!(
                !config.config_path_overrides_typed_default("themeVariables.sequenceNumberColor"),
                "theme {theme} source {source} must not own an unrelated Sequence variable"
            );
        }
    }

    #[test]
    fn extended_theme_transformed_dependencies_share_value_and_ownership_semantics() {
        for theme in [
            "neo",
            "neo-dark",
            "redux",
            "redux-dark",
            "redux-color",
            "redux-dark-color",
        ] {
            let mut background = MermaidConfig::from_value(json!({ "theme": theme }));
            background.deep_merge_explicit(&json!({
                "themeVariables": { "background": "#ffffff" }
            }));
            apply_theme_defaults(&mut background).unwrap();
            for (target, expected) in [
                ("lineColor", "#000000"),
                ("transitionColor", "#000000"),
                ("sequenceNumberColor", "#ffffff"),
            ] {
                assert_eq!(
                    background.get_str(&format!("themeVariables.{target}")),
                    Some(expected),
                    "theme {theme} has incorrect background-derived {target} value"
                );
                assert!(
                    background
                        .config_path_overrides_typed_default(&format!("themeVariables.{target}")),
                    "theme {theme} background must own derived {target}"
                );
            }

            let mut secondary = MermaidConfig::from_value(json!({ "theme": theme }));
            secondary.deep_merge_explicit(&json!({
                "themeVariables": { "secondaryColor": "#22c55e" }
            }));
            apply_theme_defaults(&mut secondary).unwrap();
            let expected_activation_border = theme_color::darken("#22c55e", 10.0).unwrap();
            assert_eq!(
                secondary.get_str("themeVariables.activationBorderColor"),
                Some(expected_activation_border.as_str()),
                "theme {theme} has incorrect activationBorderColor value"
            );
            for target in ["activationBorderColor", "activationBkgColor"] {
                assert!(
                    secondary
                        .config_path_overrides_typed_default(&format!("themeVariables.{target}")),
                    "theme {theme} secondaryColor must own derived {target}"
                );
            }
        }
    }

    #[test]
    fn variable_dependency_ownership_requires_an_owned_source_and_yields_to_explicit_targets() {
        let mut unowned = MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": { "mainBkg": "#0f172a" }
        }));
        apply_theme_defaults(&mut unowned).unwrap();
        assert_eq!(
            unowned.get_str("themeVariables.labelBoxBkgColor"),
            Some("#0f172a")
        );
        assert!(!unowned.config_path_overrides_typed_default("themeVariables.labelBoxBkgColor"));

        let mut explicit_target = MermaidConfig::from_value(json!({ "theme": "base" }));
        explicit_target.deep_merge_explicit(&json!({
            "themeVariables": {
                "mainBkg": "#0f172a",
                "actorBkg": "#1e293b",
                "labelBoxBkgColor": "#334155"
            }
        }));
        apply_theme_defaults(&mut explicit_target).unwrap();
        assert_eq!(
            explicit_target.get_str("themeVariables.actorBkg"),
            Some("#1e293b")
        );
        assert_eq!(
            explicit_target.get_str("themeVariables.labelBoxBkgColor"),
            Some("#334155")
        );

        for theme in [
            "neo",
            "neo-dark",
            "redux",
            "redux-dark",
            "redux-color",
            "redux-dark-color",
        ] {
            let mut unowned_route = MermaidConfig::from_value(json!({ "theme": theme }));
            unowned_route.deep_merge_explicit(&json!({
                "themeVariables": { "nodeBkg": "#0ea5e9" }
            }));
            apply_theme_defaults(&mut unowned_route).unwrap();
            assert!(
                !unowned_route
                    .config_path_overrides_typed_default("themeVariables.labelBoxBkgColor"),
                "theme {theme} must not route nodeBkg ownership into Sequence Loop paint"
            );
        }
    }

    #[test]
    fn unconditional_sequence_intermediates_ignore_explicit_replay_for_downstream_provenance() {
        for (theme, expected_actor_line, expected_label_box_background) in [
            ("default", "#9370DB", "#ECECFF"),
            ("dark", "#ccc", "#1f2020"),
        ] {
            let mut config = MermaidConfig::from_value(json!({ "theme": theme }));
            config.deep_merge_explicit(&json!({
                "themeVariables": {
                    "actorBorder": "#ec4899",
                    "actorBkg": "#0f172a"
                }
            }));

            apply_theme_defaults(&mut config).unwrap();

            assert_eq!(
                config.get_str("themeVariables.actorBorder"),
                Some("#ec4899"),
                "theme {theme} must replay explicit actorBorder"
            );
            assert_eq!(
                config.get_str("themeVariables.actorBkg"),
                Some("#0f172a"),
                "theme {theme} must replay explicit actorBkg"
            );
            for (target, expected) in [
                ("actorLineColor", expected_actor_line),
                ("labelBoxBkgColor", expected_label_box_background),
            ] {
                let path = format!("themeVariables.{target}");
                assert_eq!(
                    config.get_str(&path),
                    Some(expected),
                    "theme {theme} must calculate {target} before explicit replay"
                );
                assert!(
                    !config.config_path_overrides_typed_default(&path),
                    "theme {theme} explicit intermediate must not own downstream {target}"
                );
            }
        }

        let mut forest = MermaidConfig::from_value(json!({ "theme": "forest" }));
        forest.deep_merge_explicit(&json!({
            "themeVariables": { "primaryColor": "#ec4899" }
        }));
        apply_theme_defaults(&mut forest).unwrap();
        assert_eq!(forest.get_str("themeVariables.mainBkg"), Some("#cde498"));
        assert!(
            !forest.config_path_overrides_typed_default("themeVariables.mainBkg"),
            "Forest primaryColor does not feed the independently constructed mainBkg"
        );
    }

    #[test]
    fn source_text_color_does_not_rematerialize_signal_color_through_the_parse_pipeline() {
        let metadata = crate::Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "secure": [
                    "secure",
                    "securityLevel",
                    "startOnLoad",
                    "maxTextSize",
                    "suppressErrorRendering",
                    "maxEdges"
                ]
            })))
            .parse_metadata_sync(
                r##"%%{init: {"themeVariables": {"textColor": "#22c55e"}}}%%
sequenceDiagram
Alice->>Bob: Hello
"##,
            )
            .expect("parse Sequence init textColor");

        assert_eq!(
            metadata.config.get_str("themeVariables.textColor"),
            Some("#22c55e")
        );
        assert_eq!(
            metadata
                .effective_config
                .get_str("themeVariables.textColor"),
            Some("#22c55e")
        );
        assert_eq!(
            metadata
                .effective_config
                .get_str("themeVariables.signalColor"),
            Some("#333")
        );
        assert!(
            !metadata
                .effective_config
                .config_path_overrides_typed_default("themeVariables.signalColor")
        );
    }

    #[test]
    fn dark_extended_themes_preserve_constructor_arrowhead_after_background_override() {
        const SOURCE: &str = "flowchart LR\nA --> B\n";
        let expected_arrowhead = theme_color::invert("#333").unwrap();

        for theme in ["neo-dark", "redux-dark", "redux-dark-color"] {
            for origin in ["site", "source"] {
                let config = json!({
                    "theme": theme,
                    "themeVariables": {"background": "#010203"}
                });
                let (engine, source) = if origin == "site" {
                    (
                        crate::Engine::new().with_site_config(MermaidConfig::from_value(config)),
                        SOURCE.to_string(),
                    )
                } else {
                    (
                        crate::Engine::new().with_site_config(MermaidConfig::from_value(json!({
                            "secure": []
                        }))),
                        format!("%%{{init: {config}}}%%\n{SOURCE}"),
                    )
                };
                let metadata = engine.parse_metadata_sync(&source).unwrap_or_else(|error| {
                    panic!("parse {origin} {theme} arrowhead snapshot config: {error}")
                });

                assert_eq!(
                    metadata
                        .effective_config
                        .get_str("themeVariables.arrowheadColor"),
                    Some(expected_arrowhead.as_str()),
                    "{origin} {theme} must retain its constructor arrowhead snapshot"
                );
                assert!(
                    !metadata
                        .effective_config
                        .config_path_overrides_typed_default("themeVariables.arrowheadColor"),
                    "{origin} {theme} background ownership must not propagate into the constructor-seeded arrowhead"
                );
                assert_eq!(
                    metadata
                        .effective_config
                        .get_str("themeVariables.lineColor"),
                    Some("#fefdfc"),
                    "{origin} {theme} must still recompute lineColor from explicit background"
                );
                assert!(
                    metadata
                        .effective_config
                        .config_path_overrides_typed_default("themeVariables.lineColor"),
                    "{origin} {theme} explicit background must still own derived lineColor"
                );
            }
        }
    }

    #[test]
    fn venn_color_dependencies_follow_non_default_calculation_and_default_snapshots() {
        const SOURCE: &str = "venn-beta\ntitle Ownership\nset A\n";

        fn effective_config(theme: &str, theme_variables: Value, origin: &str) -> MermaidConfig {
            let config = json!({
                "theme": theme,
                "themeVariables": theme_variables
            });
            let (engine, source) = if origin == "site" {
                (
                    crate::Engine::new().with_site_config(MermaidConfig::from_value(config)),
                    SOURCE.to_string(),
                )
            } else {
                (
                    crate::Engine::new().with_site_config(MermaidConfig::from_value(json!({
                        "secure": []
                    }))),
                    format!("%%{{init: {config}}}%%\n{SOURCE}"),
                )
            };
            engine
                .parse_metadata_sync(&source)
                .unwrap_or_else(|error| panic!("parse {origin} {theme} Venn config: {error}"))
                .effective_config
        }

        for origin in ["site", "source"] {
            let base = effective_config(
                "base",
                json!({
                    "titleColor": "#123456",
                    "textColor": "#abcdef"
                }),
                origin,
            );
            for (target, expected) in [
                ("vennTitleTextColor", "#123456"),
                ("vennSetTextColor", "#abcdef"),
            ] {
                let path = format!("themeVariables.{target}");
                assert_eq!(
                    base.get_str(&path),
                    Some(expected),
                    "{origin} base {target}"
                );
                assert!(
                    base.config_path_overrides_typed_default(&path),
                    "{origin} base source must own derived {target}"
                );
            }

            let neutral = effective_config(
                "neutral",
                json!({
                    "text": "#345678",
                    "textColor": "#abcdef"
                }),
                origin,
            );
            let neutral_expectations = if origin == "site" {
                [
                    ("vennTitleTextColor", "#345678", true),
                    ("vennSetTextColor", "#abcdef", true),
                ]
            } else {
                // Mermaid's directive sanitizer does not admit the internal Neutral `text`
                // constructor variable. Source config therefore retains the `#333` title
                // snapshot, while the public `textColor` input still owns Venn set text.
                [
                    ("vennTitleTextColor", "#333", false),
                    ("vennSetTextColor", "#abcdef", true),
                ]
            };
            for (target, expected, owned) in neutral_expectations {
                let path = format!("themeVariables.{target}");
                assert_eq!(
                    neutral.get_str(&path),
                    Some(expected),
                    "{origin} neutral {target}"
                );
                assert_eq!(
                    neutral.config_path_overrides_typed_default(&path),
                    owned,
                    "{origin} neutral source has incorrect derived ownership for {target}"
                );
            }
            assert_eq!(
                neutral.get_str("themeVariables.text"),
                Some(if origin == "site" { "#345678" } else { "#333" }),
                "{origin} neutral directive sanitizer scope"
            );

            let explicit_targets = effective_config(
                "base",
                json!({
                    "titleColor": "#123456",
                    "textColor": "#abcdef",
                    "vennTitleTextColor": "#fedcba",
                    "vennSetTextColor": "#654321"
                }),
                origin,
            );
            assert_eq!(
                explicit_targets.get_str("themeVariables.vennTitleTextColor"),
                Some("#fedcba")
            );
            assert_eq!(
                explicit_targets.get_str("themeVariables.vennSetTextColor"),
                Some("#654321")
            );

            let default = effective_config(
                "default",
                json!({
                    "titleColor": "#123456",
                    "textColor": "#abcdef"
                }),
                origin,
            );
            for (target, expected) in [("vennTitleTextColor", "#333"), ("vennSetTextColor", "#333")]
            {
                let path = format!("themeVariables.{target}");
                assert_eq!(
                    default.get_str(&path),
                    Some(expected),
                    "{origin} default must retain the constructor snapshot for {target}"
                );
                assert!(
                    !default.config_path_overrides_typed_default(&path),
                    "{origin} default constructor snapshot must not acquire source ownership"
                );
            }
        }
    }

    #[test]
    fn theme_dependency_ledger_preserves_site_and_source_values_and_ownership() {
        const SOURCE: &str = "stateDiagram-v2\nReady --> Done: Finish\n";
        struct Case {
            theme: &'static str,
            variables: Value,
            owned: Vec<(&'static str, String)>,
            unowned: Vec<(&'static str, String)>,
        }

        let note_border = mk_border("#abcdef", false).unwrap();
        let dark_note_background = theme_color::lighten("#123456", 16.0).unwrap();
        let dark_default_note_background = theme_color::lighten("#1f2020", 16.0).unwrap();
        let forest_actor_border = theme_color::darken("#89abcd", 20.0).unwrap();
        let neutral_actor_border = theme_color::lighten("#456789", 23.0).unwrap();
        let cases = vec![
            Case {
                theme: "base",
                variables: json!({
                    "noteBkgColor": "#abcdef",
                    "lineColor": "#123456",
                    "textColor": "#234567",
                    "mainBkg": "#345678"
                }),
                owned: vec![
                    ("noteBorderColor", note_border.clone()),
                    ("transitionColor", "#123456".to_string()),
                    ("specialStateColor", "#123456".to_string()),
                    ("transitionLabelColor", "#234567".to_string()),
                    ("stateBkg", "#345678".to_string()),
                    ("labelBackgroundColor", "#345678".to_string()),
                ],
                unowned: vec![],
            },
            Case {
                theme: "neo",
                variables: json!({ "noteBkgColor": "#abcdef" }),
                owned: vec![("noteBorderColor", note_border.clone())],
                unowned: vec![],
            },
            Case {
                theme: "neo-dark",
                variables: json!({ "noteBkgColor": "#abcdef" }),
                owned: vec![("noteBorderColor", note_border)],
                unowned: vec![],
            },
            Case {
                theme: "forest",
                variables: json!({
                    "border2": "#456789",
                    "actorTextColor": "#56789a",
                    "lineColor": "#6789ab",
                    "textColor": "#789abc",
                    "mainBkg": "#89abcd"
                }),
                owned: vec![
                    ("actorBorder", forest_actor_border.clone()),
                    ("actorLineColor", forest_actor_border),
                    ("actorBkg", "#89abcd".to_string()),
                    ("labelBoxBkgColor", "#89abcd".to_string()),
                    ("labelTextColor", "#56789a".to_string()),
                    ("loopTextColor", "#56789a".to_string()),
                    ("noteBorderColor", "#456789".to_string()),
                    ("noteTextColor", "#56789a".to_string()),
                    ("transitionColor", "#6789ab".to_string()),
                    ("specialStateColor", "#6789ab".to_string()),
                    ("transitionLabelColor", "#789abc".to_string()),
                    ("stateBkg", "#89abcd".to_string()),
                    ("labelBackgroundColor", "#89abcd".to_string()),
                ],
                unowned: vec![],
            },
            Case {
                theme: "neutral",
                variables: json!({
                    "noteBkgColor": "#abcdef",
                    "border1": "#456789",
                    "lineColor": "#123456",
                    "textColor": "#234567",
                    "mainBkg": "#345678"
                }),
                owned: vec![
                    ("actorBorder", neutral_actor_border.clone()),
                    ("actorLineColor", neutral_actor_border.clone()),
                    ("labelBoxBorderColor", neutral_actor_border),
                    ("actorBkg", "#345678".to_string()),
                    ("labelBoxBkgColor", "#345678".to_string()),
                    ("transitionLabelColor", "#234567".to_string()),
                    ("stateBkg", "#345678".to_string()),
                    ("labelBackgroundColor", "#345678".to_string()),
                ],
                unowned: vec![
                    ("noteBorderColor", "#999".to_string()),
                    ("transitionColor", "#000".to_string()),
                    ("specialStateColor", "#222".to_string()),
                ],
            },
            Case {
                theme: "dark",
                variables: json!({
                    "mainBkg": "#123456",
                    "secondBkg": "#abcdef",
                    "secondaryBorderColor": "#234567",
                    "secondaryTextColor": "#345678",
                    "lineColor": "#fedcba",
                    "textColor": "#56789a"
                }),
                owned: vec![
                    ("noteBorderColor", "#234567".to_string()),
                    ("noteBkgColor", dark_note_background),
                    ("noteTextColor", "#345678".to_string()),
                    ("transitionLabelColor", "#56789a".to_string()),
                    ("stateBkg", "#123456".to_string()),
                    ("labelBackgroundColor", "#123456".to_string()),
                ],
                unowned: vec![
                    ("transitionColor", "lightgrey".to_string()),
                    ("specialStateColor", "#f4f4f4".to_string()),
                ],
            },
            Case {
                theme: "dark",
                variables: json!({ "secondBkg": "#abcdef" }),
                owned: vec![],
                unowned: vec![
                    ("noteBkgColor", dark_default_note_background.clone()),
                    ("activationBkgColor", dark_default_note_background),
                ],
            },
            Case {
                theme: "dark",
                variables: json!({ "lineColor": "#fedcba" }),
                owned: vec![],
                unowned: vec![
                    ("transitionColor", "lightgrey".to_string()),
                    ("specialStateColor", "#f4f4f4".to_string()),
                ],
            },
            Case {
                theme: "default",
                variables: json!({
                    "border2": "#456789",
                    "actorTextColor": "#56789a",
                    "lineColor": "#123456",
                    "textColor": "#234567",
                    "mainBkg": "#345678"
                }),
                owned: vec![
                    ("noteBorderColor", "#456789".to_string()),
                    ("noteTextColor", "#56789a".to_string()),
                    ("specialStateColor", "#123456".to_string()),
                ],
                unowned: vec![
                    ("transitionColor", "#333333".to_string()),
                    ("transitionLabelColor", "#333".to_string()),
                    ("stateBkg", "#ECECFF".to_string()),
                    ("labelBackgroundColor", "#ECECFF".to_string()),
                ],
            },
        ];

        for case in cases {
            for origin in ["site", "source"] {
                let config = json!({
                    "theme": case.theme,
                    "themeVariables": case.variables.clone()
                });
                let (engine, source) = if origin == "site" {
                    (
                        crate::Engine::new().with_site_config(MermaidConfig::from_value(config)),
                        SOURCE.to_string(),
                    )
                } else {
                    (
                        crate::Engine::new().with_site_config(MermaidConfig::from_value(json!({
                            "secure": []
                        }))),
                        format!("%%{{init: {config}}}%%\n{SOURCE}"),
                    )
                };
                let metadata = engine.parse_metadata_sync(&source).unwrap_or_else(|error| {
                    panic!("parse {origin} {} dependency config: {error}", case.theme)
                });

                for (target, expected) in &case.owned {
                    let path = format!("themeVariables.{target}");
                    assert_eq!(
                        metadata.effective_config.get_str(&path),
                        Some(expected.as_str()),
                        "{origin} {} has incorrect owned {target} value",
                        case.theme
                    );
                    assert!(
                        metadata
                            .effective_config
                            .config_path_overrides_typed_default(&path),
                        "{origin} {} source variables must own derived {target}",
                        case.theme
                    );
                }
                for (target, expected) in &case.unowned {
                    let path = format!("themeVariables.{target}");
                    assert_eq!(
                        metadata.effective_config.get_str(&path),
                        Some(expected.as_str()),
                        "{origin} {} has incorrect unowned {target} value",
                        case.theme
                    );
                    assert!(
                        !metadata
                            .effective_config
                            .config_path_overrides_typed_default(&path),
                        "{origin} {} must preserve unowned {target} semantics",
                        case.theme
                    );
                }
            }
        }
    }

    #[test]
    fn state_terminal_dependency_ledger_preserves_ordered_values_and_ownership() {
        fn config_with_explicit_theme_variables(
            theme: &str,
            theme_variables: Value,
        ) -> MermaidConfig {
            let mut config = MermaidConfig::from_value(json!({ "theme": theme }));
            config.deep_merge_explicit(&json!({ "themeVariables": theme_variables }));
            config
        }

        fn assert_owned(config: &MermaidConfig, target: &str, expected: &str) {
            let path = format!("themeVariables.{target}");
            assert_eq!(
                config.get_str(&path),
                Some(expected),
                "incorrect {target} value"
            );
            assert!(
                config.config_path_overrides_typed_default(&path),
                "explicit source must own derived {target}"
            );
        }

        fn assert_unowned(config: &MermaidConfig, target: &str, expected: &str) {
            let path = format!("themeVariables.{target}");
            assert_eq!(
                config.get_str(&path),
                Some(expected),
                "incorrect {target} value"
            );
            assert!(
                !config.config_path_overrides_typed_default(&path),
                "calculated default must not own {target}"
            );
        }

        let mut default = config_with_explicit_theme_variables(
            "default",
            json!({
                "background": "#123456",
                "border1": "#234567",
                "lineColor": "#345678",
                "textColor": "#456789"
            }),
        );
        apply_theme_defaults(&mut default).unwrap();
        for (target, expected) in [
            ("nodeBorder", "#234567"),
            ("innerEndBackground", "#234567"),
            ("specialStateColor", "#345678"),
            ("titleColor", "#456789"),
        ] {
            assert_owned(&default, target, expected);
        }
        assert_unowned(&default, "compositeBackground", "white");
        assert_unowned(&default, "transitionColor", "#333333");

        let mut base = config_with_explicit_theme_variables(
            "base",
            json!({
                "background": "#123456",
                "lineColor": "#234567",
                "mainBkg": "#345678",
                "primaryBorderColor": "#456789",
                "primaryTextColor": "#56789a",
                "tertiaryTextColor": "#6789ab"
            }),
        );
        apply_theme_defaults(&mut base).unwrap();
        for (target, expected) in [
            ("compositeBackground", "#123456"),
            ("specialStateColor", "#234567"),
            ("stateBkg", "#345678"),
            ("labelBackgroundColor", "#345678"),
            ("compositeTitleBackground", "#345678"),
            ("nodeBorder", "#456789"),
            ("innerEndBackground", "#456789"),
            ("stateLabelColor", "#56789a"),
            ("titleColor", "#6789ab"),
        ] {
            assert_owned(&base, target, expected);
        }

        let mut explicit_state_background = config_with_explicit_theme_variables(
            "base",
            json!({
                "mainBkg": "#123456",
                "primaryTextColor": "#234567",
                "stateBkg": "#345678"
            }),
        );
        apply_theme_defaults(&mut explicit_state_background).unwrap();
        assert_owned(&explicit_state_background, "stateLabelColor", "#345678");

        let mut derived_state_background =
            config_with_explicit_theme_variables("base", json!({ "mainBkg": "#123456" }));
        apply_theme_defaults(&mut derived_state_background).unwrap();
        assert_owned(&derived_state_background, "stateBkg", "#123456");
        assert_unowned(&derived_state_background, "stateLabelColor", "#333");

        let dark_edge_label = theme_color::lighten("#234567", 25.0).unwrap();
        for (theme, variables, owned, unowned) in [
            (
                "dark",
                json!({
                    "background": "#123456",
                    "labelBackground": "#234567",
                    "mainBkg": "#345678",
                    "primaryBorderColor": "#456789"
                }),
                vec![
                    ("compositeBackground", "#123456".to_string()),
                    ("edgeLabelBackground", dark_edge_label),
                    ("stateBkg", "#345678".to_string()),
                    ("labelBackgroundColor", "#345678".to_string()),
                    ("compositeTitleBackground", "#345678".to_string()),
                    ("innerEndBackground", "#456789".to_string()),
                ],
                vec![("specialStateColor", "#f4f4f4".to_string())],
            ),
            (
                "forest",
                json!({
                    "background": "#123456",
                    "lineColor": "#234567",
                    "mainBkg": "#345678",
                    "primaryBorderColor": "#456789"
                }),
                vec![
                    ("compositeBackground", "#123456".to_string()),
                    ("specialStateColor", "#234567".to_string()),
                    ("stateBkg", "#345678".to_string()),
                    ("labelBackgroundColor", "#345678".to_string()),
                    ("compositeTitleBackground", "#345678".to_string()),
                    ("innerEndBackground", "#456789".to_string()),
                ],
                vec![("titleColor", "#333".to_string())],
            ),
            (
                "neutral",
                json!({
                    "background": "#123456",
                    "mainBkg": "#234567",
                    "primaryBorderColor": "#345678",
                    "text": "#456789"
                }),
                vec![
                    ("compositeBackground", "#123456".to_string()),
                    ("stateBkg", "#234567".to_string()),
                    ("labelBackgroundColor", "#234567".to_string()),
                    ("compositeTitleBackground", "#234567".to_string()),
                    ("innerEndBackground", "#345678".to_string()),
                    ("titleColor", "#456789".to_string()),
                ],
                vec![("specialStateColor", "#222".to_string())],
            ),
        ] {
            let mut config = config_with_explicit_theme_variables(theme, variables);
            apply_theme_defaults(&mut config).unwrap();
            for (target, expected) in owned {
                assert_owned(&config, target, &expected);
            }
            for (target, expected) in unowned {
                assert_unowned(&config, target, &expected);
            }
        }

        for (theme, expected_composite, expected_title) in [
            ("neo", "#123456", "#345678"),
            ("neo-dark", "#123456", "#345678"),
            ("redux", "#123456", "#F9F9FB"),
            ("redux-dark", "#16141F", "#16141F"),
            ("redux-color", "#123456", "#345678"),
            ("redux-dark-color", "#123456", "#345678"),
        ] {
            let mut config = config_with_explicit_theme_variables(
                theme,
                json!({
                    "background": "#123456",
                    "lineColor": "#234567",
                    "mainBkg": "#345678",
                    "nodeBorder": "#456789",
                    "primaryTextColor": "#56789a",
                    "tertiaryTextColor": "#6789ab"
                }),
            );
            apply_theme_defaults(&mut config).unwrap();

            assert_owned(&config, "titleColor", "#6789ab");
            assert_owned(&config, "innerEndBackground", "#456789");
            assert_owned(&config, "specialStateColor", "#234567");
            assert_owned(&config, "stateLabelColor", "#56789a");
            if theme == "redux-dark" {
                assert_unowned(&config, "compositeBackground", expected_composite);
            } else {
                assert_owned(&config, "compositeBackground", expected_composite);
            }
            if matches!(theme, "redux" | "redux-dark") {
                assert_unowned(&config, "compositeTitleBackground", expected_title);
            } else {
                assert_owned(&config, "compositeTitleBackground", expected_title);
            }
        }

        let mut default_gradient = config_with_explicit_theme_variables(
            "default",
            json!({
                "primaryBorderColor": "#123456",
                "secondaryBorderColor": "#234567"
            }),
        );
        apply_theme_defaults(&mut default_gradient).unwrap();
        assert_unowned(
            &default_gradient,
            "gradientStart",
            &mk_border("#ECECFF", false).unwrap(),
        );
        assert_unowned(
            &default_gradient,
            "gradientStop",
            &mk_border("#ffffde", false).unwrap(),
        );

        let mut base_gradient = config_with_explicit_theme_variables(
            "base",
            json!({
                "primaryBorderColor": "#123456",
                "secondaryBorderColor": "#234567"
            }),
        );
        apply_theme_defaults(&mut base_gradient).unwrap();
        assert_owned(&base_gradient, "gradientStart", "#123456");
        assert_owned(&base_gradient, "gradientStop", "#234567");

        for (theme, gradient_start, gradient_stop) in [
            ("dark", "#cccccc", "hsl(180, 0%, 18.3529411765%)"),
            (
                "forest",
                "hsl(78.1578947368, 18.4615384615%, 64.5098039216%)",
                "hsl(98.961038961, 60%, 74.9019607843%)",
            ),
            (
                "neutral",
                "hsl(0, 0%, 83.3333333333%)",
                "hsl(0, 0%, 88.9215686275%)",
            ),
        ] {
            let mut config = config_with_explicit_theme_variables(
                theme,
                json!({
                    "primaryBorderColor": "#123456",
                    "secondaryBorderColor": "#234567"
                }),
            );
            apply_theme_defaults(&mut config).unwrap();
            assert_unowned(&config, "gradientStart", gradient_start);
            assert_unowned(&config, "gradientStop", gradient_stop);
        }

        let mut base_sources = config_with_explicit_theme_variables(
            "base",
            json!({
                "background": "#123456",
                "primaryTextColor": "#234567",
                "secondaryColor": "#345678",
                "tertiaryColor": "#456789"
            }),
        );
        apply_theme_defaults(&mut base_sources).unwrap();
        let derived_line = theme_color::invert("#123456").unwrap();
        let derived_tertiary_text = theme_color::invert("#456789").unwrap();
        let derived_secondary_border = mk_border("#345678", false).unwrap();
        let derived_activation_border = theme_color::darken("#345678", 10.0).unwrap();
        let derived_sequence_number = theme_color::invert(&derived_line).unwrap();
        for (target, expected) in [
            ("lineColor", derived_line.as_str()),
            ("transitionColor", derived_line.as_str()),
            ("specialStateColor", derived_line.as_str()),
            ("sequenceNumberColor", derived_sequence_number.as_str()),
            ("textColor", "#234567"),
            ("signalTextColor", "#234567"),
            ("transitionLabelColor", "#234567"),
            ("tertiaryTextColor", derived_tertiary_text.as_str()),
            ("titleColor", derived_tertiary_text.as_str()),
            ("secondaryBorderColor", derived_secondary_border.as_str()),
            ("gradientStop", derived_secondary_border.as_str()),
            ("activationBorderColor", derived_activation_border.as_str()),
        ] {
            assert_owned(&base_sources, target, expected);
        }

        for theme in [
            "neo",
            "neo-dark",
            "redux",
            "redux-dark",
            "redux-color",
            "redux-dark-color",
        ] {
            let mut config = config_with_explicit_theme_variables(
                theme,
                json!({
                    "primaryBorderColor": "#123456",
                    "secondaryBorderColor": "#234567"
                }),
            );
            apply_theme_defaults(&mut config).unwrap();
            assert_unowned(&config, "gradientStart", "#0042eb");
            assert_unowned(&config, "gradientStop", "#eb0042");
        }
    }

    #[test]
    fn gantt_terminal_fill_dependencies_preserve_values_and_ownership() {
        fn config_with_explicit_theme_variables(
            theme: &str,
            theme_variables: Value,
        ) -> MermaidConfig {
            let mut config = MermaidConfig::from_value(json!({ "theme": theme }));
            config.deep_merge_explicit(&json!({ "themeVariables": theme_variables }));
            config
        }

        fn assert_owned(config: &MermaidConfig, target: &str, expected: &str) {
            let path = format!("themeVariables.{target}");
            assert_eq!(
                config.get_str(&path),
                Some(expected),
                "incorrect {target} value"
            );
            assert!(
                config.config_path_overrides_typed_default(&path),
                "the explicit Gantt source must own derived {target}"
            );
        }

        fn assert_unowned(config: &MermaidConfig, target: &str, expected: &str) {
            let path = format!("themeVariables.{target}");
            assert_eq!(
                config.get_str(&path),
                Some(expected),
                "incorrect {target} value"
            );
            assert!(
                !config.config_path_overrides_typed_default(&path),
                "the pinned Gantt constant must not inherit unrelated ownership for {target}"
            );
        }

        let mut base =
            config_with_explicit_theme_variables("base", json!({ "primaryColor": "#123456" }));
        apply_theme_defaults(&mut base).unwrap();
        assert_owned(&base, "taskBkgColor", "#123456");
        assert_owned(
            &base,
            "activeTaskBkgColor",
            &theme_color::lighten("#123456", 23.0).unwrap(),
        );

        let mut dark = config_with_explicit_theme_variables(
            "dark",
            json!({
                "mainBkg": "#123456",
                "mainContrastColor": "#abcdef"
            }),
        );
        apply_theme_defaults(&mut dark).unwrap();
        assert_owned(
            &dark,
            "taskBkgColor",
            &theme_color::lighten("#123456", 23.0).unwrap(),
        );
        assert_owned(&dark, "doneTaskBkgColor", "#abcdef");

        let mut forest =
            config_with_explicit_theme_variables("forest", json!({ "mainBkg": "#123456" }));
        apply_theme_defaults(&mut forest).unwrap();
        assert_owned(&forest, "activeTaskBkgColor", "#123456");

        let mut neutral = config_with_explicit_theme_variables(
            "neutral",
            json!({
                "contrast": "#123456",
                "mainBkg": "#234567",
                "done": "#345678",
                "critical": "#456789"
            }),
        );
        apply_theme_defaults(&mut neutral).unwrap();
        for (target, expected) in [
            ("taskBkgColor", "#123456"),
            ("activeTaskBkgColor", "#234567"),
            ("doneTaskBkgColor", "#345678"),
            ("critBkgColor", "#456789"),
        ] {
            assert_owned(&neutral, target, expected);
        }

        let active_dark = theme_color::lighten("#123456", 23.0).unwrap();
        for theme in ["neo-dark", "redux-dark", "redux-dark-color"] {
            let mut config =
                config_with_explicit_theme_variables(theme, json!({ "primaryColor": "#123456" }));
            apply_theme_defaults(&mut config).unwrap();
            assert_owned(&config, "taskBkgColor", "#123456");
            assert_owned(&config, "activeTaskBkgColor", &active_dark);
        }

        let pinned_light_active = theme_color::lighten("#ECECFE", 23.0).unwrap();
        for theme in ["neo", "redux", "redux-color"] {
            let mut config =
                config_with_explicit_theme_variables(theme, json!({ "primaryColor": "#123456" }));
            apply_theme_defaults(&mut config).unwrap();
            assert_unowned(&config, "taskBkgColor", "#ECECFE");
            assert_unowned(&config, "activeTaskBkgColor", &pinned_light_active);
        }

        let mut explicit_targets = config_with_explicit_theme_variables(
            "default",
            json!({
                "taskBkgColor": "#111111",
                "activeTaskBkgColor": "#222222",
                "doneTaskBkgColor": "#333333",
                "critBkgColor": "#444444"
            }),
        );
        apply_theme_defaults(&mut explicit_targets).unwrap();
        for (target, expected) in [
            ("taskBkgColor", "#111111"),
            ("activeTaskBkgColor", "#222222"),
            ("doneTaskBkgColor", "#333333"),
            ("critBkgColor", "#444444"),
        ] {
            assert_owned(&explicit_targets, target, expected);
        }
    }

    #[test]
    fn neutral_site_text_owns_unconditional_sequence_derivations() {
        let metadata = crate::Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "theme": "neutral",
                "themeVariables": { "text": "#56789a" }
            })))
            .parse_metadata_sync("sequenceDiagram\nAlice->>Bob: Hello\n")
            .expect("parse site-owned Neutral text");

        assert_eq!(
            metadata.effective_config.get_str("themeVariables.text"),
            Some("#56789a")
        );
        for target in [
            "actorTextColor",
            "signalColor",
            "signalTextColor",
            "labelTextColor",
            "loopTextColor",
        ] {
            let path = format!("themeVariables.{target}");
            assert_eq!(
                metadata.effective_config.get_str(&path),
                Some("#56789a"),
                "Neutral site text has incorrect derived {target} value"
            );
            assert!(
                metadata
                    .effective_config
                    .config_path_overrides_typed_default(&path),
                "Neutral site text must own derived {target}"
            );
        }
    }

    #[test]
    fn dark_site_main_contrast_owns_unconditional_state_and_sequence_derivations() {
        let metadata = crate::Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "theme": "dark",
                "themeVariables": { "mainContrastColor": "#456789" }
            })))
            .parse_metadata_sync("stateDiagram-v2\nReady --> Done: Finish\n")
            .expect("parse site-owned Dark mainContrastColor");

        for target in [
            "lineColor",
            "actorTextColor",
            "signalColor",
            "signalTextColor",
            "labelTextColor",
            "loopTextColor",
            "transitionColor",
        ] {
            let path = format!("themeVariables.{target}");
            assert_eq!(
                metadata.effective_config.get_str(&path),
                Some("#456789"),
                "Dark site mainContrastColor has incorrect derived {target} value"
            );
            assert!(
                metadata
                    .effective_config
                    .config_path_overrides_typed_default(&path),
                "Dark site mainContrastColor must own derived {target}"
            );
        }
    }

    #[test]
    fn dark_mode_branch_matches_generated_mermaid_oracle_for_every_theme() {
        for &theme in MermaidThemeId::NAMES {
            let mut config = MermaidConfig::from_value(json!({
                "theme": theme,
                "themeVariables": { "darkMode": true }
            }));
            apply_theme_defaults(&mut config).unwrap();
            let actual = config
                .as_value()
                .get("themeVariables")
                .and_then(Value::as_object)
                .unwrap();
            let expected = ThemeProgram::resolve(MermaidThemeId::parse(theme).unwrap())
                .resolved_dark_mode_true();
            assert_eq!(actual, expected, "theme {theme}");
        }
    }

    #[test]
    fn generated_mermaid_oracle_locks_override_value_semantics() {
        let mut mismatches = Vec::new();
        for case in &generated_theme_oracles().oracle_cases {
            let id = case.get("id").and_then(Value::as_str).unwrap();
            let theme = case.get("theme").and_then(Value::as_str).unwrap();
            let overrides = case.get("overrides").cloned().unwrap();
            let expected_status = case.get("status").and_then(Value::as_str).unwrap();
            let mut config = MermaidConfig::from_value(json!({
                "theme": theme,
                "themeVariables": overrides
            }));
            let result = apply_theme_defaults(&mut config);

            if expected_status == "error" {
                if result.is_ok() {
                    mismatches.push(format!("{theme}/{id}: expected an error"));
                }
                continue;
            }
            if let Err(error) = result {
                mismatches.push(format!("{theme}/{id}: unexpected error: {error}"));
                continue;
            }

            let actual = config.as_value().get("themeVariables").unwrap();
            let selected = case.get("selected").and_then(Value::as_object).unwrap();
            for (path, expected) in selected {
                let state = expected.get("state").and_then(Value::as_str).unwrap();
                let actual_value = value_at_path(actual, path);
                match state {
                    "missing" if actual_value.is_some() => mismatches.push(format!(
                        "{theme}/{id}/{path}: expected missing, found {}",
                        actual_value.unwrap()
                    )),
                    "value" if actual_value != expected.get("value") => mismatches.push(format!(
                        "{theme}/{id}/{path}: expected {}, found {}",
                        expected.get("value").unwrap(),
                        actual_value
                            .map(Value::to_string)
                            .unwrap_or_else(|| "missing".to_string())
                    )),
                    "missing" | "value" => {}
                    other => mismatches
                        .push(format!("{theme}/{id}/{path}: unknown oracle state {other}")),
                }
            }
        }

        assert!(
            mismatches.is_empty(),
            "Mermaid theme oracle mismatches:\n{}",
            mismatches.join("\n")
        );
    }

    #[test]
    fn font_only_override_preserves_upstream_derived_palette_for_public_themes() {
        for &theme in MermaidThemeId::NAMES {
            let mut cfg = MermaidConfig::from_value(json!({
                "theme": theme,
                "themeVariables": {
                    "fontFamily": "Inter, sans-serif"
                }
            }));
            apply_theme_defaults(&mut cfg).unwrap();

            let actual = cfg
                .as_value()
                .get("themeVariables")
                .and_then(Value::as_object)
                .unwrap();
            let expected = ThemeProgram::resolve(MermaidThemeId::parse(theme).unwrap())
                .resolved_without_overrides();

            for key in [
                "cScale0",
                "cScale1",
                "cScalePeer0",
                "cScaleInv0",
                "cScaleLabel0",
            ] {
                assert_eq!(
                    actual.get(key),
                    expected.get(key),
                    "theme {theme} has derived palette drift at {key}"
                );
            }
            for (key, expected_value) in expected {
                if key == "fontFamily" {
                    continue;
                }
                assert_eq!(
                    actual.get(key),
                    Some(expected_value),
                    "theme {theme} has a non-font snapshot drift at {key}"
                );
            }
            assert_eq!(
                actual.get("fontFamily").and_then(Value::as_str),
                Some("Inter, sans-serif"),
                "theme {theme} should replay the explicit font override"
            );
        }
    }

    #[test]
    fn explicit_scale_override_recomputes_peer_and_inverse_from_override_stage() {
        // Oracle values from Mermaid 11.16.1 `getThemeVariables()` with the same overrides.
        let cases = [
            (
                "default",
                "hsl(240, 100%, 61.2745098039%)",
                "hsl(60, 100%, 86.2745098039%)",
            ),
            ("dark", "hsl(210, 68%, 90.3921568627%)", "#543210"),
            (
                "forest",
                "hsl(210, 68%, 45.3921568627%)",
                "hsl(30, 68%, 70.3921568627%)",
            ),
            ("neutral", "hsl(210, 68%, 70.3921568627%)", "#543210"),
            (
                "base",
                "hsl(210, 68%, 45.3921568627%)",
                "rgb(191.1000000002, 113.7500000001, 36.4)",
            ),
        ];

        for (theme, expected_peer, expected_inverse) in cases {
            let mut cfg = MermaidConfig::from_value(json!({
                "theme": theme,
                "themeVariables": {
                    "primaryColor": "#123456",
                    "cScale0": "#abcdef"
                }
            }));
            apply_theme_defaults(&mut cfg).unwrap();
            let actual = cfg
                .as_value()
                .get("themeVariables")
                .and_then(Value::as_object)
                .unwrap();

            assert_eq!(
                actual.get("cScale0").and_then(Value::as_str),
                Some("#abcdef"),
                "theme {theme} must replay explicit cScale0"
            );
            assert_eq!(
                actual.get("cScalePeer0").and_then(Value::as_str),
                Some(expected_peer),
                "theme {theme} must derive cScalePeer0 from the override stage"
            );
            assert_eq!(
                actual.get("cScaleInv0").and_then(Value::as_str),
                Some(expected_inverse),
                "theme {theme} must derive cScaleInv0 from the override stage"
            );
        }
    }

    #[test]
    fn extended_theme_scale_override_replays_after_source_ordered_derivations() {
        // Oracle: Mermaid 11.16.1 getThemeVariables({ cScale0: '#abcdef' }). Theme names do not
        // imply darkMode; only an explicit darkMode value changes the scale transform branch.
        let cases = [
            (
                "neo",
                "hsl(210, 68%, 45.3921568627%)",
                "rgb(191.1000000002, 113.7500000001, 36.4)",
                "#333",
            ),
            (
                "neo-dark",
                "hsl(210, 68%, 45.3921568627%)",
                "rgb(191.1000000002, 113.7500000001, 36.4)",
                "#e0dfdf",
            ),
            (
                "redux",
                "hsl(0, 0%, 65%)",
                "rgb(63.75, 63.75, 63.75)",
                "#28253D",
            ),
            (
                "redux-dark",
                "hsl(210, 68%, 45.3921568627%)",
                "rgb(191.1000000002, 113.7500000001, 36.4)",
                "#e0dfdf",
            ),
            (
                "redux-color",
                "hsl(210, 68%, 70.3921568627%)",
                "#543210",
                "#28253D",
            ),
            (
                "redux-dark-color",
                "hsl(210, 68%, 70.3921568627%)",
                "#543210",
                "hsl(210, 68%, 5.3921568627%)",
            ),
        ];

        for (theme, expected_peer, expected_inverse, expected_label) in cases {
            let mut config = MermaidConfig::from_value(json!({
                "theme": theme,
                "themeVariables": { "cScale0": "#abcdef" }
            }));
            apply_theme_defaults(&mut config).unwrap();
            let variables = config
                .as_value()
                .get("themeVariables")
                .and_then(Value::as_object)
                .unwrap();

            for (key, expected) in [
                ("cScale0", "#abcdef"),
                ("cScalePeer0", expected_peer),
                ("cScaleInv0", expected_inverse),
                ("cScaleLabel0", expected_label),
            ] {
                assert_eq!(
                    variables.get(key).and_then(Value::as_str),
                    Some(expected),
                    "theme {theme} has incorrect {key}"
                );
            }
        }
    }

    #[test]
    fn primary_color_override_follows_each_upstream_theme_scale_contract() {
        // Oracle values from Mermaid 11.16.1 `getThemeVariables()` with the same overrides.
        let cases = [
            (
                "default",
                "hsl(240, 100%, 76.2745098039%)",
                "hsl(240, 100%, 61.2745098039%)",
                "hsl(60, 100%, 86.2745098039%)",
            ),
            (
                "dark",
                "#123456",
                "hsl(210, 65.3846153846%, 30.3921568627%)",
                "#edcba9",
            ),
            (
                "forest",
                "hsl(210, 65.3846153846%, 10.3921568627%)",
                "hsl(210, 65.3846153846%, 0%)",
                "hsl(30, 65.3846153846%, 10.3921568627%)",
            ),
            ("neutral", "#555", "hsl(0, 0%, 23.3333333333%)", "#aaaaaa"),
            (
                "base",
                "hsl(210, 65.3846153846%, 0%)",
                "hsl(210, 65.3846153846%, 0%)",
                "#ffffff",
            ),
        ];

        for (theme, expected_scale, expected_peer, expected_inverse) in cases {
            let mut cfg = MermaidConfig::from_value(json!({
                "theme": theme,
                "themeVariables": {
                    "primaryColor": "#123456"
                }
            }));
            apply_theme_defaults(&mut cfg).unwrap();
            let actual = cfg
                .as_value()
                .get("themeVariables")
                .and_then(Value::as_object)
                .unwrap();

            for (key, expected) in [
                ("cScale0", expected_scale),
                ("cScalePeer0", expected_peer),
                ("cScaleInv0", expected_inverse),
            ] {
                assert_eq!(
                    actual.get(key).and_then(Value::as_str),
                    Some(expected),
                    "theme {theme} has incorrect {key} after primaryColor override"
                );
            }
        }
    }

    #[test]
    fn quadrant_primary_override_matches_mermaid_11_16_theme_lifecycles() {
        // Oracle: Mermaid 11.16.1 `mermaid.initialize()` + `mermaidAPI.getConfig()`.
        let cases = [
            ("default", "#ECECFF", "#f1f1ff", "hsl(240, 100%, NaN%)"),
            (
                "dark",
                "#123456",
                "#17395b",
                "hsl(210, 65.3846153846%, NaN%)",
            ),
            (
                "forest",
                "#123456",
                "#17395b",
                "hsl(210, 65.3846153846%, NaN%)",
            ),
            (
                "neutral",
                "#123456",
                "#17395b",
                "hsl(210, 65.3846153846%, NaN%)",
            ),
            (
                "base",
                "#123456",
                "#17395b",
                "hsl(210, 65.3846153846%, NaN%)",
            ),
            ("neo", "#ECECFE", "#f1f1ff", "hsl(240, 90%, NaN%)"),
            (
                "neo-dark",
                "#123456",
                "#17395b",
                "hsl(210, 65.3846153846%, NaN%)",
            ),
            ("redux", "#ECECFE", "#f1f1ff", "hsl(240, 90%, NaN%)"),
            (
                "redux-dark",
                "#123456",
                "#17395b",
                "hsl(210, 65.3846153846%, NaN%)",
            ),
            ("redux-color", "#ECECFE", "#f1f1ff", "hsl(240, 90%, NaN%)"),
            (
                "redux-dark-color",
                "#123456",
                "#17395b",
                "hsl(210, 65.3846153846%, NaN%)",
            ),
        ];

        for (theme, expected_q1, expected_q2, expected_point) in cases {
            let mut config = MermaidConfig::from_value(json!({
                "theme": theme,
                "themeVariables": { "primaryColor": "#123456" }
            }));
            apply_theme_defaults(&mut config).unwrap();
            let variables = config
                .as_value()
                .get("themeVariables")
                .and_then(Value::as_object)
                .unwrap();

            for (key, expected) in [
                ("quadrant1Fill", expected_q1),
                ("quadrant2Fill", expected_q2),
                ("quadrantPointFill", expected_point),
            ] {
                assert_eq!(
                    variables.get(key).and_then(Value::as_str),
                    Some(expected),
                    "theme {theme} has incorrect {key}"
                );
            }
        }
    }

    #[test]
    fn quadrant_partial_and_text_overrides_match_mermaid_11_16_replay_order() {
        let mut config = MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": {
                "primaryTextColor": "#123456",
                "quadrant1Fill": "rgba(18, 52, 86, 0.5)",
                "quadrant2Fill": "#abcdef"
            }
        }));
        apply_theme_defaults(&mut config).unwrap();
        let variables = config
            .as_value()
            .get("themeVariables")
            .and_then(Value::as_object)
            .unwrap();

        for (key, expected) in [
            ("quadrant1Fill", "rgba(18, 52, 86, 0.5)"),
            ("quadrant2Fill", "#abcdef"),
            ("quadrant3Fill", "#fffee7"),
            ("quadrant1TextFill", "#123456"),
            ("quadrant2TextFill", "#0d2f51"),
            ("quadrantPointFill", "hsla(210, 65.3846153846%, NaN%, 0.5)"),
        ] {
            assert_eq!(variables.get(key).and_then(Value::as_str), Some(expected));
        }

        let mut default_config = MermaidConfig::from_value(json!({
            "theme": "default",
            "themeVariables": { "primaryTextColor": "#123456" }
        }));
        apply_theme_defaults(&mut default_config).unwrap();
        assert_eq!(
            default_config.get_str("themeVariables.quadrant1TextFill"),
            Some("#131300")
        );
    }

    #[test]
    fn quadrant_accepts_khroma_named_rgb_and_alpha_colors() {
        // Oracle: Mermaid 11.16.1 `mermaid.initialize()` + `mermaidAPI.getConfig()`.
        let cases = [
            (
                "rebeccapurple",
                "#6b389e",
                "hsl(270, 50%, NaN%)",
                "hsl(270, 10%, 30%)",
            ),
            (
                "rgb(18, 52, 86)",
                "#17395b",
                "hsl(210, 65.3846153846%, NaN%)",
                "hsl(210, 25.3846153846%, 10.3921568627%)",
            ),
            (
                "rgba(18, 52, 86, 0.5)",
                "rgba(23, 57, 91, 0.5)",
                "hsla(210, 65.3846153846%, NaN%, 0.5)",
                "hsla(210, 25.3846153846%, 10.3921568627%, 0.5)",
            ),
        ];

        for (primary, expected_q2, expected_point, expected_border) in cases {
            let mut config = MermaidConfig::from_value(json!({
                "theme": "base",
                "themeVariables": { "primaryColor": primary }
            }));
            apply_theme_defaults(&mut config).unwrap();
            let variables = config
                .as_value()
                .get("themeVariables")
                .and_then(Value::as_object)
                .unwrap();

            for (key, expected) in [
                ("quadrant2Fill", expected_q2),
                ("quadrantPointFill", expected_point),
                ("quadrantInternalBorderStrokeFill", expected_border),
            ] {
                assert_eq!(
                    variables.get(key).and_then(Value::as_str),
                    Some(expected),
                    "primaryColor {primary} has incorrect {key}"
                );
            }
        }
    }

    #[test]
    fn invalid_color_timing_matches_mermaid_11_16_initialize_matrix() {
        // Oracle: Mermaid 11.16.1 `mermaid.initialize()` + `mermaidAPI.getConfig()` using
        // `not-a-color` for each field independently. Fields absent from `errors` are deliberate
        // pass-through values at theme-calculation time and must not be validated early.
        let keys = [
            "primaryColor",
            "secondaryColor",
            "tertiaryColor",
            "background",
            "primaryBorderColor",
            "secondaryBorderColor",
            "tertiaryBorderColor",
            "border1",
            "cScale0",
            "cScale1",
            "git0",
            "git1",
            "gitInv0",
            "pie1",
            "pie3",
            "quadrant1Fill",
        ];
        let cases: [(&str, &[&str]); 11] = [
            (
                "default",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "cScale0",
                    "cScale1",
                    "git0",
                    "git1",
                    "quadrant1Fill",
                ],
            ),
            (
                "dark",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "background",
                    "cScale0",
                    "cScale1",
                    "quadrant1Fill",
                ],
            ),
            (
                "forest",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "tertiaryColor",
                    "cScale0",
                    "cScale1",
                    "git0",
                    "git1",
                    "quadrant1Fill",
                ],
            ),
            (
                "neutral",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "border1",
                    "cScale0",
                    "cScale1",
                    "quadrant1Fill",
                ],
            ),
            (
                "base",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "tertiaryColor",
                    "background",
                    "cScale0",
                    "cScale1",
                    "git0",
                    "git1",
                    "quadrant1Fill",
                ],
            ),
            (
                "neo",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "tertiaryColor",
                    "background",
                    "cScale0",
                    "cScale1",
                    "git0",
                    "git1",
                    "quadrant1Fill",
                ],
            ),
            (
                "neo-dark",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "tertiaryColor",
                    "background",
                    "cScale0",
                    "cScale1",
                    "git0",
                    "git1",
                    "quadrant1Fill",
                ],
            ),
            (
                "redux",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "tertiaryColor",
                    "background",
                    "git0",
                    "git1",
                    "quadrant1Fill",
                ],
            ),
            (
                "redux-dark",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "tertiaryColor",
                    "background",
                    "cScale0",
                    "cScale1",
                    "git0",
                    "git1",
                    "quadrant1Fill",
                ],
            ),
            (
                "redux-color",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "tertiaryColor",
                    "background",
                    "cScale0",
                    "cScale1",
                    "git0",
                    "git1",
                    "quadrant1Fill",
                ],
            ),
            (
                "redux-dark-color",
                &[
                    "primaryColor",
                    "secondaryColor",
                    "tertiaryColor",
                    "background",
                    "cScale0",
                    "cScale1",
                    "git0",
                    "git1",
                    "quadrant1Fill",
                ],
            ),
        ];

        let mut mismatches = Vec::new();
        for (theme, error_keys) in cases {
            for key in keys {
                let mut config = MermaidConfig::from_value(json!({
                    "theme": theme,
                    "themeVariables": { (key): "not-a-color" }
                }));
                let actual_error = apply_theme_defaults(&mut config).is_err();
                let expected_error = error_keys.contains(&key);
                if actual_error != expected_error {
                    mismatches.push(format!(
                        "{theme}.{key}: expected error={expected_error}, actual={actual_error}"
                    ));
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "invalid-color timing mismatches:\n{}",
            mismatches.join("\n")
        );
    }

    #[test]
    fn invalid_theme_colors_fail_at_direct_and_site_config_operation_boundaries() {
        let mut config = MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": { "primaryColor": "not-a-color" }
        }));
        assert!(matches!(
            apply_theme_defaults(&mut config),
            Err(ThemeResolutionError::Color(
                ColorError::UnsupportedFormat { .. }
            ))
        ));

        let engine = crate::Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": { "primaryColor": "not-a-color" }
        })));
        assert!(matches!(
            engine.parse_metadata_sync("flowchart TD\n  A"),
            Err(crate::Error::ThemeColor(
                ColorError::UnsupportedFormat { .. }
            ))
        ));
    }

    #[test]
    fn invalid_frontmatter_theme_color_fails_at_parse_operation_boundary() {
        let engine = crate::Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "secure": [
                "secure",
                "securityLevel",
                "startOnLoad",
                "maxTextSize",
                "suppressErrorRendering",
                "maxEdges"
            ]
        })));
        let source = r#"---
config:
  theme: base
  themeVariables:
    primaryColor: not-a-color
---
flowchart TD
  A
"#;

        assert!(matches!(
            engine.parse_metadata_sync(source),
            Err(crate::Error::ThemeColor(
                ColorError::UnsupportedFormat { .. }
            ))
        ));
    }

    #[test]
    fn staged_theme_failure_is_transactional_for_config_and_compatibility_ownership() {
        let binding = crate::config::ThemeParseBinding::try_new(
            [0x5a; 32],
            MermaidConfig::from_value(json!({
                "theme": "dark",
                "themeVariables": {
                    "mainBkg": "#abcdef",
                    "nodeBkg": "#123456"
                }
            })),
        )
        .expect("valid dark compatibility binding");
        let mut config = MermaidConfig::from_theme_parse_binding(binding);
        // Keep compatibility ownership attached while presenting the staged executor with one
        // invalid color and one null override that normalization would otherwise retire.
        config.set_value_preserving_theme_compatibility(
            "themeVariables.mainBkg",
            Value::String("not-a-color".to_string()),
        );
        config.set_value_preserving_theme_compatibility("themeVariables.nodeBkg", Value::Null);

        let before_value = config.as_value().clone();
        let mut expected_compatibility = config.clone();
        expected_compatibility.freeze_theme_compatibility();

        assert!(apply_theme_defaults(&mut config).is_err());
        assert_eq!(config.as_value(), &before_value);

        config.freeze_theme_compatibility();
        assert_eq!(
            config.mermaid_compatibility_fields(),
            expected_compatibility.mermaid_compatibility_fields(),
            "a failed staged execution must not retire compatibility ownership"
        );
    }

    #[test]
    fn legacy_theme_failure_is_transactional_for_config_and_compatibility_ownership() {
        let binding = crate::config::ThemeParseBinding::try_new(
            [0x6a; 32],
            MermaidConfig::from_value(json!({
                "theme": "base",
                "themeVariables": {
                    "nodeBkg": "#123456",
                    "cScale2": "#abcdef"
                }
            })),
        )
        .expect("valid Base compatibility binding");
        let mut config = MermaidConfig::from_theme_parse_binding(binding);
        config.set_value_preserving_theme_compatibility(
            "themeVariables.cScale2",
            Value::String("not-a-color".to_string()),
        );
        config.set_value_preserving_theme_compatibility("themeVariables.nodeBkg", Value::Null);

        let before_value = config.as_value().clone();
        let mut expected_compatibility = config.clone();
        expected_compatibility.freeze_theme_compatibility();

        assert!(apply_theme_defaults(&mut config).is_err());
        assert_eq!(config.as_value(), &before_value);

        config.freeze_theme_compatibility();
        assert_eq!(
            config.mermaid_compatibility_fields(),
            expected_compatibility.mermaid_compatibility_fields(),
            "a failed legacy execution must not retire compatibility ownership"
        );
    }

    #[test]
    fn staged_theme_color_limit_fails_closed_before_mutation() {
        for value in [json!(65), json!("0x41"), json!("Infinity")] {
            let mut config = MermaidConfig::from_value(json!({
                "theme": "dark",
                "themeVariables": {
                    "THEME_COLOR_LIMIT": value,
                    "nodeBkg": "#123456"
                }
            }));
            let before = config.as_value().clone();

            let error = apply_theme_defaults(&mut config).unwrap_err();
            let ThemeResolutionError::EvaluationLimit(error) = error else {
                panic!("expected a theme evaluation limit error");
            };
            assert_eq!(error.limit, "THEME_COLOR_LIMIT");
            assert_eq!(error.max, staged::MAX_THEME_COLOR_ITERATIONS);
            assert_eq!(config.as_value(), &before);
        }

        let engine = crate::Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "theme": "dark",
            "themeVariables": { "THEME_COLOR_LIMIT": 65 }
        })));
        assert!(matches!(
            engine.parse_metadata_sync("flowchart TD\n  A"),
            Err(crate::Error::ThemeEvaluationLimit(
                crate::ThemeEvaluationLimitExceeded {
                    limit: "THEME_COLOR_LIMIT",
                    max: staged::MAX_THEME_COLOR_ITERATIONS,
                    ..
                }
            ))
        ));
    }

    #[test]
    fn staged_object_dependencies_remain_field_local() {
        let metadata = crate::Engine::new()
            .with_site_config(MermaidConfig::from_value(json!({
                "theme": "dark",
                "themeVariables": { "background": "#123456" }
            })))
            .parse_metadata_sync("flowchart TD\n  A")
            .expect("materialize a Dark theme with an explicit background");
        let config = metadata.effective_config;

        assert_eq!(
            config.get_str("themeVariables.xyChart.backgroundColor"),
            Some("#123456")
        );
        assert!(
            config.config_path_overrides_typed_default("themeVariables.xyChart.backgroundColor"),
            "the derived XY background must retain ownership from the explicit root background"
        );
        assert!(
            !config.config_path_overrides_typed_default("themeVariables.xyChart.plotColorPalette"),
            "one derived object field must not claim its constructor-owned siblings"
        );
        assert!(
            !config.config_path_overrides_typed_default("themeVariables.xyChart.titleColor"),
            "field-local ownership must not widen to the whole XY theme object"
        );
    }

    #[test]
    fn staged_theme_interface_matches_upstream_stage_oracles_and_ownership() {
        let mut mismatches = Vec::new();
        for case in &generated_theme_oracles().stage_oracle_cases {
            let theme_name = case.get("theme").and_then(Value::as_str).unwrap();
            let theme = MermaidThemeId::parse(theme_name).unwrap();
            let id = case.get("id").and_then(Value::as_str).unwrap();
            let program = *ThemeProgram::resolve(theme);
            let Some(staged_program) = program.staged_program() else {
                mismatches.push(format!(
                    "{theme_name}/{id}: stage oracle targets a non-staged theme program"
                ));
                continue;
            };
            let raw = case
                .get("overrides")
                .and_then(Value::as_object)
                .unwrap()
                .clone();
            let explicit = program.normalize_overrides(raw);
            let resolution = staged::Resolution::execute(
                staged_program,
                program.prepared_constructor(),
                &explicit,
            )
            .unwrap();
            let trace = resolution.trace();
            let actual_stages = [
                ("constructorPrepared", &trace.constructor_prepared),
                ("overridesApplied", &trace.overrides_applied),
                ("afterUpdate", &trace.after_update),
                ("explicitReplay", &trace.explicit_replay),
            ];
            let expected_stages = case.get("stages").and_then(Value::as_object).unwrap();
            for (stage_name, actual) in actual_stages {
                let expected = expected_stages
                    .get(stage_name)
                    .and_then(Value::as_object)
                    .unwrap();
                let actual = Value::Object(actual.variables.clone());
                for (path, observation) in expected {
                    let state = observation.get("state").and_then(Value::as_str).unwrap();
                    let actual_value = value_at_path(&actual, path);
                    match state {
                        "missing" if actual_value.is_some() => mismatches.push(format!(
                            "{theme_name}/{id}/{stage_name}/{path}: expected missing, found {}",
                            actual_value.unwrap()
                        )),
                        "value" if actual_value != observation.get("value") => {
                            mismatches.push(format!(
                                "{theme_name}/{id}/{stage_name}/{path}: expected {}, found {}",
                                observation.get("value").unwrap(),
                                actual_value
                                    .map(Value::to_string)
                                    .unwrap_or_else(|| "missing".to_string())
                            ));
                        }
                        "missing" | "value" => {}
                        other => panic!("unexpected stage observation state `{other}`"),
                    }
                }
            }

            match id {
                "primary-and-derived-replay" => {
                    assert!(
                        trace.after_update.depends_on("fillType0", "primaryColor"),
                        "{theme_name} Journey fill must inherit primaryColor ownership"
                    );
                    assert!(
                        !trace.after_update.depends_on("mainBkg", "primaryColor"),
                        "{theme_name} constructor-owned mainBkg must remain independent"
                    );
                    match theme {
                        MermaidThemeId::Dark => {
                            assert!(trace.after_update.depends_on("cScale0", "primaryColor"));
                            assert!(!trace.after_update.depends_on("cScale2", "primaryColor"));
                        }
                        MermaidThemeId::Forest => {
                            assert!(trace.after_update.depends_on("cScale0", "primaryColor"));
                            assert!(!trace.after_update.depends_on("cScale2", "primaryColor"));
                            assert!(trace.after_update.depends_on("cScale3", "primaryColor"));
                        }
                        MermaidThemeId::Neutral => {
                            for target in ["cScale0", "cScale2", "cScale3"] {
                                assert!(
                                    !trace.after_update.depends_on(target, "primaryColor"),
                                    "Neutral {target} is constructor/update constant"
                                );
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                "color-limit-and-tag-replay" => {
                    for target in ["cScaleInv8", "cScalePeer8", "cScaleLabel8"] {
                        assert!(
                            !trace.after_update.depends_on(target, target),
                            "{theme_name} {target} must be replaced inside the active loop bound"
                        );
                        assert!(
                            !trace.after_update.depends_on(target, "THEME_COLOR_LIMIT"),
                            "loop control must not become value ownership for {theme_name} {target}"
                        );
                    }
                    for target in ["cScaleInv9", "cScalePeer9", "cScaleLabel9"] {
                        assert!(
                            trace.after_update.depends_on(target, target),
                            "{theme_name} {target} must remain the skipped explicit value"
                        );
                        assert!(
                            !trace.after_update.depends_on(target, "THEME_COLOR_LIMIT"),
                            "{theme_name} {target} must not be written beyond the loop bound"
                        );
                    }
                    if theme == MermaidThemeId::Forest {
                        for target in ["pie8", "pie9"] {
                            assert!(!trace.after_update.depends_on(target, "THEME_COLOR_LIMIT"));
                            assert!(!trace.after_update.depends_on(target, target));
                            assert!(trace.explicit_replay.depends_on(target, target));
                        }
                    } else {
                        assert!(!trace.after_update.depends_on("pie8", "pie8"));
                        assert!(!trace.after_update.depends_on("pie8", "THEME_COLOR_LIMIT"));
                        assert!(trace.after_update.depends_on("pie9", "pie9"));
                        assert!(!trace.after_update.depends_on("pie9", "THEME_COLOR_LIMIT"));
                    }
                    assert!(
                        !trace
                            .after_update
                            .depends_on("tagLabelBorder", "tagLabelBorder"),
                        "the unconditional Tag assignment must replace explicit ownership"
                    );
                    assert!(
                        trace
                            .explicit_replay
                            .depends_on("tagLabelBorder", "tagLabelBorder"),
                        "explicit replay must restore Tag ownership"
                    );
                    if matches!(theme, MermaidThemeId::Dark | MermaidThemeId::Neutral) {
                        assert!(
                            trace.after_update.depends_on("scaleLabelColor", "darkMode"),
                            "{theme_name} false darkMode branch must retain control provenance"
                        );
                    }
                }
                "falsy-or-chain" => {
                    assert!(
                        trace
                            .after_update
                            .depends_on("stateLabelColor", "primaryTextColor")
                    );
                    assert!(!trace.after_update.depends_on("stateLabelColor", "stateBkg"));
                }
                "operator-and-replay" => {
                    for target in ["primaryBorderColor", "gradientStart", "pieOpacity", "venn1"] {
                        assert!(trace.explicit_replay.depends_on(target, target));
                    }
                    assert!(
                        !trace.after_update.depends_on("pieOpacity", "pieOpacity"),
                        "JavaScript || must replace explicit numeric zero before replay"
                    );
                    assert!(
                        trace.after_update.depends_on("venn1", "venn1"),
                        "JavaScript ?? must retain explicit numeric zero"
                    );
                }
                "null-elision" => {
                    assert!(
                        !trace.overrides_applied.depends_on("venn1", "venn1"),
                        "public initialize semantics must elide a top-level null override"
                    );
                    assert!(!trace.explicit_replay.depends_on("venn1", "venn1"));
                }
                other => panic!("unexpected staged oracle `{theme_name}/{other}`"),
            }
        }

        assert!(
            mismatches.is_empty(),
            "staged Mermaid theme mismatches:\n{}",
            mismatches.join("\n")
        );
    }

    #[test]
    fn default_theme_populates_mermaid_theme_variables() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "default"
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("background").and_then(|v| v.as_str()), Some("white"));
        assert_eq!(
            tv.get("primaryColor").and_then(|v| v.as_str()),
            Some("#ECECFF")
        );
        assert_eq!(
            tv.get("secondaryColor").and_then(|v| v.as_str()),
            Some("#ffffde")
        );
        assert_eq!(tv.get("pie1").and_then(|v| v.as_str()), Some("#ECECFF"));
        assert_eq!(tv.get("pie2").and_then(|v| v.as_str()), Some("#ffffde"));
        assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#ECECFF"));
        assert_eq!(
            tv.get("nodeBorder").and_then(|v| v.as_str()),
            Some("#9370DB")
        );
        assert_eq!(
            tv.get("edgeLabelBackground").and_then(|v| v.as_str()),
            Some("rgba(232,232,232, 0.8)")
        );
        assert_eq!(
            tv.get("classText").and_then(|v| v.as_str()),
            Some("#131300")
        );
        assert_eq!(
            tv.get("noteTextColor").and_then(|v| v.as_str()),
            Some("black")
        );
        assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            tv.get("gradientStart").and_then(|v| v.as_str()),
            Some("hsl(240, 60%, 86.2745098039%)")
        );

        let xy = tv.get("xyChart").and_then(|v| v.as_object()).unwrap();
        assert_eq!(
            xy.get("backgroundColor").and_then(|v| v.as_str()),
            Some("white")
        );
        assert_eq!(
            xy.get("dataLabelColor").and_then(|v| v.as_str()),
            Some("#131300")
        );
    }

    #[test]
    fn default_theme_preserves_user_overrides_after_derivation() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "default",
            "themeVariables": {
                "primaryColor": "#111111",
                "mainBkg": "#101010",
                "classText": "#abcdef",
                "xyChart": {
                    "titleColor": "red"
                }
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(
            tv.get("primaryColor").and_then(|v| v.as_str()),
            Some("#111111")
        );
        assert_eq!(tv.get("pie1").and_then(|v| v.as_str()), Some("#ECECFF"));
        assert_eq!(tv.get("pie2").and_then(|v| v.as_str()), Some("#ffffde"));
        assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#101010"));
        assert_eq!(tv.get("nodeBkg").and_then(|v| v.as_str()), Some("#101010"));
        assert_eq!(
            tv.get("classText").and_then(|v| v.as_str()),
            Some("#abcdef")
        );
        assert_eq!(
            tv.get("primaryTextColor").and_then(|v| v.as_str()),
            Some("#131300")
        );

        let xy = tv.get("xyChart").and_then(|v| v.as_object()).unwrap();
        assert_eq!(xy.get("titleColor").and_then(|v| v.as_str()), Some("red"));
        assert_eq!(xy.get("dataLabelColor"), None);
    }

    #[test]
    fn default_theme_merges_unrelated_theme_variable_overrides_without_hsl_rewriting_pie_base() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "default",
            "themeVariables": {
                "pieOuterStrokeWidth": "5px"
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("pie1").and_then(|v| v.as_str()), Some("#ECECFF"));
        assert_eq!(tv.get("pie2").and_then(|v| v.as_str()), Some("#ffffde"));
        assert_eq!(
            tv.get("pieOuterStrokeWidth").and_then(|v| v.as_str()),
            Some("5px")
        );
    }

    #[test]
    fn raw_mermaid_config_preserves_upstream_unsupported_theme_fallback() {
        for requested in ["unknown", "null"] {
            let mut config = MermaidConfig::from_value(json!({
                "theme": requested
            }));
            assert!(MermaidThemeId::parse(requested).is_err());
            apply_theme_defaults(&mut config).unwrap();

            let variables = config
                .as_value()
                .get("themeVariables")
                .and_then(Value::as_object)
                .unwrap();

            assert_eq!(
                variables,
                ThemeProgram::resolve(MermaidThemeId::Default).resolved_without_overrides()
            );
            assert_eq!(config.get_str("theme"), Some(requested));
        }
    }

    #[test]
    fn mermaid_11_16_extended_theme_names_use_their_snapshots() {
        let cases = [
            ("neo", "#cccccc", "#000000"),
            ("neo-dark", "#1f2020", "#ccc"),
            ("redux", "#cccccc", "#28253D"),
            ("redux-dark", "#1f2020", "#FFFFFF"),
            ("redux-color", "#cccccc", "#28253D"),
            ("redux-dark-color", "#1f2020", "#FFFFFF"),
        ];

        for (theme, primary, node_border) in cases {
            let mut cfg = MermaidConfig::from_value(json!({
                "theme": theme
            }));
            apply_theme_defaults(&mut cfg).unwrap();

            let tv = cfg
                .as_value()
                .get("themeVariables")
                .and_then(|v| v.as_object())
                .unwrap();

            assert_eq!(
                tv.get("primaryColor").and_then(|v| v.as_str()),
                Some(primary),
                "theme {theme}"
            );
            assert_eq!(
                tv.get("nodeBorder").and_then(|v| v.as_str()),
                Some(node_border),
                "theme {theme}"
            );
        }
    }

    #[test]
    fn extended_theme_preserves_explicit_theme_variable_overrides() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "redux",
            "themeVariables": {
                "primaryColor": "#123456",
                "nodeBorder": "#abcdef"
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(
            tv.get("primaryColor").and_then(|v| v.as_str()),
            Some("#123456")
        );
        assert_eq!(
            tv.get("nodeBorder").and_then(|v| v.as_str()),
            Some("#abcdef")
        );
        assert_eq!(
            tv.get("fontFamily").and_then(|v| v.as_str()),
            Some("\"Recursive Variable\", arial, sans-serif")
        );
    }

    #[test]
    fn extended_theme_recomputes_visible_derivations_from_base_overrides() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "redux",
            "themeVariables": {
                "primaryColor": "#123456"
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(
            tv.get("primaryColor").and_then(|v| v.as_str()),
            Some("#123456")
        );
        assert_eq!(tv.get("nodeBkg").and_then(|v| v.as_str()), Some("#123456"));
        assert_eq!(
            tv.get("secondaryColor").and_then(|v| v.as_str()),
            Some("hsl(90, 65.3846153846%, 20.3921568627%)")
        );
        assert_eq!(
            tv.get("edgeLabelBackground").and_then(|v| v.as_str()),
            Some("hsl(90, 65.3846153846%, 20.3921568627%)")
        );
        assert_eq!(
            tv.get("tagLabelBackground").and_then(|v| v.as_str()),
            Some("#123456")
        );
        assert_eq!(
            tv.get("fontFamily").and_then(|v| v.as_str()),
            Some("\"Recursive Variable\", arial, sans-serif")
        );
        assert_eq!(
            tv.get("git0").and_then(|v| v.as_str()),
            Some("hsl(240, 90%, 71.0784313725%)")
        );
    }

    #[test]
    fn extended_theme_explicit_derived_overrides_still_win() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "redux",
            "themeVariables": {
                "primaryColor": "#123456",
                "nodeBkg": "#abcdef",
                "edgeLabelBackground": "#fedcba"
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("nodeBkg").and_then(|v| v.as_str()), Some("#abcdef"));
        assert_eq!(
            tv.get("edgeLabelBackground").and_then(|v| v.as_str()),
            Some("#fedcba")
        );
        assert_eq!(
            tv.get("secondaryColor").and_then(|v| v.as_str()),
            Some("hsl(90, 65.3846153846%, 20.3921568627%)")
        );
    }

    #[test]
    fn extended_theme_recomputes_background_and_main_background_derivations() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "redux",
            "themeVariables": {
                "background": "#010203",
                "mainBkg": "#101112"
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        for key in [
            "lineColor",
            "arrowheadColor",
            "defaultLinkColor",
            "archEdgeColor",
            "archEdgeArrowColor",
            "relationColor",
            "transitionColor",
            "specialStateColor",
        ] {
            assert_eq!(
                tv.get(key).and_then(|v| v.as_str()),
                Some("#fefdfc"),
                "key {key}"
            );
        }

        for key in [
            "actorBkg",
            "labelBoxBkgColor",
            "personBkg",
            "stateBkg",
            "labelBackgroundColor",
        ] {
            assert_eq!(
                tv.get(key).and_then(|v| v.as_str()),
                Some("#101112"),
                "key {key}"
            );
        }
        assert_eq!(tv.get("nodeBkg").and_then(|v| v.as_str()), Some("#cccccc"));
    }

    #[test]
    fn dark_extended_theme_recomputes_primary_visible_derivations() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "redux-dark",
            "themeVariables": {
                "primaryColor": "#123456"
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        for key in ["requirementBackground", "pie1", "quadrant1Fill"] {
            assert_eq!(
                tv.get(key).and_then(|v| v.as_str()),
                Some("#123456"),
                "key {key}"
            );
        }
        assert_eq!(
            tv.get("git0").and_then(|v| v.as_str()),
            Some("hsl(210, 65.3846153846%, 0%)")
        );
        assert_eq!(
            tv.get("git1").and_then(|v| v.as_str()),
            Some("hsl(180, 1.5873015873%, 3.3529411765%)")
        );
        assert_eq!(
            tv.get("git3").and_then(|v| v.as_str()),
            Some("hsl(180, 65.3846153846%, 0%)")
        );
        assert_eq!(tv.get("gitInv0").and_then(|v| v.as_str()), Some("#ffffff"));
        assert_eq!(
            tv.get("gitInv1").and_then(|v| v.as_str()),
            Some("rgb(246.5857142856, 246.3142857142, 246.3142857142)")
        );
    }

    #[test]
    fn extended_theme_explicit_git_color_derives_git_inverse_unless_explicit() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "redux",
            "themeVariables": {
                "git0": "#000000",
                "git1": "#111111",
                "gitInv1": "#222222"
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("git0").and_then(|v| v.as_str()), Some("#000000"));
        assert_eq!(tv.get("gitInv0").and_then(|v| v.as_str()), Some("#ffffff"));
        assert_eq!(tv.get("git1").and_then(|v| v.as_str()), Some("#111111"));
        assert_eq!(tv.get("gitInv1").and_then(|v| v.as_str()), Some("#222222"));
    }

    #[test]
    fn base_theme_derivation_matches_upstream_fixture_values() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "base",
            "themeVariables": {
                "primaryColor": "#411d4e",
                "titleColor": "white",
                "darkMode": true
            }
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("textColor").and_then(|v| v.as_str()), Some("#eee"));
        assert_eq!(
            tv.get("lineColor").and_then(|v| v.as_str()),
            Some("#0b0b0b")
        );
        assert_eq!(
            tv.get("nodeBorder").and_then(|v| v.as_str()),
            Some("hsl(284.0816326531, 5.7943925234%, 30.9803921569%)")
        );
        assert_eq!(
            tv.get("secondaryBorderColor").and_then(|v| v.as_str()),
            Some("hsl(164.0816326531, 5.7943925234%, 30.9803921569%)")
        );
        assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            tv.get("gradientStart").and_then(|v| v.as_str()),
            Some("hsl(284.0816326531, 5.7943925234%, 30.9803921569%)")
        );
        assert_eq!(
            tv.get("gradientStop").and_then(|v| v.as_str()),
            Some("hsl(164.0816326531, 5.7943925234%, 30.9803921569%)")
        );
        assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#411d4e"));
        assert_eq!(
            tv.get("clusterBkg").and_then(|v| v.as_str()),
            Some("hsl(104.0816326531, 45.7943925234%, 25.9803921569%)")
        );
        assert_eq!(
            tv.get("clusterBorder").and_then(|v| v.as_str()),
            Some("hsl(104.0816326531, 5.7943925234%, 35.9803921569%)")
        );
        assert_eq!(
            tv.get("edgeLabelBackground").and_then(|v| v.as_str()),
            Some("hsl(164.0816326531, 45.7943925234%, 0%)")
        );
        assert_eq!(
            tv.get("errorBkgColor").and_then(|v| v.as_str()),
            Some("hsl(104.0816326531, 45.7943925234%, 25.9803921569%)")
        );
        assert_eq!(
            tv.get("errorTextColor").and_then(|v| v.as_str()),
            Some("rgb(202.9906542056, 158.4112149531, 219.0887850467)")
        );
        assert_eq!(tv.get("titleColor").and_then(|v| v.as_str()), Some("white"));
    }

    #[test]
    fn forest_theme_derives_cscale_palette_like_upstream() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "forest"
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(
            tv.get("cScale0").and_then(|v| v.as_str()),
            Some("hsl(78.1578947368, 58.4615384615%, 64.5098039216%)")
        );
        assert_eq!(
            tv.get("cScalePeer0").and_then(|v| v.as_str()),
            Some("hsl(78.1578947368, 58.4615384615%, 39.5098039216%)")
        );
        assert_eq!(
            tv.get("cScalePeer1").and_then(|v| v.as_str()),
            Some("hsl(98.961038961, 100%, 39.9019607843%)")
        );
        assert_eq!(
            tv.get("cScalePeer2").and_then(|v| v.as_str()),
            Some("hsl(78.1578947368, 58.4615384615%, 44.5098039216%)")
        );
        assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            tv.get("gradientStart").and_then(|v| v.as_str()),
            Some("hsl(78.1578947368, 18.4615384615%, 64.5098039216%)")
        );
        assert_eq!(
            tv.get("gradientStop").and_then(|v| v.as_str()),
            Some("hsl(98.961038961, 60%, 74.9019607843%)")
        );
    }

    #[test]
    fn dark_theme_derives_peer_and_inverted_scales_like_upstream() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "dark"
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("cScale1").and_then(|v| v.as_str()), Some("#0b0000"));
        assert_eq!(
            tv.get("cScalePeer1").and_then(|v| v.as_str()),
            Some("hsl(0, 100%, 12.1568627451%)")
        );
        assert_eq!(
            tv.get("cScaleInv1").and_then(|v| v.as_str()),
            Some("#f4ffff")
        );
        assert_eq!(
            tv.get("cScaleLabel1").and_then(|v| v.as_str()),
            Some("lightgrey")
        );
        assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            tv.get("gradientStart").and_then(|v| v.as_str()),
            Some("#cccccc")
        );
        assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#1f2020"));
        assert_eq!(
            tv.get("lineColor").and_then(|v| v.as_str()),
            Some("lightgrey")
        );
        assert_eq!(
            tv.get("actorTextColor").and_then(|v| v.as_str()),
            Some("lightgrey")
        );
        assert_eq!(
            tv.get("classText").and_then(|v| v.as_str()),
            Some("#e0dfdf")
        );
        assert_eq!(
            tv.get("noteTextColor").and_then(|v| v.as_str()),
            Some("rgb(183.8476190475, 181.5523809523, 181.5523809523)")
        );
        assert_eq!(
            tv.get("taskTextDarkColor").and_then(|v| v.as_str()),
            Some("#2c2c2c")
        );
        assert_eq!(
            tv.get("attributeBackgroundColorOdd")
                .and_then(|v| v.as_str()),
            Some("hsl(0, 0%, 32%)")
        );
    }

    #[test]
    fn neutral_theme_derives_peer_and_label_scales_like_upstream() {
        let mut cfg = MermaidConfig::from_value(json!({
            "theme": "neutral"
        }));
        apply_theme_defaults(&mut cfg).unwrap();

        let tv = cfg
            .as_value()
            .get("themeVariables")
            .and_then(|v| v.as_object())
            .unwrap();

        assert_eq!(tv.get("cScale0").and_then(|v| v.as_str()), Some("#555"));
        assert_eq!(
            tv.get("cScalePeer0").and_then(|v| v.as_str()),
            Some("hsl(0, 0%, 23.3333333333%)")
        );
        assert_eq!(
            tv.get("cScaleInv0").and_then(|v| v.as_str()),
            Some("#aaaaaa")
        );
        assert_eq!(
            tv.get("cScaleLabel0").and_then(|v| v.as_str()),
            Some("#F4F4F4")
        );
        assert_eq!(tv.get("useGradient").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            tv.get("gradientStart").and_then(|v| v.as_str()),
            Some("hsl(0, 0%, 83.3333333333%)")
        );
        assert_eq!(
            tv.get("gradientStop").and_then(|v| v.as_str()),
            Some("hsl(0, 0%, 88.9215686275%)")
        );
        assert_eq!(tv.get("mainBkg").and_then(|v| v.as_str()), Some("#eee"));
        assert_eq!(
            tv.get("textColor").and_then(|v| v.as_str()),
            Some("#000000")
        );
        assert_eq!(
            tv.get("actorTextColor").and_then(|v| v.as_str()),
            Some("#333")
        );
        assert_eq!(
            tv.get("classText").and_then(|v| v.as_str()),
            Some("#111111")
        );
        assert_eq!(
            tv.get("noteBkgColor").and_then(|v| v.as_str()),
            Some("#666")
        );
        assert_eq!(
            tv.get("noteTextColor").and_then(|v| v.as_str()),
            Some("#fff")
        );
        assert_eq!(
            tv.get("taskTextOutsideColor").and_then(|v| v.as_str()),
            Some("#333")
        );
    }
}
