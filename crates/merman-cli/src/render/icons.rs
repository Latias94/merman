use crate::error::CliError;
use crate::input::InputLimit;
use crate::invocation::ResolvedIconSources;
use crate::io::read_named_bytes_file_controlled;
#[cfg(feature = "network-icons")]
use crate::network::{NetworkAcquirer, NetworkAuthorization, NetworkPolicy, SanitizedEndpoint};
use crate::resources::{ByteLedgerKind, CheckedBytes, CountLedgerKind, ResolvedResourcePolicy};
use merman::svg::{IconPack, IconRegistry, IconRegistryBuilder, IconRegistryResourceLimitId};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
struct IconPackBodyLimits {
    local: InputLimit,
    #[cfg(feature = "network-icons")]
    remote: InputLimit,
}

pub(super) fn load_icon_registry(
    icon_sources: &ResolvedIconSources,
    resources: &ResolvedResourcePolicy,
    cwd: &Path,
    control: &merman::OperationControl,
    #[cfg(feature = "network-icons")] network: &mut dyn NetworkAcquirer,
) -> Result<Option<IconRegistry>, CliError> {
    if icon_sources.packages.is_empty() && icon_sources.named_sources.is_empty() {
        return Ok(None);
    }

    validate_icon_source_count(icon_sources, resources)?;
    let icon_packs = resolve_icon_pack_sources(icon_sources, cwd)?;
    let mut aggregate_bytes = resources.checked_bytes(ByteLedgerKind::AggregateIcons);
    let limits = resources.icons();
    #[cfg(feature = "network-icons")]
    let network_limits = resources.network();

    let mut builder = IconRegistryBuilder::new();
    let mut renderer_input_bytes = 0usize;

    for icon_pack in icon_packs {
        crate::operation::checkpoint(control, merman::OperationPhase::Admission)?;
        let renderer_input_remaining =
            renderer_icon_limit(IconRegistryResourceLimitId::MaxInputBytes)
                .checked_sub(renderer_input_bytes)
                .expect("source count and prior reads stay within the renderer input ceiling");
        let json = read_icon_pack_source(
            &icon_pack.source,
            IconPackBodyLimits {
                local: acquisition_body_limit(
                    limits.local_body_bytes,
                    crate::resources::CliResourceLimitId::MaxLocalIconBodyBytes.as_str(),
                ),
                #[cfg(feature = "network-icons")]
                remote: acquisition_body_limit(
                    limits.remote_body_bytes,
                    crate::resources::CliResourceLimitId::MaxRemoteIconBodyBytes.as_str(),
                ),
            },
            &mut aggregate_bytes,
            renderer_input_remaining,
            control,
            #[cfg(feature = "network-icons")]
            NetworkPolicy {
                authorization: network_authorization(
                    icon_sources.allow_network,
                    icon_sources.allow_private_network,
                ),
                max_redirects: network_limits.max_redirects,
                connect_timeout: network_limits.connect_timeout,
                per_hop_timeout: network_limits.per_hop_timeout,
                workflow_timeout: network_limits.workflow_timeout,
                max_body_bytes: None,
                max_body_limit_id: crate::resources::CliResourceLimitId::MaxRemoteIconBodyBytes
                    .as_str(),
            },
            #[cfg(feature = "network-icons")]
            network,
        )?;
        renderer_input_bytes = renderer_input_bytes
            .checked_add(json.len())
            .ok_or_else(|| {
                CliError::InvalidInput("renderer icon input accounting overflowed".to_string())
            })?;
        let label = icon_pack.diagnostic_label();
        let pack = match icon_pack.registration_name.as_deref() {
            Some(registration_name) => {
                IconPack::new(&json).with_registration_name(registration_name)
            }
            None => IconPack::new(&json),
        };
        builder = builder.add_pack(pack).map_err(|error| {
            icon_registry_error(
                format!("Invalid icon pack JSON for `{label}`: {error}"),
                error.kind(),
            )
        })?;
    }

    let registry = builder.build().map_err(|error| {
        icon_registry_error(
            format!("Invalid Iconify registry transaction: {error}"),
            error.kind(),
        )
    })?;
    Ok((!registry.is_empty()).then_some(registry))
}

fn icon_registry_error(message: String, kind: merman::svg::IconRegistryBuildErrorKind) -> CliError {
    if matches!(
        kind,
        merman::svg::IconRegistryBuildErrorKind::AllocationFailed
            | merman::svg::IconRegistryBuildErrorKind::ArithmeticOverflow
    ) {
        CliError::Internal(message)
    } else {
        CliError::InvalidInput(message)
    }
}

pub(super) struct ResolvedIconPackSource {
    registration_name: Option<String>,
    source: IconPackSource,
}

impl ResolvedIconPackSource {
    fn diagnostic_label(&self) -> String {
        self.source.diagnostic_label()
    }
}

enum IconPackSource {
    LocalPath(PathBuf),
    #[cfg(feature = "network-icons")]
    RemoteUrl {
        url: String,
        endpoint: SanitizedEndpoint,
    },
}

impl IconPackSource {
    fn diagnostic_label(&self) -> String {
        match self {
            Self::LocalPath(path) => format!("{path:?}"),
            #[cfg(feature = "network-icons")]
            Self::RemoteUrl { endpoint, .. } => endpoint.to_string(),
        }
    }
}

pub(crate) fn resolve_local_icon_paths(
    icon_sources: &ResolvedIconSources,
    cwd: &Path,
) -> Result<Vec<PathBuf>, CliError> {
    Ok(resolve_icon_pack_sources(icon_sources, cwd)?
        .into_iter()
        .filter_map(|pack| match pack.source {
            IconPackSource::LocalPath(path) => Some(path),
            #[cfg(feature = "network-icons")]
            IconPackSource::RemoteUrl { .. } => None,
        })
        .collect())
}

pub(crate) fn validate_icon_source_count(
    icon_sources: &ResolvedIconSources,
    resources: &ResolvedResourcePolicy,
) -> Result<(), CliError> {
    let total = icon_sources
        .packages
        .len()
        .checked_add(icon_sources.named_sources.len())
        .and_then(|count| u64::try_from(count).ok())
        .ok_or_else(|| CliError::InvalidInput("icon pack count overflow".to_string()))?;
    resources
        .checked_count(CountLedgerKind::IconPacks)
        .try_add(total)
        .map_err(resource_input_error)?;

    let renderer_max = IconRegistryResourceLimitId::MaxPacks.fixed_value();
    if total > renderer_max {
        return Err(CliError::InvalidInput(format!(
            "icon pack count {total} exceeds the {renderer_max}-pack renderer ceiling ({})",
            IconRegistryResourceLimitId::MaxPacks.stable_id()
        )));
    }
    Ok(())
}

fn resolve_icon_pack_sources(
    icon_sources: &ResolvedIconSources,
    cwd: &Path,
) -> Result<Vec<ResolvedIconPackSource>, CliError> {
    let capacity = icon_sources
        .packages
        .len()
        .checked_add(icon_sources.named_sources.len())
        .ok_or_else(|| CliError::InvalidInput("icon pack count overflow".to_string()))?;
    let mut resolved = Vec::new();
    resolved.try_reserve_exact(capacity).map_err(|_| {
        CliError::InvalidInput("failed to allocate the resolved icon source list".to_string())
    })?;

    for icon_pack in &icon_sources.packages {
        if let Some(path) = local_icon_pack_path(icon_pack, cwd)? {
            let registration_name = if looks_like_path(icon_pack) {
                None
            } else {
                Some(icon_pack_package_prefix(icon_pack)?)
            };
            resolved.push(ResolvedIconPackSource {
                registration_name,
                source: IconPackSource::LocalPath(path),
            });
            continue;
        }

        let prefix = icon_pack_package_prefix(icon_pack)?;
        #[cfg(feature = "network-icons")]
        if icon_sources.allow_network {
            resolved.push(ResolvedIconPackSource {
                registration_name: Some(prefix),
                source: remote_source(&format!("https://unpkg.com/{icon_pack}/icons.json"))?,
            });
            continue;
        }
        #[cfg(not(feature = "network-icons"))]
        let _ = prefix;
        return Err(missing_local_icon_pack_error(icon_pack));
    }

    for named_source in &icon_sources.named_sources {
        let Some((prefix, source)) = named_source.split_once('#') else {
            return Err(CliError::InvalidInput(
                "invalid --iconPacksNamesAndUrls value; expected prefix#url".to_string(),
            ));
        };
        let prefix = prefix.trim();
        let source = source.trim();
        // This is acquisition-syntax validation, not a second Iconify identifier grammar. It
        // prevents a URL fragment from being misread as `prefix#local-path` and then echoed by a
        // filesystem diagnostic; the renderer builder remains the sole prefix-policy owner.
        if prefix.is_empty() || source.is_empty() || looks_like_path(prefix) {
            return Err(CliError::InvalidInput(
                "invalid --iconPacksNamesAndUrls value; expected a safe prefix and URL".to_string(),
            ));
        }
        resolved.push(ResolvedIconPackSource {
            registration_name: Some(prefix.to_string()),
            source: icon_pack_source_from_cli(source, cwd)?,
        });
    }

    Ok(resolved)
}

fn missing_local_icon_pack_error(icon_pack: &str) -> CliError {
    #[cfg(feature = "network-icons")]
    {
        CliError::InvalidInput(format!(
            "Icon pack `{icon_pack}` was not found in node_modules or as a local JSON path. Install it locally or pass --allow-network to fetch it from unpkg."
        ))
    }

    #[cfg(not(feature = "network-icons"))]
    {
        CliError::InvalidInput(format!(
            "Icon pack `{icon_pack}` was not found in node_modules or as a local JSON path. Install it locally or build merman-cli with --features network-icons to fetch it from unpkg."
        ))
    }
}

fn icon_pack_package_prefix(icon_pack: &str) -> Result<String, CliError> {
    let icon_pack = icon_pack.trim().trim_end_matches('/');
    if !valid_npm_package_name(icon_pack) && !looks_like_path(icon_pack) {
        return Err(CliError::InvalidInput(format!(
            "invalid --iconPacks value {icon_pack:?}; expected an npm package name or local JSON path"
        )));
    }
    let prefix = icon_pack.rsplit('/').next().unwrap_or(icon_pack).trim();
    if prefix.is_empty() || prefix.starts_with('@') {
        return Err(CliError::InvalidInput(format!(
            "invalid --iconPacks value {icon_pack:?}; expected an Iconify package such as @iconify-json/logos"
        )));
    }
    Ok(prefix.to_string())
}

fn local_icon_pack_path(icon_pack: &str, cwd: &Path) -> Result<Option<PathBuf>, CliError> {
    if looks_like_path(icon_pack) {
        let path = resolve_cli_path(icon_pack, cwd);
        if path_exists(&path)? {
            return Ok(Some(path));
        }
        return Err(CliError::InvalidInput(format!(
            "local icon pack {:?} does not exist",
            path
        )));
    }

    let mut current = Some(cwd);
    while let Some(dir) = current {
        let candidate = dir.join("node_modules").join(icon_pack).join("icons.json");
        if path_exists(&candidate)? {
            return Ok(Some(candidate));
        }
        current = dir.parent();
    }
    Ok(None)
}

fn icon_pack_source_from_cli(source: &str, cwd: &Path) -> Result<IconPackSource, CliError> {
    if source.starts_with("http://") || source.starts_with("https://") {
        #[cfg(feature = "network-icons")]
        {
            remote_source(source)
        }
        #[cfg(not(feature = "network-icons"))]
        {
            Err(CliError::InvalidInput(
                "remote icon pack URLs require building merman-cli with --features network-icons"
                    .to_string(),
            ))
        }
    } else if source.starts_with("file:") {
        let path = file_url_to_path(source).ok_or_else(|| {
            CliError::InvalidInput("invalid local icon pack file URL".to_string())
        })?;
        Ok(IconPackSource::LocalPath(path))
    } else if looks_like_path(source) || url::Url::parse(source).is_err() {
        Ok(IconPackSource::LocalPath(resolve_cli_path(source, cwd)))
    } else {
        let scheme = url::Url::parse(source)
            .map(|url| url.scheme().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        Err(CliError::InvalidInput(format!(
            "icon pack URL scheme {scheme:?} is not supported; expected file, http, or https"
        )))
    }
}

#[cfg(feature = "network-icons")]
fn remote_source(source: &str) -> Result<IconPackSource, CliError> {
    let url = url::Url::parse(source).map_err(|_| crate::network::NetworkError::InvalidUrl)?;
    let endpoint = SanitizedEndpoint::from_url(&url)?;
    Ok(IconPackSource::RemoteUrl {
        url: source.to_string(),
        endpoint,
    })
}

fn path_exists(path: &Path) -> Result<bool, CliError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(CliError::file(
            crate::error::FileOperation::Inspect,
            path,
            source,
        )),
    }
}

fn read_icon_pack_source(
    source: &IconPackSource,
    body_limits: IconPackBodyLimits,
    aggregate_bytes: &mut CheckedBytes,
    renderer_input_remaining: usize,
    control: &merman::OperationControl,
    #[cfg(feature = "network-icons")] network_policy: NetworkPolicy,
    #[cfg(feature = "network-icons")] network: &mut dyn NetworkAcquirer,
) -> Result<Vec<u8>, CliError> {
    crate::operation::checkpoint(control, merman::OperationPhase::Admission)?;
    let remaining = aggregate_bytes.remaining();
    let bytes = match source {
        IconPackSource::LocalPath(path) => {
            let limit =
                effective_body_limit(body_limits.local, remaining, renderer_input_remaining);
            read_named_bytes_file_controlled(path, "icon pack", limit, control)?
        }
        #[cfg(feature = "network-icons")]
        IconPackSource::RemoteUrl { url, .. }
            if network_policy.authorization != NetworkAuthorization::Denied =>
        {
            let mut policy = network_policy;
            let limit =
                effective_body_limit(body_limits.remote, remaining, renderer_input_remaining);
            policy.max_body_bytes = limit.max_bytes;
            policy.max_body_limit_id = limit.stable_id;
            network.fetch(url, policy)?
        }
        #[cfg(feature = "network-icons")]
        IconPackSource::RemoteUrl { .. } => {
            return Err(CliError::InvalidInput(
                "Remote icon pack sources require --allow-network before merman-cli will fetch HTTP(S)"
                    .to_string(),
            ));
        }
    };
    crate::operation::checkpoint(control, merman::OperationPhase::Admission)?;
    aggregate_bytes
        .try_add(bytes.len() as u64)
        .map_err(resource_input_error)?;
    Ok(bytes)
}

fn looks_like_path(value: &str) -> bool {
    value.ends_with(".json")
        || value.starts_with('.')
        || value.contains('\\')
        || value.contains('/') && !valid_npm_package_name(value)
        || Path::new(value).is_absolute()
}

fn resolve_cli_path(value: &str, cwd: &Path) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn file_url_to_path(value: &str) -> Option<PathBuf> {
    let url = url::Url::parse(value).ok()?;
    (url.scheme() == "file")
        .then(|| url.to_file_path().ok())
        .flatten()
}

fn effective_body_limit(
    per_body: InputLimit,
    aggregate_remaining: Option<u64>,
    renderer_input_remaining: usize,
) -> InputLimit {
    let aggregate_remaining =
        aggregate_remaining.map(|remaining| usize::try_from(remaining).unwrap_or(usize::MAX));
    let limit = narrower_input_limit(
        per_body,
        InputLimit::new(
            crate::resources::CliResourceLimitId::MaxAggregateIconBytes.as_str(),
            aggregate_remaining,
        ),
    );
    narrower_input_limit(
        limit,
        InputLimit::new(
            IconRegistryResourceLimitId::MaxInputBytes.stable_id(),
            Some(renderer_input_remaining),
        ),
    )
}

fn acquisition_body_limit(cli_limit: Option<usize>, cli_limit_id: &'static str) -> InputLimit {
    narrower_input_limit(
        InputLimit::new(cli_limit_id, cli_limit),
        InputLimit::new(
            IconRegistryResourceLimitId::MaxPackBytes.stable_id(),
            Some(renderer_icon_limit(
                IconRegistryResourceLimitId::MaxPackBytes,
            )),
        ),
    )
}

fn narrower_input_limit(left: InputLimit, right: InputLimit) -> InputLimit {
    match (left.max_bytes, right.max_bytes) {
        (_, Some(right_max)) if left.max_bytes.is_none_or(|left_max| right_max < left_max) => right,
        _ => left,
    }
}

fn renderer_icon_limit(id: IconRegistryResourceLimitId) -> usize {
    usize::try_from(id.fixed_value()).expect("renderer icon limits must fit usize")
}

fn resource_input_error(error: impl std::fmt::Display) -> CliError {
    CliError::InvalidInput(error.to_string())
}

fn valid_npm_package_name(value: &str) -> bool {
    let mut segments = value.split('/');
    let first = segments.next().unwrap_or_default();
    let second = segments.next();
    if segments.next().is_some() {
        return false;
    }
    if let Some(name) = first.strip_prefix('@') {
        return !name.is_empty()
            && second.is_some_and(valid_package_segment)
            && valid_package_segment(name);
    }
    second.is_none() && valid_package_segment(first)
}

fn valid_package_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.' | b'~')
        })
}

#[cfg(feature = "network-icons")]
fn network_authorization(allow_network: bool, allow_private_network: bool) -> NetworkAuthorization {
    if allow_private_network {
        NetworkAuthorization::PrivateAllowed
    } else if allow_network {
        NetworkAuthorization::PublicOnly
    } else {
        NetworkAuthorization::Denied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman::svg::IconRegistryBuildErrorKind;

    #[test]
    fn icon_registry_internal_failures_use_the_operational_exit_category() {
        for kind in [
            IconRegistryBuildErrorKind::AllocationFailed,
            IconRegistryBuildErrorKind::ArithmeticOverflow,
        ] {
            let error = icon_registry_error("internal registry failure".to_string(), kind);
            assert!(matches!(&error, CliError::Internal(_)));
            assert_eq!(error.exit_code(), std::process::ExitCode::from(3));
        }
    }

    #[test]
    fn icon_registry_content_failures_remain_invalid_input() {
        let error = icon_registry_error(
            "invalid registry content".to_string(),
            IconRegistryBuildErrorKind::InvalidJson,
        );
        assert!(matches!(&error, CliError::InvalidInput(_)));
        assert_eq!(error.exit_code(), std::process::ExitCode::from(2));
    }
}
