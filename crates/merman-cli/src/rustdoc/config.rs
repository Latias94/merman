use crate::error::{CliError, FileOperation, safe_path};
use crate::input::{InputLimit, InputReadError, read_utf8};
use crate::resources::{ByteLedgerKind, CliResourceLimitId, ResolvedResourcePolicy};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;

const SUPPORTED_SCHEMA: u32 = 1;
const MAX_FRAGMENT_ID_BYTES: usize = 120;
const OUTPUT_ROOT: &str = "docs/generated/merman-rustdoc";
pub(super) const RECEIPT_FILE_NAME: &str = "receipt.json";

#[derive(Debug)]
pub(crate) struct Config {
    requested_path: PathBuf,
    path: PathBuf,
    identity: Arc<same_file::Handle>,
    sha256: String,
    root: PathBuf,
    root_identity: Arc<same_file::Handle>,
    output_root: PathBuf,
    fragments: Vec<Fragment>,
    acquired_input_bytes: u64,
}

impl Config {
    pub(super) fn requested_path(&self) -> &Path {
        &self.requested_path
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn file_name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("validated Rustdoc configuration paths have portable filenames")
    }

    pub(crate) fn identity(&self) -> &Arc<same_file::Handle> {
        &self.identity
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
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

    pub(crate) fn adopt_approved_output_root(&mut self, output_root: PathBuf) {
        self.output_root = output_root;
    }

    pub(crate) fn receipt_path(&self) -> PathBuf {
        self.output_root.join(RECEIPT_FILE_NAME)
    }

    pub(crate) fn fragments(&self) -> &[Fragment] {
        &self.fragments
    }

    pub(crate) fn acquired_input_bytes(&self) -> u64 {
        self.acquired_input_bytes
    }
}

#[derive(Debug)]
pub(crate) struct Fragment {
    id: String,
    source: Arc<AcquiredText>,
    logical_source: String,
    source_kind: SourceKind,
    source_display: SourceDisplay,
}

impl Fragment {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn source(&self) -> &Path {
        &self.source.canonical
    }

    pub(crate) fn logical_source(&self) -> &str {
        &self.logical_source
    }

    pub(crate) fn text(&self) -> &str {
        &self.source.text
    }

    pub(crate) fn is_markdown(&self) -> bool {
        self.source_kind == SourceKind::Markdown
    }

    pub(crate) fn source_display(&self) -> SourceDisplay {
        self.source_display
    }

    pub(crate) fn identity(&self) -> &Arc<same_file::Handle> {
        &self.source.identity
    }

    pub(super) fn acquired(&self) -> &Arc<AcquiredText> {
        &self.source
    }

    pub(crate) fn output(&self, output_root: &Path) -> PathBuf {
        output_root.join(format!("{}.md", self.id))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SourceDisplay {
    #[default]
    Hide,
    Details,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    Markdown,
    Mermaid,
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
    let mut fragments = Vec::with_capacity(raw.fragments.len());
    let mut source_cache = HashMap::<String, Arc<AcquiredText>>::new();
    let mut source_aliases = HashMap::<String, (String, usize)>::new();
    let mut source_identities = HashMap::<Arc<same_file::Handle>, (String, usize)>::new();
    let config_name = acquired
        .canonical
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            invalid(
                requested_path,
                "path",
                "configuration filename must be portable UTF-8",
            )
        })?;
    validate_portable_logical_path(config_name).map_err(|reason| {
        invalid(
            requested_path,
            "path",
            format!("configuration filename {reason}"),
        )
    })?;
    let config_alias = portable_path_alias(config_name);
    let mut acquired_input_bytes = 0_u64;
    let mut input_bytes = resources.checked_bytes(ByteLedgerKind::RustdocInput);
    for (index, fragment) in raw.fragments.into_iter().enumerate() {
        let location = format!("fragments[{index}].source");
        let (relative, source_kind) =
            validate_source_path(requested_path, &location, &fragment.source)?;
        let logical_source = portable_relative_path(&relative);
        let alias = portable_path_alias(&logical_source);
        if alias == config_alias {
            return Err(invalid(
                requested_path,
                &location,
                format!(
                    "source {logical_source:?} aliases the Rustdoc configuration filename {config_name:?} under portable path folding"
                ),
            ));
        }
        if let Some((previous_source, previous_index)) = source_aliases.get(&alias)
            && previous_source != &logical_source
        {
            return Err(invalid(
                requested_path,
                &location,
                format!(
                    "source {logical_source:?} aliases fragments[{previous_index}].source {previous_source:?} under portable path folding"
                ),
            ));
        }
        source_aliases
            .entry(alias)
            .or_insert_with(|| (logical_source.clone(), index));

        let source = if let Some(source) = source_cache.get(&logical_source) {
            Arc::clone(source)
        } else {
            let source_path = root.join(&relative);
            let source = Arc::new(
                acquire_rooted_text(
                    &source_path,
                    "Rustdoc fragment source",
                    fragment_source_limit(&relative, resources),
                    &root,
                )
                .map_err(|source| config_error(requested_path, &location, source))?,
            );
            let source_bytes = u64::try_from(source.text.len()).map_err(|_| {
                invalid(
                    requested_path,
                    &location,
                    "source byte length does not fit u64",
                )
            })?;
            input_bytes
                .try_add(source_bytes)
                .map_err(|error| config_error(requested_path, &location, error.into()))?;
            acquired_input_bytes = acquired_input_bytes
                .checked_add(source_bytes)
                .expect("the checked Rustdoc input ledger already rejected overflow");
            if source.identity == acquired.identity {
                return Err(invalid(
                    requested_path,
                    &location,
                    format!(
                        "source {logical_source:?} aliases the Rustdoc configuration by file identity"
                    ),
                ));
            }
            if let Some((previous_source, previous_index)) = source_identities.get(&source.identity)
            {
                return Err(invalid(
                    requested_path,
                    &location,
                    format!(
                        "source {logical_source:?} aliases fragments[{previous_index}].source {previous_source:?} by file identity"
                    ),
                ));
            }
            source_identities.insert(
                Arc::clone(&source.identity),
                (logical_source.clone(), index),
            );
            source_cache.insert(logical_source.clone(), Arc::clone(&source));
            source
        };
        fragments.push(Fragment {
            id: fragment.id,
            source,
            logical_source,
            source_kind,
            source_display: fragment.source_display.unwrap_or_default(),
        });
    }

    Ok(Config {
        requested_path: acquired.requested,
        path: acquired.canonical,
        identity: acquired.identity,
        sha256: acquired.sha256,
        output_root: root.join(OUTPUT_ROOT),
        root,
        root_identity,
        fragments,
        acquired_input_bytes,
    })
}

pub(super) fn fragment_source_limit(path: &Path, resources: &ResolvedResourcePolicy) -> InputLimit {
    if source_kind(path) == Some(SourceKind::Markdown) {
        InputLimit::new(
            CliResourceLimitId::MaxMarkdownDocumentBytes.as_str(),
            resources.files().markdown_document_bytes,
        )
    } else {
        InputLimit::new(
            merman::resources::InputResourceLimitId::MaxSourceBytes.as_str(),
            resources
                .input_policy()
                .value(merman::resources::InputResourceLimitId::MaxSourceBytes),
        )
    }
}

fn source_kind(path: &Path) -> Option<SourceKind> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("md" | "markdown") => Some(SourceKind::Markdown),
        Some("mmd" | "mermaid") => Some(SourceKind::Mermaid),
        _ => None,
    }
}

pub(super) fn portable_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            Component::CurDir => None,
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
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
    let column = prefix.rsplit_once('\n').map_or_else(
        || prefix.chars().count() + 1,
        |(_, tail)| tail.chars().count() + 1,
    );
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

pub(super) fn is_portable_fragment_id(id: &str) -> bool {
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

fn validate_source_path(
    path: &Path,
    location: &str,
    source: &str,
) -> Result<(PathBuf, SourceKind), CliError> {
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
    validate_portable_logical_path(source)
        .map_err(|reason| invalid(path, location, format!("source {reason}")))?;
    let Some(source_kind) = source_kind(source_path) else {
        return Err(invalid(
            path,
            location,
            format!(
                "source {} has an unsupported extension; expected .md, .markdown, .mmd, or .mermaid",
                safe_path(source_path)
            ),
        ));
    };
    Ok((source_path.to_path_buf(), source_kind))
}

pub(super) fn validate_portable_logical_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(|character| {
            character.is_control() || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*')
        })
    {
        return Err("must be a portable relative path".to_string());
    }
    for component in path.split('/') {
        if component.is_empty() || matches!(component, "." | "..") {
            return Err("must be a normalized portable relative path".to_string());
        }
        if component.ends_with(['.', ' ']) {
            return Err(format!(
                "component {component:?} must not end with a dot or space"
            ));
        }
        let stem = component.split('.').next().unwrap_or(component);
        if is_reserved_windows_stem(stem) {
            return Err(format!(
                "component {component:?} names a reserved Windows device"
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct AcquiredText {
    pub(super) requested: PathBuf,
    pub(super) canonical: PathBuf,
    pub(super) identity: Arc<same_file::Handle>,
    pub(super) text: Arc<str>,
    pub(super) sha256: String,
}

pub(super) fn acquire_text(
    path: &Path,
    label: &str,
    limit: InputLimit,
) -> Result<AcquiredText, CliError> {
    acquire_text_with_scope(path, label, limit, None)
}

pub(super) fn acquire_rooted_text(
    path: &Path,
    label: &str,
    limit: InputLimit,
    root: &Path,
) -> Result<AcquiredText, CliError> {
    acquire_text_with_scope(path, label, limit, Some(root))
}

fn acquire_text_with_scope(
    path: &Path,
    label: &str,
    limit: InputLimit,
    root: Option<&Path>,
) -> Result<AcquiredText, CliError> {
    let resource = format!("{label} {}", safe_path(path));
    let canonical = std::fs::canonicalize(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            CliError::auxiliary_input(InputReadError::NotFound {
                resource: resource.clone(),
            })
        } else {
            CliError::file(FileOperation::Canonicalize, path, source)
        }
    })?;
    if let Some(root) = root
        && canonical.strip_prefix(root).is_err()
    {
        return Err(CliError::InvalidInput(format!(
            "{label} {} escapes the configuration root {}",
            safe_path(path),
            safe_path(root)
        )));
    }
    let path_metadata = std::fs::symlink_metadata(&canonical).map_err(|source| {
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
    if !path_metadata.file_type().is_file() {
        return Err(CliError::auxiliary_input(InputReadError::NotRegularFile {
            resource,
        }));
    }
    let file = File::open(&canonical).map_err(|source| {
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
    let path_identity = same_file::Handle::from_path(&canonical)
        .map_err(|source| CliError::file(FileOperation::InspectIdentity, path, source))?;
    if *opened_identity != path_identity {
        return Err(CliError::file(
            FileOperation::InspectIdentity,
            path,
            std::io::Error::other("file identity changed during Rustdoc acquisition"),
        ));
    }

    let text: Arc<str> = read_utf8(file, resource, limit, Some(metadata.len()))
        .map_err(CliError::auxiliary_input)?
        .into();
    let current_canonical = std::fs::canonicalize(path)
        .map_err(|source| CliError::file(FileOperation::Canonicalize, path, source))?;
    let current_identity = same_file::Handle::from_path(&current_canonical)
        .map_err(|source| CliError::file(FileOperation::InspectIdentity, path, source))?;
    if current_canonical != canonical || current_identity != *opened_identity {
        return Err(CliError::file(
            FileOperation::InspectIdentity,
            path,
            std::io::Error::other("file identity changed during Rustdoc acquisition"),
        ));
    }
    let sha256 = super::sha256_hex(text.as_bytes());
    Ok(AcquiredText {
        requested: path.to_path_buf(),
        canonical,
        identity: opened_identity,
        text,
        sha256,
    })
}

pub(super) fn portable_path_alias(path: &str) -> String {
    path.nfkc().flat_map(char::to_lowercase).collect()
}

fn invalid(path: &Path, location: impl Into<String>, message: impl Into<String>) -> CliError {
    config_error(path, location, CliError::InvalidInput(message.into()))
}

fn config_error(path: &Path, location: impl Into<String>, source: CliError) -> CliError {
    CliError::rustdoc_config(path, location, source)
}

#[cfg(test)]
mod tests {
    use super::{line_column, load};
    use crate::resources::ResolvedResourcePolicy;
    use std::fs;

    #[test]
    fn diagnostic_columns_count_unicode_scalars_instead_of_utf8_bytes() {
        let text = "first\n文x";
        let offset = "first\n文".len();

        assert_eq!(line_column(text, offset), "line 2, column 2");
    }

    #[test]
    fn unique_sources_share_one_aggregate_input_budget() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("one.mmd"), "1234").unwrap();
        fs::write(root.path().join("two.mmd"), "5678").unwrap();
        fs::write(
            root.path().join("merman-rustdoc.toml"),
            concat!(
                "schema = 1\n",
                "[[fragments]]\nid = \"one\"\nsource = \"one.mmd\"\n",
                "[[fragments]]\nid = \"two\"\nsource = \"two.mmd\"\n",
            ),
        )
        .unwrap();
        let mut resources =
            ResolvedResourcePolicy::for_profile(merman::resources::CLI_DEFAULT_RESOURCE_PROFILE);
        resources
            .apply_override("max_rustdoc_input_bytes", 7)
            .unwrap();

        let error = load(&root.path().join("merman-rustdoc.toml"), &resources).unwrap_err();

        assert!(error.to_string().contains("max_rustdoc_input_bytes"));
    }

    #[test]
    fn repeated_source_is_charged_once_against_the_aggregate_budget() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("shared.mmd"), "1234").unwrap();
        fs::write(
            root.path().join("merman-rustdoc.toml"),
            concat!(
                "schema = 1\n",
                "[[fragments]]\nid = \"one\"\nsource = \"shared.mmd\"\n",
                "[[fragments]]\nid = \"two\"\nsource = \"shared.mmd\"\n",
            ),
        )
        .unwrap();
        let mut resources =
            ResolvedResourcePolicy::for_profile(merman::resources::CLI_DEFAULT_RESOURCE_PROFILE);
        resources
            .apply_override("max_rustdoc_input_bytes", 4)
            .unwrap();

        let config = load(&root.path().join("merman-rustdoc.toml"), &resources).unwrap();

        assert_eq!(config.acquired_input_bytes(), 4);
    }
}
