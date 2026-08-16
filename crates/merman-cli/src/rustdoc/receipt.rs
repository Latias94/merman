use super::config::Config;
use super::document::GeneratedRustdocBundle;
use crate::diagnostics::DiagnosticSink;
use crate::error::{CliError, safe_path};
use crate::resources::{ByteLedgerKind, ResolvedResourcePolicy};
use crate::runtime::SharedWriter;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

const RECEIPT_SCHEMA: u32 = 1;
use super::config::RECEIPT_FILE_NAME;
const RUSTDOC_PROFILE: &str = "rustdoc-static-v1";
const RECEIPT_HARD_LIMIT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct ExpectedRustdocBundle {
    generated: GeneratedRustdocBundle,
    receipt: RustdocReceipt,
    receipt_bytes: Vec<u8>,
    receipt_path: PathBuf,
}

impl ExpectedRustdocBundle {
    pub(crate) fn new(
        config: &Config,
        generated: GeneratedRustdocBundle,
        resources: &ResolvedResourcePolicy,
    ) -> Result<Self, CliError> {
        let receipt = RustdocReceipt::from_generated(config, &generated)?;
        let receipt_bytes = receipt.encode()?;
        let limit = receipt_limit(resources);
        if receipt_bytes.len() > limit {
            return Err(CliError::rustdoc_content(
                config.path(),
                1,
                1,
                format!(
                    "generated Rustdoc receipt is {} bytes and exceeds the {limit}-byte limit",
                    receipt_bytes.len()
                ),
            ));
        }
        let mut staged_bytes = resources.checked_bytes(ByteLedgerKind::StagedOutput);
        for fragment in generated.fragments() {
            staged_bytes
                .try_add(u64::try_from(fragment.bytes().len()).map_err(|_| {
                    CliError::Internal(
                        "generated Rustdoc fragment size does not fit u64".to_string(),
                    )
                })?)
                .map_err(CliError::from)?;
        }
        staged_bytes
            .try_add(u64::try_from(receipt_bytes.len()).map_err(|_| {
                CliError::Internal("generated Rustdoc receipt size does not fit u64".to_string())
            })?)
            .map_err(CliError::from)?;
        Ok(Self {
            generated,
            receipt,
            receipt_bytes,
            receipt_path: config.receipt_path(),
        })
    }

    pub(crate) fn generated(&self) -> &GeneratedRustdocBundle {
        &self.generated
    }

    pub(crate) fn receipt_bytes(&self) -> &[u8] {
        &self.receipt_bytes
    }

    pub(crate) fn receipt_path(&self) -> &Path {
        &self.receipt_path
    }

    fn check_disk(&self, resources: &ResolvedResourcePolicy) -> Result<(), CliError> {
        ensure_no_transaction_evidence(
            self.receipt_path
                .parent()
                .expect("the receipt path has a managed-root parent"),
        )?;
        let actual_receipt = read_receipt(&self.receipt_path, resources)?;
        let decoded = RustdocReceipt::decode(&actual_receipt, &self.receipt_path)?;
        if decoded != self.receipt || actual_receipt != self.receipt_bytes {
            return Err(CliError::rustdoc_stale(
                &self.receipt_path,
                receipt_difference(&self.receipt, &decoded),
            ));
        }

        for fragment in self.generated.fragments() {
            compare_managed_file(fragment.output(), fragment.bytes())?;
        }
        ensure_no_transaction_evidence(
            self.receipt_path
                .parent()
                .expect("the receipt path has a managed-root parent"),
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustdocReceipt {
    schema: u32,
    generator: GeneratorIdentity,
    inputs: Vec<ReceiptInput>,
    outputs: Vec<ReceiptOutput>,
    managed_files: Vec<String>,
}

impl RustdocReceipt {
    fn encode(&self) -> Result<Vec<u8>, CliError> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|error| {
            CliError::Internal(format!("failed to encode the Rustdoc receipt: {error}"))
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    fn from_generated(
        config: &Config,
        generated: &GeneratedRustdocBundle,
    ) -> Result<Self, CliError> {
        let config_name = config
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                CliError::rustdoc_config(
                    config.path(),
                    "path",
                    CliError::InvalidInput(
                        "configuration filename must be portable UTF-8".to_string(),
                    ),
                )
            })?;
        validate_portable_path(config_name, true).map_err(|reason| {
            CliError::rustdoc_config(config.path(), "path", CliError::InvalidInput(reason))
        })?;

        let mut inputs = Vec::with_capacity(generated.inputs().len() + 1);
        inputs.push(ReceiptInput {
            kind: ReceiptInputKind::Config,
            path: config_name.to_string(),
            sha256: config.sha256().to_string(),
        });
        inputs.extend(generated.inputs().iter().map(|input| ReceiptInput {
            kind: ReceiptInputKind::Source,
            path: input.logical_path().to_string(),
            sha256: input.sha256().to_string(),
        }));
        inputs.sort();

        let mut outputs = generated
            .fragments()
            .iter()
            .map(|fragment| {
                let path = fragment
                    .output()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        CliError::Internal(format!(
                            "managed Rustdoc output is not portable UTF-8: {}",
                            safe_path(fragment.output())
                        ))
                    })?;
                Ok(ReceiptOutput {
                    fragment: fragment.id().to_string(),
                    source: fragment.logical_source().to_string(),
                    path: path.to_string(),
                    sha256: super::sha256_hex(fragment.bytes()),
                })
            })
            .collect::<Result<Vec<_>, CliError>>()?;
        outputs.sort_by(|left, right| left.path.cmp(&right.path));

        let mut managed_files = outputs
            .iter()
            .map(|output| output.path.clone())
            .chain(std::iter::once(RECEIPT_FILE_NAME.to_string()))
            .collect::<Vec<_>>();
        managed_files.sort();

        let receipt = Self {
            schema: RECEIPT_SCHEMA,
            generator: GeneratorIdentity::current(),
            inputs,
            outputs,
            managed_files,
        };
        receipt.validate(config.receipt_path().as_path())?;
        Ok(receipt)
    }

    fn decode(bytes: &[u8], path: &Path) -> Result<Self, CliError> {
        let receipt = serde_json::from_slice::<Self>(bytes)
            .map_err(|error| CliError::rustdoc_receipt(path, format!("malformed JSON: {error}")))?;
        receipt.validate(path)?;
        Ok(receipt)
    }

    fn validate(&self, path: &Path) -> Result<(), CliError> {
        if self.schema != RECEIPT_SCHEMA {
            return Err(CliError::rustdoc_receipt(
                path,
                format!(
                    "unsupported schema {}; expected schema {RECEIPT_SCHEMA}",
                    self.schema
                ),
            ));
        }
        if !is_sha256_digest(&self.generator.capability_digest) {
            return Err(CliError::rustdoc_receipt(
                path,
                "capability digest is not a lowercase sha256 digest",
            ));
        }
        if self.generator.package != env!("CARGO_PKG_NAME")
            || self.generator.profile != RUSTDOC_PROFILE
        {
            return Err(CliError::rustdoc_receipt(
                path,
                format!(
                    "receipt generator must be {:?} with profile {RUSTDOC_PROFILE:?}",
                    env!("CARGO_PKG_NAME")
                ),
            ));
        }
        validate_sorted_unique(&self.inputs, path, "inputs")?;
        if self
            .outputs
            .windows(2)
            .any(|pair| pair[0].path >= pair[1].path)
        {
            return Err(CliError::rustdoc_receipt(
                path,
                "outputs must be sorted and unique",
            ));
        }
        validate_sorted_unique(&self.managed_files, path, "managed_files")?;
        let mut collision_keys = std::collections::BTreeSet::new();
        for managed in &self.managed_files {
            let key = managed
                .nfc()
                .flat_map(char::to_lowercase)
                .collect::<String>();
            if !collision_keys.insert(key) {
                return Err(CliError::rustdoc_receipt(
                    path,
                    "managed_files contains paths that collide under portable case folding",
                ));
            }
        }

        let config_inputs = self
            .inputs
            .iter()
            .filter(|input| input.kind == ReceiptInputKind::Config)
            .count();
        if config_inputs != 1 {
            return Err(CliError::rustdoc_receipt(
                path,
                "receipt must contain exactly one configuration input",
            ));
        }
        let mut input_aliases = std::collections::BTreeSet::new();
        let mut source_inputs = std::collections::BTreeSet::new();
        for input in &self.inputs {
            validate_portable_path(&input.path, input.kind == ReceiptInputKind::Config)
                .map_err(|reason| CliError::rustdoc_receipt(path, reason))?;
            let alias = input
                .path
                .nfkc()
                .flat_map(char::to_lowercase)
                .collect::<String>();
            if !input_aliases.insert(alias) {
                return Err(CliError::rustdoc_receipt(
                    path,
                    "inputs contains paths that collide under portable case folding",
                ));
            }
            if input.kind == ReceiptInputKind::Source {
                source_inputs.insert(input.path.as_str());
            }
            if !is_sha256(&input.sha256) {
                return Err(CliError::rustdoc_receipt(
                    path,
                    format!("input {:?} has an invalid sha256", input.path),
                ));
            }
        }

        for output in &self.outputs {
            if !super::config::is_portable_fragment_id(&output.fragment) {
                return Err(CliError::rustdoc_receipt(
                    path,
                    format!(
                        "receipt fragment {:?} is not a portable fragment identifier",
                        output.fragment
                    ),
                ));
            }
            validate_portable_path(&output.source, false)
                .map_err(|reason| CliError::rustdoc_receipt(path, reason))?;
            validate_portable_path(&output.path, true)
                .map_err(|reason| CliError::rustdoc_receipt(path, reason))?;
            if output.path != format!("{}.md", output.fragment) {
                return Err(CliError::rustdoc_receipt(
                    path,
                    format!(
                        "managed output {:?} does not match fragment {:?}",
                        output.path, output.fragment
                    ),
                ));
            }
            if !source_inputs.contains(output.source.as_str()) {
                return Err(CliError::rustdoc_receipt(
                    path,
                    format!(
                        "managed output {:?} refers to undeclared source {:?}",
                        output.path, output.source
                    ),
                ));
            }
            if !is_sha256(&output.sha256) {
                return Err(CliError::rustdoc_receipt(
                    path,
                    format!("output {:?} has an invalid sha256", output.path),
                ));
            }
        }

        let mut expected_managed = self
            .outputs
            .iter()
            .map(|output| output.path.clone())
            .chain(std::iter::once(RECEIPT_FILE_NAME.to_string()))
            .collect::<Vec<_>>();
        expected_managed.sort();
        if self.managed_files != expected_managed {
            return Err(CliError::rustdoc_receipt(
                path,
                "managed_files is inconsistent with outputs and receipt.json",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratorIdentity {
    package: String,
    version: String,
    merman_version: String,
    mermaid_version: String,
    capability_digest: String,
    profile: String,
}

impl GeneratorIdentity {
    fn current() -> Self {
        Self {
            package: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            merman_version: env!("CARGO_PKG_VERSION").to_string(),
            mermaid_version: merman::baseline::PINNED_MERMAID_BASELINE_VERSION.to_string(),
            capability_digest: crate::capabilities::capability_descriptor_digest().to_string(),
            profile: RUSTDOC_PROFILE.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ReceiptInputKind {
    Config,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptInput {
    kind: ReceiptInputKind,
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptOutput {
    fragment: String,
    source: String,
    path: String,
    sha256: String,
}

pub(crate) fn check(
    config: &Config,
    resources: &ResolvedResourcePolicy,
    control: &merman::OperationControl,
    stderr: &SharedWriter,
    quiet: bool,
) -> Result<(), CliError> {
    ensure_no_transaction_evidence(config.output_root())?;
    let receipt_path = config.receipt_path();
    let previous = read_previous(&receipt_path, resources)?;
    if let Some(previous) = previous.as_ref() {
        previous.ensure_owner(config, &receipt_path)?;
    }
    let generated = super::generate(config, resources, control, stderr)?;
    super::document::verify_input_snapshots(config, &generated, resources, control)?;
    let expected = ExpectedRustdocBundle::new(config, generated, resources)?;
    expected.check_disk(resources)?;
    super::document::verify_input_snapshots(config, expected.generated(), resources, control)?;
    ensure_no_transaction_evidence(config.output_root())?;
    DiagnosticSink::new(quiet, stderr).info(format!(
        "Rustdoc fragments are up to date ({} fragments, {} diagrams)",
        expected.generated().fragments().len(),
        expected.generated().diagrams()
    ));
    Ok(())
}

#[derive(Debug)]
pub(crate) struct PreviousRustdocReceipt {
    receipt: RustdocReceipt,
}

impl PreviousRustdocReceipt {
    pub(crate) fn ensure_owner(&self, config: &Config, path: &Path) -> Result<(), CliError> {
        let recorded = self.config_name();
        let current = config.file_name();
        if recorded == current {
            return Ok(());
        }
        Err(CliError::rustdoc_receipt(
            path,
            format!(
                "receipt belongs to Rustdoc configuration {recorded:?}, not {current:?}; each configuration must own its managed output root"
            ),
        ))
    }

    fn config_name(&self) -> &str {
        self.receipt
            .inputs
            .iter()
            .find(|input| input.kind == ReceiptInputKind::Config)
            .map(|input| input.path.as_str())
            .expect("validated receipts contain exactly one configuration input")
    }

    pub(crate) fn fragment_outputs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.receipt
            .outputs
            .iter()
            .map(|output| (output.path.as_str(), output.sha256.as_str()))
    }
}

pub(crate) fn read_previous(
    path: &Path,
    resources: &ResolvedResourcePolicy,
) -> Result<Option<PreviousRustdocReceipt>, CliError> {
    let Some(bytes) = read_receipt_optional(path, resources)? else {
        return Ok(None);
    };
    decode_previous(path, &bytes).map(Some)
}

pub(super) fn decode_previous(
    path: &Path,
    bytes: &[u8],
) -> Result<PreviousRustdocReceipt, CliError> {
    let receipt = RustdocReceipt::decode(bytes, path)?;
    if receipt.encode()? != bytes {
        return Err(CliError::rustdoc_receipt(
            path,
            "receipt is not canonically encoded",
        ));
    }
    Ok(PreviousRustdocReceipt { receipt })
}

fn read_receipt(path: &Path, resources: &ResolvedResourcePolicy) -> Result<Vec<u8>, CliError> {
    read_receipt_optional(path, resources)?
        .ok_or_else(|| CliError::rustdoc_stale(path, "receipt is missing"))
}

fn read_receipt_optional(
    path: &Path,
    resources: &ResolvedResourcePolicy,
) -> Result<Option<Vec<u8>>, CliError> {
    let path_metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CliError::rustdoc_receipt(
                path,
                format!("failed to inspect receipt path: {error}"),
            ));
        }
    };
    if !path_metadata.file_type().is_file() {
        return Err(CliError::rustdoc_receipt(
            path,
            "receipt is a symlink or non-regular file",
        ));
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            return Err(CliError::rustdoc_receipt(
                path,
                format!("failed to open receipt: {error}"),
            ));
        }
    };
    let metadata = file.metadata().map_err(|error| {
        CliError::rustdoc_receipt(path, format!("failed to inspect receipt: {error}"))
    })?;
    if !metadata.file_type().is_file() {
        return Err(CliError::rustdoc_receipt(
            path,
            "receipt is not a regular file",
        ));
    }
    let opened_identity = same_file::Handle::from_file(file.try_clone().map_err(|error| {
        CliError::rustdoc_receipt(path, format!("failed to clone receipt handle: {error}"))
    })?)
    .map_err(|error| {
        CliError::rustdoc_receipt(path, format!("failed to inspect receipt identity: {error}"))
    })?;
    let path_identity = same_file::Handle::from_path(path).map_err(|error| {
        CliError::rustdoc_receipt(
            path,
            format!("failed to inspect receipt path identity: {error}"),
        )
    })?;
    if opened_identity != path_identity {
        return Err(CliError::rustdoc_receipt(
            path,
            "receipt changed while it was opened",
        ));
    }
    let limit = receipt_limit(resources);
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            CliError::rustdoc_receipt(path, format!("failed to read receipt: {error}"))
        })?;
    if bytes.len() > limit {
        return Err(CliError::rustdoc_receipt(
            path,
            format!("receipt exceeds the {limit}-byte limit"),
        ));
    }
    let final_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        CliError::rustdoc_receipt(path, format!("failed to reinspect receipt path: {error}"))
    })?;
    if !final_metadata.file_type().is_file() {
        return Err(CliError::rustdoc_receipt(
            path,
            "receipt became a symlink or non-regular file while reading",
        ));
    }
    let final_identity = same_file::Handle::from_path(path).map_err(|error| {
        CliError::rustdoc_receipt(
            path,
            format!("failed to reinspect receipt path identity: {error}"),
        )
    })?;
    if opened_identity != final_identity {
        return Err(CliError::rustdoc_receipt(
            path,
            "receipt identity changed while reading",
        ));
    }
    Ok(Some(bytes))
}

pub(super) fn receipt_limit(resources: &ResolvedResourcePolicy) -> usize {
    resources
        .files()
        .config_bytes
        .unwrap_or(RECEIPT_HARD_LIMIT_BYTES)
        .min(RECEIPT_HARD_LIMIT_BYTES)
}

fn compare_managed_file(path: &Path, expected: &[u8]) -> Result<(), CliError> {
    let path_metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(CliError::rustdoc_stale(
                path,
                "generated fragment is missing",
            ));
        }
        Err(error) => {
            return Err(CliError::rustdoc_receipt(
                path,
                format!("failed to inspect generated fragment path: {error}"),
            ));
        }
    };
    if !path_metadata.file_type().is_file() {
        return Err(CliError::rustdoc_receipt(
            path,
            "generated fragment is a symlink or non-regular file",
        ));
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(CliError::rustdoc_stale(
                path,
                "generated fragment is missing",
            ));
        }
        Err(error) => {
            return Err(CliError::rustdoc_receipt(
                path,
                format!("failed to open generated fragment: {error}"),
            ));
        }
    };
    let metadata = file.metadata().map_err(|error| {
        CliError::rustdoc_receipt(
            path,
            format!("failed to inspect generated fragment: {error}"),
        )
    })?;
    if !metadata.file_type().is_file() {
        return Err(CliError::rustdoc_receipt(
            path,
            "generated fragment is not a regular file",
        ));
    }
    let opened_identity = same_file::Handle::from_file(file.try_clone().map_err(|error| {
        CliError::rustdoc_receipt(
            path,
            format!("failed to clone generated fragment handle: {error}"),
        )
    })?)
    .map_err(|error| {
        CliError::rustdoc_receipt(
            path,
            format!("failed to inspect generated fragment identity: {error}"),
        )
    })?;
    let path_identity = same_file::Handle::from_path(path).map_err(|error| {
        CliError::rustdoc_receipt(
            path,
            format!("failed to inspect generated fragment path identity: {error}"),
        )
    })?;
    if opened_identity != path_identity {
        return Err(CliError::rustdoc_receipt(
            path,
            "generated fragment changed while it was opened",
        ));
    }
    if metadata.len() != expected.len() as u64 {
        return Err(CliError::rustdoc_stale(
            path,
            "generated fragment bytes differ",
        ));
    }
    let mut actual = Vec::with_capacity(expected.len());
    file.take(expected.len().saturating_add(1) as u64)
        .read_to_end(&mut actual)
        .map_err(|error| {
            CliError::rustdoc_receipt(path, format!("failed to read generated fragment: {error}"))
        })?;
    if actual != expected {
        return Err(CliError::rustdoc_stale(
            path,
            "generated fragment bytes differ",
        ));
    }
    let final_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        CliError::rustdoc_receipt(
            path,
            format!("failed to reinspect generated fragment path: {error}"),
        )
    })?;
    if !final_metadata.file_type().is_file() {
        return Err(CliError::rustdoc_receipt(
            path,
            "generated fragment became a symlink or non-regular file while reading",
        ));
    }
    let final_identity = same_file::Handle::from_path(path).map_err(|error| {
        CliError::rustdoc_receipt(
            path,
            format!("failed to reinspect generated fragment path identity: {error}"),
        )
    })?;
    if opened_identity != final_identity {
        return Err(CliError::rustdoc_receipt(
            path,
            "generated fragment identity changed while reading",
        ));
    }
    Ok(())
}

fn ensure_no_transaction_evidence(root: &Path) -> Result<(), CliError> {
    let evidence = root.join(".merman.transaction");
    match std::fs::symlink_metadata(&evidence) {
        Ok(_) => Err(CliError::rustdoc_receipt(
            &evidence,
            "unfinished publication evidence requires `rustdoc build` recovery before check",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CliError::rustdoc_receipt(
            &evidence,
            format!("failed to inspect publication evidence: {error}"),
        )),
    }
}

fn receipt_difference(expected: &RustdocReceipt, actual: &RustdocReceipt) -> &'static str {
    if expected.generator != actual.generator {
        "generator identity or capability set changed"
    } else if expected.inputs != actual.inputs {
        "configuration or source inputs changed"
    } else if expected.outputs != actual.outputs {
        "generated output identities changed"
    } else if expected.managed_files != actual.managed_files {
        "managed file set changed"
    } else {
        "receipt is not canonically encoded"
    }
}

fn validate_portable_path(path: &str, direct: bool) -> Result<(), String> {
    super::config::validate_portable_logical_path(path)
        .map_err(|reason| format!("receipt path {path:?} {reason}"))?;
    let components = path.split('/').count();
    if direct && components != 1 {
        return Err(format!(
            "receipt path {path:?} must be a direct managed filename"
        ));
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(is_sha256)
}

fn validate_sorted_unique<T: Ord>(values: &[T], path: &Path, field: &str) -> Result<(), CliError> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CliError::rustdoc_receipt(
            path,
            format!("{field} must be sorted and unique"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::ResolvedResourcePolicy;
    use std::fs;

    fn resources() -> ResolvedResourcePolicy {
        ResolvedResourcePolicy::for_profile(merman::resources::CLI_DEFAULT_RESOURCE_PROFILE)
    }

    fn stderr() -> SharedWriter {
        SharedWriter::new(Vec::<u8>::new())
    }

    fn expected(root: &Path) -> ExpectedRustdocBundle {
        fs::write(
            root.join("source.md"),
            "```mermaid\nflowchart LR\nA-->B\n```\n",
        )
        .unwrap();
        fs::write(
            root.join("merman-rustdoc.toml"),
            "schema = 1\n[[fragments]]\nid = \"api\"\nsource = \"source.md\"\n",
        )
        .unwrap();
        let control = merman::OperationControl::new();
        let config =
            super::super::config::load(&root.join("merman-rustdoc.toml"), &resources(), &control)
                .unwrap();
        let generated = super::super::generate(&config, &resources(), &control, &stderr()).unwrap();
        ExpectedRustdocBundle::new(&config, generated, &resources()).unwrap()
    }

    #[test]
    fn canonical_receipt_is_portable_sorted_and_newline_terminated() {
        let root = tempfile::tempdir().unwrap();
        let expected = expected(root.path());
        let text = std::str::from_utf8(expected.receipt_bytes()).unwrap();

        assert!(text.ends_with('\n'));
        assert!(!text.contains(root.path().to_str().unwrap()));
        let decoded =
            RustdocReceipt::decode(expected.receipt_bytes(), expected.receipt_path()).unwrap();
        assert_eq!(decoded, expected.receipt);
        assert_eq!(decoded.managed_files, vec!["api.md", "receipt.json"]);
    }

    #[test]
    fn generated_receipt_obeys_the_same_limit_as_receipt_acquisition() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("source.md"),
            "```mermaid\nflowchart LR\nA-->B\n```\n",
        )
        .unwrap();
        fs::write(
            root.path().join("merman-rustdoc.toml"),
            "schema = 1\n[[fragments]]\nid = \"api\"\nsource = \"source.md\"\n",
        )
        .unwrap();
        let control = merman::OperationControl::new();
        let config = super::super::config::load(
            &root.path().join("merman-rustdoc.toml"),
            &resources(),
            &control,
        )
        .unwrap();
        let generated = super::super::generate(&config, &resources(), &control, &stderr()).unwrap();
        let mut limited = resources();
        limited.apply_override("max_config_bytes", 64).unwrap();

        let error = ExpectedRustdocBundle::new(&config, generated, &limited).unwrap_err();

        assert!(error.to_string().contains("generated Rustdoc receipt"));
        assert!(error.to_string().contains("64-byte limit"));
        assert!(!config.output_root().exists());
    }

    #[test]
    fn receipt_counts_toward_the_prepublication_staged_byte_limit() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("source.md"),
            "```mermaid\nflowchart LR\nA-->B\n```\n",
        )
        .unwrap();
        fs::write(
            root.path().join("merman-rustdoc.toml"),
            "schema = 1\n[[fragments]]\nid = \"api\"\nsource = \"source.md\"\n",
        )
        .unwrap();
        let control = merman::OperationControl::new();
        let config = super::super::config::load(
            &root.path().join("merman-rustdoc.toml"),
            &resources(),
            &control,
        )
        .unwrap();
        let generated = super::super::generate(&config, &resources(), &control, &stderr()).unwrap();
        let fragment_bytes = generated
            .fragments()
            .iter()
            .map(|fragment| fragment.bytes().len() as u64)
            .sum::<u64>();
        let mut limited = resources();
        limited
            .apply_override("max_staged_bytes", fragment_bytes)
            .unwrap();

        let error = ExpectedRustdocBundle::new(&config, generated, &limited).unwrap_err();

        assert!(error.to_string().contains("max_staged_bytes"), "{error}");
        assert!(!config.output_root().exists());
    }

    #[test]
    fn receipt_rejects_nonportable_paths_and_inconsistent_managed_set() {
        let root = tempfile::tempdir().unwrap();
        let expected = expected(root.path());
        let mut receipt = expected.receipt.clone();
        receipt.outputs[0].path = "..\\escape.md".to_string();
        let bytes = serde_json::to_vec(&receipt).unwrap();
        let error = RustdocReceipt::decode(&bytes, expected.receipt_path()).unwrap_err();
        assert!(error.to_string().contains("portable relative path"));

        let mut receipt = expected.receipt.clone();
        receipt.managed_files.push("unknown.md".to_string());
        receipt.managed_files.sort();
        let bytes = serde_json::to_vec(&receipt).unwrap();
        let error = RustdocReceipt::decode(&bytes, expected.receipt_path()).unwrap_err();
        assert!(error.to_string().contains("inconsistent"));
    }

    #[test]
    fn receipt_deletion_authority_is_bound_to_the_rustdoc_dialect_and_inputs() {
        let root = tempfile::tempdir().unwrap();
        let expected = expected(root.path());

        let mut receipt = expected.receipt.clone();
        receipt.generator.profile = "other-profile".to_string();
        let error = RustdocReceipt::decode(
            &serde_json::to_vec(&receipt).unwrap(),
            expected.receipt_path(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("rustdoc-static-v1"), "{error}");

        let mut receipt = expected.receipt.clone();
        receipt.outputs[0].source = "ghost.md".to_string();
        let error = RustdocReceipt::decode(
            &serde_json::to_vec(&receipt).unwrap(),
            expected.receipt_path(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("undeclared source"), "{error}");

        let mut receipt = expected.receipt.clone();
        receipt.outputs[0].fragment = "CON".to_string();
        receipt.outputs[0].path = "CON.md".to_string();
        receipt.managed_files = vec!["CON.md".to_string(), "receipt.json".to_string()];
        let error = RustdocReceipt::decode(
            &serde_json::to_vec(&receipt).unwrap(),
            expected.receipt_path(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("portable fragment"), "{error}");
    }

    #[test]
    fn check_is_read_only_and_compares_receipt_and_fragment_bytes() {
        let root = tempfile::tempdir().unwrap();
        let expected = expected(root.path());
        fs::create_dir_all(expected.receipt_path().parent().unwrap()).unwrap();
        for fragment in expected.generated().fragments() {
            fs::write(fragment.output(), fragment.bytes()).unwrap();
        }
        fs::write(expected.receipt_path(), expected.receipt_bytes()).unwrap();
        let receipt_mtime = fs::metadata(expected.receipt_path())
            .unwrap()
            .modified()
            .unwrap();

        expected.check_disk(&resources()).unwrap();
        assert!(
            read_previous(expected.receipt_path(), &resources())
                .unwrap()
                .is_some()
        );
        assert_eq!(
            fs::metadata(expected.receipt_path())
                .unwrap()
                .modified()
                .unwrap(),
            receipt_mtime
        );

        fs::write(expected.generated().fragments()[0].output(), b"tampered").unwrap();
        let error = expected.check_disk(&resources()).unwrap_err();
        assert_eq!(error.exit_code(), std::process::ExitCode::from(1));
        assert!(error.to_string().contains("bytes differ"));
    }
}
