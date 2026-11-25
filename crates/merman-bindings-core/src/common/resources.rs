//! Binding resource policies, transport ceilings, and options byte admission.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

use crate::artifact_contract::{DEFAULT_ARTIFACT_SNAPSHOT, ValidatedArtifactContract};
use crate::resource_contract::{
    BINDING_OPTIONS_JSON_LIMIT_ID, BINDING_OPTIONS_JSON_MAX_BYTES, BindingResourceScope,
    binding_resource_contract, resource_limit_descriptor, resource_profile_value,
};

#[cfg(feature = "analysis")]
use super::artifact_analysis_options;
use super::{
    BINDING_OPTIONS_SCHEMA_VERSION, BaseBindingOptions, BindingError, BindingOptions,
    BindingStatus, ResourceOptionsJson, base_options_from_wire, internal_json_error, options_value,
    options_value::OptionsValue, parse_options_value, parse_options_with_theme,
    reject_unavailable_option_groups, validate_artifact_resource_options,
    validate_compiled_resource_options, validate_output_options_for_scope,
    validate_resource_limits_for_scope,
};

pub(crate) fn binding_input_resource_policy(
    resources: Option<&ResourceOptionsJson>,
) -> Result<merman::model::resources::InputResourcePolicy, BindingError> {
    let profile = binding_resource_profile(resources)?;
    let mut policy = merman::model::resources::InputResourcePolicy::for_profile(profile);
    if let Some(resources) = resources {
        for (id, value) in &resources.limits {
            if let Some(input_id) =
                merman::model::resources::InputResourceLimitId::from_stable_id(id)
            {
                policy.apply_limit(input_id, *value).map_err(|error| {
                    BindingError::new(BindingStatus::InvalidArgument, error.to_string())
                })?;
                continue;
            }
            if resource_limit_descriptor(id).is_some() {
                continue;
            }
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("resource limit id `{id}` is not available for this artifact"),
            ));
        }
    }
    Ok(policy)
}

#[cfg(feature = "ascii")]
pub(crate) fn binding_ascii_resource_policy(
    resources: Option<&ResourceOptionsJson>,
) -> Result<merman::ascii::AsciiResourcePolicy, BindingError> {
    let profile = binding_resource_profile(resources)?;
    let mut policy = merman::ascii::AsciiResourcePolicy::for_profile(profile);
    if let Some(resources) = resources {
        for (id, value) in &resources.limits {
            let Some(ascii_id) = merman::ascii::AsciiResourceLimitId::from_stable_id(id) else {
                continue;
            };
            policy.apply_limit(ascii_id, *value).map_err(|error| {
                BindingError::new(BindingStatus::InvalidArgument, error.to_string())
            })?;
        }
    }
    Ok(policy)
}

#[cfg(feature = "svg")]
pub(crate) fn binding_resource_policy(
    resources: Option<&ResourceOptionsJson>,
) -> Result<merman::svg::RenderResourcePolicy, BindingError> {
    let profile = binding_resource_profile(resources)?;
    let mut limits = merman::svg::RenderResourcePolicy::for_profile(profile);
    if let Some(resources) = resources {
        for (id, value) in &resources.limits {
            if resource_limit_descriptor(id).is_some()
                && merman::svg::ResourceLimitId::from_stable_id(id).is_none()
            {
                continue;
            }
            limits.apply_override(id, *value).map_err(|error| {
                BindingError::new(BindingStatus::InvalidArgument, error.to_string())
            })?;
        }
    }
    Ok(limits)
}

fn binding_resource_profile(
    resources: Option<&ResourceOptionsJson>,
) -> Result<merman::model::resources::ResourceProfile, BindingError> {
    Ok(resources
        .and_then(|resources| resources.profile.as_deref())
        .map(|id| {
            merman::model::resources::ResourceProfile::from_id(id).ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported resources.profile: {id}"),
                )
            })
        })
        .transpose()?
        .unwrap_or(merman::model::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE))
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub(crate) struct BindingExportResourceOptions {
    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub(crate) raster_size_limit: merman::svg::export::RasterSizeLimit,
    pub(crate) embedded_image_limit: merman::svg::export::EmbeddedImageLimit,
    #[cfg(feature = "pdf")]
    pub(crate) pdf_filter_image_limit: merman::svg::export::PdfFilterImageLimit,
    pub(crate) conversion_limits: merman::svg::export::SvgConversionLimits,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub(crate) fn binding_export_resource_options(
    resources: Option<&ResourceOptionsJson>,
) -> Result<BindingExportResourceOptions, BindingError> {
    let default_resources = ResourceOptionsJson::default();
    let values = effective_resource_limits(resources.unwrap_or(&default_resources))?;
    Ok(BindingExportResourceOptions {
        #[cfg(any(feature = "png", feature = "jpeg"))]
        raster_size_limit: merman::svg::export::RasterSizeLimit::new(
            export_resource_u32(
                &values,
                merman::svg::export::MAX_RASTER_WIDTH_RESOURCE_LIMIT_ID,
            )?,
            export_resource_u32(
                &values,
                merman::svg::export::MAX_RASTER_HEIGHT_RESOURCE_LIMIT_ID,
            )?,
            export_resource_u64(
                &values,
                merman::svg::export::MAX_RASTER_PIXELS_RESOURCE_LIMIT_ID,
            )?,
        ),
        embedded_image_limit: merman::svg::export::EmbeddedImageLimit::new(
            export_resource_u64(
                &values,
                merman::svg::export::MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
            )?,
            export_resource_u64(
                &values,
                merman::svg::export::MAX_TOTAL_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
            )?,
            export_resource_u64(
                &values,
                merman::svg::export::MAX_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
            )?,
            export_resource_u64(
                &values,
                merman::svg::export::MAX_TOTAL_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
            )?,
        ),
        #[cfg(feature = "pdf")]
        pdf_filter_image_limit: merman::svg::export::PdfFilterImageLimit::new(export_resource_u64(
            &values,
            merman::svg::export::MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
        )?),
        conversion_limits: merman::svg::export::SvgConversionLimits::default_safe(),
    })
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_resource_value(
    values: &BTreeMap<&'static str, Option<usize>>,
    stable_id: &'static str,
) -> Option<usize> {
    values
        .get(stable_id)
        .copied()
        .expect("compiled export resource descriptor must have a profile value")
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn export_resource_u32(
    values: &BTreeMap<&'static str, Option<usize>>,
    stable_id: &'static str,
) -> Result<Option<u32>, BindingError> {
    export_resource_value(values, stable_id)
        .map(|value| {
            u32::try_from(value).map_err(|_| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("resources.limits.{stable_id} exceeds the u32 export boundary"),
                )
            })
        })
        .transpose()
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_resource_u64(
    values: &BTreeMap<&'static str, Option<usize>>,
    stable_id: &'static str,
) -> Result<Option<u64>, BindingError> {
    export_resource_value(values, stable_id)
        .map(|value| {
            u64::try_from(value).map_err(|_| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("resources.limits.{stable_id} exceeds the u64 export boundary"),
                )
            })
        })
        .transpose()
}

pub fn resource_options_json(
    profile_id: Option<&str>,
    overrides: &[(&str, usize)],
) -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(&resource_options_value(profile_id, overrides)?).map_err(internal_json_error)
}

fn resource_options_value(
    profile_id: Option<&str>,
    overrides: &[(&str, usize)],
) -> Result<Value, BindingError> {
    let profile = profile_id
        .map(|profile_id| {
            merman::model::resources::ResourceProfile::from_id(profile_id).ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported resources.profile: {profile_id}"),
                )
            })
        })
        .transpose()?;
    let mut limits = BTreeMap::new();
    for &(id, value) in overrides {
        if limits.insert(id, value).is_some() {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("duplicate resource limit override: {id}"),
            ));
        }
        let Some(descriptor) = resource_limit_descriptor(id) else {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("resource limit id `{id}` is not available for this build"),
            ));
        };
        if !descriptor.overridable {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("resource limit id `{id}` is not overridable"),
            ));
        }
        if value < descriptor.minimum_value {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!(
                    "resource limit `{id}` must be at least {}",
                    descriptor.minimum_value
                ),
            ));
        }
    }

    let mut root = Map::new();
    root.insert(
        "version".to_string(),
        Value::from(BINDING_OPTIONS_SCHEMA_VERSION),
    );
    if profile.is_some() || !limits.is_empty() {
        let mut resources = Map::new();
        if let Some(profile) = profile {
            resources.insert("profile".to_string(), Value::from(profile.id()));
        }
        if !limits.is_empty() {
            resources.insert(
                "limits".to_string(),
                serde_json::to_value(limits).map_err(internal_json_error)?,
            );
        }
        root.insert("resources".to_string(), Value::Object(resources));
    }
    Ok(Value::Object(root))
}

/// Applies a transport-owned resource ceiling while preserving stricter caller limits.
///
/// The caller options may use direct analysis fields or exactly one `analysis`/`merman` wrapper.
/// A caller-selected profile is accepted only when its effective limits are no looser than the
/// transport ceiling. The returned JSON preserves an explicitly selected stricter profile so all
/// downstream policy families, including theme compilation, observe the same restriction.
pub fn apply_resource_ceiling_json(
    options_json: &[u8],
    ceiling_profile_id: &str,
    ceiling_overrides: &[(&str, usize)],
) -> Result<Vec<u8>, BindingError> {
    apply_resource_ceiling_json_with_allowed_overrides(
        options_json,
        ceiling_profile_id,
        ceiling_overrides,
        &[],
    )
}

/// Applies a transport ceiling with explicit caller overrides for host-approved limit IDs.
///
/// Only explicitly supplied, compiled, overridable limits may replace their ceiling values.
/// Omitted limits retain the host defaults, and all other limits and the selected profile must
/// remain no looser than the host ceiling. Raw-byte admission happens before JSON decoding.
pub fn apply_resource_ceiling_json_with_allowed_overrides(
    options_json: &[u8],
    ceiling_profile_id: &str,
    ceiling_overrides: &[(&str, usize)],
    allowed_request_limit_ids: &[&str],
) -> Result<Vec<u8>, BindingError> {
    let value = options_value_with_resource_ceiling(
        options_json,
        ceiling_profile_id,
        ceiling_overrides,
        allowed_request_limit_ids,
    )?;
    let serialized = serde_json::to_vec(&value.view()).map_err(internal_json_error)?;
    enforce_options_json_byte_budget(&serialized)?;
    parse_options_with_theme(options_json, value.view(), &DEFAULT_ARTIFACT_SNAPSHOT)?;
    Ok(serialized)
}

pub(crate) fn parse_one_shot_options_with_resource_ceiling(
    options_json: &[u8],
    ceiling_profile_id: &str,
    ceiling_overrides: &[(&str, usize)],
    allowed_request_limit_ids: &[&str],
    artifact_contract: &ValidatedArtifactContract,
    scope: BindingResourceScope,
    normalize: impl FnOnce(&mut Value) -> Result<(), BindingError>,
) -> Result<(BindingOptions, BaseBindingOptions), BindingError> {
    let mut wire = options_value_with_resource_ceiling(
        options_json,
        ceiling_profile_id,
        ceiling_overrides,
        allowed_request_limit_ids,
    )?;
    enforce_compact_options_budget(&wire.value)?;
    let mut options =
        parse_options_with_theme(options_json, wire.view(), &DEFAULT_ARTIFACT_SNAPSHOT)?;

    const TRANSPORT_FIELDS: [&str; 2] = ["runtime_policy", "environment"];
    let mut root = wire
        .object_mut()
        .expect("resource ceiling requires an object");
    let mut transport = Value::Object(
        TRANSPORT_FIELDS
            .iter()
            .filter_map(|key| {
                let member = root.swap_remove(key)?;
                Some(((*key).to_owned(), member.value))
            })
            .collect(),
    );
    normalize(&mut transport)?;
    let Value::Object(transport_root) = &transport else {
        return Err(BindingError::internal(
            "normalized transport options must be an object",
        ));
    };
    if transport_root
        .keys()
        .any(|key| !TRANSPORT_FIELDS.contains(&key.as_str()))
    {
        return Err(BindingError::internal(
            "transport normalization may only set runtime_policy and environment",
        ));
    }
    let transport_options = parse_options_value(&transport)?;
    options.runtime_policy = transport_options.runtime_policy;
    #[cfg(feature = "svg")]
    {
        options.environment = transport_options.environment;
        options.text_measurement_selector_explicit =
            transport_options.text_measurement_selector_explicit;
    }
    let Value::Object(transport_root) = transport else {
        unreachable!()
    };
    for (key, value) in transport_root {
        root.insert(key, value.into());
    }
    enforce_compact_options_budget(&wire.value)?;

    validate_output_options_for_scope(&wire.value, scope)?;
    if let Some(resources) = &options.analysis.resources {
        validate_resource_limits_for_scope(resources, scope)?;
    }
    reject_unavailable_option_groups(&wire.value, artifact_contract)?;
    validate_artifact_resource_options(options.analysis.resources.as_ref(), artifact_contract)?;
    let base = base_options_from_wire(wire, &options);
    Ok((options, base))
}

pub(crate) fn enforce_compact_options_budget(value: &impl Serialize) -> Result<(), BindingError> {
    struct JsonByteCounter(usize);

    impl std::io::Write for JsonByteCounter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(bytes.len());
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut counter = JsonByteCounter(0);
    serde_json::to_writer(&mut counter, value).map_err(internal_json_error)?;
    enforce_options_json_byte_len(counter.0)
}

/// Prepares typed analysis options under an explicit host resource ceiling.
///
/// The binding envelope and every compiled resource override are validated before projection.
/// Analysis consumes only its own limits and configuration; top-level parse and output options
/// retain their binding-owned validation without entering the analysis grammar. Byte admission
/// applies to the original input, since this entry point produces no normalized JSON document.
#[cfg(feature = "analysis")]
pub fn analysis_options_with_resource_ceiling(
    options_json: &[u8],
    ceiling_profile_id: &str,
    ceiling_overrides: &[(&str, usize)],
) -> Result<merman_analysis::AnalysisOptions, BindingError> {
    let value = options_value_with_resource_ceiling(
        options_json,
        ceiling_profile_id,
        ceiling_overrides,
        &[],
    )?;
    artifact_analysis_options(&parse_options_with_theme(
        options_json,
        value.view(),
        &DEFAULT_ARTIFACT_SNAPSHOT,
    )?)
}

fn options_value_with_resource_ceiling(
    options_json: &[u8],
    ceiling_profile_id: &str,
    ceiling_overrides: &[(&str, usize)],
    allowed_request_limit_ids: &[&str],
) -> Result<OptionsValue, BindingError> {
    let ceiling_value = resource_options_value(Some(ceiling_profile_id), ceiling_overrides)?;
    let mut ceiling = parse_options_value(&ceiling_value)?
        .analysis
        .resources
        .ok_or_else(|| {
            BindingError::new(
                BindingStatus::InternalError,
                "generated resource ceiling omitted resources",
            )
        })?;
    let mut value = options_json_value(options_json)?;
    let root = value.value.as_object().ok_or_else(|| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            "invalid options_json: options root must be an object",
        )
    })?;

    let wrappers = ["analysis", "merman"]
        .into_iter()
        .filter(|key| root.contains_key(*key))
        .collect::<Vec<_>>();
    for key in &wrappers {
        if !root.get(*key).is_some_and(Value::is_object) {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("invalid options_json: `{key}` wrapper must be an object"),
            ));
        }
    }
    if wrappers.len() > 1 {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options JSON must not contain both `analysis` and `merman` wrappers",
        ));
    }
    if !wrappers.is_empty() && root.contains_key("resources") {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options JSON must not mix top-level resources with an `analysis` or `merman` wrapper",
        ));
    }

    let root = value.view();
    let target = match wrappers.first() {
        Some(key) => root.get(key).expect("wrapper shape was validated"),
        None => root,
    };
    let requested = target
        .get("resources")
        .map(|resources| {
            ResourceOptionsJson::deserialize(resources).map_err(|error| {
                BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!("invalid options_json resources: {error}"),
                )
            })
        })
        .transpose()?;
    if !allowed_request_limit_ids.is_empty()
        && let Some(requested) = &requested
    {
        validate_compiled_resource_options(Some(requested))?;
        for (id, value) in &requested.limits {
            if allowed_request_limit_ids.contains(&id.as_str()) {
                ceiling.limits.insert(id.clone(), *value);
            }
        }
    }
    let resources = requested
        .as_ref()
        .map(|requested| tighten_resource_options(&ceiling, requested))
        .transpose()?
        .unwrap_or(ceiling);
    let mut root = value.object_mut().expect("options root is an object");
    let mut target = match wrappers.first() {
        Some(key) => root.child_object(key).expect("wrapper shape was validated"),
        None => root,
    };
    target.insert(
        "resources".to_string(),
        serde_json::to_value(resources)
            .map_err(internal_json_error)?
            .into(),
    );

    Ok(value)
}

pub(super) fn options_json_value(options_json: &[u8]) -> Result<OptionsValue, BindingError> {
    enforce_options_json_byte_budget(options_json)?;
    if options_json.is_empty() {
        return Ok(Value::Object(Map::new()).into());
    }
    let text = std::str::from_utf8(options_json).map_err(|error| {
        BindingError::new(
            BindingStatus::Utf8Error,
            format!("invalid options_json UTF-8: {error}"),
        )
    })?;
    options_value::decode_options(text).map_err(|error| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid options_json: {error}"),
        )
    })
}

#[doc(hidden)]
pub fn enforce_options_json_byte_budget(options_json: &[u8]) -> Result<(), BindingError> {
    enforce_options_json_byte_len(options_json.len())
}

#[doc(hidden)]
pub fn enforce_options_json_byte_len(options_json_bytes: usize) -> Result<(), BindingError> {
    if options_json_bytes > BINDING_OPTIONS_JSON_MAX_BYTES {
        return Err(BindingError::resource_limit(
            "options-json-preflight",
            BINDING_OPTIONS_JSON_LIMIT_ID,
            u64::try_from(options_json_bytes).unwrap_or(u64::MAX),
            u64::try_from(BINDING_OPTIONS_JSON_MAX_BYTES).unwrap_or(u64::MAX),
            "binding-json",
            format!(
                "options_json exceeds the preflight byte limit of {} bytes",
                BINDING_OPTIONS_JSON_MAX_BYTES
            ),
        ));
    }
    Ok(())
}

pub(super) fn tighten_resource_options(
    ceiling: &ResourceOptionsJson,
    requested: &ResourceOptionsJson,
) -> Result<ResourceOptionsJson, BindingError> {
    let ceiling_profile = binding_resource_profile(Some(ceiling))?;
    let requested_profile = binding_resource_profile(Some(requested))?;
    if requested.profile.is_some() && !requested_profile.is_no_looser_than(ceiling_profile) {
        return Err(resource_profile_ceiling_error(
            requested_profile,
            ceiling_profile,
        ));
    }

    let mut candidate = if requested.profile.is_none() {
        ceiling.clone()
    } else {
        ResourceOptionsJson {
            profile: requested.profile.clone(),
            limits: BTreeMap::new(),
        }
    };
    candidate.limits.extend(requested.limits.clone());

    let ceiling_values = effective_resource_limits(ceiling)?;
    let candidate_values = effective_resource_limits(&candidate)?;
    for (id, ceiling_value) in ceiling_values {
        let candidate_value = candidate_values
            .get(id)
            .copied()
            .expect("resource policy projections use the same stable IDs");
        match (ceiling_value, candidate_value) {
            (Some(maximum), Some(requested)) if requested <= maximum => {}
            (None, Some(_)) => {}
            (None, None) => {}
            (Some(maximum), Some(requested)) => {
                return Err(resource_ceiling_error(id, requested.to_string(), maximum));
            }
            (Some(maximum), None) => {
                return Err(resource_ceiling_error(id, "unbounded".to_string(), maximum));
            }
        }
    }
    Ok(candidate)
}

fn resource_profile_ceiling_error(
    requested: merman::model::resources::ResourceProfile,
    ceiling: merman::model::resources::ResourceProfile,
) -> BindingError {
    BindingError::new(
        BindingStatus::OptionsJsonError,
        format!(
            "resources would loosen the transport ceiling profile: requested {requested}, ceiling {ceiling}"
        ),
    )
}

fn resource_ceiling_error(id: &str, requested: String, maximum: usize) -> BindingError {
    BindingError::new(
        BindingStatus::OptionsJsonError,
        format!(
            "resources would loosen the transport ceiling for `{id}`: requested {requested}, maximum {maximum}"
        ),
    )
}

pub(super) fn effective_resource_limits(
    resources: &ResourceOptionsJson,
) -> Result<BTreeMap<&'static str, Option<usize>>, BindingError> {
    let profile = binding_resource_profile(Some(resources))?;
    validate_compiled_resource_options(Some(resources))?;

    let mut values = binding_resource_contract()
        .limits
        .into_iter()
        .map(|descriptor| {
            (
                descriptor.stable_id,
                resource_profile_value(profile, descriptor.stable_id)
                    .expect("compiled resource descriptors must have profile values"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (id, value) in &resources.limits {
        let stable_id = resource_limit_descriptor(id)
            .expect("validated resource limit must have a compiled descriptor")
            .stable_id;
        values.insert(stable_id, Some(*value));
    }
    Ok(values)
}

pub fn render_resource_options_unavailable() -> BindingError {
    BindingError::new(
        BindingStatus::UnsupportedOperation,
        "resource options requires at least one resource-aware operation",
    )
}
