use crate::MermaidConfig;
use crate::presentation::SourcePresentationConfig;
use crate::theme_color::{self, ColorAdjustment, ColorError};
use merman_model::config::MermaidThemeId;
use merman_model::presentation::ThemeEvaluationLimitExceeded;
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
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

const THEME_RUNTIME_SCHEMA_VERSION: u32 = 3;
const THEME_ORACLE_CASE_COUNT: usize = MermaidThemeId::ALL.len() * 5 + 12;
#[cfg(test)]
const THEME_AUDIT_SCHEMA_VERSION: u32 = 2;

// Generated from the content-pinned Mermaid runtime by `xtask gen-theme-snapshot`.
static GENERATED_THEME_RUNTIME: OnceLock<GeneratedThemeRuntime<'static>> = OnceLock::new();

#[cfg(test)]
static GENERATED_THEME_ORACLES: OnceLock<GeneratedThemeOracles> = OnceLock::new();

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedThemeRuntime<'a> {
    schema_version: u32,
    provenance: GeneratedThemeProvenance,
    #[serde(borrow)]
    prepared_constructors: BTreeMap<String, GeneratedThemeMap<'a>>,
    #[serde(borrow)]
    themes: BTreeMap<String, GeneratedThemeMap<'a>>,
    #[serde(borrow)]
    dark_mode_true_overrides: BTreeMap<String, GeneratedThemeMap<'a>>,
    oracle_case_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct GeneratedThemeMap<'a> {
    #[serde(borrow)]
    encoded: &'a RawValue,
    #[serde(skip)]
    decoded: OnceLock<Map<String, Value>>,
}

impl GeneratedThemeMap<'_> {
    fn resolved(&self) -> &Map<String, Value> {
        self.decoded.get_or_init(|| {
            serde_json::from_str(self.encoded.get())
                .expect("generated theme snapshot has an object")
        })
    }

    fn resolved_with_base<'base>(
        &self,
        base: impl FnOnce() -> &'base Map<String, Value>,
    ) -> &Map<String, Value> {
        self.decoded.get_or_init(|| {
            let changes = serde_json::from_str(self.encoded.get())
                .expect("generated dark mode delta has an object");
            restore_theme_overrides(base(), changes)
        })
    }
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
    Staged(staged::StagedProgram),
    Extended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeDependencyGraph {
    None,
    DarkenedScale,
    DarkenedScaleAndGit,
    DynamicGit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeVariableDependencyScope {
    Extended,
    ExtendedLight,
    ExtendedDark,
    ExtendedDarkWithoutReduxColor,
    ReduxColor,
    ReduxDarkColor,
    LegacyNonDefault,
    NeoFamily,
    NeoDark,
    ExtendedWithoutReduxDark,
    ExtendedWithoutReduxVariants,
    ExtendedWithoutReduxDarkVariants,
}

impl ThemeVariableDependencyScope {
    const fn includes(self, theme: MermaidThemeId) -> bool {
        match self {
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
            Self::ExtendedDarkWithoutReduxColor => {
                matches!(theme, MermaidThemeId::NeoDark | MermaidThemeId::ReduxDark)
            }
            Self::ReduxColor => matches!(
                theme,
                MermaidThemeId::ReduxColor | MermaidThemeId::ReduxDarkColor
            ),
            Self::ReduxDarkColor => matches!(theme, MermaidThemeId::ReduxDarkColor),
            Self::LegacyNonDefault => matches!(
                theme,
                MermaidThemeId::Neo
                    | MermaidThemeId::NeoDark
                    | MermaidThemeId::Redux
                    | MermaidThemeId::ReduxDark
                    | MermaidThemeId::ReduxColor
                    | MermaidThemeId::ReduxDarkColor
            ),
            Self::NeoFamily => matches!(theme, MermaidThemeId::Neo | MermaidThemeId::NeoDark),
            Self::NeoDark => matches!(theme, MermaidThemeId::NeoDark),
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
    SourceExplicitTruthy,
}

impl ThemeVariableDependencyCondition {
    fn permits(self, explicit_source_truthiness: Option<bool>) -> bool {
        match self {
            Self::Always => true,
            Self::SourceExplicitTruthy => explicit_source_truthiness == Some(true),
        }
    }
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

    const fn copied_when_source_explicit_truthy(
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
            condition: ThemeVariableDependencyCondition::SourceExplicitTruthy,
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

// The pinned Mermaid programs calculate through unconditional intermediate assignments
// before replaying explicit targets. Preserve the order and assignment mode of these steps.
const THEME_VARIABLE_DEPENDENCIES: &[ThemeVariableDependency] = &[
    ThemeVariableDependency::transformed(
        ThemeVariableDependencyScope::NeoFamily,
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
    copied_theme_dependency!(Extended, "primaryTextColor", "nodeTextColor"),
    copied_theme_dependency!(Extended, "tertiaryTextColor", "titleColor"),
    copied_theme_dependency!(NeoDark, "border1", "nodeBorder"),
    copied_theme_dependency!(Extended, "lineColor", "defaultLinkColor"),
    copied_theme_dependency!(Extended, "secondaryBorderColor", "flowContainerStroke"),
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
    copied_theme_dependency!(ExtendedDarkWithoutReduxColor, "primaryColor", "pie1"),
    copied_theme_dependency!(ReduxColor, "cScale0", "pie1"),
    copied_theme_dependency!(ReduxColor, "cScale1", "pie2"),
    copied_theme_dependency!(ReduxColor, "cScale2", "pie3"),
    copied_theme_dependency!(ReduxColor, "cScale3", "pie4"),
    copied_theme_dependency!(ReduxColor, "cScale4", "pie5"),
    copied_theme_dependency!(ReduxColor, "cScale5", "pie6"),
    copied_theme_dependency!(ReduxColor, "cScale6", "pie7"),
    copied_theme_dependency!(ReduxColor, "cScale7", "pie8"),
    copied_theme_dependency!(ReduxColor, "cScale8", "pie9"),
    copied_theme_dependency!(ReduxColor, "cScale9", "pie10"),
    copied_theme_dependency!(ReduxColor, "cScale10", "pie11"),
    copied_theme_dependency!(ReduxColor, "cScale11", "pie12"),
    copied_theme_dependency!(ReduxColor, "cScale0", "sectionBkgColor"),
    copied_theme_dependency!(ReduxColor, "cScale1", "sectionBkgColor2"),
    copied_theme_dependency!(ReduxDarkColor, "background", "altSectionBkgColor"),
    // Unlike theme-default, every other pinned theme creates its Venn variables during
    // calculate(). Keep Default's constructor snapshot stable while replaying the non-Default
    // nullish assignments and their operands.
    copied_theme_dependency!(LegacyNonDefault, "titleColor", "vennTitleTextColor"),
    copied_theme_dependency!(LegacyNonDefault, "textColor", "vennSetTextColor"),
    copied_theme_dependency!(LegacyNonDefault, "primaryTextColor", "stateLabelColor"),
    ThemeVariableDependency::copied_when_source_explicit_truthy(
        ThemeVariableDependencyScope::LegacyNonDefault,
        "stateBkg",
        "stateLabelColor",
    ),
];

/// Pure-Rust execution contract for one pinned Mermaid theme class.
///
/// The generated artifact owns exact release snapshots. The descriptor selects the executor
/// and records compatibility input validation and extended-theme dependencies.
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
const EXTENDED_COLOR_INPUTS: &[&str] = &[
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
        ThemeProgramKind::Staged(staged::StagedProgram::Default),
        ThemeDependencyGraph::None,
        DEFAULT_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::Base,
        ThemeProgramKind::Staged(staged::StagedProgram::Base),
        ThemeDependencyGraph::None,
        &[],
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
        EXTENDED_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::NeoDark,
        ThemeProgramKind::Extended,
        ThemeDependencyGraph::DarkenedScale,
        EXTENDED_COLOR_INPUTS,
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
        EXTENDED_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::ReduxColor,
        ThemeProgramKind::Extended,
        ThemeDependencyGraph::None,
        EXTENDED_COLOR_INPUTS,
    ),
    ThemeProgram::new(
        MermaidThemeId::ReduxDarkColor,
        ThemeProgramKind::Extended,
        ThemeDependencyGraph::DynamicGit,
        EXTENDED_COLOR_INPUTS,
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
            ThemeProgramKind::Extended => None,
        }
    }

    fn variable_dependencies(self) -> impl Iterator<Item = &'static ThemeVariableDependency> {
        THEME_VARIABLE_DEPENDENCIES
            .iter()
            .filter(move |dependency| dependency.scope.includes(self.id))
    }

    fn prepared_constructor(self) -> &'static Map<String, Value> {
        generated_theme_runtime().prepared_constructors[self.id.as_str()].resolved()
    }

    fn resolved_without_overrides(self) -> &'static Map<String, Value> {
        generated_theme_runtime().themes[self.id.as_str()].resolved()
    }

    fn resolved_dark_mode_true(self) -> &'static Map<String, Value> {
        generated_theme_runtime().dark_mode_true_overrides[self.id.as_str()]
            .resolved_with_base(|| self.resolved_without_overrides())
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

    fn resolve_extended(
        self,
        explicit: &Map<String, Value>,
    ) -> Result<Map<String, Value>, ColorError> {
        self.validate_evaluated_inputs(explicit)?;
        let mut calculated = if let Some(snapshot) = self.exact_snapshot(explicit) {
            // The generated result already contains the calculation for this darkMode branch.
            snapshot.clone()
        } else {
            let mut calculated = explicit.clone();
            merge_theme_variable_defaults(&mut calculated, self.calculation_snapshot(explicit));
            apply_extended_theme_visible_derivations(self, explicit, &mut calculated)?;
            self.apply_dependency_graph(explicit, &mut calculated)?;
            calculated
        };
        staged::replay_explicit(&mut calculated, explicit);
        Ok(calculated)
    }

    fn apply_dependency_graph(
        self,
        explicit: &Map<String, Value>,
        calculated: &mut Map<String, Value>,
    ) -> Result<(), ColorError> {
        match self.dependencies {
            ThemeDependencyGraph::None => Ok(()),
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
        if !dependency
            .condition
            .permits(explicit.get(dependency.source).map(is_js_truthy))
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

fn get_truthy_string(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

pub(crate) fn is_js_truthy(value: &Value) -> bool {
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
    match config.get("themeVariables") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    }
}

fn generated_theme_runtime() -> &'static GeneratedThemeRuntime<'static> {
    GENERATED_THEME_RUNTIME.get_or_init(parse_generated_theme_runtime)
}

fn parse_generated_theme_runtime() -> GeneratedThemeRuntime<'static> {
    let artifact: GeneratedThemeRuntime<'_> =
        serde_json::from_str(include_str!("generated/theme_variables.json"))
            .expect("generated Mermaid theme runtime JSON is valid");
    assert_eq!(artifact.schema_version, THEME_RUNTIME_SCHEMA_VERSION);
    assert_generated_theme_provenance(&artifact.provenance);
    assert_eq!(artifact.oracle_case_count, THEME_ORACLE_CASE_COUNT);
    for program in THEME_PROGRAMS {
        for section in [
            &artifact.prepared_constructors,
            &artifact.themes,
            &artifact.dark_mode_true_overrides,
        ] {
            assert!(section[program.id.as_str()].encoded.get().starts_with('{'));
        }
    }
    artifact
}

#[test]
fn generated_theme_sections_decode_on_demand() {
    let artifact = parse_generated_theme_runtime();
    let name = MermaidThemeId::ReduxColor.as_str();
    let light = &artifact.themes[name];
    let dark = &artifact.dark_mode_true_overrides[name];
    assert!(light.decoded.get().is_none());
    assert!(dark.decoded.get().is_none());
    assert!(std::ptr::eq(light.resolved(), light.resolved()));
    let dark_values = dark.resolved_with_base(|| light.resolved());
    assert!(std::ptr::eq(
        dark_values,
        dark.resolved_with_base(|| panic!("dark map was decoded already"))
    ));
    assert!(
        artifact
            .themes
            .iter()
            .all(|(other, snapshot)| (other == name) == snapshot.decoded.get().is_some())
    );
    assert!(
        artifact
            .dark_mode_true_overrides
            .iter()
            .all(|(other, snapshot)| (other == name) == snapshot.decoded.get().is_some())
    );
    assert!(
        artifact
            .prepared_constructors
            .values()
            .all(|snapshot| snapshot.decoded.get().is_none())
    );
}

// The runtime stores exact top-level set/remove deltas. Assigned nulls and objects replace
// whole values, unlike JSON merge patch. Restore the full maps once before theme evaluation.
fn restore_theme_overrides(base: &Map<String, Value>, changes: Value) -> Map<String, Value> {
    let Value::Object(mut changes) = changes else {
        panic!("generated theme overrides are an object");
    };
    let Value::Object(assignments) = changes.remove("set").expect("generated overrides have set")
    else {
        panic!("generated theme assignments are an object");
    };
    let Value::Array(removals) = changes
        .remove("remove")
        .expect("generated overrides have remove")
    else {
        panic!("generated theme removals are an array");
    };
    assert!(
        changes.is_empty(),
        "unknown generated theme override fields"
    );
    let mut restored = base.clone();
    for key in removals {
        let key = key.as_str().expect("generated theme removal is a string");
        assert!(
            restored.remove(key).is_some(),
            "generated removal requires a base key"
        );
    }
    restored.extend(assignments);
    restored
}

fn assert_generated_theme_provenance(provenance: &GeneratedThemeProvenance) {
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
        let artifact: GeneratedThemeOracles = serde_json::from_str(include_str!(
            "../../../fixtures/_verification/theme_variables_oracle.json"
        ))
        .expect("generated Mermaid theme oracle JSON is valid");
        assert_eq!(artifact.schema_version, THEME_AUDIT_SCHEMA_VERSION);
        assert_generated_theme_provenance(&artifact.provenance);
        assert_eq!(
            &artifact.provenance,
            &generated_theme_runtime().provenance,
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

fn mk_border(color: &str, dark_mode: bool) -> Result<String, ColorError> {
    theme_color::adjust(
        color,
        ColorAdjustment::hsl(0.0, -40.0, if dark_mode { 10.0 } else { -10.0 }),
    )
}

const XYCHART_TEXT_COLOR_KEYS: &[&str] = &[
    "titleColor",
    "dataLabelColor",
    "legendTextColor",
    "xAxisTitleColor",
    "xAxisLabelColor",
    "xAxisTickColor",
    "xAxisLineColor",
    "yAxisTitleColor",
    "yAxisLabelColor",
    "yAxisTickColor",
    "yAxisLineColor",
];

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

const THEME_GIT_KEYS: [&str; 8] = [
    "git0", "git1", "git2", "git3", "git4", "git5", "git6", "git7",
];
const THEME_GIT_INVERSE_KEYS: [&str; 8] = [
    "gitInv0", "gitInv1", "gitInv2", "gitInv3", "gitInv4", "gitInv5", "gitInv6", "gitInv7",
];
const THEME_SCALE_KEYS: [&str; 12] = [
    "cScale0", "cScale1", "cScale2", "cScale3", "cScale4", "cScale5", "cScale6", "cScale7",
    "cScale8", "cScale9", "cScale10", "cScale11",
];
const THEME_SCALE_INVERSE_KEYS: [&str; 12] = [
    "cScaleInv0",
    "cScaleInv1",
    "cScaleInv2",
    "cScaleInv3",
    "cScaleInv4",
    "cScaleInv5",
    "cScaleInv6",
    "cScaleInv7",
    "cScaleInv8",
    "cScaleInv9",
    "cScaleInv10",
    "cScaleInv11",
];
const THEME_SCALE_PEER_KEYS: [&str; 12] = [
    "cScalePeer0",
    "cScalePeer1",
    "cScalePeer2",
    "cScalePeer3",
    "cScalePeer4",
    "cScalePeer5",
    "cScalePeer6",
    "cScalePeer7",
    "cScalePeer8",
    "cScalePeer9",
    "cScalePeer10",
    "cScalePeer11",
];
const THEME_SCALE_LABEL_KEYS: [&str; 12] = [
    "cScaleLabel0",
    "cScaleLabel1",
    "cScaleLabel2",
    "cScaleLabel3",
    "cScaleLabel4",
    "cScaleLabel5",
    "cScaleLabel6",
    "cScaleLabel7",
    "cScaleLabel8",
    "cScaleLabel9",
    "cScaleLabel10",
    "cScaleLabel11",
];
const THEME_VENN_KEYS: [&str; 8] = [
    "venn1", "venn2", "venn3", "venn4", "venn5", "venn6", "venn7", "venn8",
];
const THEME_FILL_TYPE_KEYS: [&str; 8] = [
    "fillType0",
    "fillType1",
    "fillType2",
    "fillType3",
    "fillType4",
    "fillType5",
    "fillType6",
    "fillType7",
];

fn validate_explicit_git_transforms(
    tv: &Map<String, Value>,
    explicit: &Map<String, Value>,
    transform: ColorTransform,
) -> Result<(), ColorError> {
    for key in THEME_GIT_KEYS.iter().copied() {
        if explicit.contains_key(key) {
            transform.apply(&required_color(tv, key)?)?;
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
    for ((git_key, inverse_key), base) in THEME_GIT_KEYS
        .iter()
        .copied()
        .zip(THEME_GIT_INVERSE_KEYS.iter().copied())
        .zip(bases)
    {
        let source = if explicit.contains_key(git_key) {
            required_color(tv, git_key)?
        } else {
            base
        };
        let transformed = transform.apply(&source)?;
        if !explicit.contains_key(git_key) {
            tv.insert(git_key.to_string(), Value::String(transformed.clone()));
        }

        if !explicit.contains_key(inverse_key) {
            tv.insert(
                inverse_key.to_string(),
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
    for key in THEME_SCALE_LABEL_KEYS.iter().copied() {
        if !explicit.contains_key(key) {
            tv.insert(key.to_string(), scale_label.clone());
        }
    }
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

    for ((scale_key, inverse_key), (peer_key, base)) in THEME_SCALE_KEYS
        .iter()
        .copied()
        .zip(THEME_SCALE_INVERSE_KEYS.iter().copied())
        .zip(THEME_SCALE_PEER_KEYS.iter().copied().zip(bases))
    {
        let source = get_truthy_string(explicit, scale_key).unwrap_or(base);
        let scale = transform.apply(&source)?;
        if !explicit.contains_key(scale_key) {
            tv.insert(scale_key.to_string(), Value::String(scale.clone()));
        }
        set_derived_string_unless_explicit(tv, explicit, inverse_key, theme_color::invert(&scale)?);
        let peer = if dark_mode {
            theme_color::lighten(&scale, 10.0)?
        } else {
            theme_color::darken(&scale, 10.0)?
        };
        set_derived_string_unless_explicit(tv, explicit, peer_key, peer);
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
        for ((scale_key, inverse_key), (peer_key, label_key)) in THEME_SCALE_KEYS
            .into_iter()
            .zip(THEME_SCALE_INVERSE_KEYS.iter().copied())
            .zip(
                THEME_SCALE_PEER_KEYS
                    .iter()
                    .copied()
                    .zip(THEME_SCALE_LABEL_KEYS.iter().copied()),
            )
            .take(2)
        {
            if !explicit.contains_key(scale_key) {
                continue;
            }
            let source = required_color(tv, scale_key)?;
            let calculated = match transform {
                Some(transform) => transform.apply(&source)?,
                None => source,
            };

            if !explicit.contains_key(inverse_key) {
                tv.insert(
                    inverse_key.to_string(),
                    Value::String(theme_color::invert(&calculated)?),
                );
            }
            if !explicit.contains_key(peer_key) {
                let peer = if dark_mode {
                    theme_color::lighten(&calculated, 10.0)?
                } else {
                    theme_color::darken(&calculated, 10.0)?
                };
                tv.insert(peer_key.to_string(), Value::String(peer));
            }
            if theme == MermaidThemeId::ReduxDarkColor && !explicit.contains_key(label_key) {
                tv.insert(
                    label_key.to_string(),
                    Value::String(theme_color::darken(&calculated, 75.0)?),
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn apply_theme_defaults(
    config: &mut SourcePresentationConfig,
) -> Result<(), ThemeResolutionError> {
    let requested = config.get_str("theme").unwrap_or("default");
    // Mermaid's raw configuration runtime falls back to `default` for unknown strings. Typed
    // Merman theme APIs use `MermaidThemeId::parse` and never enter this compatibility branch.
    let theme = MermaidThemeId::parse(requested).unwrap_or_default();
    let program = ThemeProgram::resolve(theme);
    let raw = theme_variables_map(config.as_config());
    let explicit = program.normalize_overrides(raw);

    let resolved = if let Some(staged_program) = program.staged_program() {
        if explicit.is_empty() {
            staged_program.resolved_without_overrides(program)?.clone()
        } else {
            staged::Resolution::execute(staged_program, program, &explicit)?.variables
        }
    } else {
        program.resolve_extended(&explicit)?
    };
    config.set_computed_value("themeVariables", Value::Object(resolved));
    Ok(())
}

pub(crate) fn materialize_selected_theme(
    mut effective_config: SourcePresentationConfig,
    selected_theme: MermaidThemeId,
) -> Result<SourcePresentationConfig, ThemeResolutionError> {
    #[cfg(test)]
    crate::config::record_config_work(|work| work.selected_theme_materializations += 1);
    effective_config
        .set_computed_value("theme", Value::String(selected_theme.as_str().to_string()));
    apply_theme_defaults(&mut effective_config)?;
    Ok(effective_config)
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
    if explicit.contains_key("primaryColor") {
        let primary = required_color(tv, "primaryColor")?;
        set_derived_string_unless_explicit(tv, explicit, "nodeBkg", primary.clone());
        set_derived_string_unless_explicit(tv, explicit, "tagLabelBackground", primary.clone());

        if matches!(
            theme,
            MermaidThemeId::Neo | MermaidThemeId::Redux | MermaidThemeId::ReduxColor
        ) && !explicit.contains_key("secondaryColor")
        {
            let secondary = theme_color::adjust(&primary, ColorAdjustment::hsl(-120.0, 0.0, 0.0))?;
            tv.insert("secondaryColor".to_string(), Value::String(secondary));
        }
    }

    // updateColors() resolves a falsy border before Agentflow reads it. The
    // explicit value is replayed only after all dependent colors are calculated.
    if value_is_missing(tv, "secondaryBorderColor") {
        let secondary = required_color(tv, "secondaryColor")?;
        let dark_mode = tv.get("darkMode").is_some_and(is_js_truthy);
        tv.insert(
            "secondaryBorderColor".to_string(),
            Value::String(mk_border(&secondary, dark_mode)?),
        );
    }

    if explicit.contains_key("secondaryColor") || explicit.contains_key("primaryColor") {
        let secondary = required_color(tv, "secondaryColor")?;
        let dark_mode = tv.get("darkMode").is_some_and(is_js_truthy);
        let label_background = if dark_mode {
            theme_color::darken(&secondary, 30.0)?
        } else {
            secondary.clone()
        };
        for key in [
            "edgeLabelBackground",
            "activationBkgColor",
            "commitLabelBackground",
            "relationLabelBackground",
        ] {
            set_derived_string_unless_explicit(tv, explicit, key, label_background.clone());
        }
    }

    if explicit.contains_key("mainBkg") {
        let main_bkg = required_color(tv, "mainBkg")?;
        for key in [
            "actorBkg",
            "labelBoxBkgColor",
            "personBkg",
            "stateBkg",
            "labelBackgroundColor",
        ] {
            set_derived_string_unless_explicit(tv, explicit, key, main_bkg.clone());
        }
    }

    if explicit.contains_key("primaryColor")
        && matches!(
            theme,
            MermaidThemeId::NeoDark | MermaidThemeId::ReduxDark | MermaidThemeId::ReduxDarkColor
        )
    {
        let primary = required_color(tv, "primaryColor")?;
        set_derived_string_unless_explicit(tv, explicit, "requirementBackground", primary.clone());
        if theme != MermaidThemeId::ReduxDarkColor {
            set_derived_string_unless_explicit(tv, explicit, "pie1", primary);
        }
    }

    // Mermaid 12 uses a visible, direction-aware band on the light extended
    // themes, and resolves dark themes' constructor placeholder from mainBkg.
    if matches!(
        theme,
        MermaidThemeId::Neo | MermaidThemeId::Redux | MermaidThemeId::ReduxColor
    ) && explicit.contains_key("background")
        && !explicit.contains_key("rectBkgColor")
    {
        let background = required_color(tv, "background")?;
        let band = if theme_color::is_dark(&background)? {
            theme_color::lighten(&background, 4.0)?
        } else {
            theme_color::darken(&background, 4.0)?
        };
        tv.insert("rectBkgColor".to_string(), Value::String(band));
    }
    if matches!(
        theme,
        MermaidThemeId::NeoDark | MermaidThemeId::ReduxDark | MermaidThemeId::ReduxDarkColor
    ) {
        let second_is_placeholder =
            explicit.get("secondBkg").and_then(Value::as_str) == Some("calculated");
        if (explicit.contains_key("mainBkg") && !explicit.contains_key("secondBkg"))
            || second_is_placeholder
        {
            let second = theme_color::lighten(&required_color(tv, "mainBkg")?, 16.0)?;
            tv.insert("secondBkg".to_string(), Value::String(second));
        }
        if (explicit.contains_key("mainBkg") || explicit.contains_key("secondBkg"))
            && !explicit.contains_key("doneTaskBkgColor")
            && let Some(second) = tv.get("secondBkg").cloned()
        {
            tv.insert("doneTaskBkgColor".to_string(), second);
        }
    }
    if matches!(
        theme,
        MermaidThemeId::ReduxColor | MermaidThemeId::ReduxDarkColor
    ) {
        // Resolve categorical fallbacks before deriving pie and Gantt colors.
        for scale in THEME_SCALE_KEYS.iter().copied() {
            if explicit.contains_key(scale) {
                // Both Redux color themes use fixed categorical fallbacks in
                // updateColors(). Consumers read that stage, not the final replay.
                let fallback =
                    ThemeProgram::resolve(theme).calculation_snapshot(explicit)[scale].clone();
                set_if_missing(tv, scale, fallback);
            }
        }
        for (palette, prefix) in [("borderColorArray", "venn"), ("bkgColorArray", "fillType")] {
            if prefix == "fillType" && theme == MermaidThemeId::ReduxDarkColor {
                continue;
            }
            if let Some(colors) = explicit.get(palette).and_then(Value::as_array) {
                for index in 0..8 {
                    let key = if prefix == "venn" {
                        THEME_VENN_KEYS[index]
                    } else {
                        THEME_FILL_TYPE_KEYS[index]
                    };
                    if explicit.contains_key(key) {
                        continue;
                    }
                    let color = if prefix == "venn" && !colors.is_empty() {
                        colors.get(index % colors.len())
                    } else {
                        colors.get(index)
                    };
                    if let Some(color) = color {
                        tv.insert(key.to_string(), color.clone());
                    } else {
                        tv.remove(key);
                    }
                }
            }
        }
    }

    for (git_key, git_inv_key) in THEME_GIT_KEYS
        .iter()
        .copied()
        .zip(THEME_GIT_INVERSE_KEYS.iter().copied())
    {
        if explicit.contains_key(git_key) && !explicit.contains_key(git_inv_key) {
            let git = required_color(tv, git_key)?;
            let inv = theme_color::invert(&git)?;
            tv.insert(git_inv_key.to_string(), Value::String(inv));
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

#[cfg(test)]
mod tests;
