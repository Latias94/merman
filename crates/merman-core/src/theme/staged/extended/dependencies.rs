//! Ordered assignments shared by the Neo and Redux theme programs.

use super::{
    ColorAdjustment, ColorError, Map, Program, ThemeState, Value, is_js_truthy, mk_border,
    required_color, theme_color,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    All,
    Light,
    Dark,
    DarkWithoutReduxDarkColor,
    ReduxColors,
    ReduxDarkColor,
    Neo,
    NeoDark,
    WithoutReduxDark,
    NeoOrReduxColors,
    WithoutDarkRedux,
}

impl Scope {
    const fn includes(self, program: Program) -> bool {
        use Program::*;

        match self {
            Self::All => true,
            Self::Light => !program.has_dark_palette(),
            Self::Dark => program.has_dark_palette(),
            Self::DarkWithoutReduxDarkColor => matches!(program, NeoDark | ReduxDark),
            Self::ReduxColors => program.is_redux_color(),
            Self::ReduxDarkColor => matches!(program, ReduxDarkColor),
            Self::Neo => matches!(program, Neo | NeoDark),
            Self::NeoDark => matches!(program, NeoDark),
            Self::WithoutReduxDark => !matches!(program, ReduxDark),
            Self::NeoOrReduxColors => !matches!(program, Redux | ReduxDark),
            Self::WithoutDarkRedux => !matches!(program, ReduxDark | ReduxDarkColor),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Transform {
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
        Dependency::copied(Scope::$scope, $source, $target)
    };
}

macro_rules! assigned_theme_dependency {
    ($scope:ident, $source:literal, $target:literal) => {
        Dependency::assigned(Scope::$scope, $source, $target)
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Assignment {
    UnlessExplicit,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Condition {
    Always,
    SourceExplicitTruthy,
}

impl Condition {
    fn permits(self, explicit_source_truthiness: Option<bool>) -> bool {
        match self {
            Self::Always => true,
            Self::SourceExplicitTruthy => explicit_source_truthiness == Some(true),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Dependency {
    scope: Scope,
    source: &'static str,
    target: &'static str,
    transform: Transform,
    assignment: Assignment,
    condition: Condition,
}

impl Dependency {
    const fn copied(scope: Scope, source: &'static str, target: &'static str) -> Self {
        Self {
            scope,
            source,
            target,
            transform: Transform::Copy,
            assignment: Assignment::UnlessExplicit,
            condition: Condition::Always,
        }
    }

    const fn assigned(scope: Scope, source: &'static str, target: &'static str) -> Self {
        Self {
            scope,
            source,
            target,
            transform: Transform::Copy,
            assignment: Assignment::Always,
            condition: Condition::Always,
        }
    }

    const fn copied_when_source_explicit_truthy(
        scope: Scope,
        source: &'static str,
        target: &'static str,
    ) -> Self {
        Self {
            scope,
            source,
            target,
            transform: Transform::Copy,
            assignment: Assignment::UnlessExplicit,
            condition: Condition::SourceExplicitTruthy,
        }
    }

    const fn transformed(
        scope: Scope,
        source: &'static str,
        target: &'static str,
        transform: Transform,
    ) -> Self {
        Self {
            scope,
            source,
            target,
            transform,
            assignment: Assignment::UnlessExplicit,
            condition: Condition::Always,
        }
    }
}

// The pinned Mermaid programs calculate through unconditional intermediate assignments
// before replaying explicit targets. Preserve the order and assignment mode of these steps.
const DEPENDENCIES: &[Dependency] = &[
    Dependency::transformed(
        Scope::Neo,
        "noteBkgColor",
        "noteBorderColor",
        Transform::MkBorder,
    ),
    Dependency::transformed(
        Scope::Light,
        "primaryColor",
        "secondaryColor",
        Transform::AdjustHsl {
            hue: -120.0,
            saturation: 0.0,
            lightness: 0.0,
        },
    ),
    Dependency::transformed(Scope::All, "background", "lineColor", Transform::Invert),
    Dependency::transformed(
        Scope::Light,
        "background",
        "arrowheadColor",
        Transform::Invert,
    ),
    copied_theme_dependency!(All, "primaryColor", "nodeBkg"),
    copied_theme_dependency!(All, "primaryTextColor", "nodeTextColor"),
    copied_theme_dependency!(All, "tertiaryTextColor", "titleColor"),
    copied_theme_dependency!(NeoDark, "border1", "nodeBorder"),
    copied_theme_dependency!(All, "lineColor", "defaultLinkColor"),
    copied_theme_dependency!(All, "secondaryBorderColor", "flowContainerStroke"),
    Dependency::transformed(
        Scope::All,
        "secondaryColor",
        "edgeLabelBackground",
        Transform::DarkenWhenDarkMode(30.0),
    ),
    copied_theme_dependency!(All, "mainBkg", "actorBkg"),
    copied_theme_dependency!(Neo, "primaryBorderColor", "actorBorder"),
    copied_theme_dependency!(All, "primaryTextColor", "actorTextColor"),
    copied_theme_dependency!(WithoutDarkRedux, "actorBorder", "actorLineColor"),
    copied_theme_dependency!(All, "actorBkg", "labelBoxBkgColor"),
    copied_theme_dependency!(WithoutDarkRedux, "textColor", "signalColor"),
    copied_theme_dependency!(All, "textColor", "signalTextColor"),
    copied_theme_dependency!(WithoutDarkRedux, "actorBorder", "labelBoxBorderColor"),
    copied_theme_dependency!(All, "actorTextColor", "labelTextColor"),
    copied_theme_dependency!(All, "actorTextColor", "loopTextColor"),
    Dependency::transformed(
        Scope::All,
        "secondaryColor",
        "activationBorderColor",
        Transform::Darken(10.0),
    ),
    copied_theme_dependency!(All, "secondaryColor", "activationBkgColor"),
    Dependency::transformed(
        Scope::All,
        "lineColor",
        "sequenceNumberColor",
        Transform::Invert,
    ),
    assigned_theme_dependency!(All, "lineColor", "archEdgeColor"),
    assigned_theme_dependency!(All, "lineColor", "archEdgeArrowColor"),
    copied_theme_dependency!(All, "mainBkg", "personBkg"),
    copied_theme_dependency!(All, "lineColor", "transitionColor"),
    copied_theme_dependency!(All, "textColor", "transitionLabelColor"),
    copied_theme_dependency!(All, "mainBkg", "stateBkg"),
    copied_theme_dependency!(All, "stateBkg", "labelBackgroundColor"),
    copied_theme_dependency!(WithoutReduxDark, "background", "compositeBackground"),
    copied_theme_dependency!(NeoOrReduxColors, "mainBkg", "compositeTitleBackground"),
    assigned_theme_dependency!(All, "nodeBorder", "innerEndBackground"),
    assigned_theme_dependency!(All, "lineColor", "specialStateColor"),
    copied_theme_dependency!(Dark, "primaryColor", "taskBkgColor"),
    Dependency::transformed(
        Scope::Dark,
        "primaryColor",
        "activeTaskBkgColor",
        Transform::Lighten(23.0),
    ),
    copied_theme_dependency!(Dark, "primaryColor", "requirementBackground"),
    copied_theme_dependency!(All, "lineColor", "relationColor"),
    Dependency::transformed(
        Scope::All,
        "secondaryColor",
        "relationLabelBackground",
        Transform::DarkenWhenDarkMode(30.0),
    ),
    copied_theme_dependency!(All, "primaryColor", "tagLabelBackground"),
    copied_theme_dependency!(All, "secondaryColor", "commitLabelBackground"),
    copied_theme_dependency!(DarkWithoutReduxDarkColor, "primaryColor", "pie1"),
    copied_theme_dependency!(ReduxColors, "cScale0", "pie1"),
    copied_theme_dependency!(ReduxColors, "cScale1", "pie2"),
    copied_theme_dependency!(ReduxColors, "cScale2", "pie3"),
    copied_theme_dependency!(ReduxColors, "cScale3", "pie4"),
    copied_theme_dependency!(ReduxColors, "cScale4", "pie5"),
    copied_theme_dependency!(ReduxColors, "cScale5", "pie6"),
    copied_theme_dependency!(ReduxColors, "cScale6", "pie7"),
    copied_theme_dependency!(ReduxColors, "cScale7", "pie8"),
    copied_theme_dependency!(ReduxColors, "cScale8", "pie9"),
    copied_theme_dependency!(ReduxColors, "cScale9", "pie10"),
    copied_theme_dependency!(ReduxColors, "cScale10", "pie11"),
    copied_theme_dependency!(ReduxColors, "cScale11", "pie12"),
    copied_theme_dependency!(ReduxColors, "cScale0", "sectionBkgColor"),
    copied_theme_dependency!(ReduxColors, "cScale1", "sectionBkgColor2"),
    copied_theme_dependency!(ReduxDarkColor, "background", "altSectionBkgColor"),
    // These Venn values read the result of the preceding text assignments.
    copied_theme_dependency!(All, "titleColor", "vennTitleTextColor"),
    copied_theme_dependency!(All, "textColor", "vennSetTextColor"),
    copied_theme_dependency!(All, "primaryTextColor", "stateLabelColor"),
    Dependency::copied_when_source_explicit_truthy(Scope::All, "stateBkg", "stateLabelColor"),
];

pub(super) fn apply(
    program: Program,
    explicit: &Map<String, Value>,
    stage: &mut ThemeState,
) -> Result<(), ColorError> {
    for dependency in DEPENDENCIES
        .iter()
        .filter(|row| row.scope.includes(program))
    {
        if !dependency
            .condition
            .permits(explicit.get(dependency.source).map(is_js_truthy))
        {
            continue;
        }
        if dependency.assignment == Assignment::UnlessExplicit
            && explicit.contains_key(dependency.target)
        {
            continue;
        }
        let Some(source) = stage.variables.get(dependency.source) else {
            continue;
        };
        let value = dependency.value(&stage.variables, source)?;
        stage.variables.insert(dependency.target.to_owned(), value);
    }
    Ok(())
}

impl Dependency {
    fn value(
        &self,
        calculated: &Map<String, Value>,
        source_value: &Value,
    ) -> Result<Value, ColorError> {
        let source = self.source;
        Ok(match self.transform {
            Transform::Copy => source_value.clone(),
            Transform::Invert => {
                let source = required_color(calculated, source)?;
                Value::String(theme_color::invert(&source)?)
            }
            Transform::AdjustHsl {
                hue,
                saturation,
                lightness,
            } => {
                let source = required_color(calculated, source)?;
                Value::String(theme_color::adjust(
                    &source,
                    ColorAdjustment::hsl(hue, saturation, lightness),
                )?)
            }
            Transform::Darken(amount) => {
                let source = required_color(calculated, source)?;
                Value::String(theme_color::darken(&source, amount)?)
            }
            Transform::Lighten(amount) => {
                let source = required_color(calculated, source)?;
                Value::String(theme_color::lighten(&source, amount)?)
            }
            Transform::MkBorder => {
                let source = required_color(calculated, source)?;
                Value::String(mk_border(
                    &source,
                    calculated.get("darkMode").is_some_and(is_js_truthy),
                )?)
            }
            Transform::DarkenWhenDarkMode(amount) => {
                let source = required_color(calculated, source)?;
                if calculated.get("darkMode").is_some_and(is_js_truthy) {
                    Value::String(theme_color::darken(&source, amount)?)
                } else {
                    Value::String(source)
                }
            }
        })
    }
}
