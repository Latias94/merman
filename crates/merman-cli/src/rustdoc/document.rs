use super::config::{self, Config, Fragment, SourceDisplay};
use super::html::{diagram_html_len, write_diagram_html};
use super::svg::{prepare_static_svg, validate_static_svg};
use crate::error::{CliError, FileOperation, safe_path};
use crate::input::InputLimit;
use crate::input::InputReadError;
use crate::markdown::{
    MarkdownFenceLocation, MarkdownReplacement, scan_rustdoc_replacements_limited,
};
use crate::resources::{
    ByteLedgerKind, CheckedBytes, CliResourceLimitId, CountLedgerKind, ResolvedResourcePolicy,
};
use crate::runtime::SharedWriter;
use merman::OperationControl;
use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct GeneratedRustdocBundle {
    fragments: Vec<GeneratedFragment>,
    inputs: Vec<GeneratedInput>,
    diagrams: usize,
}

impl GeneratedRustdocBundle {
    pub(crate) fn fragments(&self) -> &[GeneratedFragment] {
        &self.fragments
    }

    pub(crate) fn inputs(&self) -> &[GeneratedInput] {
        &self.inputs
    }

    pub(crate) fn diagrams(&self) -> usize {
        self.diagrams
    }
}

#[derive(Debug)]
pub(crate) struct GeneratedFragment {
    id: String,
    logical_source: String,
    output: PathBuf,
    bytes: Vec<u8>,
}

impl GeneratedFragment {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn logical_source(&self) -> &str {
        &self.logical_source
    }

    pub(crate) fn output(&self) -> &Path {
        &self.output
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug)]
pub(crate) struct GeneratedInput {
    logical_path: String,
    requested_path: PathBuf,
    path: PathBuf,
    identity: Arc<same_file::Handle>,
    sha256: String,
}

impl GeneratedInput {
    pub(crate) fn logical_path(&self) -> &str {
        &self.logical_path
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn requested_path(&self) -> &Path {
        &self.requested_path
    }

    pub(crate) fn identity(&self) -> &Arc<same_file::Handle> {
        &self.identity
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }
}

struct GenerationState<'a> {
    config: &'a Config,
    resources: &'a ResolvedResourcePolicy,
    control: &'a OperationControl,
    stderr: &'a SharedWriter,
    renderers: crate::render::PreparedRustdocRenderers,
    light_cache: HashMap<String, Arc<str>>,
    dark_cache: HashMap<String, Arc<str>>,
    same_source_occurrences: HashMap<(String, String), u64>,
    staged_bytes: CheckedBytes,
    input_bytes: CheckedBytes,
    inputs: BTreeMap<String, GeneratedInput>,
    acquired_inputs: BTreeMap<String, Arc<config::AcquiredText>>,
    input_aliases: HashMap<String, String>,
    input_identities: HashMap<Arc<same_file::Handle>, String>,
}

pub(crate) fn generate(
    config: &Config,
    resources: &ResolvedResourcePolicy,
    control: &OperationControl,
    stderr: &SharedWriter,
) -> Result<GeneratedRustdocBundle, CliError> {
    let renderers = crate::render::prepare_rustdoc_renderers(resources, control)?;
    let mut input_bytes = resources.checked_bytes(ByteLedgerKind::RustdocInput);
    input_bytes
        .try_add(config.acquired_input_bytes())
        .map_err(CliError::from)?;
    let mut state = GenerationState {
        config,
        resources,
        control,
        stderr,
        renderers,
        light_cache: HashMap::new(),
        dark_cache: HashMap::new(),
        same_source_occurrences: HashMap::new(),
        staged_bytes: resources.checked_bytes(ByteLedgerKind::StagedOutput),
        input_bytes,
        inputs: BTreeMap::new(),
        acquired_inputs: BTreeMap::new(),
        input_aliases: HashMap::new(),
        input_identities: HashMap::new(),
    };
    for fragment in config.fragments() {
        state.record_fragment_input(fragment);
    }
    let mut fragments = Vec::with_capacity(config.fragments().len());
    for fragment in config.fragments() {
        fragments.push(state.generate_fragment(fragment)?);
    }
    let diagrams = state
        .same_source_occurrences
        .values()
        .copied()
        .fold(0_u64, u64::saturating_add);
    Ok(GeneratedRustdocBundle {
        fragments,
        inputs: state.inputs.into_values().collect(),
        diagrams: usize::try_from(diagrams).unwrap_or(usize::MAX),
    })
}

pub(super) fn verify_input_snapshots(
    config: &Config,
    generated: &GeneratedRustdocBundle,
    resources: &ResolvedResourcePolicy,
    control: &OperationControl,
) -> Result<(), CliError> {
    let config_snapshot = config::acquire_text(
        config.requested_path(),
        "Rustdoc configuration",
        InputLimit::new(
            CliResourceLimitId::MaxConfigBytes.as_str(),
            resources.files().config_bytes,
        ),
        control,
    )
    .map_err(|error| snapshot_error(config.requested_path(), error))?;
    if config_snapshot.canonical != config.path()
        || *config_snapshot.identity != **config.identity()
        || config_snapshot.sha256 != config.sha256()
    {
        return Err(super::operational_error(
            config.requested_path(),
            "Rustdoc configuration changed after generation",
        ));
    }

    for input in generated.inputs() {
        let snapshot = config::acquire_rooted_text(
            input.requested_path(),
            "Rustdoc input",
            config::fragment_source_limit(input.requested_path(), resources),
            config.root(),
            control,
        )
        .map_err(|error| snapshot_error(input.requested_path(), error))?;
        if snapshot.canonical != input.path()
            || *snapshot.identity != **input.identity()
            || snapshot.sha256 != input.sha256()
        {
            return Err(super::operational_error(
                input.requested_path(),
                "Rustdoc source or include changed after generation",
            ));
        }
    }
    Ok(())
}

fn snapshot_error(path: &Path, error: CliError) -> CliError {
    super::operational_error(
        path,
        format!("failed to revalidate generated input: {error}"),
    )
}

impl GenerationState<'_> {
    fn record_fragment_input(&mut self, fragment: &Fragment) {
        let acquired = Arc::clone(fragment.acquired());
        self.acquired_inputs
            .entry(fragment.logical_source().to_string())
            .or_insert_with(|| Arc::clone(&acquired));
        self.input_aliases
            .entry(config::portable_path_alias(fragment.logical_source()))
            .or_insert_with(|| fragment.logical_source().to_string());
        self.input_identities
            .entry(Arc::clone(&acquired.identity))
            .or_insert_with(|| fragment.logical_source().to_string());
        self.inputs.insert(
            fragment.logical_source().to_string(),
            GeneratedInput {
                logical_path: fragment.logical_source().to_string(),
                requested_path: acquired.requested.clone(),
                path: acquired.canonical.clone(),
                identity: Arc::clone(&acquired.identity),
                sha256: acquired.sha256.clone(),
            },
        );
    }

    fn generate_fragment(&mut self, fragment: &Fragment) -> Result<GeneratedFragment, CliError> {
        let bytes = if fragment.is_markdown() {
            self.generate_markdown(fragment)?.into_bytes()
        } else {
            let location = MarkdownFenceLocation { line: 1, column: 1 };
            let mut chart_count = self
                .resources
                .checked_count(CountLedgerKind::MarkdownCharts);
            chart_count.try_add(1).map_err(|error| {
                CliError::rustdoc_content(
                    fragment.source(),
                    location.line,
                    location.column,
                    error.to_string(),
                )
            })?;
            let mut output = String::new();
            self.append_render_html(
                &mut output,
                fragment.logical_source(),
                fragment.source(),
                location,
                fragment.text(),
                fragment.source_display(),
            )?;
            output.into_bytes()
        };
        Ok(GeneratedFragment {
            id: fragment.id().to_string(),
            logical_source: fragment.logical_source().to_string(),
            output: fragment.output(self.config.output_root()),
            bytes,
        })
    }

    fn generate_markdown(&mut self, fragment: &Fragment) -> Result<String, CliError> {
        let mut chart_count = self
            .resources
            .checked_count(CountLedgerKind::MarkdownCharts);
        let replacements =
            match scan_rustdoc_replacements_limited(fragment.text(), chart_count.max()) {
                Ok(replacements) => replacements,
                Err(crate::markdown::MarkdownReplacementScanError::ChartLimit {
                    observed,
                    line,
                    column,
                    ..
                }) => {
                    let limit_error = chart_count
                        .try_add(observed)
                        .expect_err("scanner reported a count above the same policy limit");
                    return Err(CliError::rustdoc_content(
                        fragment.source(),
                        line,
                        column,
                        limit_error.to_string(),
                    ));
                }
                Err(error) => {
                    let location = replacement_error_location(&error);
                    return Err(CliError::rustdoc_content(
                        fragment.source(),
                        location.line,
                        location.column,
                        error.to_string(),
                    ));
                }
            };
        let replacement_count = u64::try_from(replacements.len()).map_err(|_| {
            CliError::rustdoc_content(
                fragment.source(),
                1,
                1,
                "Markdown chart count does not fit u64",
            )
        })?;
        chart_count.try_add(replacement_count).map_err(|error| {
            CliError::rustdoc_content(fragment.source(), 1, 1, error.to_string())
        })?;
        if replacements.is_empty() {
            self.charge_staged_output(
                fragment.text().len(),
                fragment.source(),
                MarkdownFenceLocation { line: 1, column: 1 },
            )?;
            return Ok(fragment.text().to_string());
        }

        let mut output = String::new();
        let mut copied_until = 0;
        for replacement in replacements {
            let span = replacement.source_span();
            self.charge_staged_output(
                span.start - copied_until,
                fragment.source(),
                MarkdownFenceLocation { line: 1, column: 1 },
            )?;
            output.push_str(&fragment.text()[copied_until..span.start]);
            let location = replacement.location();
            match replacement {
                MarkdownReplacement::Chart(chart) => self.append_render_html(
                    &mut output,
                    fragment.logical_source(),
                    fragment.source(),
                    location,
                    chart.definition(),
                    fragment.source_display(),
                )?,
                MarkdownReplacement::Include(include) => {
                    let included = self.load_include(fragment, &include)?;
                    self.append_render_html(
                        &mut output,
                        &included.logical_path,
                        fragment.source(),
                        location,
                        &included.text,
                        fragment.source_display(),
                    )?;
                }
            }
            copied_until = span.end;
        }
        self.charge_staged_output(
            fragment.text().len() - copied_until,
            fragment.source(),
            MarkdownFenceLocation { line: 1, column: 1 },
        )?;
        output.push_str(&fragment.text()[copied_until..]);
        Ok(output)
    }

    fn load_include(
        &mut self,
        fragment: &Fragment,
        include: &crate::markdown::MarkdownInclude<'_>,
    ) -> Result<LoadedInclude, CliError> {
        let location = include.location();
        let relative = validate_include_path(include.path()).map_err(|message| {
            CliError::rustdoc_content(fragment.source(), location.line, location.column, message)
        })?;
        let logical_path = config::portable_relative_path(&relative);
        if let Some(acquired) = self.acquired_inputs.get(&logical_path) {
            return Ok(LoadedInclude {
                logical_path,
                text: Arc::clone(&acquired.text),
            });
        }
        let alias = config::portable_path_alias(&logical_path);
        if let Some(previous) = self.input_aliases.get(&alias) {
            return Err(CliError::rustdoc_content(
                fragment.source(),
                location.line,
                location.column,
                format!(
                    "include {logical_path:?} aliases already acquired Rustdoc input {previous:?} under portable path folding"
                ),
            ));
        }
        let acquired = Arc::new(
            config::acquire_rooted_text(
                &self.config.root().join(&relative),
                "Rustdoc Mermaid include",
                config::fragment_source_limit(&relative, self.resources),
                self.config.root(),
                self.control,
            )
            .map_err(|error| include_acquisition_error(fragment, &relative, location, error))?,
        );
        self.input_bytes
            .try_add(u64::try_from(acquired.text.len()).map_err(|_| {
                CliError::rustdoc_content(
                    fragment.source(),
                    location.line,
                    location.column,
                    "include byte length does not fit u64",
                )
            })?)
            .map_err(|error| {
                CliError::rustdoc_content(
                    fragment.source(),
                    location.line,
                    location.column,
                    error.to_string(),
                )
            })?;
        if acquired.canonical.starts_with(self.config.output_root()) {
            return Err(CliError::rustdoc_content(
                fragment.source(),
                location.line,
                location.column,
                format!(
                    "include {} overlaps the managed Rustdoc output root",
                    safe_path(&relative)
                ),
            ));
        }
        for output in self
            .config
            .fragments()
            .iter()
            .map(|fragment| fragment.output(self.config.output_root()))
            .filter(|output| output.exists())
        {
            let output_identity = same_file::Handle::from_path(&output).map_err(|error| {
                CliError::rustdoc_input(
                    fragment.source(),
                    location.line,
                    location.column,
                    CliError::file(FileOperation::InspectIdentity, &output, error),
                )
            })?;
            if output_identity == *acquired.identity {
                return Err(CliError::rustdoc_content(
                    fragment.source(),
                    location.line,
                    location.column,
                    format!(
                        "include {} aliases managed output {}",
                        safe_path(&relative),
                        safe_path(&output)
                    ),
                ));
            }
        }
        let receipt = self.config.receipt_path();
        if receipt.exists() {
            let receipt_identity = same_file::Handle::from_path(&receipt).map_err(|error| {
                CliError::rustdoc_input(
                    fragment.source(),
                    location.line,
                    location.column,
                    CliError::file(FileOperation::InspectIdentity, &receipt, error),
                )
            })?;
            if receipt_identity == *acquired.identity {
                return Err(CliError::rustdoc_content(
                    fragment.source(),
                    location.line,
                    location.column,
                    format!(
                        "include {} aliases managed receipt {}",
                        safe_path(&relative),
                        safe_path(&receipt)
                    ),
                ));
            }
        }

        if let Some(previous) = self.input_identities.get(&acquired.identity) {
            return Err(CliError::rustdoc_content(
                fragment.source(),
                location.line,
                location.column,
                format!(
                    "include {logical_path:?} aliases already acquired Rustdoc input {previous:?} by file identity"
                ),
            ));
        }
        self.input_aliases.insert(alias, logical_path.clone());
        self.input_identities
            .insert(Arc::clone(&acquired.identity), logical_path.clone());
        self.inputs.insert(
            logical_path.clone(),
            GeneratedInput {
                logical_path: logical_path.clone(),
                requested_path: acquired.requested.clone(),
                path: acquired.canonical.clone(),
                identity: Arc::clone(&acquired.identity),
                sha256: acquired.sha256.clone(),
            },
        );
        self.acquired_inputs
            .insert(logical_path.clone(), Arc::clone(&acquired));
        Ok(LoadedInclude {
            logical_path,
            text: Arc::clone(&acquired.text),
        })
    }

    fn append_render_html(
        &mut self,
        output: &mut String,
        logical_path: &str,
        diagnostic_path: &Path,
        location: MarkdownFenceLocation,
        source: &str,
        source_display: SourceDisplay,
    ) -> Result<(), CliError> {
        let source_hash = super::sha256_hex(source.as_bytes());
        let occurrence = self
            .same_source_occurrences
            .entry((logical_path.to_string(), source_hash.clone()))
            .or_insert(0);
        let current_occurrence = *occurrence;
        *occurrence = occurrence.saturating_add(1);
        let base_id = stable_base_id(logical_path, &source_hash, current_occurrence);

        let light = render_cached(
            &mut self.light_cache,
            &self.renderers.light,
            source,
            self.stderr,
            diagnostic_path,
            location,
            self.resources,
            self.control,
        )?;
        let dark = render_cached(
            &mut self.dark_cache,
            &self.renderers.dark,
            source,
            self.stderr,
            diagnostic_path,
            location,
            self.resources,
            self.control,
        )?;
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .with_resource_policy(self.resources.render_policy())
            .begin_session_with_control(self.control.clone())
            .map_err(merman::RenderError::from)
            .map_err(CliError::from)?;
        let light =
            merman_render::svg::rebase_svg_ids(&light, format!("{base_id}-light"), &session)
                .map_err(|error| {
                    CliError::rustdoc_content(
                        diagnostic_path,
                        location.line,
                        location.column,
                        error.to_string(),
                    )
                })?;
        let dark = merman_render::svg::rebase_svg_ids(&dark, format!("{base_id}-dark"), &session)
            .map_err(|error| {
            CliError::rustdoc_content(
                diagnostic_path,
                location.line,
                location.column,
                error.to_string(),
            )
        })?;
        validate_static_svg(
            &light,
            diagnostic_path,
            location,
            self.resources.render_policy(),
            self.control,
        )?;
        validate_static_svg(
            &dark,
            diagnostic_path,
            location,
            self.resources.render_policy(),
            self.control,
        )?;
        let wrapper_id = format!("{base_id}-wrapper");
        let output_bytes = diagram_html_len(&wrapper_id, source, &light, &dark, source_display)
            .ok_or_else(|| {
                CliError::rustdoc_content(
                    diagnostic_path,
                    location.line,
                    location.column,
                    "generated Rustdoc diagram size overflow",
                )
            })?;
        self.charge_staged_output(output_bytes, diagnostic_path, location)?;
        write_diagram_html(output, &wrapper_id, source, &light, &dark, source_display).map_err(
            |error| {
                CliError::rustdoc_content(
                    diagnostic_path,
                    location.line,
                    location.column,
                    format!("failed to assemble Rustdoc diagram HTML: {error}"),
                )
            },
        )
    }

    fn charge_staged_output(
        &mut self,
        bytes: usize,
        diagnostic_path: &Path,
        location: MarkdownFenceLocation,
    ) -> Result<(), CliError> {
        let bytes = u64::try_from(bytes).map_err(|_| {
            CliError::rustdoc_content(
                diagnostic_path,
                location.line,
                location.column,
                "generated fragment size does not fit u64",
            )
        })?;
        self.staged_bytes.try_add(bytes).map_err(|error| {
            CliError::rustdoc_content(
                diagnostic_path,
                location.line,
                location.column,
                error.to_string(),
            )
        })
    }
}

fn include_acquisition_error(
    fragment: &Fragment,
    relative: &Path,
    location: MarkdownFenceLocation,
    error: CliError,
) -> CliError {
    if matches!(
        &error,
        CliError::Input {
            error: InputReadError::NotFound { .. }
                | InputReadError::LimitExceeded { .. }
                | InputReadError::InvalidUtf8 { .. },
            ..
        } | CliError::InvalidInput(_)
    ) {
        CliError::rustdoc_content(
            fragment.source(),
            location.line,
            location.column,
            format!("failed to read include {}: {error}", safe_path(relative)),
        )
    } else {
        CliError::rustdoc_input(fragment.source(), location.line, location.column, error)
    }
}

fn render_cached(
    cache: &mut HashMap<String, Arc<str>>,
    renderer: &crate::render::PreparedGraphicalRender,
    source: &str,
    stderr: &SharedWriter,
    diagnostic_path: &Path,
    location: MarkdownFenceLocation,
    resources: &ResolvedResourcePolicy,
    control: &OperationControl,
) -> Result<Arc<str>, CliError> {
    if let Some(svg) = cache.get(source) {
        return Ok(Arc::clone(svg));
    }
    let bytes = crate::render::execute_rustdoc_svg_raw(renderer, source, control, stderr).map_err(
        |error| {
            rustdoc_render_error(
                error,
                diagnostic_path,
                location,
                format!(
                    "failed to render Mermaid source near {:?}",
                    source_preview(source)
                ),
            )
        },
    )?;
    let svg = String::from_utf8(bytes).map_err(|error| {
        CliError::rustdoc_content(
            diagnostic_path,
            location.line,
            location.column,
            format!("renderer returned non-UTF-8 SVG: {error}"),
        )
    })?;
    let svg: Arc<str> =
        prepare_static_svg(&svg, diagnostic_path, location, resources, control)?.into();
    cache.insert(source.to_string(), Arc::clone(&svg));
    Ok(svg)
}

fn rustdoc_render_error(
    error: CliError,
    diagnostic_path: &Path,
    location: MarkdownFenceLocation,
    context: String,
) -> CliError {
    match error {
        error @ CliError::Render(
            merman::RenderError::Cancelled(_)
            | merman::RenderError::RuntimePolicy(_)
            | merman::RenderError::ResourceLimitExceeded(_),
        ) => error,
        error => CliError::rustdoc_content(
            diagnostic_path,
            location.line,
            location.column,
            format!("{context}: {error}"),
        ),
    }
}

struct LoadedInclude {
    logical_path: String,
    text: Arc<str>,
}

fn validate_include_path(path: &str) -> Result<PathBuf, String> {
    if path.trim().is_empty() {
        return Err("include_mmd! path must not be empty".to_string());
    }
    if path.contains('\\') {
        return Err("include_mmd! path must use portable '/' separators".to_string());
    }
    let path = Path::new(path);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
    {
        return Err("include_mmd! path must remain relative to the configuration root".to_string());
    }
    config::validate_portable_logical_path(
        path.to_str()
            .ok_or_else(|| "include_mmd! path must be portable UTF-8".to_string())?,
    )
    .map_err(|reason| format!("include_mmd! path {reason}"))?;
    let supported = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "mmd" | "mermaid"));
    if !supported {
        return Err("include_mmd! path must end in .mmd or .mermaid".to_string());
    }
    Ok(path.to_path_buf())
}

fn stable_base_id(logical_path: &str, source_hash: &str, occurrence: u64) -> String {
    let path_hash = super::sha256_hex(logical_path.as_bytes());
    format!(
        "merman-rustdoc-{}-{}-{occurrence}",
        &path_hash[..16],
        &source_hash[..16]
    )
}

fn source_preview(source: &str) -> String {
    let preview = source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("<empty>");
    if preview.chars().count() <= 80 {
        preview.to_string()
    } else {
        format!("{}...", preview.chars().take(80).collect::<String>())
    }
}

fn replacement_error_location(
    error: &crate::markdown::MarkdownReplacementScanError,
) -> MarkdownFenceLocation {
    match error {
        crate::markdown::MarkdownReplacementScanError::ChartLimit { line, column, .. }
        | crate::markdown::MarkdownReplacementScanError::UnclosedMermaidFence { line, column }
        | crate::markdown::MarkdownReplacementScanError::InvalidInclude { line, column, .. } => {
            MarkdownFenceLocation {
                line: *line,
                column: *column,
            }
        }
    }
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

    fn load_config(path: &Path, resources: &ResolvedResourcePolicy) -> Result<Config, CliError> {
        config::load(path, resources, &OperationControl::new())
    }

    fn generate_test(
        config: &Config,
        resources: &ResolvedResourcePolicy,
        stderr: &SharedWriter,
    ) -> Result<GeneratedRustdocBundle, CliError> {
        generate(config, resources, &OperationControl::new(), stderr)
    }

    fn verify_input_snapshots_test(
        config: &Config,
        bundle: &GeneratedRustdocBundle,
        resources: &ResolvedResourcePolicy,
    ) -> Result<(), CliError> {
        verify_input_snapshots(config, bundle, resources, &OperationControl::new())
    }

    fn write_config(root: &Path, source: &str, display: &str) -> Config {
        fs::write(root.join("source.md"), source).unwrap();
        fs::write(
            root.join("merman-rustdoc.toml"),
            format!(
                "schema = 1\n\n[[fragments]]\nid = \"api\"\nsource = \"source.md\"\nsource_display = \"{display}\"\n"
            ),
        )
        .unwrap();
        load_config(&root.join("merman-rustdoc.toml"), &resources()).unwrap()
    }

    #[test]
    fn preserves_markdown_bytes_and_renders_fences_and_includes() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("included.mmd"),
            "sequenceDiagram\nA->>B: Included\n",
        )
        .unwrap();
        let source = concat!(
            "# API\r\n\r\n",
            "Before.\r\n\r\n",
            "```mermaid\r\nflowchart LR\r\nA-->B\r\n```\r\n\r\n",
            "include_mmd!(\"included.mmd\")\r\n\r\nAfter.\r\n",
        );
        let config = write_config(root.path(), source, "details");

        let first = generate_test(&config, &resources(), &stderr()).unwrap();
        let second = generate_test(&config, &resources(), &stderr()).unwrap();
        let output = std::str::from_utf8(first.fragments()[0].bytes()).unwrap();

        assert_eq!(first.fragments()[0].bytes(), second.fragments()[0].bytes());
        assert_eq!(first.diagrams(), 2);
        assert!(output.starts_with("# API\r\n\r\nBefore.\r\n\r\n<style>"));
        assert!(output.ends_with("\r\n\r\nAfter.\r\n"));
        assert_eq!(output.matches("data-merman-rustdoc=\"true\"").count(), 2);
        assert!(output.contains("Mermaid source"));
        assert!(!output.contains("```mermaid"));
        assert!(!output.contains("include_mmd!"));
        assert_eq!(first.inputs().len(), 2);
    }

    #[test]
    fn repeated_declared_sources_share_one_acquired_snapshot() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("shared.mmd"), "flowchart LR\nA-->B\n").unwrap();
        fs::write(
            root.path().join("merman-rustdoc.toml"),
            concat!(
                "schema = 1\n",
                "[[fragments]]\nid = \"one\"\nsource = \"shared.mmd\"\n",
                "[[fragments]]\nid = \"two\"\nsource = \"shared.mmd\"\n",
            ),
        )
        .unwrap();

        let config = load_config(&root.path().join("merman-rustdoc.toml"), &resources()).unwrap();

        assert!(Arc::ptr_eq(
            config.fragments()[0].acquired(),
            config.fragments()[1].acquired()
        ));
    }

    #[test]
    fn includes_reuse_all_declared_source_snapshots_before_rendering() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("wrapper.md"),
            "include_mmd!(\"shared.mmd\")\n",
        )
        .unwrap();
        fs::write(
            root.path().join("shared.mmd"),
            "flowchart LR\nOld-->Graph\n",
        )
        .unwrap();
        fs::write(
            root.path().join("merman-rustdoc.toml"),
            concat!(
                "schema = 1\n",
                "[[fragments]]\nid = \"wrapper\"\nsource = \"wrapper.md\"\n",
                "[[fragments]]\nid = \"shared\"\nsource = \"shared.mmd\"\n",
            ),
        )
        .unwrap();
        let config = load_config(&root.path().join("merman-rustdoc.toml"), &resources()).unwrap();
        fs::write(root.path().join("shared.mmd"), "not-a-diagram\n").unwrap();

        let bundle = generate_test(&config, &resources(), &stderr()).unwrap();

        assert_eq!(bundle.diagrams(), 2);
        assert_eq!(bundle.inputs().len(), 2);
        for fragment in bundle.fragments() {
            let output = std::str::from_utf8(fragment.bytes()).unwrap();
            assert!(output.contains("Old"), "{output}");
            assert!(!output.contains("not-a-diagram"), "{output}");
        }
        let error = verify_input_snapshots_test(&config, &bundle, &resources()).unwrap_err();
        assert_eq!(error.exit_code(), std::process::ExitCode::from(3));
        assert!(error.to_string().contains("changed after generation"));
    }

    #[test]
    fn include_file_identity_aliases_are_rejected_independently_of_fragment_order() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("wrapper.md"),
            "include_mmd!(\"alias.mmd\")\n",
        )
        .unwrap();
        fs::write(root.path().join("declared.mmd"), "flowchart LR\nA-->B\n").unwrap();
        fs::hard_link(
            root.path().join("declared.mmd"),
            root.path().join("alias.mmd"),
        )
        .unwrap();
        fs::write(
            root.path().join("merman-rustdoc.toml"),
            concat!(
                "schema = 1\n",
                "[[fragments]]\nid = \"wrapper\"\nsource = \"wrapper.md\"\n",
                "[[fragments]]\nid = \"declared\"\nsource = \"declared.mmd\"\n",
            ),
        )
        .unwrap();
        let config = load_config(&root.path().join("merman-rustdoc.toml"), &resources()).unwrap();

        let error = generate_test(&config, &resources(), &stderr()).unwrap_err();

        assert!(
            error.to_string().contains("aliases already acquired"),
            "{error}"
        );
        assert!(error.to_string().contains("file identity"), "{error}");
    }

    #[test]
    fn include_content_and_operational_failures_keep_distinct_exit_codes() {
        let missing = tempfile::tempdir().unwrap();
        let config = write_config(missing.path(), "include_mmd!(\"missing.mmd\")\n", "hide");
        let error = generate_test(&config, &resources(), &stderr()).unwrap_err();
        assert_eq!(error.exit_code(), std::process::ExitCode::from(1));

        let non_regular = tempfile::tempdir().unwrap();
        fs::create_dir(non_regular.path().join("directory.mmd")).unwrap();
        let config = write_config(
            non_regular.path(),
            "include_mmd!(\"directory.mmd\")\n",
            "hide",
        );
        let error = generate_test(&config, &resources(), &stderr()).unwrap_err();
        assert_eq!(error.exit_code(), std::process::ExitCode::from(3));
        assert!(error.to_string().contains("not a regular file"), "{error}");
    }

    #[test]
    fn includes_join_declared_sources_in_the_aggregate_input_budget() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("source.md"),
            "include_mmd!(\"included.mmd\")\n",
        )
        .unwrap();
        fs::write(root.path().join("included.mmd"), "flowchart LR\nA-->B\n").unwrap();
        fs::write(
            root.path().join("merman-rustdoc.toml"),
            "schema = 1\n[[fragments]]\nid = \"api\"\nsource = \"source.md\"\n",
        )
        .unwrap();
        let source_bytes = fs::metadata(root.path().join("source.md")).unwrap().len();
        let mut limited = resources();
        limited
            .apply_override("max_rustdoc_input_bytes", source_bytes)
            .unwrap();
        let config = load_config(&root.path().join("merman-rustdoc.toml"), &limited).unwrap();

        let error = generate_test(&config, &limited, &stderr()).unwrap_err();

        assert!(error.to_string().contains("max_rustdoc_input_bytes"));
        assert_eq!(error.exit_code(), std::process::ExitCode::from(1));
    }

    #[test]
    fn snapshot_verification_covers_config_sources_and_includes() {
        for changed in ["config", "source", "include"] {
            let root = tempfile::tempdir().unwrap();
            fs::write(
                root.path().join("source.md"),
                "include_mmd!(\"included.mmd\")\n",
            )
            .unwrap();
            fs::write(root.path().join("included.mmd"), "flowchart LR\nA-->B\n").unwrap();
            fs::write(
                root.path().join("merman-rustdoc.toml"),
                "schema = 1\n[[fragments]]\nid = \"api\"\nsource = \"source.md\"\n",
            )
            .unwrap();
            let config =
                load_config(&root.path().join("merman-rustdoc.toml"), &resources()).unwrap();
            let bundle = generate_test(&config, &resources(), &stderr()).unwrap();
            verify_input_snapshots_test(&config, &bundle, &resources()).unwrap();

            let path = match changed {
                "config" => root.path().join("merman-rustdoc.toml"),
                "source" => root.path().join("source.md"),
                "include" => root.path().join("included.mmd"),
                _ => unreachable!(),
            };
            fs::write(path, format!("changed-{changed}\n")).unwrap();

            let error = verify_input_snapshots_test(&config, &bundle, &resources()).unwrap_err();
            assert_eq!(
                error.exit_code(),
                std::process::ExitCode::from(3),
                "{changed}: {error}"
            );
            assert!(
                error.to_string().contains("changed after generation"),
                "{changed}: {error}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_verification_rejects_logical_symlink_retargeting() {
        use std::os::unix::fs::symlink;

        for changed in ["config", "source", "include"] {
            let root = tempfile::tempdir().unwrap();
            let config_text = "schema = 1\n[[fragments]]\nid = \"api\"\nsource = \"source.md\"\n";
            let source_text = if changed == "include" {
                "include_mmd!(\"included.mmd\")\n"
            } else {
                "flowchart LR\nA-->B\n"
            };

            fs::write(root.path().join("config-a.toml"), config_text).unwrap();
            fs::write(root.path().join("config-b.toml"), config_text).unwrap();
            fs::write(root.path().join("source-a.md"), source_text).unwrap();
            fs::write(root.path().join("source-b.md"), source_text).unwrap();
            fs::write(root.path().join("include-a.mmd"), "flowchart LR\nA-->B\n").unwrap();
            fs::write(root.path().join("include-b.mmd"), "flowchart LR\nA-->B\n").unwrap();

            symlink("config-a.toml", root.path().join("merman-rustdoc.toml")).unwrap();
            symlink("source-a.md", root.path().join("source.md")).unwrap();
            symlink("include-a.mmd", root.path().join("included.mmd")).unwrap();

            let config =
                load_config(&root.path().join("merman-rustdoc.toml"), &resources()).unwrap();
            let bundle = generate_test(&config, &resources(), &stderr()).unwrap();

            let (link, replacement) = match changed {
                "config" => ("merman-rustdoc.toml", "config-b.toml"),
                "source" => ("source.md", "source-b.md"),
                "include" => ("included.mmd", "include-b.mmd"),
                _ => unreachable!(),
            };
            fs::remove_file(root.path().join(link)).unwrap();
            symlink(replacement, root.path().join(link)).unwrap();

            let error = verify_input_snapshots_test(&config, &bundle, &resources()).unwrap_err();
            assert_eq!(
                error.exit_code(),
                std::process::ExitCode::from(3),
                "{changed}: {error}"
            );
            assert!(
                error.to_string().contains("changed after generation"),
                "{changed}: {error}"
            );
        }
    }

    #[test]
    fn same_source_occurrence_changes_only_repeated_source_ids() {
        let root = tempfile::tempdir().unwrap();
        let repeated = "```mermaid\nflowchart LR\nA-->B\n```\n";
        let config = write_config(root.path(), &format!("{repeated}{repeated}"), "hide");
        let bundle = generate_test(&config, &resources(), &stderr()).unwrap();
        let output = std::str::from_utf8(bundle.fragments()[0].bytes()).unwrap();

        assert!(output.contains("-0-wrapper"));
        assert!(output.contains("-1-wrapper"));
    }

    #[test]
    fn staged_output_budget_counts_each_reused_diagram_before_append() {
        let diagram = "```mermaid\nflowchart LR\nA-->B\n```\n";
        let single_root = tempfile::tempdir().unwrap();
        let single_config = write_config(single_root.path(), diagram, "hide");
        let single = generate_test(&single_config, &resources(), &stderr()).unwrap();
        let single_bytes = u64::try_from(single.fragments()[0].bytes().len()).unwrap();

        let repeated_root = tempfile::tempdir().unwrap();
        let repeated_config =
            write_config(repeated_root.path(), &format!("{diagram}{diagram}"), "hide");
        let mut limited = resources();
        limited
            .apply_override("max_staged_bytes", single_bytes)
            .unwrap();

        let error = generate_test(&repeated_config, &limited, &stderr()).unwrap_err();

        assert!(error.to_string().contains("max_staged_bytes"), "{error}");
    }

    #[test]
    fn raw_mermaid_source_generates_one_static_fragment() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("diagram.mmd"), "flowchart LR\nA-->B\n").unwrap();
        fs::write(
            root.path().join("merman-rustdoc.toml"),
            "schema = 1\n[[fragments]]\nid = \"diagram\"\nsource = \"diagram.mmd\"\n",
        )
        .unwrap();
        let config = load_config(&root.path().join("merman-rustdoc.toml"), &resources()).unwrap();

        let bundle = generate_test(&config, &resources(), &stderr()).unwrap();

        assert_eq!(bundle.diagrams(), 1);
        let output = std::str::from_utf8(bundle.fragments()[0].bytes()).unwrap();
        assert!(output.contains("<svg"));
        assert!(!output.contains("```"));
    }

    #[test]
    fn markdown_without_diagrams_is_preserved_exactly() {
        let root = tempfile::tempdir().unwrap();
        let source = "# API\r\n\r\nPlain `include_mmd!(\"ignored.mmd\")` prose.\r\n";
        let config = write_config(root.path(), source, "hide");

        let bundle = generate_test(&config, &resources(), &stderr()).unwrap();

        assert_eq!(bundle.diagrams(), 0);
        assert_eq!(bundle.fragments()[0].bytes(), source.as_bytes());
    }

    #[test]
    fn inserting_an_unrelated_diagram_does_not_churn_existing_ids() {
        let root = tempfile::tempdir().unwrap();
        let target = "flowchart LR\nTarget-->Stable\n";
        let original = format!("```mermaid\n{target}```\n");
        let config = write_config(root.path(), &original, "hide");
        let first = generate_test(&config, &resources(), &stderr()).unwrap();
        let expected = stable_base_id("source.md", &super::super::sha256_hex(target.as_bytes()), 0);
        assert!(
            std::str::from_utf8(first.fragments()[0].bytes())
                .unwrap()
                .contains(&format!("{expected}-wrapper"))
        );

        let changed = format!(
            "```mermaid\nsequenceDiagram\nA->>B: Unrelated\n```\n\n```mermaid\n{target}```\n"
        );
        let config = write_config(root.path(), &changed, "hide");
        let second = generate_test(&config, &resources(), &stderr()).unwrap();

        assert!(
            std::str::from_utf8(second.fragments()[0].bytes())
                .unwrap()
                .contains(&format!("{expected}-wrapper"))
        );
    }

    #[test]
    fn complete_layout_and_math_capabilities_render_in_rustdoc_mode() {
        let root = tempfile::tempdir().unwrap();
        let source = concat!(
            "```mermaid\n",
            "architecture-beta\n  group api(cloud)[API]\n  service server(server)[Server] in api\n",
            "```\n",
            "```mermaid\nflowchart-elk TD\n  A --> B\n```\n",
            "```mermaid\nflowchart TD\n  A[\"$$x^2$$\"] --> B\n```\n",
        );
        let config = write_config(root.path(), source, "hide");

        let bundle = generate_test(&config, &resources(), &stderr()).unwrap();
        let output = std::str::from_utf8(bundle.fragments()[0].bytes()).unwrap();

        assert_eq!(bundle.diagrams(), 3);
        assert_eq!(output.matches("data-merman-rustdoc=\"true\"").count(), 3);
        assert!(!output.contains("$$x^2$$"));
    }

    #[test]
    fn readable_fallback_preserves_html_label_text_before_strict_sanitizing() {
        let root = tempfile::tempdir().unwrap();
        let config = write_config(
            root.path(),
            "```mermaid\nflowchart LR\nA[\"HTML label words\"] --> B\n```\n",
            "hide",
        );

        let bundle = generate_test(&config, &resources(), &stderr()).unwrap();
        let output = std::str::from_utf8(bundle.fragments()[0].bytes()).unwrap();

        assert!(output.contains("HTML label words"));
        assert!(!output.contains("<foreignObject"));
    }

    #[test]
    fn complete_c4_person_output_keeps_bounded_inline_raster_icons() {
        let root = tempfile::tempdir().unwrap();
        let config = write_config(
            root.path(),
            "```mermaid\nC4Context\nPerson(customer, \"Customer\")\n```\n",
            "hide",
        );

        let bundle = generate_test(&config, &resources(), &stderr()).unwrap();
        let output = std::str::from_utf8(bundle.fragments()[0].bytes()).unwrap();

        assert!(output.contains("data:image/png;base64,"), "{output}");
        assert_eq!(output.matches("data-merman-rustdoc=\"true\"").count(), 1);
    }

    #[test]
    fn sequence_suffix_selectors_survive_id_rebasing() {
        let root = tempfile::tempdir().unwrap();
        let config = write_config(
            root.path(),
            "```mermaid\nsequenceDiagram\nA->>B: Message\n```\n",
            "hide",
        );

        let bundle = generate_test(&config, &resources(), &stderr()).unwrap();
        let output = std::str::from_utf8(bundle.fragments()[0].bytes()).unwrap();

        assert!(output.contains("[id$=&quot;-arrowhead&quot;]"), "{output}");
        assert!(!output.contains("[id$=&quot;merman-rustdoc-"), "{output}");
    }

    #[test]
    fn resource_limits_and_render_failures_report_source_locations() {
        let root = tempfile::tempdir().unwrap();
        let source = concat!(
            "# API\n\n",
            "```mermaid\nflowchart LR\nA-->B\n```\n",
            "  ```mermaid\nflowchart LR\nB-->C\n  ```\n",
        );
        let config = write_config(root.path(), source, "hide");
        let mut limited = resources();
        limited.apply_override("max_markdown_charts", 1).unwrap();

        let error = generate_test(&config, &limited, &stderr()).unwrap_err();
        let error = error.to_string();
        assert!(error.contains("line 7, column 3"), "{error}");
        assert!(error.contains("max_markdown_charts"), "{error}");

        let invalid = write_config(
            root.path(),
            "before\n\n ```mermaid\nnot-a-diagram\n ```\n",
            "hide",
        );
        let error = generate_test(&invalid, &resources(), &stderr()).unwrap_err();
        let error = error.to_string();
        assert!(error.contains("line 3, column 2"), "{error}");
        assert!(error.contains("not-a-diagram"), "{error}");
    }

    #[test]
    fn markdown_chart_limit_is_scoped_to_each_declared_document() {
        let root = tempfile::tempdir().unwrap();
        for name in ["one.md", "two.md"] {
            fs::write(
                root.path().join(name),
                "```mermaid\nflowchart LR\nA-->B\n```\n",
            )
            .unwrap();
        }
        fs::write(
            root.path().join("merman-rustdoc.toml"),
            concat!(
                "schema = 1\n",
                "[[fragments]]\nid = \"one\"\nsource = \"one.md\"\n",
                "[[fragments]]\nid = \"two\"\nsource = \"two.md\"\n",
            ),
        )
        .unwrap();
        let mut limited = resources();
        limited.apply_override("max_markdown_charts", 1).unwrap();
        let config = load_config(&root.path().join("merman-rustdoc.toml"), &limited).unwrap();

        let bundle = generate_test(&config, &limited, &stderr()).unwrap();

        assert_eq!(bundle.diagrams(), 2);
        assert_eq!(bundle.fragments().len(), 2);
    }
}
