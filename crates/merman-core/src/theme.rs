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
use staged::ThemeProgram;

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

const THEME_RUNTIME_SCHEMA_VERSION: u32 = 4;
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
    prepared_constructors: GeneratedThemeConstructors<'a>,
    #[serde(borrow)]
    themes: BTreeMap<String, GeneratedThemeMap<'a>>,
    #[serde(borrow)]
    dark_mode_true_overrides: BTreeMap<String, GeneratedThemeMap<'a>>,
    oracle_case_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedThemeConstructors<'a> {
    #[serde(borrow)]
    base: GeneratedThemeMap<'a>,
    #[serde(borrow)]
    dark: GeneratedThemeMap<'a>,
    #[serde(borrow)]
    default: GeneratedDefaultConstructor<'a>,
    #[serde(borrow)]
    forest: GeneratedThemeMap<'a>,
    #[serde(borrow)]
    neutral: GeneratedThemeMap<'a>,
}

impl<'a> GeneratedThemeConstructors<'a> {
    fn snapshots(&self) -> [&GeneratedThemeMap<'a>; 5] {
        [
            &self.base,
            &self.dark,
            &self.default.xy_chart,
            &self.forest,
            &self.neutral,
        ]
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedDefaultConstructor<'a> {
    #[serde(borrow)]
    xy_chart: GeneratedThemeMap<'a>,
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
    for snapshot in artifact.prepared_constructors.snapshots() {
        assert!(snapshot.encoded.get().starts_with('{'));
    }
    for theme in MermaidThemeId::ALL {
        for section in [&artifact.themes, &artifact.dark_mode_true_overrides] {
            assert!(section[theme.as_str()].encoded.get().starts_with('{'));
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
            .snapshots()
            .into_iter()
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

pub(crate) fn apply_theme_defaults(
    config: &mut SourcePresentationConfig,
) -> Result<(), ThemeResolutionError> {
    let requested = config.get_str("theme").unwrap_or("default");
    // Mermaid's raw configuration runtime falls back to `default` for unknown strings. Typed
    // Merman theme APIs use `MermaidThemeId::parse` and never enter this compatibility branch.
    let theme = MermaidThemeId::parse(requested).unwrap_or_default();
    let raw = theme_variables_map(config.as_config());
    let resolved = ThemeProgram::resolve(theme).resolve_variables(raw)?;
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

#[cfg(test)]
mod tests;
