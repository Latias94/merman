use super::TransactionError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

pub(super) const JOURNAL_SCHEMA: &str = "merman-transaction";
pub(super) const JOURNAL_VERSION: u32 = 2;
const MANIFEST_SCHEMA: &str = "merman-generation";
const MANIFEST_VERSION: u32 = 2;
pub(super) const MAX_STATE_BYTES: u64 = 1024 * 1024;
const MAX_ENTRIES: usize = 65_536;
const MAX_MANIFEST_ARTIFACTS: usize = MAX_ENTRIES - 1;
const MAX_COMPONENTS: usize = 1_024;
const MAX_ENCODED_COMPONENT_BYTES: usize = 8_192;
const RESERVED_COMPONENTS: [&str; 2] = [".merman.lock", ".merman.transaction"];

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct RelativeTarget {
    components: Vec<OsString>,
}

impl RelativeTarget {
    pub(crate) fn from_absolute(
        root: impl AsRef<Path>,
        target: impl AsRef<Path>,
    ) -> Result<Self, TransactionError> {
        let requested_root = root.as_ref();
        let root = std::fs::canonicalize(requested_root).map_err(|source| {
            TransactionError::operational("canonicalize transaction root", requested_root, source)
        })?;
        let target = target.as_ref();
        if !target.is_absolute() {
            return Err(TransactionError::invalid_state(
                &root,
                format!("transaction target must be absolute: {target:?}"),
            ));
        }
        let relative = target
            .strip_prefix(requested_root)
            .or_else(|_| target.strip_prefix(&root))
            .map_err(|_| {
                TransactionError::invalid_state(
                    &root,
                    format!("transaction target escapes canonical root: {target:?}"),
                )
            })?;
        Self::from_relative_path(relative, &root)
    }

    pub(crate) fn to_path(&self, root: impl AsRef<Path>) -> Result<PathBuf, TransactionError> {
        self.validate()?;
        let mut result = root.as_ref().to_path_buf();
        for component in &self.components {
            result.push(component);
        }
        Ok(result)
    }

    #[cfg(test)]
    pub(crate) fn components(&self) -> impl ExactSizeIterator<Item = &OsStr> {
        self.components.iter().map(OsString::as_os_str)
    }

    #[cfg(test)]
    pub(crate) fn order(&self) -> &[OsString] {
        &self.components
    }

    fn from_relative_path(relative: &Path, evidence: &Path) -> Result<Self, TransactionError> {
        let components = relative
            .components()
            .map(|component| match component {
                Component::Normal(value) => Ok(value.to_os_string()),
                _ => Err(TransactionError::invalid_state(
                    evidence,
                    format!("transaction target contains a forbidden component: {relative:?}"),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_components(components, evidence)
    }

    pub(super) fn from_components(
        components: Vec<OsString>,
        _evidence: &Path,
    ) -> Result<Self, TransactionError> {
        let result = Self { components };
        result.validate()?;
        Ok(result)
    }

    pub(super) fn from_encoded(
        components: Vec<String>,
        encoding: PathEncoding,
        evidence: &Path,
    ) -> Result<Self, TransactionError> {
        if encoding != PathEncoding::native() {
            return Err(TransactionError::invalid_state(
                evidence,
                format!(
                    "journal path encoding {:?} does not match this host ({:?})",
                    encoding,
                    PathEncoding::native()
                ),
            ));
        }
        if components.is_empty() || components.len() > MAX_COMPONENTS {
            return Err(TransactionError::invalid_state(
                evidence,
                "transaction target has an invalid component count",
            ));
        }
        let decoded = components
            .into_iter()
            .map(|component| decode_component(&component, evidence))
            .collect::<Result<Vec<_>, _>>()?;
        let result = Self {
            components: decoded,
        };
        result.validate()?;
        Ok(result)
    }

    pub(super) fn encoded(&self) -> Result<Vec<String>, TransactionError> {
        self.validate()?;
        self.components
            .iter()
            .map(|component| encode_component(component))
            .collect()
    }

    fn collision_key(&self) -> Result<Vec<String>, TransactionError> {
        self.validate()?;
        self.components
            .iter()
            .map(|component| {
                if let Some(component) = component.to_str() {
                    let normalized = component
                        .nfc()
                        .flat_map(char::to_lowercase)
                        .collect::<String>();
                    encode_component(OsStr::new(&normalized))
                } else {
                    encode_ascii_casefolded_component(component)
                }
            })
            .collect()
    }

    fn validate(&self) -> Result<(), TransactionError> {
        if self.components.is_empty() || self.components.len() > MAX_COMPONENTS {
            return Err(TransactionError::invalid_state(
                Path::new(".merman.transaction"),
                "transaction target has an invalid component count",
            ));
        }
        for component in &self.components {
            validate_component(component)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GenerationDialect {
    NativeBatchV1,
    Mmdc11_16_0,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactNamespace {
    directory: Vec<OsString>,
    stem: OsString,
    extension: OsString,
}

impl ArtifactNamespace {
    pub(crate) fn from_absolute(
        root: impl AsRef<Path>,
        directory: impl AsRef<Path>,
        stem: impl AsRef<OsStr>,
        extension: impl AsRef<OsStr>,
    ) -> Result<Self, TransactionError> {
        let requested_root = root.as_ref();
        let root = std::fs::canonicalize(requested_root).map_err(|source| {
            TransactionError::operational("canonicalize transaction root", requested_root, source)
        })?;
        let directory = directory.as_ref();
        if !directory.is_absolute() {
            return Err(TransactionError::invalid_state(
                &root,
                format!("artifact namespace directory must be absolute: {directory:?}"),
            ));
        }
        let relative = directory
            .strip_prefix(requested_root)
            .or_else(|_| directory.strip_prefix(&root))
            .map_err(|_| {
                TransactionError::invalid_state(
                    &root,
                    format!("artifact namespace escapes canonical root: {directory:?}"),
                )
            })?;
        let directory = relative
            .components()
            .map(|component| match component {
                Component::Normal(value) => Ok(value.to_os_string()),
                _ => Err(TransactionError::invalid_state(
                    &root,
                    format!("artifact namespace contains a forbidden component: {directory:?}"),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let namespace = Self {
            directory,
            stem: stem.as_ref().to_os_string(),
            extension: extension.as_ref().to_os_string(),
        };
        namespace.validate(&root)?;
        Ok(namespace)
    }

    pub(crate) fn target(&self, index: usize) -> Result<RelativeTarget, TransactionError> {
        if index == 0 {
            return Err(TransactionError::invalid_state(
                Path::new(".merman-manifest.json"),
                "artifact namespace indices start at one",
            ));
        }
        let mut components = self.directory.clone();
        components.push(numbered_file_name(&self.stem, index, &self.extension)?);
        RelativeTarget::from_components(components, Path::new(".merman-manifest.json"))
    }

    pub(super) fn validate(&self, evidence: &Path) -> Result<(), TransactionError> {
        if self.directory.len() >= MAX_COMPONENTS
            || self.stem.is_empty()
            || self.extension.is_empty()
        {
            return Err(TransactionError::invalid_state(
                evidence,
                "artifact namespace has invalid directory, stem, or extension",
            ));
        }
        for component in &self.directory {
            validate_component(component)?;
        }
        let _ = self.target(1)?;
        Ok(())
    }

    fn validate_artifacts(
        &self,
        artifacts: &[RelativeTarget],
        evidence: &Path,
    ) -> Result<(), TransactionError> {
        let actual = artifacts.iter().collect::<HashSet<_>>();
        if actual.len() != artifacts.len() {
            return Err(TransactionError::invalid_state(
                evidence,
                "generation manifest contains duplicate artifacts",
            ));
        }
        for index in 1..=artifacts.len() {
            let expected = self.target(index)?;
            if !actual.contains(&expected) {
                return Err(TransactionError::invalid_state(
                    evidence,
                    "generation manifest artifacts are not exactly one contiguous numbered namespace",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn has_same_series_as(&self, other: &Self) -> bool {
        self.directory == other.directory && self.stem == other.stem
    }

    pub(crate) fn extension(&self) -> &OsStr {
        &self.extension
    }

    fn encoded(&self) -> Result<ArtifactNamespaceWire, TransactionError> {
        Ok(ArtifactNamespaceWire {
            directory: self
                .directory
                .iter()
                .map(|component| encode_component(component))
                .collect::<Result<Vec<_>, _>>()?,
            stem: encode_component(&self.stem)?,
            extension: encode_component(&self.extension)?,
        })
    }

    fn from_encoded(
        wire: ArtifactNamespaceWire,
        encoding: PathEncoding,
        evidence: &Path,
    ) -> Result<Self, TransactionError> {
        let namespace = Self {
            directory: wire
                .directory
                .into_iter()
                .map(|component| decode_component(&component, evidence))
                .collect::<Result<Vec<_>, _>>()?,
            stem: decode_component(&wire.stem, evidence)?,
            extension: decode_component(&wire.extension, evidence)?,
        };
        if encoding != PathEncoding::native() {
            return Err(TransactionError::invalid_state(
                evidence,
                "artifact namespace path encoding does not match this host",
            ));
        }
        namespace.validate(evidence)?;
        Ok(namespace)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerationOwner {
    dialect: GenerationDialect,
    owner: RelativeTarget,
    namespace: ArtifactNamespace,
}

impl GenerationOwner {
    pub(crate) fn new(
        dialect: GenerationDialect,
        owner: RelativeTarget,
        namespace: ArtifactNamespace,
    ) -> Result<Self, TransactionError> {
        owner.validate()?;
        namespace.validate(Path::new(".merman-manifest.json"))?;
        Ok(Self {
            dialect,
            owner,
            namespace,
        })
    }

    pub(crate) fn namespace(&self) -> &ArtifactNamespace {
        &self.namespace
    }

    pub(crate) fn dialect(&self) -> GenerationDialect {
        self.dialect
    }

    pub(crate) fn has_same_subject_as(&self, other: &Self) -> bool {
        self.dialect == other.dialect && self.owner == other.owner
    }

    pub(super) fn validate(&self, evidence: &Path) -> Result<(), TransactionError> {
        self.owner.validate()?;
        self.namespace.validate(evidence)
    }

    fn encoded(&self) -> Result<GenerationOwnerWire, TransactionError> {
        Ok(GenerationOwnerWire {
            dialect: self.dialect,
            owner: self.owner.encoded()?,
            namespace: self.namespace.encoded()?,
        })
    }

    fn from_encoded(
        wire: GenerationOwnerWire,
        encoding: PathEncoding,
        evidence: &Path,
    ) -> Result<Self, TransactionError> {
        Self::new(
            wire.dialect,
            RelativeTarget::from_encoded(wire.owner, encoding, evidence)?,
            ArtifactNamespace::from_encoded(wire.namespace, encoding, evidence)?,
        )
    }

    #[cfg(test)]
    pub(super) fn test_fixture() -> Self {
        Self::new(
            GenerationDialect::NativeBatchV1,
            RelativeTarget::from_components(
                vec![OsString::from("document.md")],
                Path::new(".merman-manifest.json"),
            )
            .expect("test owner target is valid"),
            ArtifactNamespace {
                directory: Vec::new(),
                stem: OsString::from("artifact"),
                extension: OsString::from("svg"),
            },
        )
        .expect("test generation owner is valid")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum PathEncoding {
    #[serde(rename = "unix-bytes-hex")]
    UnixBytes,
    #[serde(rename = "windows-utf16-hex")]
    WindowsUtf16,
    #[serde(rename = "utf8-hex")]
    Utf8Hex,
}

impl PathEncoding {
    pub(super) const fn native() -> Self {
        #[cfg(unix)]
        {
            Self::UnixBytes
        }
        #[cfg(windows)]
        {
            Self::WindowsUtf16
        }
        #[cfg(not(any(unix, windows)))]
        {
            Self::Utf8Hex
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TransactionRole {
    Artifact,
    Manifest,
    Document,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TransactionOperation {
    Write,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PriorState {
    Unknown,
    Missing,
    Present,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum JournalPhase {
    Staging,
    BackingUp,
    Publishing,
    RollingBack,
    RolledBack,
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct JournalEntry {
    pub(super) role: TransactionRole,
    pub(super) operation: TransactionOperation,
    pub(super) target: RelativeTarget,
    pub(super) prior: PriorState,
    pub(super) prior_mode: Option<u32>,
}

#[derive(Debug, Clone)]
pub(super) struct JournalState {
    pub(super) id: String,
    pub(super) owner: GenerationOwner,
    pub(super) sequence: u64,
    pub(super) phase: JournalPhase,
    pub(super) next_index: usize,
    pub(super) entries: Vec<JournalEntry>,
}

impl JournalState {
    pub(super) fn encode(&self, evidence: &Path) -> Result<Vec<u8>, TransactionError> {
        self.validate(evidence)?;
        let wire = JournalWire {
            schema: JOURNAL_SCHEMA.to_owned(),
            version: JOURNAL_VERSION,
            id: self.id.clone(),
            owner: self.owner.encoded()?,
            path_encoding: PathEncoding::native(),
            sequence: self.sequence,
            phase: self.phase,
            next_index: self.next_index,
            entries: self
                .entries
                .iter()
                .map(|entry| {
                    Ok(JournalEntryWire {
                        role: entry.role,
                        operation: entry.operation,
                        target: entry.target.encoded()?,
                        prior: entry.prior,
                        prior_mode: entry.prior_mode,
                    })
                })
                .collect::<Result<Vec<_>, TransactionError>>()?,
        };
        serde_json::to_vec(&wire).map_err(|source| {
            TransactionError::invalid_state(
                evidence,
                format!("failed to encode transaction journal: {source}"),
            )
        })
    }

    pub(super) fn decode_json_value(
        value: serde_json::Value,
        evidence: &Path,
    ) -> Result<Self, TransactionError> {
        let wire: JournalWire = serde_json::from_value(value).map_err(|source| {
            TransactionError::invalid_state(
                evidence,
                format!("transaction journal schema is invalid: {source}"),
            )
        })?;
        if wire.schema != JOURNAL_SCHEMA {
            return Err(TransactionError::invalid_state(
                evidence,
                format!("unsupported transaction journal schema {:?}", wire.schema),
            ));
        }
        if wire.version != JOURNAL_VERSION {
            return Err(TransactionError::invalid_state(
                evidence,
                format!("unsupported transaction journal version {}", wire.version),
            ));
        }
        if wire.path_encoding != PathEncoding::native() {
            return Err(TransactionError::invalid_state(
                evidence,
                format!(
                    "journal path encoding {:?} does not match this host ({:?})",
                    wire.path_encoding,
                    PathEncoding::native()
                ),
            ));
        }
        let state = Self {
            id: wire.id,
            owner: GenerationOwner::from_encoded(wire.owner, wire.path_encoding, evidence)?,
            sequence: wire.sequence,
            phase: wire.phase,
            next_index: wire.next_index,
            entries: wire
                .entries
                .into_iter()
                .map(|entry| {
                    Ok(JournalEntry {
                        role: entry.role,
                        operation: entry.operation,
                        target: RelativeTarget::from_encoded(
                            entry.target,
                            wire.path_encoding,
                            evidence,
                        )?,
                        prior: entry.prior,
                        prior_mode: entry.prior_mode,
                    })
                })
                .collect::<Result<Vec<_>, TransactionError>>()?,
        };
        state.validate(evidence)?;
        Ok(state)
    }

    pub(super) fn validate(&self, evidence: &Path) -> Result<(), TransactionError> {
        validate_identifier(&self.id, evidence, "transaction")?;
        self.owner.validate(evidence)?;
        validate_entries(&self.entries, evidence)?;
        for entry in &self.entries {
            validate_prior_mode(entry, evidence)?;
        }
        if self.next_index > self.entries.len() {
            return Err(TransactionError::invalid_state(
                evidence,
                "transaction journal cursor exceeds its entry count",
            ));
        }
        match self.phase {
            JournalPhase::Staging | JournalPhase::BackingUp => {
                if self.next_index != 0
                    || self
                        .entries
                        .iter()
                        .any(|entry| entry.prior != PriorState::Unknown)
                {
                    return Err(TransactionError::invalid_state(
                        evidence,
                        "pre-publication journal has an invalid cursor or prior state",
                    ));
                }
            }
            JournalPhase::Publishing | JournalPhase::RollingBack => {
                if self
                    .entries
                    .iter()
                    .any(|entry| entry.prior == PriorState::Unknown)
                {
                    return Err(TransactionError::invalid_state(
                        evidence,
                        "publication journal is missing a prior state",
                    ));
                }
            }
            JournalPhase::RolledBack | JournalPhase::Committed => {
                let expected_index = if self.phase == JournalPhase::Committed {
                    self.entries.len()
                } else {
                    0
                };
                if self.next_index != expected_index
                    || self
                        .entries
                        .iter()
                        .any(|entry| entry.prior == PriorState::Unknown)
                {
                    return Err(TransactionError::invalid_state(
                        evidence,
                        "terminal journal has an invalid cursor or prior state",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn compatible_with(
        &self,
        other: &Self,
        evidence: &Path,
    ) -> Result<(), TransactionError> {
        if self.id != other.id
            || self.owner != other.owner
            || self.entries.len() != other.entries.len()
        {
            return Err(TransactionError::invalid_state(
                evidence,
                "journal slots describe different transactions",
            ));
        }
        for (left, right) in self.entries.iter().zip(&other.entries) {
            if left.role != right.role
                || left.operation != right.operation
                || left.target != right.target
                || (left.prior != PriorState::Unknown
                    && right.prior != PriorState::Unknown
                    && left.prior != right.prior)
                || (left.prior_mode.is_some()
                    && right.prior_mode.is_some()
                    && left.prior_mode != right.prior_mode)
            {
                return Err(TransactionError::invalid_state(
                    evidence,
                    "journal slots describe incompatible transaction entries",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn validate_successor(
        &self,
        newer: &Self,
        evidence: &Path,
    ) -> Result<(), TransactionError> {
        self.compatible_with(newer, evidence)?;
        if self.sequence.checked_add(1) != Some(newer.sequence) {
            return Err(TransactionError::invalid_state(
                evidence,
                "journal slot sequences are not consecutive",
            ));
        }
        let allowed = match (self.phase, newer.phase) {
            (JournalPhase::Staging, JournalPhase::BackingUp) => true,
            (JournalPhase::BackingUp, JournalPhase::Publishing) => true,
            (JournalPhase::Publishing, JournalPhase::Publishing) => {
                newer.next_index >= self.next_index
                    && newer.next_index <= self.next_index.saturating_add(1)
            }
            (JournalPhase::Publishing, JournalPhase::RollingBack) => {
                newer.next_index >= self.next_index
                    && newer.next_index <= self.next_index.saturating_add(1)
            }
            (JournalPhase::Publishing, JournalPhase::Committed) => {
                newer.next_index == newer.entries.len()
            }
            (JournalPhase::RollingBack, JournalPhase::RollingBack) => {
                newer.next_index <= self.next_index
            }
            (JournalPhase::RollingBack, JournalPhase::RolledBack) => newer.next_index == 0,
            _ => false,
        };
        if !allowed {
            return Err(TransactionError::invalid_state(
                evidence,
                format!(
                    "journal slots contain an invalid transition from {:?} to {:?}",
                    self.phase, newer.phase
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalWire {
    schema: String,
    version: u32,
    id: String,
    owner: GenerationOwnerWire,
    path_encoding: PathEncoding,
    sequence: u64,
    phase: JournalPhase,
    next_index: usize,
    entries: Vec<JournalEntryWire>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerationOwnerWire {
    dialect: GenerationDialect,
    owner: Vec<String>,
    namespace: ArtifactNamespaceWire,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactNamespaceWire {
    directory: Vec<String>,
    stem: String,
    extension: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalEntryWire {
    role: TransactionRole,
    operation: TransactionOperation,
    target: Vec<String>,
    prior: PriorState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prior_mode: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerationManifest {
    generation_id: String,
    owner: GenerationOwner,
    document: Option<RelativeTarget>,
    artifacts: Vec<RelativeTarget>,
}

impl GenerationManifest {
    pub(crate) fn new(
        generation_id: impl Into<String>,
        owner: GenerationOwner,
        document: Option<RelativeTarget>,
        mut artifacts: Vec<RelativeTarget>,
    ) -> Result<Self, TransactionError> {
        let generation_id = generation_id.into();
        validate_identifier(
            &generation_id,
            Path::new(".merman-manifest.json"),
            "generation",
        )?;
        owner.validate(Path::new(".merman-manifest.json"))?;
        if artifacts.len() > MAX_MANIFEST_ARTIFACTS {
            return Err(TransactionError::invalid_state(
                Path::new(".merman-manifest.json"),
                "generation manifest has too many artifacts",
            ));
        }
        owner
            .namespace
            .validate_artifacts(&artifacts, Path::new(".merman-manifest.json"))?;
        artifacts.sort();
        if artifacts.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(TransactionError::invalid_state(
                Path::new(".merman-manifest.json"),
                "generation manifest contains duplicate artifacts",
            ));
        }
        if document
            .as_ref()
            .is_some_and(|document| artifacts.contains(document))
        {
            return Err(TransactionError::invalid_state(
                Path::new(".merman-manifest.json"),
                "generation manifest document is also listed as an artifact",
            ));
        }
        for target in artifacts.iter().chain(document.iter()) {
            target.validate()?;
        }
        let mut collision_keys =
            HashSet::with_capacity(artifacts.len() + usize::from(document.is_some()));
        for target in artifacts.iter().chain(document.iter()) {
            if !collision_keys.insert(target.collision_key()?) {
                return Err(TransactionError::invalid_state(
                    Path::new(".merman-manifest.json"),
                    "generation manifest contains targets that collide under portable Unicode normalization and case folding",
                ));
            }
        }
        Ok(Self {
            generation_id,
            owner,
            document,
            artifacts,
        })
    }

    #[cfg(test)]
    pub(crate) fn generation_id(&self) -> &str {
        &self.generation_id
    }

    pub(crate) fn owner(&self) -> &GenerationOwner {
        &self.owner
    }

    pub(crate) fn document(&self) -> Option<&RelativeTarget> {
        self.document.as_ref()
    }

    pub(crate) fn artifacts(&self) -> &[RelativeTarget] {
        &self.artifacts
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, TransactionError> {
        let wire = ManifestWire {
            schema: MANIFEST_SCHEMA.to_owned(),
            version: MANIFEST_VERSION,
            generation_id: self.generation_id.clone(),
            owner: self.owner.encoded()?,
            path_encoding: PathEncoding::native(),
            document: self
                .document
                .as_ref()
                .map(RelativeTarget::encoded)
                .transpose()?,
            artifacts: self
                .artifacts
                .iter()
                .map(RelativeTarget::encoded)
                .collect::<Result<Vec<_>, _>>()?,
        };
        let bytes = serde_json::to_vec_pretty(&wire).map_err(|source| {
            TransactionError::invalid_state(
                Path::new(".merman-manifest.json"),
                format!("failed to encode generation manifest: {source}"),
            )
        })?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err(TransactionError::invalid_state(
                Path::new(".merman-manifest.json"),
                "generation manifest exceeds the hard state-size limit",
            ));
        }
        Ok(bytes)
    }

    pub(crate) fn decode_bounded(
        bytes: &[u8],
        root: impl AsRef<Path>,
    ) -> Result<Self, TransactionError> {
        let root = root.as_ref();
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err(TransactionError::invalid_state(
                root,
                "generation manifest exceeds the hard state-size limit",
            ));
        }
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|source| {
            TransactionError::invalid_state(
                root,
                format!("generation manifest JSON is malformed: {source}"),
            )
        })?;
        let wire: ManifestWire = serde_json::from_value(value).map_err(|source| {
            TransactionError::invalid_state(
                root,
                format!("generation manifest schema is invalid: {source}"),
            )
        })?;
        if wire.schema != MANIFEST_SCHEMA || wire.version != MANIFEST_VERSION {
            return Err(TransactionError::invalid_state(
                root,
                format!(
                    "unsupported generation manifest schema/version {:?}/{}",
                    wire.schema, wire.version
                ),
            ));
        }
        if wire.path_encoding != PathEncoding::native() {
            return Err(TransactionError::invalid_state(
                root,
                "generation manifest path encoding does not match this host",
            ));
        }
        if wire.artifacts.len() > MAX_MANIFEST_ARTIFACTS {
            return Err(TransactionError::invalid_state(
                root,
                "generation manifest has too many artifacts",
            ));
        }
        let owner = GenerationOwner::from_encoded(wire.owner, wire.path_encoding, root)?;
        let document = wire
            .document
            .map(|target| RelativeTarget::from_encoded(target, wire.path_encoding, root))
            .transpose()?;
        let artifacts = wire
            .artifacts
            .into_iter()
            .map(|target| RelativeTarget::from_encoded(target, wire.path_encoding, root))
            .collect::<Result<Vec<_>, _>>()?;
        let manifest = Self::new(wire.generation_id, owner, document, artifacts)?;
        for target in manifest.artifacts.iter().chain(manifest.document.iter()) {
            let _ = target.to_path(root)?;
        }
        Ok(manifest)
    }

    pub(crate) fn read_bounded(
        path: impl AsRef<Path>,
        root: impl AsRef<Path>,
    ) -> Result<Self, TransactionError> {
        let path = path.as_ref();
        let mut file = open_regular_for_read(path)?;
        let bytes = read_at_most(&mut file, MAX_STATE_BYTES, path)?;
        Self::decode_bounded(&bytes, root)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestWire {
    schema: String,
    version: u32,
    generation_id: String,
    owner: GenerationOwnerWire,
    path_encoding: PathEncoding,
    document: Option<Vec<String>>,
    artifacts: Vec<Vec<String>>,
}

pub(super) fn validate_entries(
    entries: &[JournalEntry],
    evidence: &Path,
) -> Result<(), TransactionError> {
    if entries.is_empty() || entries.len() > MAX_ENTRIES {
        return Err(TransactionError::invalid_state(
            evidence,
            "transaction journal has an invalid entry count",
        ));
    }
    let mut seen = HashSet::with_capacity(entries.len());
    let mut collision_keys = HashSet::with_capacity(entries.len());
    let mut manifest_count = 0usize;
    let mut document_count = 0usize;
    let mut previous_artifact: Option<&RelativeTarget> = None;
    let mut reached_manifest = false;
    let mut reached_document = false;
    for (index, entry) in entries.iter().enumerate() {
        entry.target.validate()?;
        if !seen.insert(entry.target.clone()) {
            return Err(TransactionError::invalid_state(
                evidence,
                "transaction journal contains duplicate targets",
            ));
        }
        if !collision_keys.insert(entry.target.collision_key()?) {
            return Err(TransactionError::invalid_state(
                evidence,
                "transaction journal contains targets that collide under ASCII case folding",
            ));
        }
        match entry.role {
            TransactionRole::Artifact => {
                if reached_manifest || reached_document {
                    return Err(TransactionError::invalid_state(
                        evidence,
                        "artifact entries must precede manifest and document entries",
                    ));
                }
                if previous_artifact.is_some_and(|previous| previous >= &entry.target) {
                    return Err(TransactionError::invalid_state(
                        evidence,
                        "artifact entries are not in strict lexical order",
                    ));
                }
                previous_artifact = Some(&entry.target);
            }
            TransactionRole::Manifest => {
                reached_manifest = true;
                manifest_count += 1;
                if entry.operation != TransactionOperation::Write || reached_document {
                    return Err(TransactionError::invalid_state(
                        evidence,
                        "manifest must be one write after every artifact",
                    ));
                }
            }
            TransactionRole::Document => {
                reached_document = true;
                document_count += 1;
                if entry.operation != TransactionOperation::Write || index + 1 != entries.len() {
                    return Err(TransactionError::invalid_state(
                        evidence,
                        "document must be an optional final write",
                    ));
                }
            }
        }
        if entry.operation == TransactionOperation::Delete
            && entry.role != TransactionRole::Artifact
        {
            return Err(TransactionError::invalid_state(
                evidence,
                "only artifact entries may delete a target",
            ));
        }
    }
    if manifest_count != 1 || document_count > 1 {
        return Err(TransactionError::invalid_state(
            evidence,
            "transaction requires exactly one manifest and at most one document",
        ));
    }
    Ok(())
}

fn validate_prior_mode(entry: &JournalEntry, evidence: &Path) -> Result<(), TransactionError> {
    match entry.prior {
        PriorState::Unknown | PriorState::Missing if entry.prior_mode.is_some() => {
            Err(TransactionError::invalid_state(
                evidence,
                "journal entry has a mode without a prior file",
            ))
        }
        PriorState::Present => {
            #[cfg(unix)]
            {
                let Some(mode) = entry.prior_mode else {
                    return Err(TransactionError::invalid_state(
                        evidence,
                        "journal entry for a prior file is missing its ordinary Unix mode",
                    ));
                };
                if mode & !0o777 != 0 {
                    return Err(TransactionError::invalid_state(
                        evidence,
                        "journal entry contains non-ordinary Unix mode bits",
                    ));
                }
            }
            #[cfg(not(unix))]
            if entry.prior_mode.is_some() {
                return Err(TransactionError::invalid_state(
                    evidence,
                    "journal entry contains Unix mode bits on a non-Unix host",
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn validate_identifier(
    value: &str,
    evidence: &Path,
    kind: &str,
) -> Result<(), TransactionError> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(TransactionError::invalid_state(
            evidence,
            format!("{kind} id must be exactly 32 lowercase hexadecimal characters"),
        ));
    }
    Ok(())
}

pub(super) fn read_at_most(
    file: &mut File,
    limit: u64,
    path: &Path,
) -> Result<Vec<u8>, TransactionError> {
    let length = file
        .metadata()
        .map_err(|source| TransactionError::operational("inspect state file", path, source))?
        .len();
    if length > limit {
        return Err(TransactionError::invalid_state(
            path,
            format!("state file exceeds the hard {limit}-byte limit"),
        ));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| TransactionError::operational("read state file", path, source))?;
    if bytes.len() as u64 > limit {
        return Err(TransactionError::invalid_state(
            path,
            format!("state file exceeds the hard {limit}-byte limit"),
        ));
    }
    Ok(bytes)
}

fn validate_component(component: &OsStr) -> Result<(), TransactionError> {
    if component.is_empty() {
        return Err(TransactionError::invalid_state(
            Path::new(".merman.transaction"),
            "transaction target contains an empty component",
        ));
    }
    let mut parsed = Path::new(component).components();
    if !matches!(parsed.next(), Some(Component::Normal(value)) if value == component)
        || parsed.next().is_some()
    {
        return Err(TransactionError::invalid_state(
            Path::new(".merman.transaction"),
            format!("transaction target contains a forbidden component: {component:?}"),
        ));
    }
    if RESERVED_COMPONENTS
        .iter()
        .any(|reserved| component_ascii_eq_ignore_case(component, reserved))
    {
        return Err(TransactionError::invalid_state(
            Path::new(".merman.transaction"),
            format!("transaction target uses reserved component {component:?}"),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if component.as_bytes().contains(&0) {
            return Err(TransactionError::invalid_state(
                Path::new(".merman.transaction"),
                "transaction target component contains NUL",
            ));
        }
    }
    let encoded = encode_component(component)?;
    if encoded.is_empty() || encoded.len() > MAX_ENCODED_COMPONENT_BYTES {
        return Err(TransactionError::invalid_state(
            Path::new(".merman.transaction"),
            "transaction target component exceeds the hard encoding limit",
        ));
    }
    #[cfg(windows)]
    validate_windows_component(component)?;
    Ok(())
}

fn numbered_file_name(
    stem: &OsStr,
    index: usize,
    extension: &OsStr,
) -> Result<OsString, TransactionError> {
    let mut name = stem.to_os_string();
    name.push(format!("-{index}."));
    name.push(extension);
    validate_component(&name)?;
    Ok(name)
}

#[cfg(windows)]
fn validate_windows_component(component: &OsStr) -> Result<(), TransactionError> {
    use std::os::windows::ffi::OsStrExt;

    let units = component.encode_wide().collect::<Vec<_>>();
    if units.contains(&0)
        || units.iter().any(|unit| {
            *unit < 0x20
                || matches!(
                    *unit,
                    value
                        if value == b'/' as u16
                            || value == b'\\' as u16
                            || value == b':' as u16
                            || value == b'<' as u16
                            || value == b'>' as u16
                            || value == b'"' as u16
                            || value == b'|' as u16
                            || value == b'?' as u16
                            || value == b'*' as u16
                )
        })
        || units
            .last()
            .is_some_and(|unit| *unit == b'.' as u16 || *unit == b' ' as u16)
    {
        return Err(TransactionError::invalid_state(
            Path::new(".merman.transaction"),
            format!("transaction target contains an unsafe Windows component: {component:?}"),
        ));
    }
    let uppercase = component.to_string_lossy().to_ascii_uppercase();
    let stem = uppercase.split('.').next().unwrap_or_default();
    let numbered_device = ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    let reserved = matches!(
        stem,
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || numbered_device;
    if reserved {
        return Err(TransactionError::invalid_state(
            Path::new(".merman.transaction"),
            format!("transaction target uses a reserved Windows device name: {component:?}"),
        ));
    }
    Ok(())
}

pub(super) fn encode_component(component: &OsStr) -> Result<String, TransactionError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(hex_encode(component.as_bytes()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let mut encoded = String::new();
        for unit in component.encode_wide() {
            use std::fmt::Write as _;
            write!(&mut encoded, "{unit:04x}").expect("writing into a String cannot fail");
        }
        Ok(encoded)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let value = component.to_str().ok_or_else(|| {
            TransactionError::invalid_state(
                Path::new(".merman.transaction"),
                "transaction target component is not valid UTF-8 on this platform",
            )
        })?;
        Ok(hex_encode(value.as_bytes()))
    }
}

fn encode_ascii_casefolded_component(component: &OsStr) -> Result<String, TransactionError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let bytes = component
            .as_bytes()
            .iter()
            .map(u8::to_ascii_lowercase)
            .collect::<Vec<_>>();
        Ok(hex_encode(&bytes))
    }
    #[cfg(windows)]
    {
        use std::fmt::Write as _;
        use std::os::windows::ffi::OsStrExt;

        let mut encoded = String::new();
        for unit in component.encode_wide() {
            let folded = if unit <= u16::from(u8::MAX) {
                u16::from((unit as u8).to_ascii_lowercase())
            } else {
                unit
            };
            write!(&mut encoded, "{folded:04x}").expect("writing into a String cannot fail");
        }
        Ok(encoded)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let value = component.to_str().ok_or_else(|| {
            TransactionError::invalid_state(
                Path::new(".merman.transaction"),
                "transaction target component is not valid UTF-8 on this platform",
            )
        })?;
        Ok(hex_encode(
            &value
                .as_bytes()
                .iter()
                .map(u8::to_ascii_lowercase)
                .collect::<Vec<_>>(),
        ))
    }
}

fn component_ascii_eq_ignore_case(component: &OsStr, expected: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        component
            .as_bytes()
            .eq_ignore_ascii_case(expected.as_bytes())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let expected = expected.encode_utf16().collect::<Vec<_>>();
        let actual = component.encode_wide().collect::<Vec<_>>();
        actual.len() == expected.len()
            && actual.iter().zip(expected).all(|(actual, expected)| {
                if *actual <= u16::from(u8::MAX) && expected <= u16::from(u8::MAX) {
                    (*actual as u8).eq_ignore_ascii_case(&(expected as u8))
                } else {
                    *actual == expected
                }
            })
    }
    #[cfg(not(any(unix, windows)))]
    {
        component
            .to_str()
            .is_some_and(|component| component.eq_ignore_ascii_case(expected))
    }
}

fn decode_component(encoded: &str, evidence: &Path) -> Result<OsString, TransactionError> {
    if encoded.is_empty() || encoded.len() > MAX_ENCODED_COMPONENT_BYTES {
        return Err(TransactionError::invalid_state(
            evidence,
            "journal target component has an invalid encoded length",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(OsString::from_vec(hex_decode(encoded, evidence)?))
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        if !encoded.len().is_multiple_of(4)
            || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit())
            || encoded.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err(TransactionError::invalid_state(
                evidence,
                "journal Windows path component is not UTF-16 hexadecimal",
            ));
        }
        let units = encoded
            .as_bytes()
            .chunks_exact(4)
            .map(|chunk| {
                let value = std::str::from_utf8(chunk).expect("ASCII hex is valid UTF-8");
                u16::from_str_radix(value, 16).expect("validated hexadecimal")
            })
            .collect::<Vec<_>>();
        Ok(OsString::from_wide(&units))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let bytes = hex_decode(encoded, evidence)?;
        let decoded = String::from_utf8(bytes).map_err(|_| {
            TransactionError::invalid_state(
                evidence,
                "journal UTF-8 path component contains invalid bytes",
            )
        })?;
        Ok(OsString::from(decoded))
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

fn hex_decode(encoded: &str, evidence: &Path) -> Result<Vec<u8>, TransactionError> {
    if !encoded.len().is_multiple_of(2) || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(TransactionError::invalid_state(
            evidence,
            "journal path component is not lowercase hexadecimal",
        ));
    }
    if encoded.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(TransactionError::invalid_state(
            evidence,
            "journal path component must use lowercase hexadecimal",
        ));
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let value = std::str::from_utf8(chunk).expect("ASCII hex is valid UTF-8");
            u8::from_str_radix(value, 16).map_err(|_| {
                TransactionError::invalid_state(
                    evidence,
                    "journal path component contains invalid hexadecimal",
                )
            })
        })
        .collect()
}

fn open_regular_for_read(path: &Path) -> Result<File, TransactionError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|source| TransactionError::operational("inspect state file", path, source))?;
    if !metadata.file_type().is_file() {
        return Err(TransactionError::invalid_state(
            path,
            "state path is a symlink or non-regular file",
        ));
    }
    let file = File::open(path)
        .map_err(|source| TransactionError::operational("open state file", path, source))?;
    let opened = file
        .metadata()
        .map_err(|source| TransactionError::operational("inspect open state file", path, source))?;
    if !opened.is_file() {
        return Err(TransactionError::invalid_state(
            path,
            "opened state object is not a regular file",
        ));
    }
    let handle = same_file::Handle::from_file(
        file.try_clone()
            .map_err(|source| TransactionError::operational("clone state file", path, source))?,
    )
    .map_err(|source| TransactionError::operational("inspect open state identity", path, source))?;
    let current = same_file::Handle::from_path(path).map_err(|source| {
        TransactionError::operational("inspect state path identity", path, source)
    })?;
    if handle != current {
        return Err(TransactionError::invalid_state(
            path,
            "state path identity changed while it was opened",
        ));
    }
    Ok(file)
}
