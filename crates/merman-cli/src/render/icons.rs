use crate::error::CliError;
use crate::input::InputLimit;
use crate::io::read_named_text_file;
#[cfg(feature = "network-icons")]
use crate::network::{NetworkAuthorization, NetworkPolicy, fetch_http_body};
use crate::resources::{ByteLedgerKind, CheckedBytes, CountLedgerKind, ResolvedResourcePolicy};
use merman::svg::IconRegistry;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(super) fn load_icon_registry(
    icon_packs: &[String],
    icon_packs_names_and_urls: &[String],
    resources: &ResolvedResourcePolicy,
    #[cfg(feature = "network-icons")] allow_network: bool,
    #[cfg(feature = "network-icons")] allow_private_network: bool,
) -> Result<Option<Arc<IconRegistry>>, CliError> {
    if icon_packs.is_empty() && icon_packs_names_and_urls.is_empty() {
        return Ok(None);
    }

    let mut pack_count = resources.checked_count(CountLedgerKind::IconPacks);
    let total_packs = icon_packs
        .len()
        .checked_add(icon_packs_names_and_urls.len())
        .and_then(|count| u64::try_from(count).ok())
        .ok_or_else(|| CliError::InvalidInput("icon pack count overflow".to_string()))?;
    pack_count
        .try_add(total_packs)
        .map_err(resource_input_error)?;
    let mut aggregate_bytes = resources.checked_bytes(ByteLedgerKind::AggregateIcons);
    let limits = resources.icons();
    #[cfg(feature = "network-icons")]
    let network_limits = resources.network();

    let cwd = std::env::current_dir()?;
    let mut registry = IconRegistry::new();

    for icon_pack in icon_packs {
        let prefix = icon_pack_package_prefix(icon_pack)?;
        let source = match local_icon_pack_path(icon_pack, &cwd)? {
            Some(path) => IconPackSource::LocalPath(path),
            #[cfg(feature = "network-icons")]
            None if allow_network => {
                IconPackSource::RemoteUrl(format!("https://unpkg.com/{icon_pack}/icons.json"))
            }
            None => return Err(missing_local_icon_pack_error(icon_pack)),
        };
        let json = read_icon_pack_source(
            &source,
            limits.local_body_bytes,
            #[cfg(feature = "network-icons")]
            limits.remote_body_bytes,
            &mut aggregate_bytes,
            #[cfg(feature = "network-icons")]
            NetworkPolicy {
                authorization: network_authorization(allow_network, allow_private_network),
                max_redirects: network_limits.max_redirects,
                connect_timeout: network_limits.connect_timeout,
                per_hop_timeout: network_limits.per_hop_timeout,
                workflow_timeout: network_limits.workflow_timeout,
                max_body_bytes: None,
                max_body_limit_id: crate::resources::CliResourceLimitId::MaxRemoteIconBodyBytes
                    .as_str(),
            },
        )?;
        register_icon_pack_json(&mut registry, &json, Some(&prefix), icon_pack)?;
    }

    for icon_pack_info in icon_packs_names_and_urls {
        let (prefix, source) = icon_pack_info.split_once('#').ok_or_else(|| {
            CliError::InvalidInput(format!(
                "Invalid --iconPacksNamesAndUrls value `{icon_pack_info}`; expected prefix#url"
            ))
        })?;
        let prefix = prefix.trim();
        let source = source.trim();
        if prefix.is_empty() || source.is_empty() {
            return Err(CliError::InvalidInput(format!(
                "Invalid --iconPacksNamesAndUrls value `{icon_pack_info}`; expected non-empty prefix and URL"
            )));
        }

        let source = icon_pack_source_from_cli(source, &cwd)?;
        let json = read_icon_pack_source(
            &source,
            limits.local_body_bytes,
            #[cfg(feature = "network-icons")]
            limits.remote_body_bytes,
            &mut aggregate_bytes,
            #[cfg(feature = "network-icons")]
            NetworkPolicy {
                authorization: network_authorization(allow_network, allow_private_network),
                max_redirects: network_limits.max_redirects,
                connect_timeout: network_limits.connect_timeout,
                per_hop_timeout: network_limits.per_hop_timeout,
                workflow_timeout: network_limits.workflow_timeout,
                max_body_bytes: None,
                max_body_limit_id: crate::resources::CliResourceLimitId::MaxRemoteIconBodyBytes
                    .as_str(),
            },
        )?;
        register_icon_pack_json(&mut registry, &json, Some(prefix), icon_pack_info)?;
    }

    Ok((!registry.is_empty()).then(|| Arc::new(registry)))
}

enum IconPackSource {
    LocalPath(PathBuf),
    #[cfg(feature = "network-icons")]
    RemoteUrl(String),
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

fn register_icon_pack_json(
    registry: &mut IconRegistry,
    json: &str,
    prefix_override: Option<&str>,
    label: &str,
) -> Result<(), CliError> {
    registry
        .register_iconify_json_str(json, prefix_override)
        .map_err(|err| {
            CliError::InvalidInput(format!("Invalid icon pack JSON for `{label}`: {err}"))
        })
}

fn icon_pack_package_prefix(icon_pack: &str) -> Result<String, CliError> {
    let icon_pack = icon_pack.trim().trim_end_matches('/');
    if !valid_npm_package_name(icon_pack) && !looks_like_path(icon_pack) {
        return Err(CliError::InvalidInput(format!(
            "Invalid --iconPacks value `{icon_pack}`; expected an npm package name or local JSON path"
        )));
    }
    let prefix = icon_pack.rsplit('/').next().unwrap_or(icon_pack).trim();
    if prefix.is_empty() || prefix.starts_with('@') {
        return Err(CliError::InvalidInput(format!(
            "Invalid --iconPacks value `{icon_pack}`; expected an Iconify package such as @iconify-json/logos"
        )));
    }
    Ok(prefix.to_string())
}

fn local_icon_pack_path(icon_pack: &str, cwd: &Path) -> Result<Option<PathBuf>, CliError> {
    if looks_like_path(icon_pack) {
        let path = resolve_cli_path(icon_pack, cwd);
        if path.exists() {
            return Ok(Some(path));
        }
        return Err(missing_local_icon_pack_error(icon_pack));
    }

    let mut current = Some(cwd);
    while let Some(dir) = current {
        let candidate = dir.join("node_modules").join(icon_pack).join("icons.json");
        if candidate.exists() {
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
            Ok(IconPackSource::RemoteUrl(source.to_string()))
        }
        #[cfg(not(feature = "network-icons"))]
        {
            Err(CliError::InvalidInput(format!(
                "Icon pack URL `{source}` requires building merman-cli with --features network-icons."
            )))
        }
    } else if let Some(path) = file_url_to_path(source) {
        Ok(IconPackSource::LocalPath(path))
    } else {
        Ok(IconPackSource::LocalPath(resolve_cli_path(source, cwd)))
    }
}

fn read_icon_pack_source(
    source: &IconPackSource,
    local_body_limit: Option<usize>,
    #[cfg(feature = "network-icons")] remote_body_limit: Option<usize>,
    aggregate_bytes: &mut CheckedBytes,
    #[cfg(feature = "network-icons")] network_policy: NetworkPolicy,
) -> Result<String, CliError> {
    let remaining = aggregate_bytes.remaining();
    let bytes = match source {
        IconPackSource::LocalPath(path) => {
            let limit = effective_body_limit(
                local_body_limit,
                crate::resources::CliResourceLimitId::MaxLocalIconBodyBytes.as_str(),
                remaining,
            );
            read_named_text_file(path, "icon pack", limit)?.into_bytes()
        }
        #[cfg(feature = "network-icons")]
        IconPackSource::RemoteUrl(url)
            if network_policy.authorization != NetworkAuthorization::Denied =>
        {
            let mut policy = network_policy;
            let limit = effective_body_limit(
                remote_body_limit,
                crate::resources::CliResourceLimitId::MaxRemoteIconBodyBytes.as_str(),
                remaining,
            );
            policy.max_body_bytes = limit.max_bytes;
            policy.max_body_limit_id = limit.stable_id;
            fetch_http_body(url, policy)?
        }
        #[cfg(feature = "network-icons")]
        IconPackSource::RemoteUrl(_) => {
            return Err(CliError::InvalidInput(
                "Remote icon pack sources require --allow-network before merman-cli will fetch HTTP(S)"
                    .to_string(),
            ));
        }
    };
    aggregate_bytes
        .try_add(bytes.len() as u64)
        .map_err(resource_input_error)?;
    String::from_utf8(bytes).map_err(|_| {
        CliError::InvalidInput("icon pack content is not valid UTF-8 JSON".to_string())
    })
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
    per_body: Option<usize>,
    per_body_id: &'static str,
    aggregate_remaining: Option<u64>,
) -> InputLimit {
    let aggregate_remaining =
        aggregate_remaining.map(|remaining| usize::try_from(remaining).unwrap_or(usize::MAX));
    match (per_body, aggregate_remaining) {
        (Some(per_body), Some(remaining)) if remaining < per_body => InputLimit::new(
            crate::resources::CliResourceLimitId::MaxAggregateIconBytes.as_str(),
            Some(remaining),
        ),
        (Some(per_body), _) => InputLimit::new(per_body_id, Some(per_body)),
        (None, Some(remaining)) => InputLimit::new(
            crate::resources::CliResourceLimitId::MaxAggregateIconBytes.as_str(),
            Some(remaining),
        ),
        (None, None) => InputLimit::new(per_body_id, None),
    }
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
