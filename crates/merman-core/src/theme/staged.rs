//! Ordered execution for the classic Mermaid theme classes.
//!
//! Mermaid theme classes are mutable JavaScript objects, but their public behavior is a small,
//! deterministic four-stage protocol. This module keeps that protocol behind one interface and
//! records ownership from the same writes that produce values. It intentionally models only the
//! three assignment operators used by the pinned theme sources: unconditional assignment,
//! JavaScript `||`, and JavaScript `??`.

use super::{ThemeResolutionError, is_js_truthy, value_is_missing};
use crate::compatibility_json::number_value;
use crate::theme_color::{self, ColorAdjustment, ColorError};
use crate::{MermaidConfig, ThemeEvaluationLimitExceeded};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const MAX_THEME_COLOR_ITERATIONS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StagedProgram {
    Dark,
    Forest,
    Neutral,
}

#[derive(Debug, Clone)]
pub(super) struct ThemeState {
    pub(super) variables: Map<String, Value>,
    dependencies: BTreeMap<String, BTreeSet<String>>,
    theme_color_iterations: usize,
}

pub(super) struct Resolution {
    state: ThemeState,
    #[cfg(test)]
    trace: ResolutionTrace,
}

#[cfg(test)]
pub(super) struct ResolutionTrace {
    pub(super) constructor_prepared: ThemeState,
    pub(super) overrides_applied: ThemeState,
    pub(super) after_update: ThemeState,
    pub(super) explicit_replay: ThemeState,
}

#[derive(Debug)]
struct ComputedValue {
    value: Value,
    dependencies: Vec<String>,
    nested_dependencies: BTreeMap<String, Vec<String>>,
}

impl ComputedValue {
    fn constant(value: impl Into<Value>) -> Self {
        Self {
            value: value.into(),
            dependencies: Vec::new(),
            nested_dependencies: BTreeMap::new(),
        }
    }

    fn string(value: impl Into<String>) -> Self {
        Self::constant(Value::String(value.into()))
    }

    fn number(value: i64) -> Self {
        Self::constant(Value::Number(value.into()))
    }

    fn with_dependencies(value: Value, dependencies: impl IntoIterator<Item = String>) -> Self {
        Self {
            value,
            dependencies: dependencies.into_iter().collect(),
            nested_dependencies: BTreeMap::new(),
        }
    }

    fn object(
        value: Map<String, Value>,
        nested_dependencies: BTreeMap<String, Vec<String>>,
    ) -> Self {
        Self {
            value: Value::Object(value),
            dependencies: Vec::new(),
            nested_dependencies,
        }
    }
}

impl ThemeState {
    fn constructor(variables: Map<String, Value>) -> Self {
        Self {
            variables,
            dependencies: BTreeMap::new(),
            theme_color_iterations: 0,
        }
    }

    fn overlay_explicit(&mut self, explicit: &Map<String, Value>) {
        for (key, value) in explicit {
            self.variables.insert(key.clone(), value.clone());
            self.remove_dependency_subtree(key);
            for path in explicit_dependency_leaf_paths(key, value) {
                self.dependencies
                    .insert(path.clone(), BTreeSet::from([path]));
            }
        }
    }

    fn remove_dependency_subtree(&mut self, root: &str) {
        let descendant_prefix = format!("{root}.");
        self.dependencies
            .retain(|path, _| path != root && !path.starts_with(&descendant_prefix));
    }

    fn computed_from(&self, key: &str) -> Result<ComputedValue, ColorError> {
        let value = self
            .variables
            .get(key)
            .cloned()
            .ok_or_else(|| missing_value_error(key))?;
        Ok(ComputedValue::with_dependencies(value, [key.to_string()]))
    }

    fn truthy_from(&self, key: &str) -> Option<ComputedValue> {
        self.variables
            .get(key)
            .filter(|value| is_js_truthy(value))
            .cloned()
            .map(|value| ComputedValue::with_dependencies(value, [key.to_string()]))
    }

    fn color(&self, key: &str) -> Result<String, ColorError> {
        self.variables
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| missing_value_error(key))
    }

    fn expand_dependencies(&self, dependencies: &[String]) -> BTreeSet<String> {
        dependencies
            .iter()
            .filter_map(|dependency| self.dependencies.get(dependency))
            .flat_map(|sources| sources.iter().cloned())
            .collect()
    }

    /// Execute an unconditional JavaScript assignment.
    fn assign(
        &mut self,
        target: &str,
        compute: impl FnOnce(&Self) -> Result<ComputedValue, ColorError>,
    ) -> Result<(), ColorError> {
        let ComputedValue {
            value,
            dependencies,
            nested_dependencies,
        } = compute(self)?;
        let sources = self.expand_dependencies(&dependencies);
        let nested_sources = nested_dependencies
            .into_iter()
            .map(|(path, dependencies)| (path, self.expand_dependencies(&dependencies)))
            .collect::<Vec<_>>();

        self.variables.insert(target.to_string(), value);
        self.remove_dependency_subtree(target);
        if !sources.is_empty() {
            self.dependencies.insert(target.to_string(), sources);
        }
        for (relative_path, sources) in nested_sources {
            if !sources.is_empty() {
                self.dependencies
                    .insert(format!("{target}.{relative_path}"), sources);
            }
        }
        Ok(())
    }

    /// Execute `target = target || fallback`, including short-circuit evaluation.
    fn assign_if_falsy(
        &mut self,
        target: &str,
        fallback: impl FnOnce(&Self) -> Result<ComputedValue, ColorError>,
    ) -> Result<(), ColorError> {
        if value_is_missing(&self.variables, target) {
            self.assign(target, fallback)?;
        }
        Ok(())
    }

    /// Execute `target = target ?? fallback`, without treating other falsy values as absent.
    fn assign_if_nullish(
        &mut self,
        target: &str,
        fallback: impl FnOnce(&Self) -> Result<ComputedValue, ColorError>,
    ) -> Result<(), ColorError> {
        if self.variables.get(target).is_none_or(Value::is_null) {
            self.assign(target, fallback)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn depends_on(&self, target: &str, source: &str) -> bool {
        self.dependencies
            .get(target)
            .is_some_and(|sources| sources.contains(source))
    }
}

fn explicit_dependency_leaf_paths(root: &str, value: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    let mut stack = vec![(root.to_string(), value)];
    while let Some((path, value)) = stack.pop() {
        match value {
            Value::Object(object) if !object.is_empty() => {
                for (key, child) in object.iter().rev() {
                    stack.push((format!("{path}.{key}"), child));
                }
            }
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Array(_)
            | Value::Object(_) => paths.push(path),
        }
    }
    paths
}

impl Resolution {
    pub(super) fn execute(
        program: StagedProgram,
        prepared_constructor: &Map<String, Value>,
        explicit: &Map<String, Value>,
    ) -> Result<Self, ThemeResolutionError> {
        let mut state = ThemeState::constructor(prepared_constructor.clone());
        #[cfg(test)]
        let constructor_prepared = state.clone();

        state.overlay_explicit(&explicit);
        #[cfg(test)]
        let overrides_applied = state.clone();
        state.theme_color_iterations = theme_color_iteration_count(&state)?;

        match program {
            StagedProgram::Dark => update_dark(&mut state)?,
            StagedProgram::Forest => update_forest(&mut state)?,
            StagedProgram::Neutral => update_neutral(&mut state)?,
        }
        #[cfg(test)]
        let after_update = state.clone();

        state.overlay_explicit(&explicit);
        #[cfg(test)]
        let explicit_replay = state.clone();

        Ok(Self {
            state,
            #[cfg(test)]
            trace: ResolutionTrace {
                constructor_prepared,
                overrides_applied,
                after_update,
                explicit_replay,
            },
        })
    }

    pub(super) fn materialize_into(self, config: &mut MermaidConfig) {
        let ThemeState {
            variables,
            dependencies,
            theme_color_iterations: _,
        } = self.state;
        config.set_value_preserving_theme_compatibility("themeVariables", Value::Object(variables));
        for (target, sources) in dependencies {
            for source in sources {
                if source != target {
                    config.propagate_theme_variable_ownership(&source, &target);
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) const fn trace(&self) -> &ResolutionTrace {
        &self.trace
    }
}

fn missing_value_error(key: &str) -> ColorError {
    ColorError::UnsupportedFormat {
        input: format!("missing or non-color theme value `{key}`"),
    }
}

fn transform_color(
    computed: ComputedValue,
    transform: impl FnOnce(&str) -> Result<String, ColorError>,
) -> Result<ComputedValue, ColorError> {
    let input = computed
        .value
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ColorError::UnsupportedFormat {
            input: computed.value.to_string(),
        })?;
    Ok(ComputedValue::with_dependencies(
        Value::String(transform(input)?),
        computed.dependencies,
    ))
}

fn copy(stage: &ThemeState, key: &str) -> Result<ComputedValue, ColorError> {
    stage.computed_from(key)
}

fn first_truthy(stage: &ThemeState, keys: &[&str]) -> Result<ComputedValue, ColorError> {
    let Some((last, candidates)) = keys.split_last() else {
        return Err(missing_value_error("empty JavaScript OR chain"));
    };
    if let Some(value) = candidates.iter().find_map(|key| stage.truthy_from(key)) {
        Ok(value)
    } else {
        stage.computed_from(last)
    }
}

fn for_theme_color_indices(
    stage: &mut ThemeState,
    mut execute: impl FnMut(&mut ThemeState, usize) -> Result<(), ColorError>,
) -> Result<(), ColorError> {
    for index in 0..stage.theme_color_iterations {
        execute(stage, index)?;
    }
    Ok(())
}

fn theme_color_iteration_count(stage: &ThemeState) -> Result<usize, ThemeEvaluationLimitExceeded> {
    let raw = stage.variables.get("THEME_COLOR_LIMIT");
    // Mermaid declares this field as a number. Preserve the scalar coercions reached through raw
    // YAML/JSON compatibility input, but keep the seam deliberately narrower than JavaScript's
    // general ToNumber algorithm.
    let limit = raw.map_or(f64::NAN, |value| match value {
        Value::Null | Value::Bool(false) => 0.0,
        Value::Bool(true) => 1.0,
        Value::Number(value) => value.as_f64().unwrap_or(f64::NAN),
        Value::String(value) => parse_theme_color_limit_string(value),
        Value::Array(value) => parse_theme_color_limit_array(value),
        Value::Object(_) => f64::NAN,
    });
    if limit.is_nan() || limit <= 0.0 {
        return Ok(0);
    }

    let requested_iterations = limit.ceil();
    if !requested_iterations.is_finite() || requested_iterations > MAX_THEME_COLOR_ITERATIONS as f64
    {
        return Err(ThemeEvaluationLimitExceeded {
            limit: "THEME_COLOR_LIMIT",
            requested: raw.map_or_else(|| "missing".to_string(), Value::to_string),
            max: MAX_THEME_COLOR_ITERATIONS,
        });
    }
    Ok(requested_iterations as usize)
}

fn parse_theme_color_limit_string(value: &str) -> f64 {
    let value =
        value.trim_matches(|character: char| character.is_whitespace() || character == '\u{feff}');
    if value.is_empty() {
        return 0.0;
    }
    match value {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let unsigned = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    if unsigned.eq_ignore_ascii_case("inf") || unsigned.eq_ignore_ascii_case("infinity") {
        return f64::NAN;
    }
    for (prefixes, radix) in [(["0x", "0X"], 16), (["0b", "0B"], 2), (["0o", "0O"], 8)] {
        if let Some(digits) = prefixes
            .iter()
            .find_map(|prefix| value.strip_prefix(prefix))
        {
            if digits.is_empty() || !digits.chars().all(|digit| digit.to_digit(radix).is_some()) {
                return f64::NAN;
            }
            return match usize::from_str_radix(digits, radix) {
                Ok(parsed) if parsed <= MAX_THEME_COLOR_ITERATIONS => parsed as f64,
                Ok(_) | Err(_) => (MAX_THEME_COLOR_ITERATIONS + 1) as f64,
            };
        }
    }
    value.parse::<f64>().unwrap_or(f64::NAN)
}

fn parse_theme_color_limit_array(mut value: &[Value]) -> f64 {
    loop {
        let [only] = value else {
            return if value.is_empty() { 0.0 } else { f64::NAN };
        };
        match only {
            Value::Null => return 0.0,
            Value::Bool(_) | Value::Object(_) => return f64::NAN,
            Value::Number(value) => return value.as_f64().unwrap_or(f64::NAN),
            Value::String(value) => return parse_theme_color_limit_string(value),
            Value::Array(nested) => value = nested,
        }
    }
}

fn adjusted(
    stage: &ThemeState,
    key: &str,
    adjustment: ColorAdjustment,
) -> Result<ComputedValue, ColorError> {
    transform_color(copy(stage, key)?, |color| {
        theme_color::adjust(color, adjustment)
    })
}

fn lightened(stage: &ThemeState, key: &str, amount: f64) -> Result<ComputedValue, ColorError> {
    transform_color(copy(stage, key)?, |color| {
        theme_color::lighten(color, amount)
    })
}

fn darkened(stage: &ThemeState, key: &str, amount: f64) -> Result<ComputedValue, ColorError> {
    transform_color(copy(stage, key)?, |color| {
        theme_color::darken(color, amount)
    })
}

fn inverted(stage: &ThemeState, key: &str) -> Result<ComputedValue, ColorError> {
    transform_color(copy(stage, key)?, theme_color::invert)
}

struct ObjectBuilder<'a> {
    stage: &'a ThemeState,
    source_key: &'a str,
    object: Map<String, Value>,
    dependencies: BTreeMap<String, Vec<String>>,
}

impl<'a> ObjectBuilder<'a> {
    fn new(stage: &'a ThemeState, source_key: &'a str) -> Self {
        Self {
            stage,
            source_key,
            object: Map::new(),
            dependencies: BTreeMap::new(),
        }
    }

    fn truthy_or(
        &mut self,
        field: &str,
        fallback: impl FnOnce(&ThemeState) -> Result<ComputedValue, ColorError>,
    ) -> Result<(), ColorError> {
        let current = self
            .stage
            .variables
            .get(self.source_key)
            .and_then(Value::as_object)
            .and_then(|object| object.get(field))
            .filter(|value| is_js_truthy(value))
            .cloned();
        let computed = if let Some(value) = current {
            ComputedValue::with_dependencies(value, [format!("{}.{}", self.source_key, field)])
        } else {
            fallback(self.stage)?
        };
        self.object.insert(field.to_string(), computed.value);
        self.dependencies
            .insert(field.to_string(), computed.dependencies);
        Ok(())
    }

    fn field(&mut self, field: &str, computed: ComputedValue) {
        self.object.insert(field.to_string(), computed.value);
        self.dependencies
            .insert(field.to_string(), computed.dependencies);
    }

    fn finish(self) -> ComputedValue {
        ComputedValue::object(self.object, self.dependencies)
    }
}

fn update_journey(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (target, source, hue) in [
        ("fillType0", "primaryColor", 0.0),
        ("fillType1", "secondaryColor", 0.0),
        ("fillType2", "primaryColor", 64.0),
        ("fillType3", "secondaryColor", 64.0),
        ("fillType4", "primaryColor", -64.0),
        ("fillType5", "secondaryColor", -64.0),
        ("fillType6", "primaryColor", 128.0),
        ("fillType7", "secondaryColor", 128.0),
    ] {
        stage.assign(target, |stage| {
            if hue == 0.0 {
                copy(stage, source)
            } else {
                adjusted(stage, source, ColorAdjustment::hsl(hue, 0.0, 0.0))
            }
        })?;
    }
    Ok(())
}

fn update_quadrant(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign_if_falsy("quadrant1Fill", |stage| copy(stage, "primaryColor"))?;
    for (target, amount) in [
        ("quadrant2Fill", 5.0),
        ("quadrant3Fill", 10.0),
        ("quadrant4Fill", 15.0),
    ] {
        stage.assign_if_falsy(target, |stage| {
            adjusted(
                stage,
                "primaryColor",
                ColorAdjustment::rgb(amount, amount, amount),
            )
        })?;
    }
    stage.assign_if_falsy("quadrant1TextFill", |stage| copy(stage, "primaryTextColor"))?;
    for (target, amount) in [
        ("quadrant2TextFill", -5.0),
        ("quadrant3TextFill", -10.0),
        ("quadrant4TextFill", -15.0),
    ] {
        stage.assign_if_falsy(target, |stage| {
            adjusted(
                stage,
                "primaryTextColor",
                ColorAdjustment::rgb(amount, amount, amount),
            )
        })?;
    }

    // JavaScript parses this upstream expression as
    // `(quadrantPointFill || isDark(quadrant1Fill)) ? lighten(...) : darken(...)`.
    stage.assign("quadrantPointFill", |stage| {
        let existing_is_truthy = stage
            .variables
            .get("quadrantPointFill")
            .is_some_and(is_js_truthy);
        let quadrant = stage.color("quadrant1Fill")?;
        let lighten = existing_is_truthy || theme_color::is_dark(&quadrant)?;
        let value = if lighten {
            theme_color::lighten(&quadrant, f64::NAN)?
        } else {
            theme_color::darken(&quadrant, f64::NAN)?
        };
        let dependencies = if existing_is_truthy {
            vec!["quadrantPointFill".to_string(), "quadrant1Fill".to_string()]
        } else {
            vec!["quadrant1Fill".to_string()]
        };
        Ok(ComputedValue::with_dependencies(
            Value::String(value),
            dependencies,
        ))
    })?;

    for target in [
        "quadrantPointTextFill",
        "quadrantXAxisTextFill",
        "quadrantYAxisTextFill",
        "quadrantTitleFill",
    ] {
        stage.assign_if_falsy(target, |stage| copy(stage, "primaryTextColor"))?;
    }
    for target in [
        "quadrantInternalBorderStrokeFill",
        "quadrantExternalBorderStrokeFill",
    ] {
        stage.assign_if_falsy(target, |stage| copy(stage, "primaryBorderColor"))?;
    }
    Ok(())
}

fn update_cynefin(
    stage: &mut ThemeState,
    cliff_color: &str,
    complex_background: &str,
    complicated_background: &str,
    chaotic_background: &str,
    clear_background: &str,
    confusion_background: &str,
) -> Result<(), ColorError> {
    stage.assign("cynefin", |stage| {
        let mut object = ObjectBuilder::new(stage, "cynefin");
        object.truthy_or("domainFontSize", |_| Ok(ComputedValue::number(16)))?;
        object.truthy_or("itemFontSize", |_| Ok(ComputedValue::number(12)))?;
        object.truthy_or("boundaryColor", |stage| copy(stage, "lineColor"))?;
        object.truthy_or("boundaryWidth", |_| Ok(ComputedValue::number(2)))?;
        object.truthy_or("cliffColor", |_| Ok(ComputedValue::string(cliff_color)))?;
        object.truthy_or("cliffWidth", |_| Ok(ComputedValue::number(4)))?;
        object.truthy_or("arrowColor", |stage| copy(stage, "lineColor"))?;
        object.truthy_or("arrowWidth", |_| Ok(ComputedValue::number(2)))?;
        object.truthy_or("complexBg", |_| {
            Ok(ComputedValue::string(complex_background))
        })?;
        object.truthy_or("complicatedBg", |_| {
            Ok(ComputedValue::string(complicated_background))
        })?;
        object.truthy_or("chaoticBg", |_| {
            Ok(ComputedValue::string(chaotic_background))
        })?;
        object.truthy_or("clearBg", |_| Ok(ComputedValue::string(clear_background)))?;
        object.truthy_or("confusionBg", |_| {
            Ok(ComputedValue::string(confusion_background))
        })?;
        object.truthy_or("textColor", |stage| copy(stage, "textColor"))?;
        object.truthy_or("labelColor", |stage| copy(stage, "primaryTextColor"))?;
        Ok(object.finish())
    })
}

fn update_xy_chart(stage: &mut ThemeState, palette: &str) -> Result<(), ColorError> {
    stage.assign("xyChart", |stage| {
        let mut object = ObjectBuilder::new(stage, "xyChart");
        object.truthy_or("backgroundColor", |stage| copy(stage, "background"))?;
        for field in [
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
            object.truthy_or(field, |stage| copy(stage, "primaryTextColor"))?;
        }
        object.truthy_or("plotColorPalette", |_| Ok(ComputedValue::string(palette)))?;
        Ok(object.finish())
    })
}

fn update_radar(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("radar", |stage| {
        let mut object = ObjectBuilder::new(stage, "radar");
        object.truthy_or("axisColor", |stage| copy(stage, "lineColor"))?;
        object.truthy_or("axisStrokeWidth", |_| Ok(ComputedValue::number(2)))?;
        object.truthy_or("axisLabelFontSize", |_| Ok(ComputedValue::number(12)))?;
        object.truthy_or("curveOpacity", |_| {
            Ok(ComputedValue::constant(number_value(0.5)))
        })?;
        object.truthy_or("curveStrokeWidth", |_| Ok(ComputedValue::number(2)))?;
        object.truthy_or("graticuleColor", |_| Ok(ComputedValue::string("#DEDEDE")))?;
        object.truthy_or("graticuleStrokeWidth", |_| Ok(ComputedValue::number(1)))?;
        object.truthy_or("graticuleOpacity", |_| {
            Ok(ComputedValue::constant(number_value(0.3)))
        })?;
        object.truthy_or("legendBoxSize", |_| Ok(ComputedValue::number(12)))?;
        object.truthy_or("legendFontSize", |_| Ok(ComputedValue::number(12)))?;
        Ok(object.finish())
    })
}

fn update_packet(stage: &mut ThemeState, fill_source: &str) -> Result<(), ColorError> {
    stage.assign("packet", |stage| {
        let mut object = ObjectBuilder::new(stage, "packet");
        for field in [
            "startByteColor",
            "endByteColor",
            "labelColor",
            "titleColor",
            "blockStrokeColor",
        ] {
            object.field(field, copy(stage, "primaryTextColor")?);
        }
        object.field("blockFillColor", copy(stage, fill_source)?);
        Ok(object.finish())
    })
}

fn update_wardley(
    stage: &mut ThemeState,
    evolution_color: &str,
    fill_source: &str,
) -> Result<(), ColorError> {
    stage.assign_if_falsy("wardleyEvolutionColor", |_| {
        Ok(ComputedValue::string(evolution_color))
    })?;
    stage.assign("wardley", |stage| {
        let mut object = ObjectBuilder::new(stage, "wardley");
        object.truthy_or("backgroundColor", |stage| copy(stage, "background"))?;
        object.truthy_or("axisColor", |stage| copy(stage, "lineColor"))?;
        object.truthy_or("axisTextColor", |stage| copy(stage, "primaryTextColor"))?;
        object.truthy_or("gridColor", |stage| copy(stage, "gridColor"))?;
        object.truthy_or("componentFill", |stage| copy(stage, fill_source))?;
        object.truthy_or("componentStroke", |stage| copy(stage, "lineColor"))?;
        object.truthy_or("componentLabelColor", |stage| {
            copy(stage, "primaryTextColor")
        })?;
        object.truthy_or("linkStroke", |stage| copy(stage, "lineColor"))?;
        object.truthy_or("evolutionStroke", |stage| {
            copy(stage, "wardleyEvolutionColor")
        })?;
        object.truthy_or("annotationStroke", |stage| copy(stage, "lineColor"))?;
        object.truthy_or("annotationTextColor", |stage| {
            copy(stage, "primaryTextColor")
        })?;
        object.truthy_or("annotationFill", |stage| copy(stage, fill_source))?;
        Ok(object.finish())
    })
}

fn update_requirement(
    stage: &mut ThemeState,
    relation_background: impl FnOnce(&ThemeState) -> Result<ComputedValue, ColorError>,
) -> Result<(), ColorError> {
    stage.assign_if_falsy("requirementBackground", |stage| copy(stage, "primaryColor"))?;
    stage.assign_if_falsy("requirementBorderColor", |stage| {
        copy(stage, "primaryBorderColor")
    })?;
    stage.assign_if_falsy("requirementBorderSize", |_| Ok(ComputedValue::string("1")))?;
    stage.assign_if_falsy("requirementTextColor", |stage| {
        copy(stage, "primaryTextColor")
    })?;
    stage.assign_if_falsy("relationColor", |stage| copy(stage, "lineColor"))?;
    stage.assign_if_falsy("relationLabelBackground", relation_background)?;
    stage.assign_if_falsy("relationLabelColor", |stage| copy(stage, "actorTextColor"))
}

fn update_tags(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign_if_falsy("tagLabelColor", |stage| copy(stage, "primaryTextColor"))?;
    stage.assign_if_falsy("tagLabelBackground", |stage| copy(stage, "primaryColor"))?;
    stage.assign("tagLabelBorder", |stage| {
        if let Some(value) = stage.truthy_from("tagBorder") {
            Ok(value)
        } else {
            copy(stage, "primaryBorderColor")
        }
    })?;
    stage.assign_if_falsy("tagLabelFontSize", |_| Ok(ComputedValue::string("10px")))?;
    stage.assign_if_falsy("commitLabelColor", |stage| {
        copy(stage, "secondaryTextColor")
    })?;
    stage.assign_if_falsy("commitLabelBackground", |stage| {
        copy(stage, "secondaryColor")
    })?;
    stage.assign_if_falsy("commitLabelFontSize", |_| Ok(ComputedValue::string("10px")))
}

fn update_event_modeling_light(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (target, value) in [
        ("emUiFill", "white"),
        ("emUiStroke", "#dbdada"),
        ("emProcessorFill", "#edb3f6"),
        ("emProcessorStroke", "#b88cbf"),
        ("emReadModelFill", "#d3f1a2"),
        ("emReadModelStroke", "#a3b732"),
        ("emCommandFill", "#bcd6fe"),
        ("emCommandStroke", "#679ac3"),
        ("emEventFill", "#ffb778"),
        ("emEventStroke", "#c19a0f"),
        ("emSwimlaneBackgroundOdd", "rgb(250,250,250)"),
        ("emSwimlaneBackgroundStroke", "rgb(240,240,240)"),
    ] {
        stage.assign_if_falsy(target, |_| Ok(ComputedValue::string(value)))?;
    }
    stage.assign_if_falsy("emArrowhead", |stage| copy(stage, "lineColor"))?;
    stage.assign_if_falsy("emRelationStroke", |stage| copy(stage, "lineColor"))?;
    stage.assign_if_falsy("attributeBackgroundColorOdd", |_| {
        Ok(ComputedValue::string("#ffffff"))
    })?;
    stage.assign_if_falsy("attributeBackgroundColorEven", |_| {
        Ok(ComputedValue::string("#f2f2f2"))
    })
}

fn update_pie_text(
    stage: &mut ThemeState,
    title_source: &str,
    legend_source: &str,
) -> Result<(), ColorError> {
    stage.assign_if_falsy("pieTitleTextSize", |_| Ok(ComputedValue::string("25px")))?;
    stage.assign_if_falsy("pieTitleTextColor", |stage| copy(stage, title_source))?;
    stage.assign_if_falsy("pieSectionTextSize", |_| Ok(ComputedValue::string("17px")))?;
    stage.assign_if_falsy("pieSectionTextColor", |stage| copy(stage, "textColor"))?;
    stage.assign_if_falsy("pieLegendTextSize", |_| Ok(ComputedValue::string("17px")))?;
    stage.assign_if_falsy("pieLegendTextColor", |stage| copy(stage, legend_source))?;
    stage.assign_if_falsy("pieStrokeColor", |_| Ok(ComputedValue::string("black")))?;
    stage.assign_if_falsy("pieStrokeWidth", |_| Ok(ComputedValue::string("2px")))?;
    stage.assign_if_falsy("pieOuterStrokeWidth", |_| Ok(ComputedValue::string("2px")))?;
    stage.assign_if_falsy("pieOuterStrokeColor", |_| {
        Ok(ComputedValue::string("black"))
    })?;
    stage.assign_if_falsy("pieOpacity", |_| Ok(ComputedValue::string("0.7")))
}

fn update_venn_from_scales(
    stage: &mut ThemeState,
    lighten_amount: Option<f64>,
) -> Result<(), ColorError> {
    for index in 0..8 {
        let target = format!("venn{}", index + 1);
        let source = format!("cScale{index}");
        stage.assign_if_nullish(&target, |stage| {
            if let Some(amount) = lighten_amount {
                lightened(stage, &source, amount)
            } else {
                copy(stage, &source)
            }
        })?;
    }
    stage.assign_if_nullish("vennTitleTextColor", |stage| copy(stage, "titleColor"))?;
    stage.assign_if_nullish("vennSetTextColor", |stage| copy(stage, "textColor"))
}

fn update_surfaces(
    stage: &mut ThemeState,
    hue: f64,
    saturation: f64,
    surface_lightness: impl Fn(usize) -> f64,
    peer_lightness: impl Fn(usize) -> f64,
) -> Result<(), ColorError> {
    for index in 0..5 {
        let surface = format!("surface{index}");
        stage.assign_if_falsy(&surface, |stage| {
            adjusted(
                stage,
                "mainBkg",
                ColorAdjustment::hsl(hue, saturation, surface_lightness(index)),
            )
        })?;
        let peer = format!("surfacePeer{index}");
        stage.assign_if_falsy(&peer, |stage| {
            adjusted(
                stage,
                "mainBkg",
                ColorAdjustment::hsl(hue, saturation, peer_lightness(index)),
            )
        })?;
    }
    Ok(())
}

fn update_dark(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("secondBkg", |stage| lightened(stage, "mainBkg", 16.0))?;
    stage.assign("lineColor", |stage| copy(stage, "mainContrastColor"))?;
    stage.assign("arrowheadColor", |stage| copy(stage, "mainContrastColor"))?;

    for (target, source) in [
        ("nodeBkg", "mainBkg"),
        ("nodeBorder", "border1"),
        ("clusterBkg", "secondBkg"),
        ("clusterBorder", "border2"),
        ("defaultLinkColor", "lineColor"),
    ] {
        stage.assign(target, |stage| copy(stage, source))?;
    }
    stage.assign("edgeLabelBackground", |stage| {
        lightened(stage, "labelBackground", 25.0)
    })?;

    for (target, source) in [
        ("actorBorder", "border1"),
        ("actorBkg", "mainBkg"),
        ("actorTextColor", "mainContrastColor"),
        ("actorLineColor", "actorBorder"),
        ("signalColor", "mainContrastColor"),
        ("signalTextColor", "mainContrastColor"),
        ("labelBoxBkgColor", "actorBkg"),
        ("labelBoxBorderColor", "actorBorder"),
        ("labelTextColor", "mainContrastColor"),
        ("loopTextColor", "mainContrastColor"),
        ("noteBorderColor", "secondaryBorderColor"),
        ("noteBkgColor", "secondBkg"),
        ("noteTextColor", "secondaryTextColor"),
        ("activationBorderColor", "border1"),
        ("activationBkgColor", "secondBkg"),
    ] {
        stage.assign(target, |stage| copy(stage, source))?;
    }
    stage.assign_if_falsy("rectBkgColor", |stage| copy(stage, "tertiaryColor"))?;

    stage.assign("altSectionBkgColor", |stage| copy(stage, "background"))?;
    stage.assign("taskBkgColor", |stage| lightened(stage, "mainBkg", 23.0))?;
    for (target, source) in [
        ("taskTextColor", "darkTextColor"),
        ("taskTextLightColor", "mainContrastColor"),
        ("taskTextOutsideColor", "taskTextLightColor"),
        ("gridColor", "mainContrastColor"),
        ("doneTaskBkgColor", "mainContrastColor"),
    ] {
        stage.assign(target, |stage| copy(stage, source))?;
    }
    stage.assign("taskTextDarkColor", |stage| {
        inverted(stage, "doneTaskBkgColor")
    })?;
    stage.assign("archEdgeColor", |stage| copy(stage, "lineColor"))?;
    stage.assign("archEdgeArrowColor", |stage| copy(stage, "lineColor"))?;

    update_state_dark(stage)?;
    update_journey(stage)?;

    for (index, value) in [
        "#0b0000", "#4d1037", "#3f5258", "#4f2f1b", "#6e0a0a", "#3b0048", "#995a01", "#154706",
        "#161722", "#00296f", "#01629c", "#010029",
    ]
    .into_iter()
    .enumerate()
    {
        let target = format!("cScale{}", index + 1);
        stage.assign_if_falsy(&target, |_| Ok(ComputedValue::string(value)))?;
    }
    stage.assign_if_falsy("cScale0", |stage| copy(stage, "primaryColor"))?;
    for_theme_color_indices(stage, |stage, index| {
        let scale = format!("cScale{index}");
        let inverse = format!("cScaleInv{index}");
        stage.assign_if_falsy(&inverse, |stage| inverted(stage, &scale))
    })?;
    for_theme_color_indices(stage, |stage, index| {
        let scale = format!("cScale{index}");
        let peer = format!("cScalePeer{index}");
        stage.assign_if_falsy(&peer, |stage| lightened(stage, &scale, 10.0))
    })?;
    update_surfaces(
        stage,
        30.0,
        -30.0,
        |i| 10.0 - i as f64 * 4.0,
        |i| 7.0 - i as f64 * 4.0,
    )?;

    stage.assign_if_falsy("scaleLabelColor", |stage| {
        if stage.variables.get("darkMode").is_some_and(is_js_truthy) {
            Ok(ComputedValue::with_dependencies(
                Value::String("black".to_string()),
                ["darkMode".to_string()],
            ))
        } else {
            copy(stage, "labelTextColor").map(|mut computed| {
                computed.dependencies.push("darkMode".to_string());
                computed
            })
        }
    })?;
    for_theme_color_indices(stage, |stage, index| {
        let target = format!("cScaleLabel{index}");
        stage.assign_if_falsy(&target, |stage| copy(stage, "scaleLabelColor"))
    })?;
    for_theme_color_indices(stage, |stage, index| {
        let pie = format!("pie{index}");
        let scale = format!("cScale{index}");
        stage.assign(&pie, |stage| copy(stage, &scale))
    })?;
    update_pie_text(stage, "mainContrastColor", "mainContrastColor")?;
    update_venn_from_scales(stage, Some(30.0))?;
    update_cynefin(
        stage, "#FF6B6B", "#1B5E20", "#0D47A1", "#BF360C", "#F57F17", "#4A148C",
    )?;
    update_quadrant(stage)?;
    update_xy_chart(
        stage,
        "#3498db,#2ecc71,#e74c3c,#f1c40f,#bdc3c7,#ffffff,#34495e,#9b59b6,#1abc9c,#e67e22",
    )?;
    update_packet(stage, "background")?;
    update_radar(stage)?;
    update_wardley(stage, "#ff6b6b", "mainBkg")?;
    stage.assign("classText", |stage| copy(stage, "primaryTextColor"))?;
    update_requirement(stage, |stage| {
        let dark_mode = stage.variables.get("darkMode").is_some_and(is_js_truthy);
        let mut computed = if dark_mode {
            darkened(stage, "secondaryColor", 30.0)?
        } else {
            copy(stage, "secondaryColor")?
        };
        computed.dependencies.push("darkMode".to_string());
        Ok(computed)
    })?;
    update_git_dark(stage)?;
    update_tags(stage)?;
    update_event_modeling_dark(stage)?;
    stage.assign_if_falsy("nodeBorder", |_| Ok(ComputedValue::string("#999")))?;
    Ok(())
}

fn update_state_dark(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign_if_falsy("transitionColor", |stage| copy(stage, "lineColor"))?;
    stage.assign_if_falsy("transitionLabelColor", |stage| copy(stage, "textColor"))?;
    stage.assign_if_falsy("stateLabelColor", |stage| {
        first_truthy(stage, &["stateBkg", "primaryTextColor"])
    })?;
    stage.assign_if_falsy("stateBkg", |stage| copy(stage, "mainBkg"))?;
    stage.assign_if_falsy("labelBackgroundColor", |stage| copy(stage, "stateBkg"))?;
    stage.assign_if_falsy("compositeBackground", |stage| {
        first_truthy(stage, &["background", "tertiaryColor"])
    })?;
    stage.assign_if_falsy("altBackground", |_| Ok(ComputedValue::string("#555")))?;
    stage.assign_if_falsy("compositeTitleBackground", |stage| copy(stage, "mainBkg"))?;
    stage.assign_if_falsy("compositeBorder", |stage| copy(stage, "nodeBorder"))?;
    stage.assign("innerEndBackground", |stage| {
        copy(stage, "primaryBorderColor")
    })?;
    stage.assign("specialStateColor", |_| {
        Ok(ComputedValue::string("#f4f4f4"))
    })?;
    stage.assign_if_falsy("errorBkgColor", |stage| copy(stage, "tertiaryColor"))?;
    stage.assign_if_falsy("errorTextColor", |stage| copy(stage, "tertiaryTextColor"))
}

fn update_git_dark(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("git0", |stage| lightened(stage, "secondaryColor", 20.0))?;
    for (target, pie, fallback, hue, amount) in [
        ("git1", "pie2", "secondaryColor", 0.0, 20.0),
        ("git2", "pie3", "tertiaryColor", 0.0, 20.0),
        ("git3", "pie4", "primaryColor", -30.0, 20.0),
        ("git4", "pie5", "primaryColor", -60.0, 20.0),
        ("git5", "pie6", "primaryColor", -90.0, 10.0),
        ("git6", "pie7", "primaryColor", 60.0, 10.0),
        ("git7", "pie8", "primaryColor", 120.0, 20.0),
    ] {
        stage.assign(target, |stage| {
            let source = if let Some(value) = stage.truthy_from(pie) {
                value
            } else if hue == 0.0 {
                copy(stage, fallback)?
            } else {
                adjusted(stage, fallback, ColorAdjustment::hsl(hue, 0.0, 0.0))?
            };
            transform_color(source, |color| theme_color::lighten(color, amount))
        })?;
    }
    for index in 0..8 {
        let git = format!("git{index}");
        let inverse = format!("gitInv{index}");
        stage.assign_if_falsy(&inverse, |stage| inverted(stage, &git))?;
    }
    for index in 0..8 {
        let target = format!("gitBranchLabel{index}");
        stage.assign_if_falsy(&target, |stage| {
            if matches!(index, 0 | 3) {
                inverted(stage, "labelTextColor")
            } else {
                copy(stage, "labelTextColor")
            }
        })?;
    }
    Ok(())
}

fn update_event_modeling_dark(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (target, value) in [
        ("emUiFill", "#2d2d2d"),
        ("emUiStroke", "#555"),
        ("emProcessorStroke", "#8a6d8c"),
        ("emReadModelStroke", "#6d8c5c"),
        ("emCommandStroke", "#5c6d8c"),
        ("emEventStroke", "#8c755c"),
    ] {
        stage.assign_if_falsy(target, |_| Ok(ComputedValue::string(value)))?;
    }
    for (target, color) in [
        ("emProcessorFill", "#5a3d5c"),
        ("emReadModelFill", "#3d5a2d"),
        ("emCommandFill", "#2d3d5a"),
        ("emEventFill", "#5a452d"),
    ] {
        stage.assign_if_falsy(target, |_| {
            Ok(ComputedValue::string(theme_color::lighten(color, 10.0)?))
        })?;
    }
    stage.assign_if_falsy("emSwimlaneBackgroundOdd", |stage| {
        lightened(stage, "background", 5.0)
    })?;
    stage.assign_if_falsy("emSwimlaneBackgroundStroke", |stage| {
        lightened(stage, "background", 12.0)
    })?;
    stage.assign_if_falsy("emArrowhead", |stage| copy(stage, "lineColor"))?;
    stage.assign_if_falsy("emRelationStroke", |stage| copy(stage, "lineColor"))?;
    stage.assign_if_falsy("attributeBackgroundColorOdd", |stage| {
        lightened(stage, "background", 12.0)
    })?;
    stage.assign_if_falsy("attributeBackgroundColorEven", |stage| {
        lightened(stage, "background", 2.0)
    })
}

fn update_forest(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("actorBorder", |stage| darkened(stage, "mainBkg", 20.0))?;
    stage.assign("actorBkg", |stage| copy(stage, "mainBkg"))?;
    stage.assign("labelBoxBkgColor", |stage| copy(stage, "actorBkg"))?;
    stage.assign("labelTextColor", |stage| copy(stage, "actorTextColor"))?;
    stage.assign("loopTextColor", |stage| copy(stage, "actorTextColor"))?;
    stage.assign("noteBorderColor", |stage| copy(stage, "border2"))?;
    stage.assign("noteTextColor", |stage| copy(stage, "actorTextColor"))?;
    stage.assign("actorLineColor", |stage| copy(stage, "actorBorder"))?;
    stage.assign_if_falsy("rectBkgColor", |stage| copy(stage, "tertiaryColor"))?;

    stage.assign_if_falsy("cScale0", |stage| copy(stage, "primaryColor"))?;
    stage.assign_if_falsy("cScale1", |stage| copy(stage, "secondaryColor"))?;
    stage.assign_if_falsy("cScale2", |stage| copy(stage, "tertiaryColor"))?;
    for (index, hue) in [30.0, 60.0, 90.0, 120.0, 150.0, 210.0, 270.0, 300.0, 330.0]
        .into_iter()
        .enumerate()
    {
        let target = format!("cScale{}", index + 3);
        stage.assign_if_falsy(&target, |stage| {
            adjusted(stage, "primaryColor", ColorAdjustment::hsl(hue, 0.0, 0.0))
        })?;
    }
    stage.assign_if_falsy("cScalePeer1", |stage| {
        darkened(stage, "secondaryColor", 45.0)
    })?;
    stage.assign_if_falsy("cScalePeer2", |stage| {
        darkened(stage, "tertiaryColor", 40.0)
    })?;
    for_theme_color_indices(stage, |stage, index| {
        let scale = format!("cScale{index}");
        stage.assign(&scale, |stage| darkened(stage, &scale, 10.0))?;
        let peer = format!("cScalePeer{index}");
        stage.assign_if_falsy(&peer, |stage| darkened(stage, &scale, 25.0))
    })?;
    for_theme_color_indices(stage, |stage, index| {
        let scale = format!("cScale{index}");
        let inverse = format!("cScaleInv{index}");
        stage.assign_if_falsy(&inverse, |stage| {
            adjusted(stage, &scale, ColorAdjustment::hsl(180.0, 0.0, 0.0))
        })
    })?;
    stage.assign("scaleLabelColor", |stage| {
        if stage
            .variables
            .get("scaleLabelColor")
            .is_some_and(|value| value.as_str() != Some("calculated") && is_js_truthy(value))
        {
            copy(stage, "scaleLabelColor")
        } else {
            copy(stage, "labelTextColor")
        }
    })?;
    for_theme_color_indices(stage, |stage, index| {
        let target = format!("cScaleLabel{index}");
        stage.assign_if_falsy(&target, |stage| copy(stage, "scaleLabelColor"))
    })?;
    update_surfaces(
        stage,
        30.0,
        -30.0,
        |i| -(5.0 + i as f64 * 5.0),
        |i| -(8.0 + i as f64 * 5.0),
    )?;

    for (target, source) in [
        ("nodeBkg", "mainBkg"),
        ("nodeBorder", "border1"),
        ("clusterBkg", "secondBkg"),
        ("clusterBorder", "border2"),
        ("defaultLinkColor", "lineColor"),
        ("taskBorderColor", "border1"),
        ("taskTextColor", "taskTextLightColor"),
        ("taskTextOutsideColor", "taskTextDarkColor"),
        ("activeTaskBorderColor", "taskBorderColor"),
        ("activeTaskBkgColor", "mainBkg"),
        ("archEdgeColor", "lineColor"),
        ("archEdgeArrowColor", "lineColor"),
    ] {
        stage.assign(target, |stage| copy(stage, source))?;
    }
    stage.assign_if_falsy("rowOdd", |stage| lightened(stage, "mainBkg", 75.0))?;
    stage.assign_if_falsy("rowEven", |stage| lightened(stage, "mainBkg", 20.0))?;
    update_state_forest(stage)?;
    stage.assign("classText", |stage| copy(stage, "primaryTextColor"))?;
    update_journey(stage)?;
    update_pie_forest(stage)?;
    update_venn_forest(stage)?;
    update_cynefin(
        stage, "#8B4513", "#C8E6C9", "#DCEDC8", "#FFE0B2", "#FFF9C4", "#D7CCC8",
    )?;
    update_quadrant(stage)?;
    update_packet(stage, "mainBkg")?;
    update_radar(stage)?;
    update_wardley(stage, "#dc3545", "background")?;
    update_xy_chart(
        stage,
        "#CDE498,#FF6B6B,#A0D2DB,#D7BDE2,#F0F0F0,#FFC3A0,#7FD8BE,#FF9A8B,#FAF3E0,#FFF176",
    )?;
    update_requirement(stage, |stage| copy(stage, "edgeLabelBackground"))?;
    update_git_forest(stage)?;
    update_tags(stage)?;
    update_event_modeling_light(stage)
}

fn update_state_forest(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign_if_falsy("transitionColor", |stage| copy(stage, "lineColor"))?;
    stage.assign_if_falsy("transitionLabelColor", |stage| copy(stage, "textColor"))?;
    stage.assign_if_falsy("stateLabelColor", |stage| {
        first_truthy(stage, &["stateBkg", "primaryTextColor"])
    })?;
    stage.assign_if_falsy("stateBkg", |stage| copy(stage, "mainBkg"))?;
    stage.assign_if_falsy("labelBackgroundColor", |stage| copy(stage, "stateBkg"))?;
    stage.assign_if_falsy("compositeBackground", |stage| {
        first_truthy(stage, &["background", "tertiaryColor"])
    })?;
    stage.assign_if_falsy("altBackground", |_| Ok(ComputedValue::string("#f0f0f0")))?;
    stage.assign_if_falsy("compositeTitleBackground", |stage| copy(stage, "mainBkg"))?;
    stage.assign_if_falsy("compositeBorder", |stage| copy(stage, "nodeBorder"))?;
    stage.assign("innerEndBackground", |stage| {
        copy(stage, "primaryBorderColor")
    })?;
    stage.assign("specialStateColor", |stage| copy(stage, "lineColor"))?;
    stage.assign_if_falsy("errorBkgColor", |stage| copy(stage, "tertiaryColor"))?;
    stage.assign_if_falsy("errorTextColor", |stage| copy(stage, "tertiaryTextColor"))?;
    Ok(())
}

fn update_pie_forest(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (target, source, hue, lightness) in [
        ("pie1", "primaryColor", 0.0, 0.0),
        ("pie2", "secondaryColor", 0.0, 0.0),
        ("pie3", "tertiaryColor", 0.0, 0.0),
        ("pie4", "primaryColor", 0.0, -30.0),
        ("pie5", "secondaryColor", 0.0, -30.0),
        ("pie6", "tertiaryColor", 40.0, -40.0),
        ("pie7", "primaryColor", 60.0, -10.0),
        ("pie8", "primaryColor", -60.0, -10.0),
        ("pie9", "primaryColor", 120.0, 0.0),
        ("pie10", "primaryColor", 60.0, -50.0),
        ("pie11", "primaryColor", -60.0, -50.0),
        ("pie12", "primaryColor", 120.0, -50.0),
    ] {
        stage.assign_if_falsy(target, |stage| {
            if hue == 0.0 && lightness == 0.0 {
                copy(stage, source)
            } else {
                adjusted(stage, source, ColorAdjustment::hsl(hue, 0.0, lightness))
            }
        })?;
    }
    update_pie_text(stage, "taskTextDarkColor", "taskTextDarkColor")
}

fn update_venn_forest(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (target, source, hue) in [
        ("venn1", "primaryColor", 0.0),
        ("venn2", "secondaryColor", 0.0),
        ("venn3", "tertiaryColor", 0.0),
        ("venn4", "primaryColor", 60.0),
        ("venn5", "primaryColor", -60.0),
        ("venn6", "secondaryColor", 60.0),
        ("venn7", "primaryColor", 120.0),
        ("venn8", "secondaryColor", 120.0),
    ] {
        stage.assign_if_nullish(target, |stage| {
            adjusted(stage, source, ColorAdjustment::hsl(hue, 0.0, -30.0))
        })?;
    }
    stage.assign_if_nullish("vennTitleTextColor", |stage| copy(stage, "titleColor"))?;
    stage.assign_if_nullish("vennSetTextColor", |stage| copy(stage, "textColor"))
}

fn update_git_forest(stage: &mut ThemeState) -> Result<(), ColorError> {
    for (index, source, hue) in [
        (0, "primaryColor", 0.0),
        (1, "secondaryColor", 0.0),
        (2, "tertiaryColor", 0.0),
        (3, "primaryColor", -30.0),
        (4, "primaryColor", -60.0),
        (5, "primaryColor", -90.0),
        (6, "primaryColor", 60.0),
        (7, "primaryColor", 120.0),
    ] {
        let target = format!("git{index}");
        stage.assign_if_falsy(&target, |stage| {
            if hue == 0.0 {
                copy(stage, source)
            } else {
                adjusted(stage, source, ColorAdjustment::hsl(hue, 0.0, 0.0))
            }
        })?;
    }
    let dark_mode = stage.variables.get("darkMode").is_some_and(is_js_truthy);
    for index in 0..8 {
        let target = format!("git{index}");
        stage.assign(&target, |stage| {
            let mut computed = if dark_mode {
                lightened(stage, &target, 25.0)?
            } else {
                darkened(stage, &target, 25.0)?
            };
            computed.dependencies.push("darkMode".to_string());
            Ok(computed)
        })?;
    }
    for index in 0..8 {
        let target = format!("git{index}");
        let inverse = format!("gitInv{index}");
        stage.assign_if_falsy(&inverse, |stage| inverted(stage, &target))?;
    }
    for index in 0..8 {
        let target = format!("gitBranchLabel{index}");
        stage.assign_if_falsy(&target, |stage| {
            if matches!(index, 0 | 3) {
                inverted(stage, "labelTextColor")
            } else {
                copy(stage, "labelTextColor")
            }
        })?;
    }
    Ok(())
}

fn update_neutral(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("secondBkg", |stage| lightened(stage, "contrast", 55.0))?;
    stage.assign("border2", |stage| copy(stage, "contrast"))?;
    stage.assign("actorBorder", |stage| lightened(stage, "border1", 23.0))?;
    for (target, source) in [
        ("actorBkg", "mainBkg"),
        ("actorTextColor", "text"),
        ("actorLineColor", "actorBorder"),
        ("signalColor", "text"),
        ("signalTextColor", "text"),
        ("labelBoxBkgColor", "actorBkg"),
        ("labelBoxBorderColor", "actorBorder"),
        ("labelTextColor", "text"),
        ("loopTextColor", "text"),
    ] {
        stage.assign(target, |stage| copy(stage, source))?;
    }
    stage.assign_if_falsy("rectBkgColor", |stage| copy(stage, "tertiaryColor"))?;
    for (target, value) in [
        ("noteBorderColor", "#999"),
        ("noteBkgColor", "#666"),
        ("noteTextColor", "#fff"),
    ] {
        stage.assign(target, |_| Ok(ComputedValue::string(value)))?;
    }

    for (index, value) in [
        "#555", "#F4F4F4", "#555", "#BBB", "#777", "#999", "#DDD", "#FFF", "#DDD", "#BBB", "#999",
        "#777",
    ]
    .into_iter()
    .enumerate()
    {
        let target = format!("cScale{index}");
        stage.assign_if_falsy(&target, |_| Ok(ComputedValue::string(value)))?;
    }
    let dark_mode = stage.variables.get("darkMode").is_some_and(is_js_truthy);
    for_theme_color_indices(stage, |stage, index| {
        let scale = format!("cScale{index}");
        let inverse = format!("cScaleInv{index}");
        stage.assign_if_falsy(&inverse, |stage| inverted(stage, &scale))
    })?;
    for_theme_color_indices(stage, |stage, index| {
        let scale = format!("cScale{index}");
        let peer = format!("cScalePeer{index}");
        stage.assign_if_falsy(&peer, |stage| {
            let mut computed = if dark_mode {
                lightened(stage, &scale, 10.0)?
            } else {
                darkened(stage, &scale, 10.0)?
            };
            computed.dependencies.push("darkMode".to_string());
            Ok(computed)
        })
    })?;
    stage.assign_if_falsy("scaleLabelColor", |stage| {
        if dark_mode {
            Ok(ComputedValue::with_dependencies(
                Value::String("black".to_string()),
                ["darkMode".to_string()],
            ))
        } else {
            copy(stage, "labelTextColor").map(|mut computed| {
                computed.dependencies.push("darkMode".to_string());
                computed
            })
        }
    })?;
    stage.assign_if_falsy("cScaleLabel0", |stage| copy(stage, "cScale1"))?;
    stage.assign_if_falsy("cScaleLabel2", |stage| copy(stage, "cScale1"))?;
    for_theme_color_indices(stage, |stage, index| {
        let target = format!("cScaleLabel{index}");
        stage.assign_if_falsy(&target, |stage| copy(stage, "scaleLabelColor"))
    })?;
    update_surfaces(
        stage,
        0.0,
        0.0,
        |i| -(5.0 + i as f64 * 5.0),
        |i| -(8.0 + i as f64 * 5.0),
    )?;

    for (target, source) in [
        ("nodeBkg", "mainBkg"),
        ("nodeBorder", "border1"),
        ("clusterBkg", "secondBkg"),
        ("clusterBorder", "border2"),
        ("defaultLinkColor", "lineColor"),
        ("titleColor", "text"),
    ] {
        stage.assign(target, |stage| copy(stage, source))?;
    }
    stage.assign("sectionBkgColor", |stage| {
        lightened(stage, "contrast", 30.0)
    })?;
    stage.assign("sectionBkgColor2", |stage| {
        lightened(stage, "contrast", 30.0)
    })?;
    stage.assign("taskBorderColor", |stage| darkened(stage, "contrast", 10.0))?;
    for (target, source) in [
        ("taskBkgColor", "contrast"),
        ("taskTextColor", "taskTextLightColor"),
        ("taskTextDarkColor", "text"),
        ("taskTextOutsideColor", "taskTextDarkColor"),
        ("activeTaskBorderColor", "taskBorderColor"),
        ("activeTaskBkgColor", "mainBkg"),
    ] {
        stage.assign(target, |stage| copy(stage, source))?;
    }
    stage.assign("gridColor", |stage| lightened(stage, "border1", 30.0))?;
    for (target, source) in [
        ("doneTaskBkgColor", "done"),
        ("doneTaskBorderColor", "lineColor"),
        ("critBkgColor", "critical"),
    ] {
        stage.assign(target, |stage| copy(stage, source))?;
    }
    stage.assign("critBorderColor", |stage| {
        darkened(stage, "critBkgColor", 10.0)
    })?;
    stage.assign("todayLineColor", |stage| copy(stage, "critBkgColor"))?;
    stage.assign("vertLineColor", |stage| copy(stage, "critBkgColor"))?;
    stage.assign("archEdgeColor", |stage| copy(stage, "lineColor"))?;
    stage.assign("archEdgeArrowColor", |stage| copy(stage, "lineColor"))?;

    update_state_neutral(stage)?;
    stage.assign("classText", |stage| copy(stage, "primaryTextColor"))?;
    update_journey(stage)?;
    for_theme_color_indices(stage, |stage, index| {
        let pie = format!("pie{index}");
        let scale = format!("cScale{index}");
        stage.assign(&pie, |stage| copy(stage, &scale))
    })?;
    stage.assign("pie12", |stage| copy(stage, "pie0"))?;
    update_pie_text(stage, "taskTextDarkColor", "taskTextDarkColor")?;
    update_venn_from_scales(stage, None)?;
    update_cynefin(
        stage, "#8B0000", "#E8F5E9", "#E3F2FD", "#FBE9E7", "#FFF8E1", "#F3E5F5",
    )?;
    update_quadrant(stage)?;
    update_xy_chart(
        stage,
        "#EEE,#6BB8E4,#8ACB88,#C7ACD6,#E8DCC2,#FFB2A8,#FFF380,#7E8D91,#FFD8B1,#FAF3E0",
    )?;
    update_radar(stage)?;
    update_wardley(stage, "#dc3545", "background")?;
    update_requirement(stage, |stage| copy(stage, "edgeLabelBackground"))?;
    update_git_neutral(stage)?;
    update_tags(stage)?;
    update_event_modeling_light(stage)
}

fn update_state_neutral(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign_if_falsy("transitionColor", |_| Ok(ComputedValue::string("#000")))?;
    stage.assign_if_falsy("transitionLabelColor", |stage| copy(stage, "textColor"))?;
    stage.assign_if_falsy("stateLabelColor", |stage| {
        first_truthy(stage, &["stateBkg", "primaryTextColor"])
    })?;
    stage.assign_if_falsy("stateBkg", |stage| copy(stage, "mainBkg"))?;
    stage.assign_if_falsy("labelBackgroundColor", |stage| copy(stage, "stateBkg"))?;
    stage.assign_if_falsy("compositeBackground", |stage| {
        first_truthy(stage, &["background", "tertiaryColor"])
    })?;
    stage.assign_if_falsy("altBackground", |_| Ok(ComputedValue::string("#f4f4f4")))?;
    stage.assign_if_falsy("compositeTitleBackground", |stage| copy(stage, "mainBkg"))?;
    stage.assign_if_falsy("stateBorder", |_| Ok(ComputedValue::string("#000")))?;
    stage.assign("innerEndBackground", |stage| {
        copy(stage, "primaryBorderColor")
    })?;
    stage.assign("specialStateColor", |_| Ok(ComputedValue::string("#222")))?;
    stage.assign_if_falsy("errorBkgColor", |stage| copy(stage, "tertiaryColor"))?;
    stage.assign_if_falsy("errorTextColor", |stage| copy(stage, "tertiaryTextColor"))
}

fn update_git_neutral(stage: &mut ThemeState) -> Result<(), ColorError> {
    stage.assign("git0", |stage| darkened(stage, "pie1", 25.0))?;
    for (target, pie, source, hue) in [
        ("git1", "pie2", "secondaryColor", 0.0),
        ("git2", "pie3", "tertiaryColor", 0.0),
        ("git3", "pie4", "primaryColor", -30.0),
        ("git4", "pie5", "primaryColor", -60.0),
        ("git5", "pie6", "primaryColor", -90.0),
        ("git6", "pie7", "primaryColor", 60.0),
        ("git7", "pie8", "primaryColor", 120.0),
    ] {
        stage.assign(target, |stage| {
            if let Some(value) = stage.truthy_from(pie) {
                Ok(value)
            } else if hue == 0.0 {
                copy(stage, source)
            } else {
                adjusted(stage, source, ColorAdjustment::hsl(hue, 0.0, 0.0))
            }
        })?;
    }
    for index in 0..8 {
        let git = format!("git{index}");
        let inverse = format!("gitInv{index}");
        stage.assign_if_falsy(&inverse, |stage| inverted(stage, &git))?;
    }
    stage.assign_if_falsy("branchLabelColor", |stage| copy(stage, "labelTextColor"))?;
    for index in 0..8 {
        let target = format!("gitBranchLabel{index}");
        if matches!(index, 1 | 3) {
            stage.assign(&target, |_| Ok(ComputedValue::string("white")))?;
        } else {
            stage.assign(&target, |stage| copy(stage, "branchLabelColor"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn iteration_count(value: Value) -> Result<usize, ThemeEvaluationLimitExceeded> {
        let mut variables = Map::new();
        variables.insert("THEME_COLOR_LIMIT".to_string(), value);
        theme_color_iteration_count(&ThemeState::constructor(variables))
    }

    #[test]
    fn theme_color_limit_uses_the_json_reachable_javascript_number_subset() {
        for value in [
            json!("0x410z"),
            json!("inf"),
            json!("infinity"),
            json!([true]),
            json!([1, 2]),
            json!({ "value": 2 }),
        ] {
            assert_eq!(iteration_count(value).unwrap(), 0);
        }
        for value in [json!("\u{feff}2\u{feff}"), json!([2]), json!([["2"]])] {
            assert_eq!(iteration_count(value).unwrap(), 2);
        }
        for value in [json!([]), json!([null]), json!([[[]]])] {
            assert_eq!(iteration_count(value).unwrap(), 0);
        }
        for value in [json!(65), json!("0x41"), json!("Infinity"), json!([65])] {
            assert_eq!(iteration_count(value).unwrap_err().max, 64);
        }
    }

    #[test]
    fn javascript_or_chain_does_not_evaluate_an_unselected_last_operand() {
        let mut variables = Map::new();
        variables.insert("selected".to_string(), json!("value"));
        let state = ThemeState::constructor(variables);

        let computed = first_truthy(&state, &["selected", "missing"]).unwrap();

        assert_eq!(computed.value, json!("value"));
        assert_eq!(computed.dependencies, vec!["selected"]);
    }
}
