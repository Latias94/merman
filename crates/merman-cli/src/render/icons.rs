use crate::error::CliError;
use merman::render::IconRegistry;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub(super) enum NetworkPolicy {
    Offline,
    AllowNetwork,
}

impl NetworkPolicy {
    pub(super) const fn from_allow_network(allow_network: bool) -> Self {
        if allow_network {
            Self::AllowNetwork
        } else {
            Self::Offline
        }
    }

    const fn allows_network(self) -> bool {
        matches!(self, Self::AllowNetwork)
    }
}

pub(super) fn load_icon_registry(
    icon_packs: &[String],
    icon_packs_names_and_urls: &[String],
    network_policy: NetworkPolicy,
) -> Result<Option<Arc<IconRegistry>>, CliError> {
    if icon_packs.is_empty() && icon_packs_names_and_urls.is_empty() {
        return Ok(None);
    }

    let cwd = std::env::current_dir()?;
    let mut registry = IconRegistry::new();

    for icon_pack in icon_packs {
        let prefix = icon_pack_package_prefix(icon_pack)?;
        let source = match local_icon_pack_path(icon_pack, &cwd) {
            Some(path) => IconPackSource::LocalPath(path),
            None if network_policy.allows_network() => {
                IconPackSource::RemoteUrl(format!("https://unpkg.com/{icon_pack}/icons.json"))
            }
            None => {
                return Err(CliError::InvalidInput(format!(
                    "Icon pack `{icon_pack}` was not found in node_modules or as a local JSON path. Install it locally or pass --allow-network to fetch it from unpkg."
                )));
            }
        };
        let json = read_icon_pack_source(&source, network_policy)?;
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

        let source = icon_pack_source_from_cli(source, &cwd);
        let json = read_icon_pack_source(&source, network_policy)?;
        register_icon_pack_json(&mut registry, &json, Some(prefix), icon_pack_info)?;
    }

    Ok((!registry.is_empty()).then(|| Arc::new(registry)))
}

enum IconPackSource {
    LocalPath(PathBuf),
    RemoteUrl(String),
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
    let prefix = icon_pack.rsplit('/').next().unwrap_or(icon_pack).trim();
    if prefix.is_empty() || prefix.starts_with('@') {
        return Err(CliError::InvalidInput(format!(
            "Invalid --iconPacks value `{icon_pack}`; expected an Iconify package such as @iconify-json/logos"
        )));
    }
    Ok(prefix.to_string())
}

fn local_icon_pack_path(icon_pack: &str, cwd: &Path) -> Option<PathBuf> {
    if looks_like_path(icon_pack) {
        let path = resolve_cli_path(icon_pack, cwd);
        if path.exists() {
            return Some(path);
        }
    }

    let mut current = Some(cwd);
    while let Some(dir) = current {
        let candidate = dir.join("node_modules").join(icon_pack).join("icons.json");
        if candidate.exists() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

fn icon_pack_source_from_cli(source: &str, cwd: &Path) -> IconPackSource {
    if source.starts_with("http://") || source.starts_with("https://") {
        IconPackSource::RemoteUrl(source.to_string())
    } else if let Some(path) = file_url_to_path(source) {
        IconPackSource::LocalPath(path)
    } else {
        IconPackSource::LocalPath(resolve_cli_path(source, cwd))
    }
}

fn read_icon_pack_source(
    source: &IconPackSource,
    network_policy: NetworkPolicy,
) -> Result<String, CliError> {
    match source {
        IconPackSource::LocalPath(path) => std::fs::read_to_string(path).map_err(|err| {
            CliError::InvalidInput(format!(
                "Failed to read icon pack JSON `{}`: {err}",
                path.display()
            ))
        }),
        IconPackSource::RemoteUrl(url) if network_policy.allows_network() => {
            fetch_icon_pack_json(url)
        }
        IconPackSource::RemoteUrl(url) => Err(CliError::InvalidInput(format!(
            "Icon pack URL `{url}` requires --allow-network before merman-cli will fetch HTTP(S) sources."
        ))),
    }
}

fn fetch_icon_pack_json(url: &str) -> Result<String, CliError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| {
            CliError::InvalidInput(format!("Failed to create icon pack HTTP client: {err}"))
        })?;
    let response = client.get(url).send().map_err(|err| {
        CliError::InvalidInput(format!("Failed to fetch icon pack JSON `{url}`: {err}"))
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(CliError::InvalidInput(format!(
            "Failed to fetch icon pack JSON `{url}`: HTTP {status}"
        )));
    }
    response.text().map_err(|err| {
        CliError::InvalidInput(format!("Failed to read icon pack JSON `{url}`: {err}"))
    })
}

fn looks_like_path(value: &str) -> bool {
    value.ends_with(".json")
        || value.starts_with('.')
        || value.contains('\\')
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
    let raw = value.strip_prefix("file://")?;
    let decoded = raw.replace("%20", " ");
    #[cfg(windows)]
    {
        let trimmed = decoded.strip_prefix('/').unwrap_or(&decoded);
        Some(PathBuf::from(trimmed))
    }
    #[cfg(not(windows))]
    {
        Some(PathBuf::from(decoded))
    }
}
