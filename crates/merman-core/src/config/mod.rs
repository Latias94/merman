mod overlay;

pub(crate) use overlay::ConfigOverlayApplication;
#[cfg(test)]
pub(crate) use overlay::ConfigOverlayField;
pub(crate) use overlay::ConfigOverlayLane;
pub(crate) use overlay::{
    ConfigOverlayContribution, ConfigOverlayContributionProvenance, ConfigOverlayError,
    ConfigOverlayProvenance, PostDetectionConfigOverlay, PostDetectionConfigOverlayProvider,
};

use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

pub(crate) const HARDENED_SECURE_KEYS: &[&str] = &[
    "secure",
    "securityLevel",
    "startOnLoad",
    "maxTextSize",
    "suppressErrorRendering",
    "maxEdges",
    "fontFamily",
    "altFontFamily",
    "themeCSS",
    "themeVariables",
];

pub(crate) fn apply_hardened_site_policy(config: &mut MermaidConfig) {
    config.set_value(
        "secure",
        Value::Array(
            HARDENED_SECURE_KEYS
                .iter()
                .map(|key| Value::String((*key).to_string()))
                .collect(),
        ),
    );
}

#[derive(Clone)]
pub struct MermaidConfig {
    value: Arc<Value>,
    overlay_provenance: ConfigOverlayProvenance,
    theme_compatibility: Option<ThemeCompatibilityState>,
    explicit_config_paths: Arc<BTreeSet<Arc<str>>>,
    mutation_paths: Arc<BTreeMap<Arc<str>, u64>>,
    mutation_revision: u64,
}

const MAX_THEME_COMPATIBILITY_VARIABLES: usize = 512;
const MAX_THEME_COMPATIBILITY_IDENTIFIER_BYTES: usize = 128;
const MAX_THEME_COMPATIBILITY_STRING_BYTES: usize = 1024;
const MAX_THEME_COMPATIBILITY_NUMBER_ABS: f64 = 1_000_000_000.0;

/// Validation failure while constructing a [`ThemeParseBinding`].
///
/// The binding boundary deliberately accepts only Mermaid's bounded compatibility subset. Rich
/// renderer behavior belongs to the typed theme recipe instead of this upstream-config lane.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ThemeParseBindingError {
    #[error("theme compatibility config must be an object")]
    InvalidRoot,
    #[error("unsupported theme compatibility field `{field}`")]
    UnsupportedField { field: String },
    #[error("invalid theme compatibility value for `{field}`")]
    InvalidValue { field: &'static str },
    #[error("theme compatibility limit exceeded for `{field}`")]
    LimitExceeded { field: &'static str },
}

/// Opaque, normalized pairing of a compiled recipe identity and its Mermaid compatibility input.
///
/// This is metadata only; it is never serialized into Mermaid's configuration value. Its
/// constructor owns both pieces so an engine cannot install a recipe assertion with arbitrary
/// compatibility JSON, and a render session can compare the full parse contract.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThemeParseBinding {
    recipe_identity: [u8; 32],
    compatibility_config: Arc<Value>,
}

impl ThemeParseBinding {
    /// Validates and canonicalizes the restricted Mermaid compatibility input for one recipe.
    pub(crate) fn try_new(
        recipe_identity: [u8; 32],
        compatibility_config: MermaidConfig,
    ) -> Result<Self, ThemeParseBindingError> {
        Ok(Self {
            recipe_identity,
            compatibility_config: Arc::new(normalize_theme_compatibility_config(
                compatibility_config.as_value(),
            )?),
        })
    }

    pub(crate) const fn recipe_identity(&self) -> &[u8; 32] {
        &self.recipe_identity
    }

    fn into_tracking_config(self) -> MermaidConfig {
        let ownership = ThemeCompatibilityOwnership::from_config(
            self.clone(),
            self.compatibility_config.as_ref(),
        );
        MermaidConfig {
            value: self.compatibility_config,
            overlay_provenance: ConfigOverlayProvenance::default(),
            theme_compatibility: Some(ThemeCompatibilityState::Tracking(Arc::new(ownership))),
            explicit_config_paths: Arc::new(BTreeSet::new()),
            mutation_paths: Arc::new(BTreeMap::new()),
            mutation_revision: 0,
        }
    }
}

fn normalize_theme_compatibility_config(config: &Value) -> Result<Value, ThemeParseBindingError> {
    let root = config
        .as_object()
        .ok_or(ThemeParseBindingError::InvalidRoot)?;
    for key in root.keys() {
        if !matches!(key.as_str(), "theme" | "darkMode" | "themeVariables") {
            return Err(ThemeParseBindingError::UnsupportedField { field: key.clone() });
        }
    }

    let mut normalized = Map::new();
    if let Some(theme) = root.get("theme") {
        let Value::String(theme) = theme else {
            return Err(ThemeParseBindingError::InvalidValue { field: "theme" });
        };
        normalized.insert(
            "theme".to_string(),
            Value::String(normalize_theme_compatibility_identifier(theme, "theme")?),
        );
    }

    let root_dark_mode = root
        .get("darkMode")
        .map(|value| match value {
            Value::Bool(value) => Ok(*value),
            _ => Err(ThemeParseBindingError::InvalidValue { field: "darkMode" }),
        })
        .transpose()?;
    let mut variables = BTreeMap::new();
    let mut variable_dark_mode = None;
    if let Some(raw_variables) = root.get("themeVariables") {
        let Value::Object(raw_variables) = raw_variables else {
            return Err(ThemeParseBindingError::InvalidValue {
                field: "themeVariables",
            });
        };
        let mut variable_count = 0usize;
        for (raw_key, raw_value) in raw_variables {
            let key = normalize_theme_compatibility_identifier(raw_key, "themeVariables.key")?;
            if key == "darkMode" {
                let Value::Bool(value) = raw_value else {
                    return Err(ThemeParseBindingError::InvalidValue {
                        field: "themeVariables.darkMode",
                    });
                };
                variable_dark_mode = Some(*value);
                continue;
            }
            variable_count = variable_count.saturating_add(1);
            if variable_count > MAX_THEME_COMPATIBILITY_VARIABLES {
                return Err(ThemeParseBindingError::LimitExceeded {
                    field: "themeVariables",
                });
            }
            let value = normalize_theme_compatibility_scalar(raw_value)?;
            if variables.insert(key, value).is_some() {
                return Err(ThemeParseBindingError::InvalidValue {
                    field: "themeVariables.key",
                });
            }
        }
    }

    let dark_mode = match (root_dark_mode, variable_dark_mode) {
        (Some(root), Some(variable)) if root != variable => {
            return Err(ThemeParseBindingError::InvalidValue { field: "darkMode" });
        }
        (Some(value), _) | (_, Some(value)) => Some(value),
        (None, None) => None,
    };
    if let Some(dark_mode) = dark_mode {
        normalized.insert("darkMode".to_string(), Value::Bool(dark_mode));
        variables.insert("darkMode".to_string(), Value::Bool(dark_mode));
    }
    if !variables.is_empty() {
        normalized.insert(
            "themeVariables".to_string(),
            Value::Object(variables.into_iter().collect()),
        );
    }
    Ok(Value::Object(normalized))
}

fn normalize_theme_compatibility_identifier(
    value: &str,
    field: &'static str,
) -> Result<String, ThemeParseBindingError> {
    let normalized = value.trim();
    if normalized.is_empty()
        || normalized.len() > MAX_THEME_COMPATIBILITY_IDENTIFIER_BYTES
        || normalized.chars().any(|character| {
            character.is_control()
                || !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        })
    {
        return Err(ThemeParseBindingError::InvalidValue { field });
    }
    Ok(normalized.to_string())
}

fn normalize_theme_compatibility_scalar(value: &Value) -> Result<Value, ThemeParseBindingError> {
    match value {
        Value::String(value) => {
            let normalized = value.trim();
            if normalized.is_empty() || normalized.len() > MAX_THEME_COMPATIBILITY_STRING_BYTES {
                return Err(ThemeParseBindingError::InvalidValue {
                    field: "themeVariables.value",
                });
            }
            Ok(Value::String(normalized.to_string()))
        }
        Value::Number(value) => {
            let value = value.as_f64().ok_or(ThemeParseBindingError::InvalidValue {
                field: "themeVariables.value",
            })?;
            if !value.is_finite() || value.abs() > MAX_THEME_COMPATIBILITY_NUMBER_ABS {
                return Err(ThemeParseBindingError::InvalidValue {
                    field: "themeVariables.value",
                });
            }
            let value = if value == 0.0 { 0.0 } else { value };
            Ok(Value::Number(
                serde_json::Number::from_f64(value).expect("finite number checked above"),
            ))
        }
        Value::Bool(value) => Ok(Value::Bool(*value)),
        Value::Null | Value::Array(_) | Value::Object(_) => {
            Err(ThemeParseBindingError::InvalidValue {
                field: "themeVariables.value",
            })
        }
    }
}

#[derive(Debug, Clone)]
enum ThemeCompatibilityState {
    Tracking(Arc<ThemeCompatibilityOwnership>),
    Frozen {
        binding: ThemeParseBinding,
        fields: Arc<[FrozenThemeCompatibilityField]>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrozenThemeCompatibilityField {
    opaque_id: Arc<str>,
    kind: ThemeCompatibilityFieldKind,
    surviving_paths: Arc<[Arc<str>]>,
}

impl FrozenThemeCompatibilityField {
    pub(crate) fn opaque_id(&self) -> &str {
        &self.opaque_id
    }

    pub(crate) const fn kind(&self) -> ThemeCompatibilityFieldKind {
        self.kind
    }

    pub(crate) fn surviving_paths(&self) -> &[Arc<str>] {
        &self.surviving_paths
    }
}

#[derive(Debug, Clone)]
struct ThemeCompatibilityOwnership {
    binding: ThemeParseBinding,
    fields: Vec<ThemeCompatibilityFieldOwnership>,
}

#[derive(Debug, Clone)]
struct ThemeCompatibilityFieldOwnership {
    opaque_id: Arc<str>,
    kind: ThemeCompatibilityFieldKind,
    paths: Vec<ThemeCompatibilityPathOwnership>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemeCompatibilityFieldKind {
    Theme,
    DarkMode,
    Variable,
}

#[derive(Debug, Clone)]
struct ThemeCompatibilityPathOwnership {
    path: Arc<str>,
    owned: bool,
}

impl ThemeCompatibilityOwnership {
    fn from_config(binding: ThemeParseBinding, config: &Value) -> Self {
        let mut fields = Vec::new();
        let Some(root) = config.as_object() else {
            return Self { binding, fields };
        };

        if root.contains_key("theme") {
            fields.push(ThemeCompatibilityFieldOwnership::new(
                ThemeCompatibilityFieldKind::Theme,
                ["theme"],
            ));
        }

        let variables = root.get("themeVariables").and_then(Value::as_object);
        if root.contains_key("darkMode")
            || variables.is_some_and(|variables| variables.contains_key("darkMode"))
        {
            fields.push(ThemeCompatibilityFieldOwnership::new(
                ThemeCompatibilityFieldKind::DarkMode,
                ["darkMode", "themeVariables.darkMode"],
            ));
        }

        if let Some(variables) = variables {
            fields.extend(
                variables
                    .keys()
                    .filter(|key| key.as_str() != "darkMode")
                    .map(|key| {
                        ThemeCompatibilityFieldOwnership::new(
                            ThemeCompatibilityFieldKind::Variable,
                            [format!("themeVariables.{key}")],
                        )
                    }),
            );
        }

        Self { binding, fields }
    }

    fn shadow_path(&mut self, dotted_path: &str) {
        for field in &mut self.fields {
            for path in &mut field.paths {
                if path.owned && dotted_paths_overlap(&path.path, dotted_path) {
                    path.owned = false;
                }
            }
        }
    }

    fn freeze_fields(&self) -> Arc<[FrozenThemeCompatibilityField]> {
        self.fields
            .iter()
            .filter_map(|field| {
                let surviving_paths = field
                    .paths
                    .iter()
                    .filter(|path| path.owned)
                    .map(|path| Arc::clone(&path.path))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                (!surviving_paths.is_empty()).then(|| FrozenThemeCompatibilityField {
                    opaque_id: Arc::clone(&field.opaque_id),
                    kind: field.kind,
                    surviving_paths: surviving_paths.into(),
                })
            })
            .collect::<Vec<_>>()
            .into()
    }
}

impl ThemeCompatibilityFieldOwnership {
    fn new<I, P>(kind: ThemeCompatibilityFieldKind, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<Arc<str>>,
    {
        let paths = paths.into_iter().map(Into::into).collect::<Vec<Arc<str>>>();
        let primary_path = paths
            .first()
            .expect("theme compatibility conceptual fields must own at least one path");
        let opaque_id = match kind {
            ThemeCompatibilityFieldKind::Theme => Arc::from("mermaid.theme"),
            ThemeCompatibilityFieldKind::DarkMode => Arc::from("mermaid.darkMode"),
            ThemeCompatibilityFieldKind::Variable => Arc::from(format!("mermaid.{primary_path}")),
        };
        Self {
            opaque_id,
            kind,
            paths: paths
                .into_iter()
                .map(|path| ThemeCompatibilityPathOwnership {
                    path: path.into(),
                    owned: true,
                })
                .collect(),
        }
    }
}

impl fmt::Debug for MermaidConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("MermaidConfig")
            .field(&self.value)
            .finish()
    }
}

impl PartialEq for MermaidConfig {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Default for MermaidConfig {
    fn default() -> Self {
        Self::empty_object()
    }
}

impl MermaidConfig {
    pub fn empty_object() -> Self {
        Self::from_value(Value::Object(Map::new()))
    }

    pub fn from_value(value: Value) -> Self {
        Self {
            value: Arc::new(value),
            overlay_provenance: ConfigOverlayProvenance::default(),
            theme_compatibility: None,
            explicit_config_paths: Arc::new(BTreeSet::new()),
            mutation_paths: Arc::new(BTreeMap::new()),
            mutation_revision: 0,
        }
    }

    pub fn as_value(&self) -> &Value {
        self.value.as_ref()
    }

    /// Clones this config without recursion while enforcing a retained-size and nesting budget.
    ///
    /// `Ok(None)` means the owned value would exceed either budget. Cancellation remains distinct
    /// from budget rejection so callers can abandon an enclosing operation immediately.
    pub fn clone_value_bounded_controlled(
        &self,
        max_retained_bytes: usize,
        max_nesting_depth: usize,
        control: &OperationControl,
    ) -> OperationControlResult<Option<Value>> {
        clone_value_nonrecursive_controlled(
            self.as_value(),
            max_retained_bytes,
            max_nesting_depth,
            control,
        )
    }

    pub(crate) fn is_empty_object(&self) -> bool {
        matches!(self.as_value(), Value::Object(map) if map.is_empty())
    }

    pub(crate) fn has_tracking_theme_compatibility(&self) -> bool {
        matches!(
            self.theme_compatibility,
            Some(ThemeCompatibilityState::Tracking(_))
        )
    }

    pub(crate) fn overlay_provenance(&self) -> &ConfigOverlayProvenance {
        &self.overlay_provenance
    }

    pub(crate) fn set_overlay_provenance(&mut self, provenance: ConfigOverlayProvenance) {
        self.overlay_provenance = provenance;
    }

    pub(crate) fn theme_parse_binding(&self) -> Option<&ThemeParseBinding> {
        match self.theme_compatibility.as_ref() {
            Some(ThemeCompatibilityState::Frozen { binding, .. }) => Some(binding),
            Some(ThemeCompatibilityState::Tracking(_)) | None => None,
        }
    }

    /// Returns explicit Mermaid compatibility fields still owned by the parsed theme.
    #[cfg(test)]
    pub(crate) fn mermaid_compatibility_residual_count(&self) -> usize {
        match self.theme_compatibility.as_ref() {
            Some(ThemeCompatibilityState::Frozen { fields, .. }) => fields.len(),
            Some(ThemeCompatibilityState::Tracking(_)) | None => 0,
        }
    }

    pub(crate) fn mermaid_compatibility_fields(
        &self,
    ) -> Option<Arc<[FrozenThemeCompatibilityField]>> {
        match self.theme_compatibility.as_ref() {
            Some(ThemeCompatibilityState::Frozen { fields, .. }) => Some(Arc::clone(fields)),
            Some(ThemeCompatibilityState::Tracking(_)) | None => None,
        }
    }

    pub(crate) fn from_theme_parse_binding(binding: ThemeParseBinding) -> Self {
        binding.into_tracking_config()
    }

    pub(crate) fn adopt_tracking_theme_compatibility_from(&mut self, source: &mut Self) {
        let Some(state @ ThemeCompatibilityState::Tracking(_)) = source.theme_compatibility.take()
        else {
            return;
        };
        self.theme_compatibility = Some(state);
    }

    pub(crate) fn retain_theme_compatibility_paths_applied_after(
        &mut self,
        target: &Self,
        before: &Self,
    ) {
        let Some(ThemeCompatibilityState::Tracking(ownership)) = self.theme_compatibility.as_mut()
        else {
            return;
        };
        for field in &mut Arc::make_mut(ownership).fields {
            for path in &mut field.paths {
                if path.owned && !target.path_was_mutated_after(before, &path.path) {
                    path.owned = false;
                }
            }
        }
    }

    pub(crate) fn retain_normalized_theme_compatibility_variables(
        &mut self,
        normalized: &Map<String, Value>,
    ) {
        let Some(ThemeCompatibilityState::Tracking(ownership)) = self.theme_compatibility.as_mut()
        else {
            return;
        };
        for field in &mut Arc::make_mut(ownership).fields {
            for path in &mut field.paths {
                let Some(key) = path.path.strip_prefix("themeVariables.") else {
                    continue;
                };
                if path.owned && !normalized.contains_key(key) {
                    path.owned = false;
                }
            }
        }
    }

    pub(crate) fn freeze_theme_compatibility(&mut self) {
        let Some(ThemeCompatibilityState::Tracking(ownership)) = self.theme_compatibility.take()
        else {
            return;
        };
        self.theme_compatibility = Some(ThemeCompatibilityState::Frozen {
            binding: ownership.binding.clone(),
            fields: ownership.freeze_fields(),
        });
    }

    pub(crate) fn path_was_mutated_after(&self, before: &Self, dotted_path: &str) -> bool {
        self.mutation_paths.iter().any(|(candidate, revision)| {
            before
                .mutation_paths
                .get(candidate)
                .is_none_or(|before_revision| revision > before_revision)
                && dotted_paths_overlap(candidate.as_ref(), dotted_path)
        })
    }

    pub(crate) fn explicit_config_owns_path(&self, dotted_path: &str) -> bool {
        self.explicit_config_paths
            .iter()
            .any(|candidate| dotted_paths_overlap(candidate, dotted_path))
    }

    /// Reports whether a surviving compatibility fallback assignment owns this exact path.
    ///
    /// Fallback ownership is deliberately kept separate from typed/default ownership: the
    /// assignment may be consumed by a legacy-compatible terminal writer, but it must not be
    /// reported as a typed theme winner.
    pub(crate) fn fallback_overlay_owns_path(&self, dotted_path: &str) -> bool {
        self.overlay_provenance
            .fallback_contributions()
            .any(|contribution| {
                contribution
                    .surviving_assignment_paths()
                    .any(|path| path == dotted_path)
            })
    }

    pub(crate) fn config_path_overrides_typed_default(&self, dotted_path: &str) -> bool {
        self.explicit_config_owns_path(dotted_path)
            || matches!(
                self.theme_compatibility.as_ref(),
                Some(ThemeCompatibilityState::Frozen { fields, .. })
                    if fields
                        .iter()
                        .flat_map(|field| field.surviving_paths.iter())
                        .any(|candidate| dotted_paths_overlap(candidate, dotted_path))
            )
    }

    pub(crate) fn propagate_theme_variable_ownership(&mut self, source: &str, target: &str) {
        let source_path = format!("themeVariables.{source}");
        let target_path: Arc<str> = Arc::from(format!("themeVariables.{target}"));

        // Derived ownership follows exact calculation inputs. A parent `themeVariables` owner
        // must not implicitly claim every variable produced by a theme program.
        if self.explicit_config_paths.contains(source_path.as_str()) {
            Arc::make_mut(&mut self.explicit_config_paths).insert(Arc::clone(&target_path));
        }

        match self.theme_compatibility.as_mut() {
            Some(ThemeCompatibilityState::Tracking(ownership)) => {
                for field in &mut Arc::make_mut(ownership).fields {
                    let owns_source = field
                        .paths
                        .iter()
                        .any(|path| path.owned && path.path.as_ref() == source_path.as_str());
                    if owns_source
                        && !field
                            .paths
                            .iter()
                            .any(|path| path.path.as_ref() == target_path.as_ref())
                    {
                        field.paths.push(ThemeCompatibilityPathOwnership {
                            path: Arc::clone(&target_path),
                            owned: true,
                        });
                    }
                }
            }
            Some(ThemeCompatibilityState::Frozen { fields, .. }) => {
                for field in Arc::make_mut(fields) {
                    if field
                        .surviving_paths
                        .iter()
                        .any(|path| path.as_ref() == source_path.as_str())
                        && !field
                            .surviving_paths
                            .iter()
                            .any(|path| path.as_ref() == target_path.as_ref())
                    {
                        let mut surviving_paths = field.surviving_paths.to_vec();
                        surviving_paths.push(Arc::clone(&target_path));
                        surviving_paths.sort_unstable();
                        surviving_paths.dedup();
                        field.surviving_paths = surviving_paths.into();
                    }
                }
            }
            None => {}
        }
    }

    pub(crate) fn mark_mutations_after_as_explicit(&mut self, before: &Self) {
        let paths = self
            .mutation_paths
            .iter()
            .filter(|(candidate, revision)| {
                before
                    .mutation_paths
                    .get(*candidate)
                    .is_none_or(|before_revision| *revision > before_revision)
            })
            .map(|(path, _)| Arc::clone(path))
            .collect::<Vec<_>>();
        Arc::make_mut(&mut self.explicit_config_paths).extend(paths);
    }

    fn mark_explicit_config_path(&mut self, dotted_path: &str) {
        Arc::make_mut(&mut self.explicit_config_paths).insert(Arc::from(dotted_path));
    }

    #[cfg(test)]
    pub(crate) fn estimated_owned_heap_bytes(&self) -> usize {
        estimated_value_owned_heap_bytes(self.as_value())
    }

    pub fn as_value_mut(&mut self) -> &mut Value {
        self.shadow_theme_compatibility_path("");
        self.record_mutation("");
        self.value_mut()
    }

    pub fn get_str(&self, dotted_path: &str) -> Option<&str> {
        let mut cur: &Value = self.value.as_ref();
        for segment in dotted_path.split('.') {
            cur = cur.as_object()?.get(segment)?;
        }
        cur.as_str()
    }

    pub fn get_bool(&self, dotted_path: &str) -> Option<bool> {
        let mut cur: &Value = self.value.as_ref();
        for segment in dotted_path.split('.') {
            cur = cur.as_object()?.get(segment)?;
        }
        cur.as_bool()
    }

    pub fn set_value(&mut self, dotted_path: &str, value: Value) {
        self.shadow_theme_compatibility_path(dotted_path);
        self.record_mutation(dotted_path);
        self.set_value_without_theme_compatibility_shadow(dotted_path, value);
    }

    pub(crate) fn set_value_explicit(&mut self, dotted_path: &str, value: Value) {
        self.set_value(dotted_path, value);
        self.mark_explicit_config_path(dotted_path);
    }

    pub(crate) fn set_value_preserving_theme_compatibility(
        &mut self,
        dotted_path: &str,
        value: Value,
    ) {
        self.record_mutation(dotted_path);
        self.set_value_without_theme_compatibility_shadow(dotted_path, value);
    }

    fn set_value_without_theme_compatibility_shadow(&mut self, dotted_path: &str, value: Value) {
        let root_value = self.value_mut();
        // Be defensive: callers can construct `MermaidConfig` from any JSON value via
        // `from_value`. Mermaid configs are objects; if we see a non-object here, coerce it
        // to an object so this API never panics on user input.
        if !root_value.is_object() {
            replace_value_nonrecursive(root_value, Value::Object(Map::new()));
        }

        let Value::Object(root) = root_value else {
            return;
        };
        let mut cur: &mut Map<String, Value> = root;
        let mut segments = dotted_path.split('.').peekable();
        while let Some(seg) = segments.next() {
            if segments.peek().is_none() {
                if let Some(old) = cur.insert(seg.to_string(), value) {
                    drop_value_nonrecursive(old);
                }
                return;
            }
            let slot = cur.entry(seg).or_insert_with(|| Value::Object(Map::new()));
            if !slot.is_object() {
                replace_value_nonrecursive(slot, Value::Object(Map::new()));
            }
            let Some(next) = slot.as_object_mut() else {
                return;
            };
            cur = next;
        }
    }

    pub fn deep_merge(&mut self, other: &Value) {
        self.deep_merge_with_explicit_ownership(other, false);
    }

    pub(crate) fn deep_merge_explicit(&mut self, other: &Value) {
        self.deep_merge_with_explicit_ownership(other, true);
    }

    fn deep_merge_with_explicit_ownership(&mut self, other: &Value, explicit: bool) {
        // Preserve Mermaid's existing empty-object no-op. In particular, an empty patch must not
        // coerce a null root into an empty object or claim ownership of the whole configuration.
        if matches!(other, Value::Object(map) if map.is_empty()) {
            return;
        }
        let mutation_paths = {
            let base = self.value_mut();
            deep_merge_value_with_mutation_paths(base, other)
        };
        for path in mutation_paths {
            self.shadow_theme_compatibility_path(&path);
            self.record_mutation(&path);
            if explicit {
                self.mark_explicit_config_path(&path);
            }
        }
    }

    pub(crate) fn secure_filtered_overrides(&self, overrides: &MermaidConfig) -> MermaidConfig {
        let mut filtered = clone_value_nonrecursive(overrides.as_value());
        remove_secure_keys_recursive(self.as_value(), &mut filtered);
        MermaidConfig::from_value(filtered)
    }

    fn value_mut(&mut self) -> &mut Value {
        if Arc::strong_count(&self.value) != 1 || Arc::weak_count(&self.value) != 0 {
            self.value = Arc::new(clone_value_nonrecursive(self.value.as_ref()));
        }
        Arc::make_mut(&mut self.value)
    }

    fn record_mutation(&mut self, dotted_path: &str) {
        self.mutation_revision = self.mutation_revision.saturating_add(1);
        Arc::make_mut(&mut self.mutation_paths)
            .insert(Arc::from(dotted_path), self.mutation_revision);
    }

    fn shadow_theme_compatibility_path(&mut self, dotted_path: &str) {
        match self.theme_compatibility.as_mut() {
            Some(ThemeCompatibilityState::Tracking(ownership)) => {
                Arc::make_mut(ownership).shadow_path(dotted_path);
            }
            Some(ThemeCompatibilityState::Frozen { .. }) => {
                self.theme_compatibility = None;
            }
            None => {}
        }
    }
}

fn dotted_paths_overlap(left: &str, right: &str) -> bool {
    left.is_empty()
        || right.is_empty()
        || left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('.'))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

impl Drop for MermaidConfig {
    fn drop(&mut self) {
        if let Some(value) = Arc::get_mut(&mut self.value) {
            let old = std::mem::replace(value, Value::Null);
            drop_value_nonrecursive(old);
        }
    }
}

fn remove_secure_keys_recursive(site_config: &Value, overrides: &mut Value) {
    let secure_keys = site_config
        .get("secure")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

    let mut stack = vec![overrides];
    while let Some(current) = stack.pop() {
        match current {
            Value::Object(map) => {
                if let Some(old) = map.remove("secure") {
                    drop_value_nonrecursive(old);
                }
                for key in &secure_keys {
                    if let Some(old) = map.remove(*key) {
                        drop_value_nonrecursive(old);
                    }
                }
                for child in map.values_mut().rev() {
                    stack.push(child);
                }
            }
            Value::Array(items) => {
                for child in items.iter_mut().rev() {
                    stack.push(child);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
}

pub(crate) fn mirror_legacy_font_family_into_theme_variables(config: &mut MermaidConfig) {
    let value = config.value_mut();
    mirror_legacy_font_family_into_theme_variables_value(value);
}

pub(crate) fn mirror_legacy_font_family_into_theme_variables_value(value: &mut Value) {
    let Some(root) = value.as_object_mut() else {
        return;
    };
    let Some(font_family) = root
        .get("fontFamily")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
    else {
        return;
    };

    let has_theme_font_family = root
        .get("themeVariables")
        .and_then(Value::as_object)
        .and_then(|theme_variables| theme_variables.get("fontFamily"))
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    if has_theme_font_family {
        return;
    }

    let theme_variables = root
        .entry("themeVariables")
        .or_insert_with(|| Value::Object(Map::new()));
    if !theme_variables.is_object() {
        replace_value_nonrecursive(theme_variables, Value::Object(Map::new()));
    }
    if let Some(theme_variables) = theme_variables.as_object_mut()
        && let Some(old) =
            theme_variables.insert("fontFamily".to_string(), Value::String(font_family))
    {
        drop_value_nonrecursive(old);
    }
}

fn deep_merge_value_with_mutation_paths(base: &mut Value, incoming: &Value) -> BTreeSet<String> {
    // Mermaid 11.16.1 uses `assignWithDepth(dst, src)` with its default depth of two for site,
    // frontmatter, and directive configuration. Record ownership from that same execution rather
    // than maintaining a second approximation of which assignments were effective.
    let mut mutation_paths = BTreeSet::new();
    let mut path = Vec::new();
    assign_with_depth_recording(base, incoming, 2, &mut path, &mut |segments| {
        mutation_paths.insert(segments.join("."));
    });
    mutation_paths
}

fn assign_with_depth_recording<'a, F>(
    destination: &mut Value,
    source: &'a Value,
    depth: usize,
    path: &mut Vec<&'a str>,
    record_mutation: &mut F,
) where
    F: FnMut(&[&str]),
{
    if let Value::Array(source_items) = source {
        match destination {
            Value::Array(destination_items) => {
                if !source_items.is_empty() {
                    record_mutation(path);
                }
                merge_arrays(destination_items, source_items);
            }
            Value::Object(_) => merge_array_of_sources_recording(
                destination,
                source_items,
                depth,
                path,
                record_mutation,
            ),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
        return;
    }

    if destination.is_null() {
        // Mermaid 11.16.1 treats a null destination as an absent value and returns the source.
        // Mutating the slot in place is the JSON equivalent of the caller assigning that return
        // value back to the parent property.
        record_mutation(path);
        replace_value_nonrecursive(destination, clone_value_nonrecursive(source));
        return;
    }

    if source.is_null() {
        // `assignWithDepth` ignores null-valued object properties when the destination is present.
        return;
    }

    if depth == 0 {
        object_assign_recording(destination, source, path, record_mutation);
        return;
    }

    let (Value::Object(destination_map), Value::Object(source_map)) = (destination, source) else {
        return;
    };

    for (key, source_child) in source_map {
        if is_non_null_js_object(source_child) {
            match destination_map.get_mut(key) {
                Some(destination_child) if is_js_object(destination_child) => {
                    path.push(key);
                    assign_with_depth_recording(
                        destination_child,
                        source_child,
                        depth - 1,
                        path,
                        record_mutation,
                    );
                    path.pop();
                }
                Some(_) => {}
                None => {
                    destination_map.insert(key.clone(), empty_container_for(source_child));
                    if let Some(destination_child) = destination_map.get_mut(key) {
                        path.push(key);
                        assign_with_depth_recording(
                            destination_child,
                            source_child,
                            depth - 1,
                            path,
                            record_mutation,
                        );
                        path.pop();
                    }
                }
            }
        } else if !source_child.is_null()
            && destination_map
                .get(key)
                .is_none_or(|value| !is_js_object(value))
        {
            path.push(key);
            record_mutation(path);
            path.pop();
            insert_cloned(destination_map, key, source_child);
        }
    }
}

fn merge_array_of_sources_recording<'a, F>(
    destination: &mut Value,
    source_items: &'a [Value],
    depth: usize,
    path: &mut Vec<&'a str>,
    record_mutation: &mut F,
) where
    F: FnMut(&[&str]),
{
    let mut stack = source_items.iter().rev().collect::<Vec<_>>();
    while let Some(source) = stack.pop() {
        if let Value::Array(items) = source {
            stack.extend(items.iter().rev());
        } else {
            assign_with_depth_recording(destination, source, depth, path, record_mutation);
        }
    }
}

fn merge_arrays(destination: &mut Vec<Value>, source: &[Value]) {
    for source_item in source {
        let already_present = is_json_primitive(source_item)
            && destination
                .iter()
                .any(|destination_item| same_json_primitive(destination_item, source_item));
        if !already_present {
            destination.push(clone_value_nonrecursive(source_item));
        }
    }
}

fn object_assign_recording<'a, F>(
    destination: &mut Value,
    source: &'a Value,
    path: &mut Vec<&'a str>,
    record_mutation: &mut F,
) where
    F: FnMut(&[&str]),
{
    match (destination, source) {
        (Value::Object(destination_map), Value::Object(source_map)) => {
            for (key, source_child) in source_map {
                path.push(key);
                record_mutation(path);
                path.pop();
                insert_cloned(destination_map, key, source_child);
            }
        }
        // JavaScript arrays can own named properties, while JSON arrays cannot represent them.
        // Numeric config keys are not part of Mermaid's public config shape, so leave this
        // unrepresentable object-to-array case unchanged.
        (Value::Array(_), Value::Object(_)) => {}
        (destination, source) => {
            record_mutation(path);
            replace_value_nonrecursive(destination, clone_value_nonrecursive(source));
        }
    }
}

fn insert_cloned(destination: &mut Map<String, Value>, key: &str, source: &Value) {
    if let Some(previous) = destination.insert(key.to_string(), clone_value_nonrecursive(source)) {
        drop_value_nonrecursive(previous);
    }
}

fn empty_container_for(value: &Value) -> Value {
    if value.is_array() {
        Value::Array(Vec::new())
    } else {
        Value::Object(Map::new())
    }
}

fn is_js_object(value: &Value) -> bool {
    matches!(value, Value::Null | Value::Array(_) | Value::Object(_))
}

fn is_non_null_js_object(value: &Value) -> bool {
    matches!(value, Value::Array(_) | Value::Object(_))
}

fn is_json_primitive(value: &Value) -> bool {
    matches!(
        value,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
    )
}

fn same_json_primitive(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Number(left), Value::Number(right)) => left.as_f64() == right.as_f64(),
        (Value::String(left), Value::String(right)) => left == right,
        _ => false,
    }
}

pub(crate) fn replace_value_nonrecursive(slot: &mut Value, value: Value) {
    let old = std::mem::replace(slot, value);
    drop_value_nonrecursive(old);
}

pub(crate) fn clone_value_nonrecursive(value: &Value) -> Value {
    let control = OperationControl::new();
    clone_value_nonrecursive_with_control(value, &control)
        .expect("a private operation control cannot be cancelled")
}

pub(crate) fn clone_value_nonrecursive_with_control(
    value: &Value,
    control: &OperationControl,
) -> OperationControlResult<Value> {
    clone_value_nonrecursive_controlled(value, usize::MAX, usize::MAX, control)
        .map(|value| value.expect("unbounded config cloning cannot exceed its budget"))
}

fn clone_value_nonrecursive_controlled(
    value: &Value,
    max_retained_bytes: usize,
    max_nesting_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<Value>> {
    let mut cloned: HashMap<*const Value, Value> = HashMap::new();
    let mut stack = vec![(value, false, 0usize)];
    let mut retained_bytes = 0usize;
    let mut visited_nodes = 0usize;

    while let Some((current, visited, depth)) = stack.pop() {
        if visited_nodes.is_multiple_of(64)
            && let Err(cancelled) = control.checkpoint()
        {
            drop_cloned_values(cloned);
            return Err(cancelled);
        }
        visited_nodes = visited_nodes.saturating_add(1);
        let current_ptr = std::ptr::from_ref(current);
        if visited {
            retained_bytes = retained_bytes.saturating_add(value_clone_weight(current));
            if retained_bytes > max_retained_bytes {
                drop_cloned_values(cloned);
                return Ok(None);
            }
            let value = match current {
                Value::Null => Value::Null,
                Value::Bool(v) => Value::Bool(*v),
                Value::Number(v) => Value::Number(v.clone()),
                Value::String(v) => Value::String(v.clone()),
                Value::Array(items) => {
                    let mut out = Vec::with_capacity(items.len());
                    for (index, item) in items.iter().enumerate() {
                        if index.is_multiple_of(64)
                            && let Err(cancelled) = control.checkpoint()
                        {
                            drop_value_nonrecursive(Value::Array(out));
                            drop_cloned_values(cloned);
                            return Err(cancelled);
                        }
                        if let Some(value) = cloned.remove(&std::ptr::from_ref(item)) {
                            out.push(value);
                        }
                    }
                    Value::Array(out)
                }
                Value::Object(entries) => {
                    let mut out = Map::new();
                    for (index, (key, child)) in entries.iter().enumerate() {
                        if index.is_multiple_of(64)
                            && let Err(cancelled) = control.checkpoint()
                        {
                            drop_value_nonrecursive(Value::Object(out));
                            drop_cloned_values(cloned);
                            return Err(cancelled);
                        }
                        if let Some(value) = cloned.remove(&std::ptr::from_ref(child)) {
                            out.insert(key.clone(), value);
                        }
                    }
                    Value::Object(out)
                }
            };
            cloned.insert(current_ptr, value);
        } else {
            let has_children = matches!(current, Value::Array(items) if !items.is_empty())
                || matches!(current, Value::Object(entries) if !entries.is_empty());
            if has_children && depth >= max_nesting_depth {
                drop_cloned_values(cloned);
                return Ok(None);
            }
            let structural_weight = value_structural_weight(current);
            retained_bytes = retained_bytes.saturating_add(structural_weight);
            if retained_bytes > max_retained_bytes {
                drop_cloned_values(cloned);
                return Ok(None);
            }
            stack.push((current, true, depth));
            match current {
                Value::Array(items) => {
                    for item in items.iter().rev() {
                        stack.push((item, false, depth.saturating_add(1)));
                    }
                }
                Value::Object(entries) => {
                    for child in entries.values().rev() {
                        stack.push((child, false, depth.saturating_add(1)));
                    }
                }
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
            }
        }
    }

    if let Err(cancelled) = control.checkpoint() {
        drop_cloned_values(cloned);
        return Err(cancelled);
    }
    Ok(Some(
        cloned
            .remove(&std::ptr::from_ref(value))
            .unwrap_or(Value::Null),
    ))
}

fn value_structural_weight(value: &Value) -> usize {
    match value {
        Value::Array(items) => items.len().saturating_mul(size_of::<Value>()),
        Value::Object(entries) => entries.iter().fold(
            entries.len().saturating_mul(
                size_of::<String>()
                    .saturating_add(size_of::<Value>())
                    .saturating_add(size_of::<usize>().saturating_mul(4)),
            ),
            |weight, (key, _)| weight.saturating_add(key.len()),
        ),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
    }
}

fn value_clone_weight(value: &Value) -> usize {
    size_of::<Value>().saturating_add(match value {
        Value::String(value) => value.len(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => 0,
    })
}

#[cfg(test)]
fn estimated_value_owned_heap_bytes(value: &Value) -> usize {
    let mut retained_bytes = 0usize;
    let mut stack = vec![value];
    while let Some(current) = stack.pop() {
        retained_bytes = retained_bytes
            .saturating_add(value_structural_weight(current))
            .saturating_add(value_clone_weight(current));
        match current {
            Value::Array(items) => stack.extend(items),
            Value::Object(entries) => stack.extend(entries.values()),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    retained_bytes
}

fn drop_cloned_values(cloned: HashMap<*const Value, Value>) {
    for value in cloned.into_values() {
        drop_value_nonrecursive(value);
    }
}

pub(crate) fn drop_value_nonrecursive(value: Value) {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            Value::Array(items) => {
                stack.extend(items);
            }
            Value::Object(entries) => {
                stack.extend(entries.into_values());
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dotted_path_overlap_is_symmetric_for_parent_and_child_paths() {
        assert!(dotted_paths_overlap(
            "themeVariables",
            "themeVariables.darkMode"
        ));
        assert!(dotted_paths_overlap(
            "themeVariables.darkMode",
            "themeVariables"
        ));
        assert!(!dotted_paths_overlap(
            "themeVariables.darkMode",
            "themeVariables.useGradient"
        ));
    }

    #[test]
    fn dark_mode_ownership_remains_one_concept_when_one_mirrored_path_survives() {
        let compatibility = json!({
            "darkMode": true,
            "themeVariables": {
                "darkMode": true
            }
        });
        let binding = ThemeParseBinding::try_new(
            [0; 32],
            MermaidConfig::from_value(clone_value_nonrecursive(&compatibility)),
        )
        .expect("valid mirrored dark-mode compatibility");
        let mut ownership = ThemeCompatibilityOwnership::from_config(binding, &compatibility);

        ownership.shadow_path("darkMode");

        let fields = ownership.freeze_fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].kind(), ThemeCompatibilityFieldKind::DarkMode);
        assert_eq!(
            fields[0].surviving_paths(),
            &[Arc::<str>::from("themeVariables.darkMode")]
        );
    }

    #[test]
    fn explicit_ownership_uses_effective_assign_with_depth_paths() {
        let mut config = MermaidConfig::from_value(json!({
            "flowchart": {
                "nodeSpacing": 50
            },
            "retainedScalar": "site"
        }));

        config.deep_merge_explicit(&json!({
            "flowchart": {
                "nodeSpacing": 50
            },
            "retainedScalar": {
                "child": true
            }
        }));

        assert!(config.explicit_config_owns_path("flowchart.nodeSpacing"));
        assert!(config.explicit_config_owns_path("flowchart"));
        assert!(config.explicit_config_owns_path("flowchart.nodeSpacing.unmaterializedDescendant"));
        assert!(!config.explicit_config_owns_path("flowchart.rankSpacing"));
        assert!(!config.explicit_config_owns_path("retainedScalar.child"));
    }

    #[test]
    fn mirror_legacy_font_family_populates_missing_theme_variable() {
        let mut cfg = MermaidConfig::from_value(json!({
            "fontFamily": "Courier"
        }));

        mirror_legacy_font_family_into_theme_variables(&mut cfg);

        assert_eq!(cfg.get_str("themeVariables.fontFamily"), Some("Courier"));
    }

    #[test]
    fn mirror_legacy_font_family_preserves_explicit_theme_variable() {
        let mut cfg = MermaidConfig::from_value(json!({
            "fontFamily": "Courier",
            "themeVariables": {
                "fontFamily": "Inter"
            }
        }));

        mirror_legacy_font_family_into_theme_variables(&mut cfg);

        assert_eq!(cfg.get_str("themeVariables.fontFamily"), Some("Inter"));
    }

    #[test]
    fn deep_merge_ignores_null_source_values() {
        let mut config = MermaidConfig::from_value(json!({
            "theme": "default",
            "flowchart": {
                "htmlLabels": true
            }
        }));

        config.deep_merge(&json!({
            "theme": null,
            "flowchart": {
                "htmlLabels": null,
                "curve": "basis"
            },
            "newValue": null
        }));

        assert_eq!(
            config.as_value(),
            &json!({
                "theme": "default",
                "flowchart": {
                    "htmlLabels": true,
                    "curve": "basis"
                }
            })
        );

        config.deep_merge(&Value::Null);
        assert_eq!(config.get_str("theme"), Some("default"));
    }

    #[test]
    fn deep_merge_empty_object_is_a_noop_for_a_null_root() {
        let mut config = MermaidConfig::from_value(Value::Null);

        config.deep_merge(&json!({}));

        assert_eq!(config.as_value(), &Value::Null);
    }

    #[test]
    fn deep_merge_preserves_null_values_inside_arrays() {
        let mut config = MermaidConfig::from_value(json!({
            "values": ["old"]
        }));

        config.deep_merge(&json!({
            "values": [1, null, 2]
        }));

        assert_eq!(config.as_value()["values"], json!(["old", 1, null, 2]));
    }

    #[test]
    fn deep_merge_preserves_null_values_at_the_depth_boundary() {
        let mut config = MermaidConfig::default();

        config.deep_merge(&json!({
            "flowchart": {
                "curve": "basis",
                "nested": {
                    "ignored": null,
                    "kept": true
                },
                "values": [1, null, { "insideArray": null }]
            }
        }));

        assert_eq!(
            config.as_value()["flowchart"],
            json!({
                "curve": "basis",
                "nested": {
                    "ignored": null,
                    "kept": true
                },
                "values": [1, null, { "insideArray": null }]
            })
        );
    }

    #[test]
    fn deep_merge_deduplicates_primitives_in_new_arrays() {
        let mut config = MermaidConfig::default();

        config.deep_merge(&json!({
            "values": [1, 1.0, null, null, { "value": 1 }, { "value": 1 }]
        }));

        assert_eq!(
            config.as_value()["values"],
            json!([1, null, { "value": 1 }, { "value": 1 }])
        );
    }

    #[test]
    fn deep_merge_does_not_clobber_dissimilar_types_but_replaces_null_destinations() {
        let mut config = MermaidConfig::from_value(json!({
            "object": { "kept": true },
            "scalar": "kept",
            "nullObject": null,
            "nullScalar": null
        }));

        config.deep_merge(&json!({
            "object": "ignored",
            "scalar": { "ignored": true },
            "nullObject": { "accepted": true },
            "nullScalar": "accepted"
        }));

        assert_eq!(
            config.as_value(),
            &json!({
                "object": { "kept": true },
                "scalar": "kept",
                "nullObject": { "accepted": true },
                "nullScalar": null
            })
        );

        let mut root_null = MermaidConfig::from_value(Value::Null);
        root_null.deep_merge(&json!({ "accepted": true }));
        assert_eq!(root_null.as_value(), &json!({ "accepted": true }));
    }

    #[test]
    fn deep_merge_uses_object_assign_at_the_default_depth_boundary() {
        let mut config = MermaidConfig::from_value(json!({
            "bar": {
                "bar": {
                    "foo": {
                        "message": "old",
                        "willBe": "clobbered"
                    },
                    "preservedSibling": true
                }
            }
        }));

        config.deep_merge(&json!({
            "bar": {
                "bar": {
                    "foo": {
                        "message": "new"
                    }
                }
            }
        }));

        assert_eq!(
            config.as_value(),
            &json!({
                "bar": {
                    "bar": {
                        "foo": {
                            "message": "new"
                        },
                        "preservedSibling": true
                    }
                }
            })
        );
    }

    #[test]
    fn deep_merge_replaces_nested_null_at_the_depth_boundary() {
        let mut config = MermaidConfig::from_value(json!({
            "outer": {
                "inner": {
                    "slot": null
                }
            }
        }));

        config.deep_merge(&json!({
            "outer": {
                "inner": {
                    "slot": { "accepted": true }
                }
            }
        }));

        assert_eq!(
            config.as_value()["outer"]["inner"]["slot"],
            json!({ "accepted": true })
        );
    }

    fn deep_config_value(depth: usize) -> Value {
        let mut value = Value::String("leaf".to_string());
        for idx in (0..depth).rev() {
            let mut map = Map::new();
            map.insert(format!("k{idx}"), value);
            value = Value::Object(map);
        }
        value
    }

    #[test]
    fn clone_on_write_handles_deep_config_with_small_stack() {
        const DEPTH: usize = 2_048;
        let value = deep_config_value(DEPTH);
        let handle = std::thread::Builder::new()
            .name("mermaid-config-deep-clone-on-write".to_string())
            .stack_size(64 * 1024)
            .spawn(move || {
                let original = MermaidConfig::from_value(value);
                let mut cloned = original.clone();
                cloned.set_value("theme", Value::String("default".to_string()));
                assert_eq!(cloned.get_str("theme"), Some("default"));
            })
            .expect("spawn deep config clone-on-write test");
        handle
            .join()
            .expect("deep config clone-on-write should finish without stack overflow");
    }

    #[test]
    fn bounded_controlled_config_clone_preserves_values_within_budget() {
        let config = MermaidConfig::from_value(json!({
            "theme": "dark",
            "flowchart": { "htmlLabels": false },
        }));
        let control = OperationControl::new();

        let cloned = config
            .clone_value_bounded_controlled(64 * 1024, 16, &control)
            .expect("active control")
            .expect("small config fits the materialization budget");

        assert_eq!(&cloned, config.as_value());
    }

    #[test]
    fn bounded_controlled_config_clone_rejects_weight_and_depth_before_cloning() {
        let oversized = MermaidConfig::from_value(json!({ "payload": "x".repeat(4 * 1024) }));
        let deep = MermaidConfig::from_value(deep_config_value(8));
        let control = OperationControl::new();

        assert!(
            oversized
                .clone_value_bounded_controlled(1_024, 16, &control)
                .expect("active control")
                .is_none()
        );
        assert!(
            deep.clone_value_bounded_controlled(64 * 1024, 4, &control)
                .expect("active control")
                .is_none()
        );
    }

    #[test]
    fn bounded_controlled_config_clone_observes_cancellation() {
        let config = MermaidConfig::from_value(json!({ "theme": "dark" }));
        let control = OperationControl::new();
        control.cancel();

        assert!(matches!(
            config.clone_value_bounded_controlled(64 * 1024, 16, &control),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn upstream_secure_key_list_matches_mermaid_runtime() {
        let upstream = crate::generated::upstream_default_config();
        let secure = upstream
            .as_value()
            .get("secure")
            .and_then(Value::as_array)
            .expect("upstream secure array")
            .iter()
            .map(|value| value.as_str().expect("secure key string"))
            .collect::<Vec<_>>();

        assert_eq!(
            secure,
            [
                "secure",
                "securityLevel",
                "startOnLoad",
                "maxTextSize",
                "suppressErrorRendering",
                "maxEdges"
            ]
        );
    }

    #[test]
    fn default_site_config_applies_hardened_secure_policy() {
        let default = crate::generated::default_site_config();
        let secure = default
            .as_value()
            .get("secure")
            .and_then(Value::as_array)
            .expect("hardened secure array")
            .iter()
            .map(|value| value.as_str().expect("secure key string"))
            .collect::<Vec<_>>();

        assert_eq!(secure, HARDENED_SECURE_KEYS);
    }

    #[test]
    fn secure_filtered_overrides_removes_default_secure_keys_recursively() {
        let site_config = crate::generated::default_site_config();
        let overrides = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "fontFamily": "diagram-font",
            "flowchart": {
                "securityLevel": "sandbox",
                "htmlLabels": false,
                "nested": [
                    {
                        "securityLevel": "loose",
                        "shape": "rect"
                    }
                ]
            }
        }));

        let filtered = site_config.secure_filtered_overrides(&overrides);

        assert!(filtered.get_str("fontFamily").is_none());
        assert_eq!(filtered.get_bool("flowchart.htmlLabels"), Some(false));
        assert_eq!(
            filtered.as_value()["flowchart"]["nested"][0]["shape"],
            json!("rect")
        );
        assert!(filtered.get_str("securityLevel").is_none());
        assert!(filtered.get_str("flowchart.securityLevel").is_none());
        assert!(
            filtered.as_value()["flowchart"]["nested"][0]
                .get("securityLevel")
                .is_none()
        );
    }

    #[test]
    fn secure_filtered_overrides_keeps_the_merged_secure_policy() {
        let mut site_config = crate::generated::default_site_config();
        site_config.deep_merge(&json!({
            "secure": ["fontSize"]
        }));
        let overrides = MermaidConfig::from_value(json!({
            "secure": ["theme"],
            "fontSize": 99,
            "securityLevel": "loose",
            "theme": "dark"
        }));

        let filtered = site_config.secure_filtered_overrides(&overrides);

        assert!(filtered.as_value().get("secure").is_none());
        assert!(filtered.as_value().get("fontSize").is_none());
        assert!(filtered.get_str("securityLevel").is_none());
        assert_eq!(filtered.get_str("theme"), Some("dark"));
    }
}
