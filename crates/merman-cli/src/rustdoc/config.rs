use crate::error::{CliError, FileOperation, safe_path};
use crate::input::{InputLimit, InputReadError, read_utf8};
use crate::resources::{CliResourceLimitId, ResolvedResourcePolicy};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;

const SUPPORTED_SCHEMA: u32 = 1;
const MAX_FRAGMENT_ID_BYTES: usize = 120;
const OUTPUT_ROOT: &str = "docs/generated/merman-rustdoc";

#[derive(Debug)]
pub(crate) struct Config {
    path: PathBuf,
    identity: Arc<same_file::Handle>,
    root: PathBuf,
    root_identity: Arc<same_file::Handle>,
    output_root: PathBuf,
    fragments: Vec<Fragment>,
}

impl Config {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn identity(&self) -> &Arc<same_file::Handle> {
        &self.identity
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn root_identity(&self) -> &Arc<same_file::Handle> {
        &self.root_identity
    }

    pub(crate) fn output_root(&self) -> &Path {
        &self.output_root
    }

    pub(crate) fn fragments(&self) -> &[Fragment] {
        &self.fragments
    }
}

#[derive(Debug)]
pub(crate) struct Fragment {
    id: String,
    source: PathBuf,
    identity: Arc<same_file::Handle>,
    _text: String,
    _source_display: Option<SourceDisplay>,
}

impl Fragment {
    pub(crate) fn source(&self) -> &Path {
        &self.source
    }

    pub(crate) fn identity(&self) -> &Arc<same_file::Handle> {
        &self.identity
    }

    pub(crate) fn output(&self, output_root: &Path) -> PathBuf {
        output_root.join(format!("{}.md", self.id))
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SourceDisplay {
    Hide,
    Details,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    schema: u32,
    fragments: Vec<RawFragment>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFragment {
    id: String,
    source: String,
    source_display: Option<SourceDisplay>,
}

pub(crate) fn load(
    requested_path: &Path,
    resources: &ResolvedResourcePolicy,
) -> Result<Config, CliError> {
    let acquired = acquire_text(
        requested_path,
        "Rustdoc configuration",
        InputLimit::new(
            CliResourceLimitId::MaxConfigBytes.as_str(),
            resources.files().config_bytes,
        ),
    )
    .map_err(|source| config_error(requested_path, "document", source))?;
    let raw = parse_config(requested_path, &acquired.text)?;

    if raw.schema != SUPPORTED_SCHEMA {
        return Err(invalid(
            requested_path,
            "schema",
            format!(
                "unsupported schema {}; expected schema {SUPPORTED_SCHEMA}",
                raw.schema
            ),
        ));
    }
    if raw.fragments.is_empty() {
        return Err(invalid(
            requested_path,
            "fragments",
            "configuration must contain at least one fragment",
        ));
    }

    validate_fragment_ids(requested_path, &raw.fragments)?;

    let root = acquired
        .canonical
        .parent()
        .expect("a canonical file path has a parent")
        .to_path_buf();
    let root_identity = Arc::new(
        same_file::Handle::from_path(&root)
            .map_err(|source| CliError::file(FileOperation::InspectIdentity, &root, source))?,
    );
    let source_limit = InputLimit::new(
        merman::resources::InputResourceLimitId::MaxSourceBytes.as_str(),
        resources
            .input_policy()
            .value(merman::resources::InputResourceLimitId::MaxSourceBytes),
    );
    let mut fragments = Vec::with_capacity(raw.fragments.len());
    for (index, fragment) in raw.fragments.into_iter().enumerate() {
        let location = format!("fragments[{index}].source");
        let relative = validate_source_path(requested_path, &location, &fragment.source)?;
        let source_path = root.join(&relative);
        let source = acquire_text(&source_path, "Rustdoc fragment source", source_limit)
            .map_err(|source| config_error(requested_path, &location, source))?;
        if source.canonical.strip_prefix(&root).is_err() {
            return Err(invalid(
                requested_path,
                &location,
                format!(
                    "source {} escapes the configuration root {}",
                    safe_path(&relative),
                    safe_path(&root)
                ),
            ));
        }
        fragments.push(Fragment {
            id: fragment.id,
            source: source.canonical,
            identity: source.identity,
            _text: source.text,
            _source_display: fragment.source_display,
        });
    }

    Ok(Config {
        path: acquired.canonical,
        identity: acquired.identity,
        output_root: root.join(OUTPUT_ROOT),
        root,
        root_identity,
        fragments,
    })
}

fn parse_config(path: &Path, text: &str) -> Result<RawConfig, CliError> {
    toml::from_str(text).map_err(|error: toml::de::Error| {
        let location = error
            .span()
            .map(|span| line_column(text, span.start))
            .unwrap_or_else(|| "document".to_string());
        invalid(path, location, error.message())
    })
}

fn line_column(text: &str, offset: usize) -> String {
    let prefix = &text[..offset.min(text.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len() + 1, |(_, tail)| tail.len() + 1);
    format!("line {line}, column {column}")
}

fn validate_fragment_ids(path: &Path, fragments: &[RawFragment]) -> Result<(), CliError> {
    let mut aliases = HashMap::<String, (usize, &str)>::new();
    for (index, fragment) in fragments.iter().enumerate() {
        let location = format!("fragments[{index}].id");
        if fragment.id.is_empty() {
            return Err(invalid(path, location, "fragment id must not be empty"));
        }
        let alias = fragment_alias(&fragment.id);
        if let Some((previous_index, previous_id)) = aliases.insert(alias, (index, &fragment.id)) {
            return Err(invalid(
                path,
                location,
                format!(
                    "fragment id {:?} aliases fragment[{previous_index}] id {:?}",
                    fragment.id, previous_id
                ),
            ));
        }
    }

    for (index, fragment) in fragments.iter().enumerate() {
        if !is_portable_fragment_id(&fragment.id) {
            return Err(invalid(
                path,
                format!("fragments[{index}].id"),
                format!(
                    "fragment id {:?} must be a portable ASCII identifier of at most {MAX_FRAGMENT_ID_BYTES} bytes, start and end with an alphanumeric character, use only '.', '-', or '_' internally, and not name a reserved device",
                    fragment.id
                ),
            ));
        }
    }
    Ok(())
}

fn fragment_alias(id: &str) -> String {
    id.nfkc().flat_map(char::to_lowercase).collect::<String>()
}

fn is_portable_fragment_id(id: &str) -> bool {
    if id.len() > MAX_FRAGMENT_ID_BYTES {
        return false;
    }
    let bytes = id.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        || !bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return false;
    }
    let stem = id.split('.').next().expect("an identifier is non-empty");
    !is_reserved_windows_stem(stem)
}

fn is_reserved_windows_stem(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper
            .strip_prefix("COM")
            .or_else(|| upper.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

fn validate_source_path(path: &Path, location: &str, source: &str) -> Result<PathBuf, CliError> {
    if source.trim().is_empty() {
        return Err(invalid(path, location, "source must not be empty"));
    }
    if source.contains('\\') {
        return Err(invalid(
            path,
            location,
            "source must use portable '/' path separators",
        ));
    }
    let source_path = Path::new(source);
    if source_path.is_absolute()
        || source_path
            .components()
            .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
    {
        return Err(invalid(
            path,
            location,
            "source must be relative to the configuration root",
        ));
    }
    if source_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(invalid(
            path,
            location,
            "source must not contain parent components",
        ));
    }
    let supported = source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "md" | "markdown" | "mmd" | "mermaid"));
    if !supported {
        return Err(invalid(
            path,
            location,
            format!(
                "source {} has an unsupported extension; expected .md, .markdown, .mmd, or .mermaid",
                safe_path(source_path)
            ),
        ));
    }
    Ok(source_path.to_path_buf())
}

struct AcquiredText {
    canonical: PathBuf,
    identity: Arc<same_file::Handle>,
    text: String,
}

fn acquire_text(path: &Path, label: &str, limit: InputLimit) -> Result<AcquiredText, CliError> {
    let resource = format!("{label} {}", safe_path(path));
    let file = File::open(path).map_err(|source| {
        CliError::auxiliary_input(if source.kind() == std::io::ErrorKind::NotFound {
            InputReadError::NotFound {
                resource: resource.clone(),
            }
        } else {
            InputReadError::Io {
                resource: resource.clone(),
                source,
            }
        })
    })?;
    let metadata = file.metadata().map_err(|source| {
        CliError::auxiliary_input(InputReadError::Io {
            resource: resource.clone(),
            source,
        })
    })?;
    if !metadata.file_type().is_file() {
        return Err(CliError::auxiliary_input(InputReadError::NotRegularFile {
            resource,
        }));
    }

    let opened_identity = Arc::new(
        same_file::Handle::from_file(
            file.try_clone()
                .map_err(|source| CliError::file(FileOperation::InspectIdentity, path, source))?,
        )
        .map_err(|source| CliError::file(FileOperation::InspectIdentity, path, source))?,
    );
    let canonical = std::fs::canonicalize(path)
        .map_err(|source| CliError::file(FileOperation::Canonicalize, path, source))?;
    let path_identity = same_file::Handle::from_path(&canonical)
        .map_err(|source| CliError::file(FileOperation::InspectIdentity, path, source))?;
    if *opened_identity != path_identity {
        return Err(CliError::file(
            FileOperation::InspectIdentity,
            path,
            std::io::Error::other("file identity changed during Rustdoc acquisition"),
        ));
    }

    let text = read_utf8(file, resource, limit, Some(metadata.len()))
        .map_err(CliError::auxiliary_input)?;
    Ok(AcquiredText {
        canonical,
        identity: opened_identity,
        text,
    })
}

fn invalid(path: &Path, location: impl Into<String>, message: impl Into<String>) -> CliError {
    config_error(path, location, CliError::InvalidInput(message.into()))
}

fn config_error(path: &Path, location: impl Into<String>, source: CliError) -> CliError {
    CliError::rustdoc_config(path, location, source)
}
