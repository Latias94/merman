//! Options schema admission and reusable request overlays.

use serde::Deserialize;
use serde_json::{Map, Value};
use std::sync::Arc;

use crate::artifact_contract::{DEFAULT_ARTIFACT_SNAPSHOT, ValidatedArtifactContract};
use crate::option_contract::BindingOptionGroupKey;
use crate::resource_contract::BindingResourceScope;

use super::options_value::{OptionsValue, OptionsValueRef};
use super::options_wire::{BindingAnalysisOptionsJson, BindingOptions, ResourceOptionsJson};
use super::resources::{
    options_json_value, parse_request_resource_options, tighten_resource_options,
    validate_artifact_resource_options,
};
use super::{
    BINDING_OPTIONS_SCHEMA_VERSION, BindingError, BindingRequestOptions, BindingStatus,
    internal_json_error,
};

const BINDING_ANALYSIS_OPTION_KEYS: [&str; 6] = [
    "fixed_today",
    "fixed_unix_millis",
    "fixed_local_offset_minutes",
    "site_config",
    "resources",
    "lint",
];

#[derive(Debug, Clone)]
pub(crate) struct BaseBindingOptions {
    normalized_wire: Arc<OptionsValue>,
    resource_ceiling: ResourceOptionsJson,
    #[cfg(feature = "svg")]
    has_analysis_wrapper: bool,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug)]
pub(crate) enum BindingThemeOverlay {
    Inherit,
    Replace(Option<merman::theme::ThemeSelection>),
}

#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "The overlay keeps the validated request payload and resource policy together."
)]
pub(crate) enum BindingRequestOverlay {
    Unchanged,
    Override {
        normalized_wire: OptionsValue,
        requested_resources: Option<ResourceOptionsJson>,
        #[cfg(any(feature = "svg", feature = "ascii"))]
        theme: BindingThemeOverlay,
    },
}

#[cfg(feature = "svg")]
pub(crate) struct LightSvgOverride {
    pub(crate) diagram_id: Option<String>,
}

#[cfg(test)]
pub(crate) fn parse_options(bytes: &[u8]) -> Result<BindingOptions, BindingError> {
    let wire = options_json_value(bytes)?;
    parse_options_with_theme(bytes, wire.view(), &DEFAULT_ARTIFACT_SNAPSHOT)
}

#[cfg(test)]
fn parse_base_options(bytes: &[u8]) -> Result<(BindingOptions, BaseBindingOptions), BindingError> {
    parse_base_options_for_contract(bytes, &DEFAULT_ARTIFACT_SNAPSHOT)
}

pub(crate) fn parse_base_options_for_artifact(
    bytes: &[u8],
    artifact_contract: &ValidatedArtifactContract,
) -> Result<(BindingOptions, BaseBindingOptions), BindingError> {
    parse_base_options_for_contract(bytes, artifact_contract)
}

fn parse_base_options_for_contract(
    bytes: &[u8],
    artifact_contract: &ValidatedArtifactContract,
) -> Result<(BindingOptions, BaseBindingOptions), BindingError> {
    let wire = options_json_value(bytes)?;
    let typed = parse_options_with_theme(bytes, wire.view(), artifact_contract)?;
    let base = base_options_from_wire(wire, &typed);
    Ok((typed, base))
}

pub(super) fn base_options_from_wire(
    wire: OptionsValue,
    typed: &BindingOptions,
) -> BaseBindingOptions {
    let resource_ceiling = typed.analysis.resources.clone().unwrap_or_default();
    // Constructor decoding and overlay merging can see different fields from a wrapper.
    #[cfg(feature = "svg")]
    let has_analysis_wrapper =
        wire.value.get("analysis").is_some() || wire.value.get("merman").is_some();
    let normalized_wire = normalize_analysis_wrapper(wire);
    #[cfg(any(feature = "svg", feature = "ascii"))]
    let normalized_wire = {
        let mut normalized_wire = normalized_wire;
        normalized_wire
            .object_mut()
            .expect("validated binding options always normalize to an object")
            .swap_remove("theme");
        normalized_wire
    };
    BaseBindingOptions {
        normalized_wire: Arc::new(normalized_wire),
        resource_ceiling,
        #[cfg(feature = "svg")]
        has_analysis_wrapper,
    }
}

pub(super) fn parse_options_with_theme(
    bytes: &[u8],
    wire: OptionsValueRef<'_>,
    artifact: &ValidatedArtifactContract,
) -> Result<BindingOptions, BindingError> {
    let options = parse_options_value_for_contract(wire, artifact)?;
    #[cfg(any(feature = "svg", feature = "ascii"))]
    let options = BindingOptions {
        theme: crate::native_theme::binding_theme(bytes, wire.value)?,
        ..options
    };
    #[cfg(not(any(feature = "svg", feature = "ascii")))]
    let _ = bytes;
    Ok(options)
}

pub(super) fn parse_options_value(value: &Value) -> Result<BindingOptions, BindingError> {
    parse_options_value_for_contract(value.into(), &DEFAULT_ARTIFACT_SNAPSHOT)
}

fn parse_options_value_for_contract(
    wire: OptionsValueRef<'_>,
    artifact_contract: &ValidatedArtifactContract,
) -> Result<BindingOptions, BindingError> {
    let value = wire.value;
    validate_options_schema_version(value)?;
    reject_ambiguous_analysis_wrappers(value)?;
    reject_removed_host_theme(value)?;
    reject_removed_presentation(value)?;
    reject_unavailable_option_groups(value, artifact_contract)?;
    #[cfg(feature = "svg")]
    reject_removed_layout_fields(value)?;
    reject_untrusted_binding_css_options(wire)?;
    #[cfg(feature = "ascii")]
    reject_removed_ascii_resource_field(value)?;
    reject_removed_nested_analysis_parse_option(value)?;
    let options = if value.is_object() {
        let entries = wire.entries().filter(|(key, _)| {
            !matches!(*key, "analysis" | "merman" | "theme")
                && !BINDING_ANALYSIS_OPTION_KEYS.contains(key)
        });
        BindingOptions::deserialize(serde::de::value::MapDeserializer::new(entries))
    } else {
        BindingOptions::deserialize(wire)
    };
    let mut options = options.map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid options_json: {err}"),
        )
    })?;
    options.version = Some(BINDING_OPTIONS_SCHEMA_VERSION);
    #[cfg(feature = "svg")]
    {
        options.text_measurement_selector_explicit = value
            .as_object()
            .and_then(|root| root.get("environment"))
            .and_then(Value::as_object)
            .is_some_and(|environment| environment.contains_key("text_measurement"));
    }
    reject_unknown_options_json_fields(wire)?;
    options.analysis = binding_analysis_options_json_from_json_value(wire)?;
    validate_artifact_resource_options(options.analysis.resources.as_ref(), artifact_contract)?;
    Ok(options)
}

fn reject_removed_host_theme(value: &Value) -> Result<(), BindingError> {
    if value
        .as_object()
        .is_some_and(|options| options.contains_key("host_theme"))
    {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options group `host_theme` was removed; use top-level `theme` for compiled diagram themes, `site_config` for Mermaid behavior overrides, and `svg` for output policy",
        ));
    }
    Ok(())
}

fn reject_removed_presentation(value: &Value) -> Result<(), BindingError> {
    if value
        .as_object()
        .is_some_and(|options| options.contains_key("presentation"))
    {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options group `presentation` was removed; use top-level `theme`, `site_config`, layout configuration, and `svg` as independent owners",
        ));
    }
    Ok(())
}

fn reject_untrusted_binding_css_options(wire: OptionsValueRef<'_>) -> Result<(), BindingError> {
    let (options_value, _) = binding_analysis_options_root_value(wire)?;
    if let Some(site_config) = options_value
        .get("site_config")
        .filter(|site_config| site_config.value.is_object())
    {
        if site_config.get("themeCSS").is_some() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                "site_config.themeCSS is not accepted by general bindings; use the typed `theme` schema instead, or let a trusted Rust or native CLI host own raw CSS explicitly",
            ));
        }
        if site_config.get("secure").is_some() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                "site_config.secure is not accepted by general bindings because callers cannot replace the host security boundary; configure it only through a trusted Rust or native CLI host",
            ));
        }
        for field in ["maxEdges", "max_edges"] {
            if site_config.get(field).is_some() {
                return Err(BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!(
                        "site_config.{field} is not accepted by general bindings because it can raise the parser edge budget beyond the host resource ceiling"
                    ),
                ));
            }
        }
        for field in [
            "securityLevel",
            "security_level",
            "dompurifyConfig",
            "dompurify_config",
        ] {
            if site_config.get(field).is_some() {
                return Err(BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!(
                        "site_config.{field} is not accepted by general bindings because it can replace the host SVG security policy"
                    ),
                ));
            }
        }
        for field in ["themeVariables", "theme_variables"] {
            if let Some(theme_variables) = site_config.get(field) {
                let path = format!("site_config.{field}");
                reject_untrusted_theme_variable_values(theme_variables, &path)?;
            }
        }
    }

    #[cfg(feature = "svg")]
    let Some(svg) = wire
        .value
        .as_object()
        .and_then(|options| options.get("svg"))
        .and_then(Value::as_object)
    else {
        return Ok(());
    };

    #[cfg(feature = "svg")]
    for field in [
        "scoped_css",
        "scopedCss",
        "css_override_policy",
        "cssOverridePolicy",
    ] {
        if svg.contains_key(field) {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!(
                    "svg.{field} is not accepted by general bindings; host-owned CSS controls are restricted to a trusted Rust or native CLI host, so use the typed `theme` schema or `svg.root_background_color` instead"
                ),
            ));
        }
    }

    Ok(())
}

fn reject_untrusted_theme_variable_values(
    wire: OptionsValueRef<'_>,
    path: &str,
) -> Result<(), BindingError> {
    match wire.value {
        Value::String(value) if !is_safe_binding_css_value(value) => Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            format!(
                "{path} contains a CSS value that is not accepted by general bindings; use a typed theme value or a trusted Rust or native CLI host"
            ),
        )),
        Value::Array(values) => wire
            .items(values)
            .enumerate()
            .try_for_each(|(index, value)| {
                reject_untrusted_theme_variable_values(value, &format!("{path}[{index}]"))
            }),
        Value::Object(_) => wire.entries().try_for_each(|(key, value)| {
            reject_untrusted_theme_variable_values(value, &format!("{path}.{key}"))
        }),
        _ => Ok(()),
    }
}

fn is_safe_binding_css_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !value.bytes().any(|byte| {
        byte.is_ascii_control() || matches!(byte, b'<' | b'>' | b'{' | b'}' | b';' | b'\\')
    }) && !value.contains("/*")
        && !value.contains("*/")
        && !lower.contains("url(")
        && !lower.contains("expression(")
        && !lower.contains("@import")
        && !lower.contains("<!--")
        && !lower.contains("-->")
}

fn validate_options_schema_version(value: &Value) -> Result<(), BindingError> {
    let Some(raw_version) = value.get("version") else {
        return Ok(());
    };
    let Some(version) = raw_version
        .as_u64()
        .and_then(|version| u32::try_from(version).ok())
    else {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options_json schema version must be an unsigned 32-bit integer",
        ));
    };
    if version != BINDING_OPTIONS_SCHEMA_VERSION {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            format!(
                "unsupported options_json schema version {version}; expected {BINDING_OPTIONS_SCHEMA_VERSION}"
            ),
        ));
    }
    Ok(())
}

fn reject_ambiguous_analysis_wrappers(value: &Value) -> Result<(), BindingError> {
    let Some(root) = value.as_object() else {
        return Ok(());
    };
    if root.contains_key("analysis") && root.contains_key("merman") {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options JSON must not contain both `analysis` and `merman` wrappers",
        ));
    }
    Ok(())
}

fn reject_unknown_options_json_fields(wire: OptionsValueRef<'_>) -> Result<(), BindingError> {
    for (key, nested) in wire.entries() {
        if matches!(key, "analysis" | "merman") {
            if !nested.value.is_object() {
                return Err(BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!("options JSON `{key}` wrapper must be an object"),
                ));
            }
            // The wrapped analysis root is forward-compatible. Its owner validates known fields
            // and ignores future fields; the binding layer must not maintain a second key table.
            continue;
        }

        if BindingOptionGroupKey::from_id(key).is_none() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!(
                    "unknown options_json field `{key}` for schema {BINDING_OPTIONS_SCHEMA_VERSION}"
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn parse_request_overlay(
    request_options_json: &[u8],
    resource_scope: BindingResourceScope,
) -> Result<BindingRequestOverlay, BindingError> {
    parse_request_overlay_for_artifact(
        BindingRequestOptions::Raw(request_options_json),
        resource_scope,
        &DEFAULT_ARTIFACT_SNAPSHOT,
    )
}

pub(crate) fn parse_request_overlay_for_artifact(
    request_options: BindingRequestOptions<'_>,
    resource_scope: BindingResourceScope,
    artifact_contract: &ValidatedArtifactContract,
) -> Result<BindingRequestOverlay, BindingError> {
    let (request_options_json, request_value) = match request_options {
        BindingRequestOptions::Raw(json) => (json, options_json_value(json)?),
        BindingRequestOptions::Decoded(options) => (options.json.as_bytes(), options.value),
    };
    if is_unchanged_request(&request_value.value) {
        return Ok(BindingRequestOverlay::Unchanged);
    }

    validate_output_options_for_scope(&request_value.value, resource_scope)?;
    if request_selects_runtime_policy(&request_value.value) {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "request options_json cannot set runtime_policy; configure it when creating the engine",
        ));
    }
    let options = parse_options_with_theme(
        request_options_json,
        request_value.view(),
        artifact_contract,
    )?;
    let mut normalized_wire = normalize_analysis_wrapper(request_value);
    #[cfg(any(feature = "svg", feature = "ascii"))]
    let theme = if normalized_wire
        .object_mut()
        .expect("validated options normalize to an object")
        .swap_remove("theme")
        .is_some()
    {
        BindingThemeOverlay::Replace(options.theme)
    } else {
        BindingThemeOverlay::Inherit
    };
    #[cfg(not(any(feature = "svg", feature = "ascii")))]
    let _ = options;
    let resources = normalized_wire
        .object_mut()
        .expect("validated options normalize to an object")
        .swap_remove("resources");
    let requested_resources =
        parse_request_resource_options(resources.as_ref().map(OptionsValue::view), resource_scope)?;
    Ok(BindingRequestOverlay::Override {
        normalized_wire,
        requested_resources,
        #[cfg(any(feature = "svg", feature = "ascii"))]
        theme,
    })
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl BindingRequestOverlay {
    #[cfg(feature = "svg")]
    pub(crate) fn take_light_svg_override(&mut self) -> Option<LightSvgOverride> {
        let Self::Override {
            normalized_wire,
            requested_resources: None,
            theme: BindingThemeOverlay::Inherit,
        } = self
        else {
            return None;
        };
        let options = normalized_wire.value.as_object_mut()?;
        if options.len() != 1 + usize::from(options.contains_key("version")) {
            return None;
        }
        let svg = options.get_mut("svg")?.as_object_mut()?;
        if svg.len() != 1 {
            return None;
        }
        // The typed request was validated before constructing the overlay.
        let diagram_id = match svg.get_mut("diagram_id")?.take() {
            Value::String(id) => Some(id),
            Value::Null => None,
            _ => unreachable!("validated svg.diagram_id is a string or null"),
        };
        Some(LightSvgOverride { diagram_id })
    }

    pub(crate) fn select_theme(
        &self,
        base_theme: Option<&merman::theme::ThemeSelection>,
    ) -> Option<merman::theme::ThemeSelection> {
        match self {
            Self::Override {
                theme: BindingThemeOverlay::Replace(theme),
                ..
            } => theme.clone(),
            _ => base_theme.cloned(),
        }
    }

    #[cfg(feature = "svg")]
    pub(crate) fn validate_prepared_output(&self) -> Result<(), BindingError> {
        if let Self::Override {
            normalized_wire, ..
        } = self
        {
            let disallowed = normalized_wire.view().entries().find(|(key, _)| {
                !matches!(
                    *key,
                    "version" | "background" | "environment" | "layout" | "svg"
                )
            });
            if let Some((key, _)) = disallowed {
                return Err(BindingError::invalid_options_json(format!(
                    "prepared output cannot set `{key}`. Prepare a new source to change parse configuration"
                )));
            }
        }
        Ok(())
    }
}

impl BaseBindingOptions {
    #[cfg(feature = "svg")]
    pub(crate) const fn has_analysis_wrapper(&self) -> bool {
        self.has_analysis_wrapper
    }

    #[cfg(feature = "svg")]
    pub(crate) fn prepared_output_defaults(
        &self,
        artifact: &ValidatedArtifactContract,
    ) -> Result<(BindingOptions, Self), BindingError> {
        let mut normalized = self.normalized_wire.as_ref().clone();
        normalized
            .object_mut()
            .expect("base options are normalized objects")
            .retain(|key| {
                matches!(
                    key,
                    "version" | "resources" | "background" | "environment" | "layout" | "svg"
                )
            });
        let options = parse_options_value_for_contract(normalized.view(), artifact)?;
        Ok((
            options,
            Self {
                normalized_wire: Arc::new(normalized),
                resource_ceiling: self.resource_ceiling.clone(),
                has_analysis_wrapper: self.has_analysis_wrapper,
            },
        ))
    }

    pub(crate) fn validate_unchanged_request(
        &self,
        artifact_contract: &ValidatedArtifactContract,
    ) -> Result<(), BindingError> {
        parse_options_value_for_contract(self.normalized_wire.view(), artifact_contract).map(drop)
    }

    pub(crate) fn apply_overlay(
        &self,
        overlay: BindingRequestOverlay,
        artifact_contract: &ValidatedArtifactContract,
    ) -> Result<BindingOptions, BindingError> {
        let BindingRequestOverlay::Override {
            normalized_wire,
            requested_resources,
            ..
        } = overlay
        else {
            unreachable!("unchanged request overlays borrow the base engine");
        };

        let mut merged = self.normalized_wire.as_ref().clone();
        merged.merge(normalized_wire);
        if let Some(requested_resources) = requested_resources {
            let resources = tighten_resource_options(&self.resource_ceiling, &requested_resources)?;
            merged
                .object_mut()
                .expect("validated binding options always normalize to an object")
                .insert(
                    "resources".to_string(),
                    serde_json::to_value(resources)
                        .map_err(internal_json_error)?
                        .into(),
                );
        }
        parse_options_value_for_contract(merged.view(), artifact_contract)
    }
}

fn is_unchanged_request(value: &Value) -> bool {
    let Some(options) = value.as_object() else {
        return false;
    };
    options.is_empty()
        || (options.len() == 1
            && options
                .get("version")
                .and_then(Value::as_u64)
                .is_some_and(|version| version == u64::from(BINDING_OPTIONS_SCHEMA_VERSION)))
}

pub(super) fn reject_unavailable_option_groups(
    value: &Value,
    artifact_contract: &ValidatedArtifactContract,
) -> Result<(), BindingError> {
    let Some(options) = value.as_object() else {
        return Ok(());
    };
    for key in BindingOptionGroupKey::ALL {
        let group = key.id();
        let present = options.contains_key(group)
            || (group == "lint"
                && ["analysis", "merman"].iter().any(|wrapper| {
                    options
                        .get(*wrapper)
                        .and_then(Value::as_object)
                        .is_some_and(|nested| nested.contains_key(group))
                }));
        let available = artifact_contract.exposes_option_group(*key);
        if present && !available {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!(
                    "options group `{group}` is not available because it is not exposed by target `{}`",
                    artifact_contract.target().id()
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_output_options_for_scope(
    value: &Value,
    scope: BindingResourceScope,
) -> Result<(), BindingError> {
    let Some(options) = value.as_object() else {
        return Ok(());
    };
    for group in ["export_matte", "raster", "jpeg", "pdf"] {
        let accepted = match group {
            "export_matte" => matches!(
                scope,
                BindingResourceScope::Png | BindingResourceScope::Jpeg | BindingResourceScope::Pdf
            ),
            "raster" => matches!(
                scope,
                BindingResourceScope::Png | BindingResourceScope::Jpeg
            ),
            "jpeg" => matches!(scope, BindingResourceScope::Jpeg),
            "pdf" => matches!(scope, BindingResourceScope::Pdf),
            _ => unreachable!("output option groups are closed"),
        };
        if options.contains_key(group) && !accepted {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("request options group `{group}` does not apply to this operation"),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_one_shot_resource_options(
    options_json: &[u8],
    resource_scope: BindingResourceScope,
) -> Result<(), BindingError> {
    let wire = options_json_value(options_json)?;
    validate_output_options_for_scope(&wire.value, resource_scope)?;
    if !wire.value.is_object() {
        return Err(BindingError::invalid_options_json(
            "invalid request options_json: options root must be an object",
        ));
    }
    // Preflight observes the same resource field as wrapper normalization. Full schema and
    // wrapper validation still follows operation-scoped resource validation.
    let root = wire.view();
    let resources = normalization_wrapper(&wire.value)
        .and_then(|wrapper| root.get(wrapper)?.get("resources"))
        .or_else(|| root.get("resources"));
    parse_request_resource_options(resources, resource_scope).map(drop)
}

fn request_selects_runtime_policy(value: &Value) -> bool {
    value.as_object().is_some_and(|options| {
        options.contains_key("runtime_policy")
            || ["analysis", "merman"].iter().any(|wrapper| {
                options
                    .get(*wrapper)
                    .and_then(Value::as_object)
                    .is_some_and(|wrapped| wrapped.contains_key("runtime_policy"))
            })
    })
}

fn normalization_wrapper(value: &Value) -> Option<&'static str> {
    ["merman", "analysis"]
        .into_iter()
        .find(|wrapper| value.get(*wrapper).is_some_and(Value::is_object))
}

fn normalize_analysis_wrapper(mut value: OptionsValue) -> OptionsValue {
    let Some(wrapper) = normalization_wrapper(&value.value) else {
        return value;
    };
    let mut options = value
        .object_mut()
        .expect("options with an analysis wrapper");
    let mut analysis = options.swap_remove(wrapper).expect("selected wrapper");
    let mut analysis = analysis
        .object_mut()
        .expect("selected wrapper is an object");
    for key in BINDING_ANALYSIS_OPTION_KEYS {
        if let Some(value) = analysis.swap_remove(key) {
            options.insert(key.to_string(), value);
        }
    }
    value
}

#[cfg(feature = "svg")]
fn reject_removed_layout_fields(value: &Value) -> Result<(), BindingError> {
    let Some(layout) = value.get("layout").and_then(Value::as_object) else {
        return Ok(());
    };
    for (legacy, replacement) in [
        ("text_measurer", "environment.text_measurement"),
        ("math_renderer", "environment.math_renderer"),
        ("viewport_width", "layout.container_width"),
        ("viewport_height", "layout.container_height"),
    ] {
        if layout.contains_key(legacy) {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("layout.{legacy} was removed; use {replacement}"),
            ));
        }
    }
    Ok(())
}

#[cfg(feature = "ascii")]
fn reject_removed_ascii_resource_field(value: &Value) -> Result<(), BindingError> {
    let Some(ascii) = value.get("ascii").and_then(Value::as_object) else {
        return Ok(());
    };
    if ascii.contains_key("max_grid_cells") || ascii.contains_key("maxGridCells") {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "ascii.max_grid_cells was removed; use resources.limits.max_ascii_grid_cells",
        ));
    }
    Ok(())
}

#[cfg(feature = "analysis")]
fn binding_analysis_options_json_from_json_value(
    wire: OptionsValueRef<'_>,
) -> Result<BindingAnalysisOptionsJson, BindingError> {
    reject_removed_nested_analysis_parse_option(wire.value)?;
    let (options_value, wrapped) = binding_analysis_options_root_value(wire)?;
    let resources = options_value
        .get("resources")
        .filter(|value| !value.value.is_null())
        .map(|value| {
            ResourceOptionsJson::deserialize(value).map_err(|err| {
                BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!("invalid analysis options JSON: {err}"),
                )
            })
        })
        .transpose()?;

    let mut analysis_value = options_value.to_ordered();
    let mut analysis = analysis_value.object_mut().ok_or_else(|| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            "invalid analysis options JSON: analysis options JSON must be an object",
        )
    })?;
    if !wrapped {
        analysis.retain(|key| BINDING_ANALYSIS_OPTION_KEYS.contains(&key));
    }
    let analysis_contract = merman_analysis::AnalysisConfigContract::current();
    if let Some(mut resource_options) = analysis.child_object("resources") {
        resource_options.swap_remove("profile");
        if let Some(mut limits) = resource_options.child_object("limits") {
            limits.retain(|limit_id| analysis_contract.accepts_resource_limit(limit_id));
        }
    }
    let decoded = analysis_contract
        .decode_ordered_json(analysis_value.view())
        .map_err(BindingError::from)?;
    Ok(BindingAnalysisOptionsJson {
        fixed_today: decoded.fixed_today,
        fixed_unix_millis: decoded.fixed_unix_millis,
        fixed_local_offset_minutes: decoded.fixed_local_offset_minutes,
        site_config: decoded.site_config,
        resources,
        lint: decoded.lint,
    })
}

#[cfg(not(feature = "analysis"))]
fn binding_analysis_options_json_from_json_value(
    wire: OptionsValueRef<'_>,
) -> Result<BindingAnalysisOptionsJson, BindingError> {
    reject_removed_nested_analysis_parse_option(wire.value)?;
    let (options_value, _) = binding_analysis_options_root_value(wire)?;
    BindingAnalysisOptionsJson::deserialize(options_value).map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid analysis options JSON: {err}"),
        )
    })
}

fn binding_analysis_options_root_value(
    wire: OptionsValueRef<'_>,
) -> Result<(OptionsValueRef<'_>, bool), BindingError> {
    let Value::Object(map) = wire.value else {
        return Ok((wire, false));
    };

    let wrapper = ["merman", "analysis"]
        .into_iter()
        .find(|key| map.contains_key(*key));
    if binding_analysis_option_keys_present(map) {
        if wrapper.is_some() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                "options JSON must not mix top-level analysis options with `analysis` or `merman` wrappers",
            ));
        }
        return Ok((wire, false));
    }

    if let Some(key) = wrapper {
        let wrapped = wire
            .get(key)
            .expect("selected wrapper key must exist in the options object");
        if !wrapped.value.is_object() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("options JSON `{key}` wrapper must be an object"),
            ));
        }
        return Ok((wrapped, true));
    }

    Ok((wire, false))
}

fn binding_analysis_option_keys_present(map: &Map<String, Value>) -> bool {
    BINDING_ANALYSIS_OPTION_KEYS
        .iter()
        .any(|key| map.contains_key(*key))
}

fn reject_removed_nested_analysis_parse_option(value: &Value) -> Result<(), BindingError> {
    let Value::Object(map) = value else {
        return Ok(());
    };

    if ["merman", "analysis"].iter().any(|key| {
        map.get(*key)
            .and_then(Value::as_object)
            .is_some_and(|options| options.contains_key("parse"))
    }) {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "analysis option `parse` was removed; use top-level `parse` only for parse, render, or ASCII operations",
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn resolve_request_options(
    base: &[u8],
    request: &[u8],
    scope: BindingResourceScope,
) -> Result<BindingOptions, BindingError> {
    let (_, base) = parse_base_options(base)?;
    Ok(match parse_request_overlay(request, scope)? {
        BindingRequestOverlay::Unchanged => {
            base.validate_unchanged_request(&DEFAULT_ARTIFACT_SNAPSHOT)?;
            parse_options_value_for_contract(
                base.normalized_wire.view(),
                &DEFAULT_ARTIFACT_SNAPSHOT,
            )?
        }
        overlay => base.apply_overlay(overlay, &DEFAULT_ARTIFACT_SNAPSHOT)?,
    })
}
