use crate::error::CliError;
#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
use crate::error::InputRole;
#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
use crate::error::{FileOperation, safe_path};
#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
use crate::input::InputReadError;
use crate::invocation::ResolvedInput;
use crate::invocation::ResolvedInvocation;
use std::path::{Path, PathBuf};

#[cfg(feature = "markdown")]
use std::collections::HashMap;
#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
use std::ffi::{OsStr, OsString};
#[cfg(all(
    any(feature = "analysis", feature = "svg", feature = "ascii"),
    any(unix, windows)
))]
use std::fs::File;
#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
use std::io::Write;
#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
use std::path::Component;
#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
use std::sync::{Arc, OnceLock};

pub(crate) struct LocalPreflight {
    invocation: ResolvedInvocation,
    publications: PublicationGuards,
}

impl LocalPreflight {
    pub(crate) fn into_parts(self) -> (ResolvedInvocation, PublicationGuards) {
        (self.invocation, self.publications)
    }

    pub(crate) fn path_free(invocation: ResolvedInvocation) -> Result<Self, CliError> {
        let is_path_free = match &invocation {
            ResolvedInvocation::Capabilities(_) => true,
            #[cfg(feature = "analysis")]
            ResolvedInvocation::LintRules(_) => true,
            #[cfg(feature = "shell-completions")]
            ResolvedInvocation::Completion(_) => true,
            _ => false,
        };
        if !is_path_free {
            return Err(CliError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "current working directory is unavailable",
            )));
        }
        Ok(Self {
            invocation,
            publications: PublicationGuards::new(None),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PublicationGuards {
    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    protected: Vec<GuardedInput>,
    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    cwd: Option<PathBuf>,
    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    scopes: Vec<Arc<PublicationScope>>,
    #[cfg(feature = "markdown")]
    transaction_root: Option<Arc<DirectoryGuard>>,
}

#[cfg(feature = "markdown")]
#[derive(Debug, Clone)]
pub(crate) struct ApprovedTransactionRoot {
    path: PathBuf,
    identity: Arc<DirectoryIdentity>,
}

#[cfg(feature = "markdown")]
#[derive(Debug, Clone)]
pub(crate) struct ApprovedTransactionTarget {
    path: PathBuf,
    generation: crate::transaction::TargetGeneration,
}

#[cfg(feature = "markdown")]
impl ApprovedTransactionTarget {
    pub(crate) fn into_parts(self) -> (PathBuf, crate::transaction::TargetGeneration) {
        (self.path, self.generation)
    }
}

#[cfg(feature = "markdown")]
impl ApprovedTransactionRoot {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn verify(&self) -> Result<(), CliError> {
        let canonical = std::fs::canonicalize(&self.path).map_err(|source| {
            CliError::file(FileOperation::VerifyPublication, &self.path, source)
        })?;
        if canonical != self.path {
            return Err(publication_identity_changed(&self.path));
        }
        let current = DirectoryIdentity::open(&self.path).map_err(|source| {
            CliError::file(FileOperation::VerifyPublication, &self.path, source)
        })?;
        if !current.same_file(&self.identity) {
            return Err(publication_identity_changed(&self.path));
        }
        Ok(())
    }

    pub(crate) fn verify_same_filesystem(&self, path: &Path) -> Result<(), CliError> {
        let current = DirectoryIdentity::open(path)
            .map_err(|source| CliError::file(FileOperation::VerifyPublication, path, source))?;
        if !current.same_filesystem(&self.identity) {
            return Err(CliError::InvalidOutput(format!(
                "transaction path {} is on a different filesystem than root {}",
                safe_path(path),
                safe_path(&self.path)
            )));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn for_test(path: &Path) -> Result<Self, CliError> {
        let path = std::fs::canonicalize(path)
            .map_err(|source| CliError::file(FileOperation::Canonicalize, path, source))?;
        let identity =
            Arc::new(DirectoryIdentity::open(&path).map_err(|source| {
                CliError::file(FileOperation::VerifyPublication, &path, source)
            })?);
        Ok(Self { path, identity })
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone)]
struct GuardedInput {
    role: &'static str,
    requested: PathBuf,
    absolute: PathBuf,
    identity: Arc<same_file::Handle>,
}

impl PublicationGuards {
    fn new(cwd: Option<&Path>) -> Self {
        #[cfg(not(any(feature = "analysis", feature = "svg", feature = "ascii")))]
        let _ = cwd;
        Self {
            #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
            protected: Vec::new(),
            #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
            cwd: cwd.map(Path::to_path_buf),
            #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
            scopes: Vec::new(),
            #[cfg(feature = "markdown")]
            transaction_root: None,
        }
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    fn protect(&mut self, inputs: &[ProtectedInput]) {
        self.protected
            .extend(inputs.iter().map(|input| GuardedInput {
                role: input.role,
                requested: input.requested.clone(),
                absolute: input.absolute.clone(),
                identity: Arc::clone(&input.identity),
            }));
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    pub(crate) fn verify(&self) -> Result<(), CliError> {
        for input in &self.protected {
            let identity = same_file::Handle::from_path(&input.absolute).map_err(|source| {
                CliError::file(FileOperation::VerifyPublication, &input.requested, source)
            })?;
            if identity != *input.identity {
                return Err(CliError::file(
                    FileOperation::VerifyPublication,
                    &input.requested,
                    std::io::Error::other(format!(
                        "protected {} changed identity after preflight",
                        input.role
                    )),
                ));
            }
        }
        Ok(())
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    fn approve_exact(&mut self, target: FileTarget) -> Result<(), CliError> {
        self.approve_exact_with_policy(target, false)
    }

    #[cfg(feature = "analysis")]
    fn approve_fix_write(&mut self, target: FileTarget) -> Result<(), CliError> {
        self.approve_exact_with_policy(target, true)
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    fn approve_exact_with_policy(
        &mut self,
        target: FileTarget,
        allow_protected_target: bool,
    ) -> Result<(), CliError> {
        let cwd = self.working_directory()?;
        self.approve_scope(PublicationScope {
            matcher: PublicationMatcher::Exact {
                path: lexical_absolute(&target.requested, cwd),
                identity: target.identity,
            },
            parent: target.parent,
            allow_protected_target,
        })
    }

    #[cfg(feature = "markdown")]
    fn approve_numbered(
        &mut self,
        namespace: crate::markdown::NumberedOutputNamespace,
        guard: NumberedTargetGuard,
    ) -> Result<(), CliError> {
        let cwd = self.working_directory()?;
        self.approve_scope(PublicationScope {
            matcher: PublicationMatcher::Numbered {
                directory: lexical_absolute(namespace.directory(), cwd),
                namespace,
                existing: guard.existing,
            },
            parent: guard.parent,
            allow_protected_target: false,
        })
    }

    #[cfg(feature = "markdown")]
    fn approve_transaction_root(&mut self, guard: DirectoryGuard) -> Result<(), CliError> {
        if self.transaction_root.is_some() {
            return Err(CliError::InvalidOutput(
                "one invocation cannot own more than one transaction root".to_string(),
            ));
        }
        self.transaction_root = Some(Arc::new(guard));
        Ok(())
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    fn approve_scope(&mut self, scope: PublicationScope) -> Result<(), CliError> {
        #[cfg(feature = "markdown")]
        for existing in &self.scopes {
            if existing.matcher.directory() == scope.matcher.directory()
                && !existing.parent.is_consistent_with(&scope.parent)
            {
                return Err(publication_identity_changed(scope.matcher.directory()));
            }
        }
        self.scopes.push(Arc::new(scope));
        Ok(())
    }

    #[cfg(feature = "markdown")]
    pub(crate) fn prepare_transaction_root(&self) -> Result<ApprovedTransactionRoot, CliError> {
        let guard = self.transaction_root.as_ref().ok_or_else(|| {
            CliError::InvalidOutput(
                "Markdown output has no transaction root approved by local preflight".to_string(),
            )
        })?;
        self.verify()?;
        guard.verify_anchor()?;
        let directory = &guard.expected;
        let identity = guard.create_and_seal()?;
        Ok(ApprovedTransactionRoot {
            path: directory.clone(),
            identity,
        })
    }

    #[cfg(feature = "markdown")]
    pub(crate) fn prepare_directory(
        &self,
        path: &Path,
    ) -> Result<ApprovedTransactionRoot, CliError> {
        let matching = self.approved_directory_scopes(path)?;
        let directory = &matching[0].parent.expected;
        let identity = matching[0].parent.create_and_seal()?;
        for scope in matching.iter().skip(1) {
            scope.parent.seal_as(&identity)?;
        }
        Ok(ApprovedTransactionRoot {
            path: directory.clone(),
            identity,
        })
    }

    #[cfg(feature = "markdown")]
    pub(crate) fn approved_directory_path(&self, path: &Path) -> Result<PathBuf, CliError> {
        let matching = self.approved_directory_scopes(path)?;
        Ok(matching[0].parent.expected.clone())
    }

    #[cfg(feature = "markdown")]
    pub(crate) fn approved_transaction_target(
        &self,
        requested: &Path,
    ) -> Result<ApprovedTransactionTarget, CliError> {
        let lexical = lexical_absolute(requested, self.working_directory()?);
        let file_name = lexical.file_name().ok_or_else(|| {
            CliError::InvalidOutput(format!(
                "output target {} must name a file",
                safe_path(requested)
            ))
        })?;
        let scope = self
            .scopes
            .iter()
            .find(|scope| {
                scope.matcher.matches(&lexical)
                    || (scope.parent.expected.join(file_name) == lexical
                        && scope.matcher.contains_file_name(file_name))
            })
            .ok_or_else(|| {
                CliError::InvalidOutput(format!(
                    "output target {} was not approved by local preflight",
                    safe_path(requested)
                ))
            })?;
        self.verify()?;
        scope.parent.verify_anchor()?;
        Ok(ApprovedTransactionTarget {
            path: scope.parent.expected.join(file_name),
            generation: crate::transaction::TargetGeneration::from_preflight_identity(
                scope.matcher.target_identity(file_name),
            ),
        })
    }

    #[cfg(feature = "markdown")]
    fn approved_directory_scopes(
        &self,
        path: &Path,
    ) -> Result<Vec<&Arc<PublicationScope>>, CliError> {
        let lexical = lexical_absolute(path, self.working_directory()?);
        let matching = self
            .scopes
            .iter()
            .filter(|scope| scope.matcher.directory() == lexical)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return Err(CliError::InvalidOutput(format!(
                "output directory {} was not approved by local preflight",
                safe_path(path)
            )));
        }
        self.verify()?;
        for scope in &matching {
            scope.parent.verify_anchor()?;
            if !scope.parent.is_consistent_with(&matching[0].parent) {
                return Err(publication_identity_changed(path));
            }
        }
        Ok(matching)
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    fn publication_for(&self, requested: &Path) -> Result<ApprovedPublication, CliError> {
        self.verify()?;
        let lexical = lexical_absolute(requested, self.working_directory()?);
        let file_name = lexical.file_name().ok_or_else(|| {
            CliError::InvalidOutput(format!(
                "output target {} must name a file",
                safe_path(requested)
            ))
        })?;
        let scope = self
            .scopes
            .iter()
            .find(|scope| scope.matcher.matches(&lexical))
            .ok_or_else(|| {
                CliError::InvalidOutput(format!(
                    "output target {} was not approved by local preflight",
                    safe_path(requested)
                ))
            })?;
        Ok(ApprovedPublication {
            path: scope.parent.expected.join(file_name),
            parent_identity: scope.parent.seal()?,
            target_identity: scope.matcher.target_identity(file_name),
            allow_protected_target: scope.allow_protected_target,
        })
    }

    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    fn working_directory(&self) -> Result<&Path, CliError> {
        self.cwd.as_deref().ok_or_else(|| {
            CliError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "current working directory is unavailable",
            ))
        })
    }
}

pub(crate) fn preflight(
    mut invocation: ResolvedInvocation,
    cwd: &Path,
) -> Result<LocalPreflight, CliError> {
    #[allow(unused_mut)]
    let mut publications = PublicationGuards::new(Some(cwd));
    match &mut invocation {
        #[cfg(feature = "analysis")]
        ResolvedInvocation::Fix(args) => {
            let inputs = preflight_fix(args, cwd, &mut publications)?;
            publications.protect(&inputs);
        }
        #[cfg(any(feature = "svg", feature = "ascii"))]
        ResolvedInvocation::Render(args) => {
            let inputs = render_inputs(&args.input, &args.common, None)?;
            if let Some(target) = args.output.destination().file() {
                let target = preflight_file_target(target, cwd, &inputs, MissingParent::Reject)?;
                publications.approve_exact(target)?;
            }
            publications.protect(&inputs);
        }
        #[cfg(feature = "markdown")]
        ResolvedInvocation::Batch(args) => {
            let inputs = render_inputs(&args.input, &args.common, None)?;
            let transaction_root = prospective_directory(
                &anchored_absolute(&args.output_root, cwd),
                &args.output_root,
                MissingParent::Allow,
            )?;
            let target = args
                .output
                .destination()
                .file()
                .expect("native batch normalization always selects a file target");
            let namespace = crate::markdown::NumberedOutputNamespace::new(
                target,
                args.output.format(),
                Some(&args.output_root),
            );
            let target = preflight_file_target(target, cwd, &inputs, MissingParent::Allow)?;
            publications.approve_exact(target)?;
            let manifest_path = crate::markdown::native_manifest_path(&args.output_root);
            let manifest =
                preflight_file_target(&manifest_path, cwd, &inputs, MissingParent::Allow)?;
            require_transaction_descendant(&transaction_root, &manifest.parent, &manifest_path)?;
            publications.approve_exact(manifest)?;
            let parent =
                preflight_numbered_namespace(&namespace, cwd, &inputs, MissingParent::Allow)?;
            require_transaction_descendant(
                &transaction_root,
                &parent.parent,
                namespace.directory(),
            )?;
            publications.approve_transaction_root(transaction_root)?;
            publications.approve_numbered(namespace, parent)?;
            publications.protect(&inputs);
        }
        #[cfg(feature = "svg")]
        ResolvedInvocation::Mmdc(args) => {
            let inputs = render_inputs(
                &args.input,
                &args.common,
                args.compatibility.puppeteer_config_file.as_deref(),
            )?;
            if let Some(target) = args.output.destination().file() {
                #[cfg(feature = "markdown")]
                if matches!(
                    args.workflow,
                    crate::invocation::ResolvedWorkflow::MarkdownBatch
                ) {
                    let output_parent = anchored_absolute(target, cwd)
                        .parent()
                        .map(Path::to_path_buf)
                        .ok_or_else(|| {
                            CliError::InvalidOutput(format!(
                                "mmdc Markdown output {} has no parent directory",
                                safe_path(target)
                            ))
                        })?;
                    let transaction_root =
                        prospective_directory(&output_parent, target, MissingParent::Reject)?;
                    if crate::markdown::is_markdown_path(target) {
                        let target =
                            preflight_file_target(target, cwd, &inputs, MissingParent::Reject)?;
                        publications.approve_exact(target)?;
                    }
                    let namespace = crate::markdown::NumberedOutputNamespace::new(
                        target,
                        args.output.format(),
                        args.compatibility.artefacts.as_deref(),
                    );
                    let missing_parent = if args.compatibility.artefacts.is_some() {
                        MissingParent::Allow
                    } else {
                        MissingParent::Reject
                    };
                    let parent =
                        preflight_numbered_namespace(&namespace, cwd, &inputs, missing_parent)?;
                    let manifest_path = crate::markdown::strict_manifest_path(target)?;
                    let manifest =
                        preflight_file_target(&manifest_path, cwd, &inputs, MissingParent::Reject)?;
                    require_transaction_descendant(
                        &transaction_root,
                        &parent.parent,
                        namespace.directory(),
                    )?;
                    require_transaction_descendant(
                        &transaction_root,
                        &manifest.parent,
                        &manifest_path,
                    )?;
                    publications.approve_exact(manifest)?;
                    publications.approve_transaction_root(transaction_root)?;
                    publications.approve_numbered(namespace, parent)?;
                } else {
                    let target =
                        preflight_file_target(target, cwd, &inputs, MissingParent::Reject)?;
                    publications.approve_exact(target)?;
                }

                #[cfg(not(feature = "markdown"))]
                {
                    let target =
                        preflight_file_target(target, cwd, &inputs, MissingParent::Reject)?;
                    publications.approve_exact(target)?;
                }
            }
            publications.protect(&inputs);
        }
        _ => {}
    }

    anchor_acquisition_paths(&mut invocation, cwd);
    Ok(LocalPreflight {
        invocation,
        publications,
    })
}

fn anchor_acquisition_paths(invocation: &mut ResolvedInvocation, cwd: &Path) {
    match invocation {
        ResolvedInvocation::Capabilities(_) => {}
        ResolvedInvocation::Detect(args) => {
            anchor_input(&mut args.input, cwd);
            anchor_optional_path(&mut args.engine.config_file, cwd);
        }
        ResolvedInvocation::Parse(args) => {
            anchor_input(&mut args.input, cwd);
            anchor_optional_path(&mut args.parse.config_file, cwd);
        }
        #[cfg(feature = "svg")]
        ResolvedInvocation::Layout(args) => {
            anchor_input(&mut args.input, cwd);
            anchor_optional_path(&mut args.parse.config_file, cwd);
        }
        #[cfg(feature = "analysis")]
        ResolvedInvocation::Lint(args) => {
            anchor_input(&mut args.input, cwd);
            anchor_optional_path(&mut args.analysis.config_file, cwd);
        }
        #[cfg(feature = "analysis")]
        ResolvedInvocation::Fix(args) => {
            anchor_input(&mut args.input, cwd);
            anchor_optional_path(&mut args.analysis.config_file, cwd);
        }
        #[cfg(feature = "analysis")]
        ResolvedInvocation::LintRules(_) => {}
        #[cfg(any(feature = "svg", feature = "ascii"))]
        ResolvedInvocation::Render(args) => {
            anchor_input(&mut args.input, cwd);
            anchor_render_inputs(&mut args.common, cwd);
        }
        #[cfg(feature = "markdown")]
        ResolvedInvocation::Batch(args) => {
            anchor_input(&mut args.input, cwd);
            anchor_render_inputs(&mut args.common, cwd);
        }
        #[cfg(feature = "svg")]
        ResolvedInvocation::Mmdc(args) => {
            anchor_input(&mut args.input, cwd);
            anchor_render_inputs(&mut args.common, cwd);
            anchor_optional_path(&mut args.compatibility.puppeteer_config_file, cwd);
        }
        #[cfg(feature = "shell-completions")]
        ResolvedInvocation::Completion(_) => {}
    }
}

fn anchor_input(input: &mut ResolvedInput, cwd: &Path) {
    if let ResolvedInput::File(path) = input {
        *path = anchored_absolute(path, cwd);
    }
}

fn anchor_optional_path(path: &mut Option<PathBuf>, cwd: &Path) {
    if let Some(path) = path {
        *path = anchored_absolute(path, cwd);
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn anchor_render_inputs(common: &mut crate::invocation::ResolvedRenderCommon, cwd: &Path) {
    anchor_optional_path(&mut common.parse.config_file, cwd);
    #[cfg(feature = "svg")]
    anchor_optional_path(&mut common.css_file, cwd);
}

#[cfg(feature = "analysis")]
fn preflight_fix(
    args: &mut crate::invocation::ResolvedFix,
    cwd: &Path,
    publications: &mut PublicationGuards,
) -> Result<Vec<ProtectedInput>, CliError> {
    use crate::invocation::ResolvedFixMode;

    let mut inputs = Vec::new();
    if let Some(path) = args.input.file() {
        inputs.push(ProtectedInput::inspect(
            "Input file",
            InputRole::Primary,
            path,
            cwd,
        )?);
    }
    if let Some(path) = args.analysis.config_file.as_deref() {
        inputs.push(ProtectedInput::inspect(
            "configuration input",
            InputRole::Auxiliary,
            path,
            cwd,
        )?);
    }

    match &mut args.mode {
        ResolvedFixMode::Stdout | ResolvedFixMode::Check | ResolvedFixMode::Diff => {}
        ResolvedFixMode::Output(target) => {
            if args.input.file().is_some_and(|input| input == target) {
                return Err(CliError::InvalidOutput(format!(
                    "fix output {} aliases the primary input; use --write for in-place fixes",
                    safe_path(target)
                )));
            }
            let target = preflight_file_target(target, cwd, &inputs, MissingParent::Reject)?;
            publications.approve_exact(target)?;
        }
        ResolvedFixMode::WriteInput(target) => {
            let primary = inputs.first().ok_or_else(|| {
                CliError::InvalidInput("--write requires a file input, not stdin".to_string())
            })?;
            for protected in inputs.iter().skip(1) {
                reject_alias(primary, protected, "fix --write target")?;
            }
            *target = primary.canonical.clone();
            let target = preflight_file_target(target, cwd, &[], MissingParent::Reject)?;
            publications.approve_fix_write(target)?;
        }
    }
    Ok(inputs)
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn render_inputs(
    input: &ResolvedInput,
    common: &crate::invocation::ResolvedRenderCommon,
    puppeteer_config: Option<&Path>,
) -> Result<Vec<ProtectedInput>, CliError> {
    #[cfg(feature = "icons")]
    crate::render::validate_icon_source_count(&common.icons, &common.resources)?;

    let mut inputs = Vec::new();
    if let Some(path) = input.file() {
        inputs.push(ProtectedInput::inspect(
            "Input file",
            InputRole::Primary,
            path,
            &common.cwd,
        )?);
    }
    if let Some(path) = common.parse.config_file.as_deref() {
        inputs.push(ProtectedInput::inspect(
            "configuration input",
            InputRole::Auxiliary,
            path,
            &common.cwd,
        )?);
    }
    #[cfg(feature = "svg")]
    if let Some(path) = common.css_file.as_deref() {
        inputs.push(ProtectedInput::inspect(
            "CSS input",
            InputRole::Auxiliary,
            path,
            &common.cwd,
        )?);
    }
    if let Some(path) = puppeteer_config {
        inputs.push(ProtectedInput::inspect(
            "Puppeteer configuration file",
            InputRole::Auxiliary,
            path,
            &common.cwd,
        )?);
    }
    #[cfg(feature = "icons")]
    {
        for path in crate::render::resolve_local_icon_paths(&common.icons, &common.cwd)? {
            inputs.push(ProtectedInput::inspect(
                "local icon input",
                InputRole::Auxiliary,
                &path,
                &common.cwd,
            )?);
        }
    }
    Ok(inputs)
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
#[derive(Debug)]
struct ProtectedInput {
    role: &'static str,
    requested: PathBuf,
    absolute: PathBuf,
    lexical: PathBuf,
    canonical: PathBuf,
    identity: Arc<same_file::Handle>,
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
impl ProtectedInput {
    fn inspect(
        role: &'static str,
        input_role: InputRole,
        requested: &Path,
        cwd: &Path,
    ) -> Result<Self, CliError> {
        let absolute = anchored_absolute(requested, cwd);
        let lexical = lexical_absolute(requested, cwd);
        let resource = format!("{role} {}", safe_path(requested));
        let metadata = match std::fs::metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(input_error(
                    input_role,
                    InputReadError::NotFound { resource },
                ));
            }
            Err(source) => {
                return Err(input_error(
                    input_role,
                    InputReadError::Io { resource, source },
                ));
            }
        };
        if !metadata.is_file() {
            return Err(input_error(
                input_role,
                InputReadError::NotRegularFile { resource },
            ));
        }
        let canonical = std::fs::canonicalize(&absolute)
            .map_err(|source| CliError::file(FileOperation::Canonicalize, requested, source))?;
        let identity =
            Arc::new(same_file::Handle::from_path(&canonical).map_err(|source| {
                CliError::file(FileOperation::InspectIdentity, requested, source)
            })?);
        Ok(Self {
            role,
            requested: requested.to_path_buf(),
            absolute,
            lexical,
            canonical,
            identity,
        })
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn input_error(role: InputRole, error: InputReadError) -> CliError {
    match role {
        InputRole::Primary => CliError::primary_input(error),
        InputRole::Auxiliary => CliError::auxiliary_input(error),
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
#[derive(Clone, Copy)]
enum MissingParent {
    Reject,
    #[cfg(feature = "markdown")]
    Allow,
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
#[derive(Debug)]
struct PublicationScope {
    matcher: PublicationMatcher,
    parent: DirectoryGuard,
    allow_protected_target: bool,
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
#[derive(Debug)]
enum PublicationMatcher {
    Exact {
        path: PathBuf,
        identity: Option<Arc<same_file::Handle>>,
    },
    #[cfg(feature = "markdown")]
    Numbered {
        directory: PathBuf,
        namespace: crate::markdown::NumberedOutputNamespace,
        existing: HashMap<OsString, Arc<same_file::Handle>>,
    },
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
impl PublicationMatcher {
    fn matches(&self, lexical: &Path) -> bool {
        match self {
            Self::Exact { path, .. } => lexical == path,
            #[cfg(feature = "markdown")]
            Self::Numbered { directory, .. } => {
                lexical.parent().is_some_and(|parent| parent == directory)
                    && lexical
                        .file_name()
                        .is_some_and(|file_name| self.contains_file_name(file_name))
            }
        }
    }

    #[cfg(feature = "markdown")]
    fn contains_file_name(&self, file_name: &OsStr) -> bool {
        match self {
            Self::Exact { path, .. } => path.file_name() == Some(file_name),
            #[cfg(feature = "markdown")]
            Self::Numbered { namespace, .. } => namespace.contains_file_name(file_name),
        }
    }

    fn target_identity(&self, _file_name: &OsStr) -> Option<Arc<same_file::Handle>> {
        match self {
            Self::Exact { identity, .. } => identity.as_ref().map(Arc::clone),
            #[cfg(feature = "markdown")]
            Self::Numbered { existing, .. } => existing.get(_file_name).map(Arc::clone),
        }
    }

    #[cfg(feature = "markdown")]
    fn directory(&self) -> &Path {
        match self {
            Self::Exact { path, .. } => path
                .parent()
                .expect("an approved exact output always has a parent"),
            Self::Numbered { directory, .. } => directory,
        }
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
#[derive(Debug)]
struct ApprovedPublication {
    path: PathBuf,
    parent_identity: Arc<DirectoryIdentity>,
    target_identity: Option<Arc<same_file::Handle>>,
    allow_protected_target: bool,
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
#[derive(Debug)]
struct DirectoryIdentity {
    handle: same_file::Handle,
    #[cfg(feature = "markdown")]
    filesystem: FileSystemIdentity,
    #[cfg(all(feature = "markdown", any(unix, windows)))]
    directory_handle: File,
}

#[cfg(feature = "markdown")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileSystemIdentity(u64);

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
impl DirectoryIdentity {
    fn open(path: &Path) -> std::io::Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;

            const FILE_SHARE_READ: u32 = 0x0000_0001;
            const FILE_SHARE_WRITE: u32 = 0x0000_0002;
            const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

            let guard = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .open(path)?;
            #[cfg(feature = "markdown")]
            let filesystem = windows_filesystem_identity(&guard)?;
            let handle = same_file::Handle::from_file(guard.try_clone()?)?;
            return Ok(Self {
                handle,
                #[cfg(feature = "markdown")]
                filesystem,
                #[cfg(feature = "markdown")]
                directory_handle: guard,
            });
        }

        #[cfg(unix)]
        {
            Self::from_unix_file(File::open(path)?)
        }

        #[cfg(not(any(unix, windows)))]
        {
            Ok(Self {
                handle: same_file::Handle::from_path(path)?,
                #[cfg(feature = "markdown")]
                filesystem: FileSystemIdentity(0),
            })
        }
    }

    #[cfg(unix)]
    fn from_unix_file(directory_handle: File) -> std::io::Result<Self> {
        #[cfg(feature = "markdown")]
        use std::os::unix::fs::MetadataExt;

        #[cfg(feature = "markdown")]
        let metadata = directory_handle.metadata()?;
        let handle = same_file::Handle::from_file(directory_handle.try_clone()?)?;
        Ok(Self {
            handle,
            #[cfg(feature = "markdown")]
            filesystem: FileSystemIdentity(metadata.dev()),
            #[cfg(feature = "markdown")]
            directory_handle,
        })
    }

    fn same_file(&self, other: &Self) -> bool {
        self.handle == other.handle
    }

    #[cfg(feature = "markdown")]
    fn same_filesystem(&self, other: &Self) -> bool {
        self.filesystem == other.filesystem
    }
}

#[cfg(all(windows, feature = "markdown"))]
fn windows_filesystem_identity(file: &File) -> std::io::Result<FileSystemIdentity> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // The handle remains owned by `file`; Windows writes only the fixed-size output structure.
    let succeeded =
        unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut information) };
    if succeeded == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(FileSystemIdentity(u64::from(
        information.dwVolumeSerialNumber,
    )))
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
#[derive(Debug)]
struct CanonicalDirectory {
    path: PathBuf,
    identity: Arc<DirectoryIdentity>,
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
#[derive(Debug)]
struct DirectoryGuard {
    expected: PathBuf,
    anchor_path: PathBuf,
    anchor_identity: Arc<DirectoryIdentity>,
    parent_identity: OnceLock<Arc<DirectoryIdentity>>,
    #[cfg(feature = "markdown")]
    existed_at_preflight: bool,
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
impl DirectoryGuard {
    fn existing(directory: CanonicalDirectory) -> Self {
        let CanonicalDirectory { path, identity } = directory;
        let parent_identity = OnceLock::new();
        parent_identity
            .set(Arc::clone(&identity))
            .expect("a new directory identity cell is empty");
        Self {
            expected: path.clone(),
            anchor_path: path,
            anchor_identity: identity,
            parent_identity,
            #[cfg(feature = "markdown")]
            existed_at_preflight: true,
        }
    }

    fn projected(expected: PathBuf, anchor: CanonicalDirectory) -> Self {
        Self {
            expected,
            anchor_path: anchor.path,
            anchor_identity: anchor.identity,
            parent_identity: OnceLock::new(),
            #[cfg(feature = "markdown")]
            existed_at_preflight: false,
        }
    }

    fn verify_anchor(&self) -> Result<(), CliError> {
        let current = DirectoryIdentity::open(&self.anchor_path).map_err(|source| {
            CliError::file(FileOperation::VerifyPublication, &self.anchor_path, source)
        })?;
        if !current.same_file(&self.anchor_identity) {
            return Err(publication_identity_changed(&self.anchor_path));
        }
        Ok(())
    }

    #[cfg(feature = "markdown")]
    fn is_consistent_with(&self, other: &Self) -> bool {
        if self.expected != other.expected
            || self.anchor_path != other.anchor_path
            || !self.anchor_identity.same_file(&other.anchor_identity)
        {
            return false;
        }
        match (self.parent_identity.get(), other.parent_identity.get()) {
            (Some(left), Some(right)) => left.same_file(right),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        }
    }

    #[cfg(feature = "markdown")]
    fn same_filesystem(&self, other: &Self) -> bool {
        self.anchor_identity.same_filesystem(&other.anchor_identity)
    }

    fn seal(&self) -> Result<Arc<DirectoryIdentity>, CliError> {
        self.freeze_identity(self.current_identity()?)
    }

    #[cfg(feature = "markdown")]
    fn create_and_seal(&self) -> Result<Arc<DirectoryIdentity>, CliError> {
        if self.existed_at_preflight {
            return self.seal();
        }
        let created = create_projected_directory(self)?;
        self.seal_as(&created)
    }

    #[cfg(feature = "markdown")]
    fn seal_as(
        &self,
        expected: &Arc<DirectoryIdentity>,
    ) -> Result<Arc<DirectoryIdentity>, CliError> {
        let current = self.current_identity()?;
        if !current.same_file(expected) {
            return Err(publication_identity_changed(&self.expected));
        }
        self.freeze_identity(Arc::clone(expected))
    }

    fn current_identity(&self) -> Result<Arc<DirectoryIdentity>, CliError> {
        self.verify_anchor()?;
        let canonical = std::fs::canonicalize(&self.expected).map_err(|source| {
            CliError::file(FileOperation::VerifyPublication, &self.expected, source)
        })?;
        if canonical != self.expected {
            return Err(publication_identity_changed(&self.expected));
        }
        let metadata = std::fs::metadata(&self.expected).map_err(|source| {
            CliError::file(FileOperation::VerifyPublication, &self.expected, source)
        })?;
        if !metadata.is_dir() {
            return Err(CliError::file(
                FileOperation::VerifyPublication,
                &self.expected,
                std::io::Error::other(
                    "output directory became a non-directory after local preflight",
                ),
            ));
        }
        let current = Arc::new(DirectoryIdentity::open(&self.expected).map_err(|source| {
            CliError::file(FileOperation::VerifyPublication, &self.expected, source)
        })?);
        Ok(current)
    }

    fn freeze_identity(
        &self,
        current: Arc<DirectoryIdentity>,
    ) -> Result<Arc<DirectoryIdentity>, CliError> {
        if let Some(expected) = self.parent_identity.get() {
            if !current.same_file(expected) {
                return Err(publication_identity_changed(&self.expected));
            }
            return Ok(Arc::clone(expected));
        }
        if self.parent_identity.set(Arc::clone(&current)).is_ok() {
            return Ok(current);
        }
        let expected = self
            .parent_identity
            .get()
            .expect("a competing directory seal initialized the identity");
        if !current.same_file(expected) {
            return Err(publication_identity_changed(&self.expected));
        }
        Ok(Arc::clone(expected))
    }
}

#[cfg(all(
    feature = "markdown",
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn create_projected_directory(guard: &DirectoryGuard) -> Result<Arc<DirectoryIdentity>, CliError> {
    use rustix::fs::{Mode, OFlags, mkdirat, openat};

    let components = projected_directory_components(guard)?;
    let mut current_path = guard.anchor_path.clone();
    let mut current = guard
        .anchor_identity
        .directory_handle
        .try_clone()
        .map_err(|source| {
            CliError::file(FileOperation::CreateDirectory, &guard.anchor_path, source)
        })?;

    for component in components {
        current_path.push(&component);
        match mkdirat(&current, &component, Mode::from_bits_truncate(0o777)) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
            Err(source) => {
                return Err(CliError::file(
                    FileOperation::CreateDirectory,
                    &current_path,
                    std::io::Error::from(source),
                ));
            }
        }

        let child = openat(
            &current,
            &component,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|source| {
            CliError::file(
                FileOperation::CreateDirectory,
                &current_path,
                std::io::Error::from(source),
            )
        })?;
        current = File::from(child);
    }

    DirectoryIdentity::from_unix_file(current)
        .map(Arc::new)
        .map_err(|source| CliError::file(FileOperation::CreateDirectory, &guard.expected, source))
}

#[cfg(all(feature = "markdown", windows))]
fn create_projected_directory(guard: &DirectoryGuard) -> Result<Arc<DirectoryIdentity>, CliError> {
    let components = projected_directory_components(guard)?;
    let mut current_path = guard.anchor_path.clone();
    let mut ancestors = Vec::with_capacity(components.len());

    for component in components {
        current_path.push(component);
        match std::fs::create_dir(&current_path) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(CliError::file(
                    FileOperation::CreateDirectory,
                    &current_path,
                    source,
                ));
            }
        }

        let metadata = std::fs::symlink_metadata(&current_path).map_err(|source| {
            CliError::file(FileOperation::CreateDirectory, &current_path, source)
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(CliError::file(
                FileOperation::CreateDirectory,
                &current_path,
                std::io::Error::other("output directory component became a link or non-directory"),
            ));
        }

        let identity = Arc::new(DirectoryIdentity::open(&current_path).map_err(|source| {
            CliError::file(FileOperation::CreateDirectory, &current_path, source)
        })?);
        let canonical = std::fs::canonicalize(&current_path).map_err(|source| {
            CliError::file(FileOperation::CreateDirectory, &current_path, source)
        })?;
        let current_identity = DirectoryIdentity::open(&current_path).map_err(|source| {
            CliError::file(FileOperation::CreateDirectory, &current_path, source)
        })?;
        if canonical != current_path || !current_identity.same_file(&identity) {
            return Err(publication_identity_changed(&current_path));
        }
        ancestors.push(identity);
    }

    ancestors
        .pop()
        .ok_or_else(|| invalid_projected_directory(guard))
}

#[cfg(all(
    feature = "markdown",
    not(any(
        windows,
        all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        )
    ))
))]
fn create_projected_directory(guard: &DirectoryGuard) -> Result<Arc<DirectoryIdentity>, CliError> {
    Err(CliError::InvalidOutput(format!(
        "creating missing output directory {} is not supported on this target",
        safe_path(&guard.expected)
    )))
}

#[cfg(feature = "markdown")]
fn projected_directory_components(guard: &DirectoryGuard) -> Result<Vec<OsString>, CliError> {
    let relative = guard
        .expected
        .strip_prefix(&guard.anchor_path)
        .map_err(|_| invalid_projected_directory(guard))?;
    let components = relative
        .components()
        .map(|component| match component {
            Component::Normal(name) => Ok(name.to_os_string()),
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => Err(invalid_projected_directory(guard)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(invalid_projected_directory(guard));
    }
    Ok(components)
}

#[cfg(feature = "markdown")]
fn invalid_projected_directory(guard: &DirectoryGuard) -> CliError {
    CliError::InvalidOutput(format!(
        "output directory {} is not a strict descendant of approved anchor {}",
        safe_path(&guard.expected),
        safe_path(&guard.anchor_path)
    ))
}

#[cfg(feature = "markdown")]
fn require_transaction_descendant(
    root: &DirectoryGuard,
    candidate: &DirectoryGuard,
    requested: &Path,
) -> Result<(), CliError> {
    if candidate.expected.strip_prefix(&root.expected).is_err() {
        return Err(CliError::InvalidOutput(format!(
            "Markdown output {} lies outside transaction root {}; split-root publication is unsupported",
            safe_path(requested),
            safe_path(&root.expected)
        )));
    }
    if !candidate.same_filesystem(root) {
        return Err(CliError::InvalidOutput(format!(
            "Markdown output {} crosses a nested filesystem beneath transaction root {}; split-root publication is unsupported",
            safe_path(requested),
            safe_path(&root.expected)
        )));
    }
    Ok(())
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn publication_identity_changed(path: &Path) -> CliError {
    CliError::file(
        FileOperation::VerifyPublication,
        path,
        std::io::Error::other("directory identity changed after local preflight"),
    )
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
struct FileTarget {
    requested: PathBuf,
    canonical: PathBuf,
    identity: Option<Arc<same_file::Handle>>,
    parent: DirectoryGuard,
}

#[cfg(feature = "markdown")]
struct NumberedTargetGuard {
    parent: DirectoryGuard,
    existing: HashMap<OsString, Arc<same_file::Handle>>,
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn preflight_file_target(
    requested: &Path,
    cwd: &Path,
    inputs: &[ProtectedInput],
    missing_parent: MissingParent,
) -> Result<FileTarget, CliError> {
    let absolute = anchored_absolute(requested, cwd);
    let lexical = lexical_absolute(requested, cwd);
    for input in inputs {
        if lexical == input.lexical {
            return Err(alias_error(requested, input, "output target"));
        }
    }

    let target = inspect_file_target(requested, &absolute, missing_parent)?;
    for input in inputs {
        if target.canonical == input.canonical
            || target
                .identity
                .as_ref()
                .is_some_and(|identity| **identity == *input.identity)
        {
            return Err(alias_error(requested, input, "output target"));
        }
    }
    Ok(target)
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn inspect_file_target(
    requested: &Path,
    absolute: &Path,
    missing_parent: MissingParent,
) -> Result<FileTarget, CliError> {
    let file_name = absolute.file_name().ok_or_else(|| {
        CliError::InvalidOutput(format!(
            "output target {} must name a file",
            safe_path(requested)
        ))
    })?;
    let parent_path = absolute.parent().ok_or_else(|| {
        CliError::InvalidOutput(format!(
            "output target {} has no parent directory",
            safe_path(requested)
        ))
    })?;
    let parent = prospective_directory(parent_path, requested, missing_parent)?;

    match std::fs::symlink_metadata(absolute) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(CliError::InvalidOutput(format!(
                    "output target {} is a symlink; choose a new regular file path",
                    safe_path(requested)
                )));
            }
            if !metadata.is_file() {
                return Err(CliError::InvalidOutput(format!(
                    "output target {} is not a regular file",
                    safe_path(requested)
                )));
            }
            let canonical = std::fs::canonicalize(absolute)
                .map_err(|source| CliError::file(FileOperation::Canonicalize, requested, source))?;
            let identity =
                Arc::new(same_file::Handle::from_path(&canonical).map_err(|source| {
                    CliError::file(FileOperation::InspectIdentity, requested, source)
                })?);
            Ok(FileTarget {
                requested: requested.to_path_buf(),
                canonical,
                identity: Some(identity),
                parent,
            })
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(FileTarget {
            requested: requested.to_path_buf(),
            canonical: parent.expected.join(file_name),
            identity: None,
            parent,
        }),
        Err(source) => Err(CliError::file(FileOperation::Inspect, requested, source)),
    }
}

#[cfg(feature = "markdown")]
fn preflight_numbered_namespace(
    namespace: &crate::markdown::NumberedOutputNamespace,
    cwd: &Path,
    inputs: &[ProtectedInput],
    missing_parent: MissingParent,
) -> Result<NumberedTargetGuard, CliError> {
    let directory = if namespace.directory().as_os_str().is_empty() {
        cwd.to_path_buf()
    } else {
        anchored_absolute(namespace.directory(), cwd)
    };
    let lexical_directory = lexical_absolute(namespace.directory(), cwd);
    let parent = prospective_directory(&directory, namespace.directory(), missing_parent)?;
    let canonical_directory = &parent.expected;

    for input in inputs {
        let lexical_match = input
            .lexical
            .parent()
            .is_some_and(|parent| parent == lexical_directory)
            && input
                .lexical
                .file_name()
                .is_some_and(|name| namespace.contains_file_name(name));
        let canonical_match = input
            .canonical
            .parent()
            .is_some_and(|candidate_parent| candidate_parent == canonical_directory)
            && input
                .canonical
                .file_name()
                .is_some_and(|name| namespace.contains_file_name(name));
        if lexical_match || canonical_match {
            return Err(alias_error(
                &input.requested,
                input,
                "reserved Markdown output",
            ));
        }
    }

    if !parent.existed_at_preflight {
        return Ok(NumberedTargetGuard {
            parent,
            existing: HashMap::new(),
        });
    }
    let mut existing = HashMap::new();
    let entries = std::fs::read_dir(&directory)
        .map_err(|source| CliError::file(FileOperation::ReadDirectory, &directory, source))?;
    for entry in entries {
        let entry = entry
            .map_err(|source| CliError::file(FileOperation::ReadDirectory, &directory, source))?;
        let Some(index) = numbered_index_hint(&entry.file_name()) else {
            continue;
        };
        let candidate = namespace.path(index);
        let candidate_absolute = anchored_absolute(&candidate, cwd);
        match std::fs::symlink_metadata(&candidate_absolute) {
            Ok(_) => {
                if candidate.file_name() != Some(entry.file_name().as_ref()) {
                    match same_file::is_same_file(&candidate_absolute, entry.path()) {
                        Ok(true) => {
                            return Err(CliError::InvalidOutput(format!(
                                "reserved Markdown output {} aliases existing filesystem entry {}; refusing to overwrite it",
                                safe_path(&candidate),
                                safe_path(entry.path())
                            )));
                        }
                        Ok(false) => continue,
                        Err(source) => {
                            return Err(CliError::file(
                                FileOperation::InspectIdentity,
                                &candidate,
                                source,
                            ));
                        }
                    }
                }
                let target = preflight_file_target(&candidate, cwd, inputs, MissingParent::Reject)?;
                let identity = target.identity.expect(
                    "an entry returned by read_dir and inspected successfully has an identity",
                );
                let file_name = entry.file_name();
                if existing.insert(file_name.clone(), identity).is_some() {
                    return Err(CliError::InvalidOutput(format!(
                        "reserved Markdown output directory returned duplicate entry {}",
                        safe_path(directory.join(file_name))
                    )));
                }
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(CliError::file(FileOperation::Inspect, &candidate, source));
            }
        }
    }
    Ok(NumberedTargetGuard { parent, existing })
}

#[cfg(feature = "markdown")]
fn numbered_index_hint(file_name: &OsStr) -> Option<usize> {
    let file_name = file_name.to_str()?;
    let (stem, _) = file_name.rsplit_once('.')?;
    let (_, index) = stem.rsplit_once('-')?;
    if index.is_empty()
        || index.starts_with('0')
        || !index.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    index.parse().ok()
}

#[cfg(feature = "analysis")]
fn reject_alias(
    output: &ProtectedInput,
    protected: &ProtectedInput,
    output_role: &str,
) -> Result<(), CliError> {
    if output.lexical == protected.lexical
        || output.canonical == protected.canonical
        || *output.identity == *protected.identity
    {
        return Err(alias_error(&output.requested, protected, output_role));
    }
    Ok(())
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn alias_error(target: &Path, input: &ProtectedInput, target_role: &str) -> CliError {
    CliError::InvalidOutput(format!(
        "{target_role} {} aliases protected {} {}",
        safe_path(target),
        input.role,
        safe_path(&input.requested)
    ))
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn prospective_directory(
    absolute: &Path,
    requested: &Path,
    missing_parent: MissingParent,
) -> Result<DirectoryGuard, CliError> {
    match std::fs::symlink_metadata(absolute) {
        Ok(metadata) => {
            let followed = if metadata.file_type().is_symlink() {
                std::fs::metadata(absolute)
                    .map_err(|source| CliError::file(FileOperation::Inspect, requested, source))?
            } else {
                metadata
            };
            if !followed.is_dir() {
                return Err(CliError::InvalidOutput(format!(
                    "Output directory {} is not a directory",
                    safe_path(requested)
                )));
            }
            let directory = open_canonical_directory(absolute, requested)?;
            Ok(DirectoryGuard::existing(directory))
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            if matches!(missing_parent, MissingParent::Reject) {
                return Err(CliError::InvalidOutput(format!(
                    "Output directory {} for target {} does not exist",
                    safe_path(absolute),
                    safe_path(requested),
                )));
            }
            let (projected, anchor) = project_from_existing_ancestor(absolute, requested)?;
            Ok(DirectoryGuard::projected(projected, anchor))
        }
        Err(source) => Err(CliError::file(FileOperation::Inspect, requested, source)),
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn project_from_existing_ancestor(
    path: &Path,
    requested: &Path,
) -> Result<(PathBuf, CanonicalDirectory), CliError> {
    let mut cursor = path;
    let mut missing = Vec::<OsString>::new();
    loop {
        match std::fs::metadata(cursor) {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err(CliError::InvalidOutput(format!(
                        "output ancestor {} is not a directory",
                        safe_path(cursor)
                    )));
                }
                let anchor = open_canonical_directory(cursor, requested)?;
                let mut projected = anchor.path.clone();
                for component in missing.iter().rev() {
                    projected.push(component);
                }
                return Ok((projected, anchor));
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                let name = cursor.file_name().ok_or_else(|| {
                    CliError::InvalidOutput(format!(
                        "Output directory {} has no existing ancestor",
                        safe_path(requested)
                    ))
                })?;
                missing.push(name.to_os_string());
                cursor = cursor.parent().ok_or_else(|| {
                    CliError::InvalidOutput(format!(
                        "Output directory {} has no existing ancestor",
                        safe_path(requested)
                    ))
                })?;
            }
            Err(source) => {
                return Err(CliError::file(FileOperation::Inspect, cursor, source));
            }
        }
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn open_canonical_directory(path: &Path, requested: &Path) -> Result<CanonicalDirectory, CliError> {
    open_canonical_directory_with(path, requested, |_| {})
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn open_canonical_directory_with(
    path: &Path,
    requested: &Path,
    after_open: impl FnOnce(&Path),
) -> Result<CanonicalDirectory, CliError> {
    let identity =
        Arc::new(DirectoryIdentity::open(path).map_err(|source| {
            CliError::file(FileOperation::VerifyPublication, requested, source)
        })?);
    after_open(path);
    let canonical = std::fs::canonicalize(path)
        .map_err(|source| CliError::file(FileOperation::Canonicalize, requested, source))?;
    let canonical_identity = DirectoryIdentity::open(&canonical)
        .map_err(|source| CliError::file(FileOperation::VerifyPublication, requested, source))?;
    if !identity.same_file(&canonical_identity) {
        return Err(publication_identity_changed(path));
    }
    Ok(CanonicalDirectory {
        path: canonical,
        identity,
    })
}

fn anchored_absolute(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn lexical_absolute(path: &Path, cwd: &Path) -> PathBuf {
    let absolute = anchored_absolute(path, cwd);
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized.file_name().is_some() {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
pub(crate) fn publish_atomic_file(
    path: &Path,
    bytes: &[u8],
    publications: &PublicationGuards,
) -> Result<(), CliError> {
    publish_with_backend(path, bytes, publications, &SystemAtomicBackend)
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
pub(crate) trait PublicationBackend {
    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    fn publish_file(
        &mut self,
        path: &Path,
        bytes: &[u8],
        publications: &PublicationGuards,
    ) -> Result<(), CliError>;

    #[cfg(feature = "analysis")]
    fn publish_file_verified(
        &mut self,
        path: &Path,
        bytes: &[u8],
        publications: &PublicationGuards,
        verify: &mut dyn FnMut(&Path) -> Result<(), CliError>,
    ) -> Result<(), CliError>;

    #[cfg(feature = "markdown")]
    fn acquire_transaction(
        &mut self,
        publications: &PublicationGuards,
    ) -> Result<AcquiredTransaction, CliError>;

    #[cfg(feature = "markdown")]
    fn begin_transaction(
        &mut self,
        acquired: AcquiredTransaction,
        plan: crate::transaction::TransactionPlan,
    ) -> Result<crate::transaction::StagingTransaction, CliError> {
        let (_, locked) = acquired.into_parts();
        locked.begin(plan).map_err(Into::into)
    }

    #[cfg(feature = "markdown")]
    fn ready_transaction(
        &mut self,
        staging: crate::transaction::StagingTransaction,
    ) -> Result<crate::transaction::ReadyTransaction, CliError> {
        staging.ready().map_err(Into::into)
    }

    #[cfg(feature = "markdown")]
    fn commit_transaction(
        &mut self,
        ready: crate::transaction::ReadyTransaction,
    ) -> Result<(), CliError> {
        ready.commit().map_err(Into::into)
    }

    #[cfg(feature = "markdown")]
    fn abort_transaction(
        &mut self,
        staging: crate::transaction::StagingTransaction,
    ) -> Result<(), CliError> {
        staging.abort()?;
        Ok(())
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
pub(crate) struct SystemPublicationBackend;

#[cfg(feature = "markdown")]
pub(crate) struct AcquiredTransaction {
    root: PathBuf,
    locked: crate::transaction::LockedRecoveredRoot,
}

#[cfg(feature = "markdown")]
impl AcquiredTransaction {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn into_parts(self) -> (PathBuf, crate::transaction::LockedRecoveredRoot) {
        (self.root, self.locked)
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
impl PublicationBackend for SystemPublicationBackend {
    #[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
    fn publish_file(
        &mut self,
        path: &Path,
        bytes: &[u8],
        publications: &PublicationGuards,
    ) -> Result<(), CliError> {
        publish_atomic_file(path, bytes, publications)
    }

    #[cfg(feature = "analysis")]
    fn publish_file_verified(
        &mut self,
        path: &Path,
        bytes: &[u8],
        publications: &PublicationGuards,
        verify: &mut dyn FnMut(&Path) -> Result<(), CliError>,
    ) -> Result<(), CliError> {
        publish_atomic_file_verified(path, bytes, publications, verify)
    }

    #[cfg(feature = "markdown")]
    fn acquire_transaction(
        &mut self,
        publications: &PublicationGuards,
    ) -> Result<AcquiredTransaction, CliError> {
        let approved = publications.prepare_transaction_root()?;
        let root = approved.path().to_path_buf();
        let locked = crate::transaction::LockedRecoveredRoot::acquire_approved(approved)?;
        Ok(AcquiredTransaction { root, locked })
    }
}

#[cfg(feature = "analysis")]
pub(crate) fn publish_atomic_file_verified(
    path: &Path,
    bytes: &[u8],
    publications: &PublicationGuards,
    verify: impl FnMut(&Path) -> Result<(), CliError>,
) -> Result<(), CliError> {
    publish_with_backend_and_verifier(path, bytes, publications, &SystemAtomicBackend, verify)
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
trait AtomicBackend {
    type Stage: Write;

    fn open(&self, path: &Path) -> std::io::Result<Self::Stage>;
    fn verify_parent(
        &self,
        stage: &Self::Stage,
        path: &Path,
        expected: &DirectoryIdentity,
    ) -> std::io::Result<()>;
    fn commit(&self, stage: Self::Stage) -> std::io::Result<()>;
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
struct SystemAtomicBackend;

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
impl AtomicBackend for SystemAtomicBackend {
    type Stage = atomic_write_file::AtomicWriteFile;

    fn open(&self, path: &Path) -> std::io::Result<Self::Stage> {
        atomic_write_file::AtomicWriteFile::open(path)
    }

    fn verify_parent(
        &self,
        stage: &Self::Stage,
        path: &Path,
        expected: &DirectoryIdentity,
    ) -> std::io::Result<()> {
        verify_atomic_parent(stage, path, expected)
    }

    fn commit(&self, stage: Self::Stage) -> std::io::Result<()> {
        stage.commit()
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn publish_with_backend<B: AtomicBackend>(
    path: &Path,
    bytes: &[u8],
    publications: &PublicationGuards,
    backend: &B,
) -> Result<(), CliError> {
    publish_with_backend_and_verifier(path, bytes, publications, backend, |_| Ok(()))
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn publish_with_backend_and_verifier<B: AtomicBackend>(
    path: &Path,
    bytes: &[u8],
    publications: &PublicationGuards,
    backend: &B,
    mut verify: impl FnMut(&Path) -> Result<(), CliError>,
) -> Result<(), CliError> {
    let approved = publications.publication_for(path)?;
    verify_publication_target(&approved, &publications.protected)?;

    let mut stage = backend
        .open(&approved.path)
        .map_err(|source| CliError::file(FileOperation::OpenAtomicStaging, path, source))?;
    backend
        .verify_parent(&stage, &approved.path, &approved.parent_identity)
        .map_err(|source| CliError::file(FileOperation::VerifyPublication, path, source))?;
    stage
        .write_all(bytes)
        .map_err(|source| CliError::file(FileOperation::WriteAtomicStaging, path, source))?;
    verify_commit_preconditions(path, &approved, publications, backend, &stage, &mut verify)?;
    // The condition is intentionally repeatable: the final call narrows, but
    // cannot eliminate, the portable compare-to-rename window.
    verify_commit_preconditions(path, &approved, publications, backend, &stage, &mut verify)?;
    backend
        .commit(stage)
        .map_err(|source| CliError::file(FileOperation::CommitAtomicReplacement, path, source))
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn verify_commit_preconditions<B: AtomicBackend>(
    requested_path: &Path,
    approved: &ApprovedPublication,
    publications: &PublicationGuards,
    backend: &B,
    stage: &B::Stage,
    verify: &mut impl FnMut(&Path) -> Result<(), CliError>,
) -> Result<(), CliError> {
    publications.verify()?;
    verify_publication_target(approved, &publications.protected)?;
    backend
        .verify_parent(stage, &approved.path, &approved.parent_identity)
        .map_err(|source| {
            CliError::file(FileOperation::VerifyPublication, requested_path, source)
        })?;
    verify(&approved.path)
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn verify_publication_target(
    approved: &ApprovedPublication,
    protected: &[GuardedInput],
) -> Result<(), CliError> {
    match std::fs::symlink_metadata(&approved.path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(CliError::file(
            FileOperation::VerifyPublication,
            &approved.path,
            std::io::Error::other("output target became a symlink after local preflight"),
        )),
        Ok(metadata) if !metadata.is_file() => Err(CliError::file(
            FileOperation::VerifyPublication,
            &approved.path,
            std::io::Error::other("output target became a non-regular file after local preflight"),
        )),
        Ok(_) => {
            let current = same_file::Handle::from_path(&approved.path).map_err(|source| {
                CliError::file(FileOperation::VerifyPublication, &approved.path, source)
            })?;
            for input in protected {
                let is_authorized_fix_target = approved.allow_protected_target
                    && approved
                        .target_identity
                        .as_deref()
                        .is_some_and(|expected| current == *expected);
                if current == *input.identity && !is_authorized_fix_target {
                    return Err(CliError::file(
                        FileOperation::VerifyPublication,
                        &approved.path,
                        std::io::Error::other(format!(
                            "output became an alias of protected {} {} after preflight",
                            input.role,
                            safe_path(&input.requested)
                        )),
                    ));
                }
            }
            match approved.target_identity.as_deref() {
                Some(expected) if current == *expected => Ok(()),
                Some(_) => Err(publication_target_changed(&approved.path)),
                None => Err(publication_target_changed(&approved.path)),
            }
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            if approved.target_identity.is_some() {
                Err(publication_target_changed(&approved.path))
            } else {
                Ok(())
            }
        }
        Err(source) => Err(CliError::file(
            FileOperation::VerifyPublication,
            &approved.path,
            source,
        )),
    }
}

#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
fn publication_target_changed(path: &Path) -> CliError {
    CliError::file(
        FileOperation::VerifyPublication,
        path,
        std::io::Error::other("output target identity changed after local preflight"),
    )
}

#[cfg(all(any(feature = "analysis", feature = "svg", feature = "ascii"), unix))]
fn verify_atomic_parent(
    stage: &atomic_write_file::AtomicWriteFile,
    path: &Path,
    expected: &DirectoryIdentity,
) -> std::io::Result<()> {
    use std::os::fd::AsFd;

    let directory = stage.directory().ok_or_else(|| {
        std::io::Error::other("atomic staging did not expose its parent directory handle")
    })?;
    let owned = directory.as_fd().try_clone_to_owned()?;
    let actual = same_file::Handle::from_file(File::from(owned))?;
    if actual != expected.handle {
        return Err(std::io::Error::other(
            "atomic staging opened a different parent directory than local preflight",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("output target has no parent directory"))?;
    let current = DirectoryIdentity::open(parent)?;
    if !current.same_file(expected) {
        return Err(std::io::Error::other(
            "output parent directory changed after atomic staging opened",
        ));
    }
    Ok(())
}

#[cfg(all(
    any(feature = "analysis", feature = "svg", feature = "ascii"),
    not(unix)
))]
fn verify_atomic_parent(
    _stage: &atomic_write_file::AtomicWriteFile,
    path: &Path,
    expected: &DirectoryIdentity,
) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("output target has no parent directory"))?;
    let actual = DirectoryIdentity::open(parent)?;
    if !actual.same_file(expected) {
        return Err(std::io::Error::other(
            "atomic staging opened a different parent directory than local preflight",
        ));
    }
    Ok(())
}

#[cfg(all(test, any(feature = "analysis", feature = "svg", feature = "ascii")))]
mod tests {
    use super::*;

    #[test]
    fn lexical_normalization_collapses_dot_components() {
        let path = lexical_absolute(Path::new("./nested/../diagram.svg"), Path::new("/work"));
        assert_eq!(path, Path::new("/work/diagram.svg"));
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn numbered_namespace_ignores_unrelated_indexed_file_names() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("out.svg");
        let expected = directory.path().join("out-1.svg");
        std::fs::write(&expected, b"old numbered output").unwrap();
        std::fs::write(directory.path().join("other-1.txt"), b"unrelated").unwrap();
        let namespace = crate::markdown::NumberedOutputNamespace::new(
            &target,
            crate::cli::RenderFormat::Svg,
            None,
        );

        let guard =
            preflight_numbered_namespace(&namespace, directory.path(), &[], MissingParent::Reject)
                .unwrap();

        assert_eq!(guard.existing.len(), 1);
        assert!(guard.existing.contains_key(OsStr::new("out-1.svg")));
    }

    #[cfg(all(feature = "markdown", unix))]
    #[test]
    fn one_lexical_scope_cannot_freeze_two_parent_directories() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        let view = directory.path().join("view");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        symlink(&first, &view).unwrap();

        let target = view.join("out.svg");
        let exact =
            preflight_file_target(&target, directory.path(), &[], MissingParent::Reject).unwrap();
        let mut guards = PublicationGuards::new(Some(directory.path()));
        guards.approve_exact(exact).unwrap();

        std::fs::remove_file(&view).unwrap();
        symlink(&second, &view).unwrap();
        let namespace = crate::markdown::NumberedOutputNamespace::new(
            &target,
            crate::cli::RenderFormat::Svg,
            None,
        );
        let numbered =
            preflight_numbered_namespace(&namespace, directory.path(), &[], MissingParent::Reject)
                .unwrap();

        let error = guards.approve_numbered(namespace, numbered).unwrap_err();
        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
    }

    #[cfg(all(feature = "markdown", unix))]
    #[test]
    fn canonical_numbered_target_uses_its_numbered_scope_generation() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let actual = directory.path().join("actual");
        let view = directory.path().join("view");
        std::fs::create_dir(&actual).unwrap();
        symlink(&actual, &view).unwrap();
        let canonical_actual = std::fs::canonicalize(&actual).unwrap();
        let manifest = view.join(".merman-manifest.json");
        let numbered = canonical_actual.join("out-1.svg");
        std::fs::write(&manifest, b"manifest").unwrap();
        std::fs::write(&numbered, b"numbered").unwrap();

        let mut guards = PublicationGuards::new(Some(directory.path()));
        let exact =
            preflight_file_target(&manifest, directory.path(), &[], MissingParent::Reject).unwrap();
        guards.approve_exact(exact).unwrap();
        let namespace = crate::markdown::NumberedOutputNamespace::new(
            &view.join("out.svg"),
            crate::cli::RenderFormat::Svg,
            None,
        );
        let numbered_guard =
            preflight_numbered_namespace(&namespace, directory.path(), &[], MissingParent::Reject)
                .unwrap();
        guards.approve_numbered(namespace, numbered_guard).unwrap();

        let (approved, generation) = guards
            .approved_transaction_target(&numbered)
            .unwrap()
            .into_parts();

        assert_eq!(approved, numbered);
        assert_eq!(
            generation,
            crate::transaction::TargetGeneration::Existing(Arc::new(
                same_file::Handle::from_path(&numbered).unwrap()
            ))
        );
    }

    #[cfg(all(feature = "markdown", unix))]
    #[test]
    fn projected_scopes_must_share_the_first_sealed_directory_identity() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let anchor = std::fs::canonicalize(&root).unwrap();
        let expected = anchor.join("output");
        let displaced = anchor.join("displaced");
        let first_anchor = open_canonical_directory(&anchor, &expected).unwrap();
        let second_anchor = open_canonical_directory(&anchor, &expected).unwrap();
        let first = DirectoryGuard::projected(expected.clone(), first_anchor);
        let second = DirectoryGuard::projected(expected.clone(), second_anchor);

        std::fs::create_dir(&expected).unwrap();
        let identity = first.seal().unwrap();
        std::fs::rename(&expected, &displaced).unwrap();
        std::fs::create_dir(&expected).unwrap();

        let error = second.seal_as(&identity).unwrap_err();
        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
    }

    #[cfg(all(
        feature = "markdown",
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn projected_directory_creation_stays_with_the_approved_anchor_identity() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let approved = directory.path().join("approved");
        let displaced = directory.path().join("displaced");
        let redirect = directory.path().join("redirect");
        std::fs::create_dir(&approved).unwrap();
        std::fs::create_dir(&redirect).unwrap();
        let anchor = std::fs::canonicalize(&approved).unwrap();
        let expected = anchor.join("new").join("nested");
        let anchor = open_canonical_directory(&anchor, &expected).unwrap();
        let guard = DirectoryGuard::projected(expected.clone(), anchor);
        guard.verify_anchor().unwrap();

        std::fs::rename(&approved, &displaced).unwrap();
        symlink(&redirect, &approved).unwrap();

        let error = guard.create_and_seal().unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
        assert!(displaced.join("new").join("nested").is_dir());
        assert!(!redirect.join("new").exists());
    }

    #[cfg(all(
        feature = "markdown",
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn projected_directory_creation_rejects_a_link_component() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        let redirect = directory.path().join("redirect");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&redirect).unwrap();
        let anchor = std::fs::canonicalize(&root).unwrap();
        let expected = anchor.join("new").join("nested");
        let anchor = open_canonical_directory(&anchor, &expected).unwrap();
        let anchor_path = anchor.path.clone();
        let guard = DirectoryGuard::projected(expected.clone(), anchor);
        symlink(&redirect, anchor_path.join("new")).unwrap();

        let error = guard.create_and_seal().unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::CreateDirectory,
                ..
            }
        ));
        assert!(!redirect.join("nested").exists());
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn post_preflight_non_directory_parent_is_an_operational_failure() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        let expected = root.join("output");
        std::fs::create_dir(&root).unwrap();
        let anchor = std::fs::canonicalize(&root).unwrap();
        let anchor = open_canonical_directory(&anchor, &expected).unwrap();
        let guard = DirectoryGuard::projected(expected.clone(), anchor);

        std::fs::write(&expected, b"replaced directory").unwrap();
        let error = guard.seal().unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
    }

    #[cfg(all(feature = "markdown", unix))]
    #[test]
    fn canonical_directory_authorization_rejects_identity_change_before_projection() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let approved = directory.path().join("approved");
        let displaced = directory.path().join("displaced");
        let redirect = directory.path().join("redirect");
        let requested = approved.join("new").join("nested");
        std::fs::create_dir(&approved).unwrap();
        std::fs::create_dir(&redirect).unwrap();

        let error = open_canonical_directory_with(&approved, &requested, |_| {
            std::fs::rename(&approved, &displaced).unwrap();
            symlink(&redirect, &approved).unwrap();
        })
        .unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
        assert!(!redirect.join("new").exists());
        assert!(!displaced.join("new").exists());
    }

    struct FailingWriteBackend;

    struct FailingStage;

    impl Write for FailingStage {
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected staging write failure"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl AtomicBackend for FailingWriteBackend {
        type Stage = FailingStage;

        fn open(&self, _path: &Path) -> std::io::Result<Self::Stage> {
            Ok(FailingStage)
        }

        fn verify_parent(
            &self,
            _stage: &Self::Stage,
            _path: &Path,
            _expected: &DirectoryIdentity,
        ) -> std::io::Result<()> {
            Ok(())
        }

        fn commit(&self, _stage: Self::Stage) -> std::io::Result<()> {
            panic!("a failed staging write must never reach commit")
        }
    }

    struct TargetSwapDuringWriteBackend;

    struct TargetSwapStage {
        stage: atomic_write_file::AtomicWriteFile,
        target: PathBuf,
        swapped: bool,
    }

    impl Write for TargetSwapStage {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if !self.swapped {
                std::fs::remove_file(&self.target)?;
                std::fs::write(&self.target, b"concurrent replacement")?;
                self.swapped = true;
            }
            self.stage.write(bytes)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.stage.flush()
        }
    }

    impl AtomicBackend for TargetSwapDuringWriteBackend {
        type Stage = TargetSwapStage;

        fn open(&self, path: &Path) -> std::io::Result<Self::Stage> {
            Ok(TargetSwapStage {
                stage: atomic_write_file::AtomicWriteFile::open(path)?,
                target: path.to_path_buf(),
                swapped: false,
            })
        }

        fn verify_parent(
            &self,
            stage: &Self::Stage,
            path: &Path,
            expected: &DirectoryIdentity,
        ) -> std::io::Result<()> {
            verify_atomic_parent(&stage.stage, path, expected)
        }

        fn commit(&self, stage: Self::Stage) -> std::io::Result<()> {
            stage.stage.commit()
        }
    }

    #[cfg(unix)]
    struct RedirectedStageBackend {
        redirected: PathBuf,
    }

    #[cfg(unix)]
    impl AtomicBackend for RedirectedStageBackend {
        type Stage = atomic_write_file::AtomicWriteFile;

        fn open(&self, _path: &Path) -> std::io::Result<Self::Stage> {
            atomic_write_file::AtomicWriteFile::open(&self.redirected)
        }

        fn verify_parent(
            &self,
            stage: &Self::Stage,
            path: &Path,
            expected: &DirectoryIdentity,
        ) -> std::io::Result<()> {
            verify_atomic_parent(stage, path, expected)
        }

        fn commit(&self, stage: Self::Stage) -> std::io::Result<()> {
            stage.commit()
        }
    }

    #[cfg(unix)]
    struct ParentSwapAfterOpenBackend {
        output_directory: PathBuf,
        displaced_directory: PathBuf,
        redirect_directory: PathBuf,
    }

    #[cfg(unix)]
    impl AtomicBackend for ParentSwapAfterOpenBackend {
        type Stage = atomic_write_file::AtomicWriteFile;

        fn open(&self, path: &Path) -> std::io::Result<Self::Stage> {
            use std::os::unix::fs::symlink;

            let stage = atomic_write_file::AtomicWriteFile::open(path)?;
            std::fs::rename(&self.output_directory, &self.displaced_directory)?;
            symlink(&self.redirect_directory, &self.output_directory)?;
            Ok(stage)
        }

        fn verify_parent(
            &self,
            stage: &Self::Stage,
            path: &Path,
            expected: &DirectoryIdentity,
        ) -> std::io::Result<()> {
            verify_atomic_parent(stage, path, expected)
        }

        fn commit(&self, stage: Self::Stage) -> std::io::Result<()> {
            stage.commit()
        }
    }

    #[test]
    fn staging_write_failure_preserves_existing_output() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("output.svg");
        std::fs::write(&target, b"complete old output").unwrap();
        let mut guards = PublicationGuards::new(Some(directory.path()));
        let target_guard =
            preflight_file_target(&target, directory.path(), &[], MissingParent::Reject).unwrap();
        guards.approve_exact(target_guard).unwrap();

        let error = publish_with_backend(&target, b"replacement", &guards, &FailingWriteBackend)
            .unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::WriteAtomicStaging,
                ..
            }
        ));
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"complete old output",
            "staging failures must leave the prior complete contents visible"
        );
    }

    #[test]
    fn target_identity_substitution_is_rejected_without_overwriting_new_contents() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("output.svg");
        std::fs::write(&target, b"preflight output").unwrap();
        let mut guards = PublicationGuards::new(Some(directory.path()));
        let target_guard =
            preflight_file_target(&target, directory.path(), &[], MissingParent::Reject).unwrap();
        guards.approve_exact(target_guard).unwrap();

        std::fs::remove_file(&target).unwrap();
        std::fs::write(&target, b"concurrent replacement").unwrap();

        let error = publish_atomic_file(&target, b"our replacement", &guards).unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
        assert_eq!(std::fs::read(&target).unwrap(), b"concurrent replacement");
    }

    #[test]
    fn target_substitution_during_staging_is_rejected_before_commit() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("output.svg");
        std::fs::write(&target, b"preflight output").unwrap();
        let mut guards = PublicationGuards::new(Some(directory.path()));
        let target_guard =
            preflight_file_target(&target, directory.path(), &[], MissingParent::Reject).unwrap();
        guards.approve_exact(target_guard).unwrap();

        let error = publish_with_backend(
            &target,
            b"our replacement",
            &guards,
            &TargetSwapDuringWriteBackend,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
        assert_eq!(std::fs::read(&target).unwrap(), b"concurrent replacement");
    }

    #[test]
    fn mutation_after_custom_verification_is_rechecked_before_commit() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("output.svg");
        std::fs::write(&target, b"acquired output").unwrap();
        let mut guards = PublicationGuards::new(Some(directory.path()));
        let target_guard =
            preflight_file_target(&target, directory.path(), &[], MissingParent::Reject).unwrap();
        guards.approve_exact(target_guard).unwrap();

        let error = publish_with_backend_and_verifier(
            &target,
            b"our replacement",
            &guards,
            &SystemAtomicBackend,
            |approved| {
                std::fs::remove_file(approved).unwrap();
                std::fs::write(approved, b"mutation after snapshot comparison").unwrap();
                Ok(())
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"mutation after snapshot comparison"
        );
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn same_inode_mutation_after_custom_verification_is_rechecked_before_commit() {
        use std::fs::OpenOptions;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("output.svg");
        std::fs::write(&target, b"acquired output").unwrap();
        let mut stdin = std::io::empty();
        let acquired = crate::io::read_fix_source(
            &crate::invocation::ResolvedInput::File(target.clone()),
            crate::input::InputLimit::new("max_source_bytes", Some(1024)),
            &mut stdin,
        )
        .unwrap();
        let original_identity = same_file::Handle::from_path(&target).unwrap();
        let mut guards = PublicationGuards::new(Some(directory.path()));
        let target_guard =
            preflight_file_target(&target, directory.path(), &[], MissingParent::Reject).unwrap();
        guards.approve_exact(target_guard).unwrap();
        let mut checks = 0_u8;

        let error = publish_with_backend_and_verifier(
            &target,
            b"our replacement",
            &guards,
            &SystemAtomicBackend,
            |approved| {
                checks = checks.saturating_add(1);
                acquired.verify_unchanged(approved)?;
                if checks == 1 {
                    let mut file = OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(approved)
                        .unwrap();
                    file.write_all(b"same inode concurrent edit").unwrap();
                    file.sync_all().unwrap();
                    assert_eq!(
                        same_file::Handle::from_path(approved).unwrap(),
                        original_identity
                    );
                }
                Ok(())
            },
        )
        .unwrap_err();

        assert!(matches!(error, CliError::ConcurrentModification { .. }));
        assert_eq!(checks, 2);
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"same inode concurrent edit"
        );
    }

    #[cfg(unix)]
    #[test]
    fn post_preflight_target_symlink_is_an_operational_failure() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("output.svg");
        let redirect = directory.path().join("redirect.svg");
        std::fs::write(&redirect, b"protected redirect").unwrap();
        let mut guards = PublicationGuards::new(Some(directory.path()));
        let target_guard =
            preflight_file_target(&target, directory.path(), &[], MissingParent::Reject).unwrap();
        guards.approve_exact(target_guard).unwrap();

        symlink(&redirect, &target).unwrap();
        let error = publish_atomic_file(&target, b"replacement", &guards).unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
        assert_eq!(std::fs::read(&redirect).unwrap(), b"protected redirect");
        assert!(
            std::fs::symlink_metadata(&target)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_stage_in_an_unapproved_directory_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let approved = directory.path().join("approved");
        let redirected = directory.path().join("redirected");
        std::fs::create_dir(&approved).unwrap();
        std::fs::create_dir(&redirected).unwrap();
        let target = approved.join("output.svg");
        let redirected_target = redirected.join("output.svg");
        let mut guards = PublicationGuards::new(Some(directory.path()));
        let target_guard =
            preflight_file_target(&target, directory.path(), &[], MissingParent::Reject).unwrap();
        guards.approve_exact(target_guard).unwrap();

        let error = publish_with_backend(
            &target,
            b"replacement",
            &guards,
            &RedirectedStageBackend {
                redirected: redirected_target.clone(),
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
        assert!(!target.exists());
        assert!(!redirected_target.exists());
    }

    #[cfg(unix)]
    #[test]
    fn parent_swap_after_atomic_open_is_rejected_before_commit() {
        let directory = tempfile::tempdir().unwrap();
        let output_directory = directory.path().join("output");
        let displaced_directory = directory.path().join("displaced");
        let redirect_directory = directory.path().join("redirect");
        std::fs::create_dir(&output_directory).unwrap();
        std::fs::create_dir(&redirect_directory).unwrap();
        let target = output_directory.join("output.svg");
        let mut guards = PublicationGuards::new(Some(directory.path()));
        let target_guard =
            preflight_file_target(&target, directory.path(), &[], MissingParent::Reject).unwrap();
        guards.approve_exact(target_guard).unwrap();

        let error = publish_with_backend(
            &target,
            b"replacement",
            &guards,
            &ParentSwapAfterOpenBackend {
                output_directory: output_directory.clone(),
                displaced_directory: displaced_directory.clone(),
                redirect_directory: redirect_directory.clone(),
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
        assert!(!displaced_directory.join("output.svg").exists());
        assert!(!redirect_directory.join("output.svg").exists());
    }

    #[cfg(unix)]
    #[test]
    fn parent_directory_substitution_is_rejected_before_publication() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let protected_dir = directory.path().join("protected");
        let output_dir = directory.path().join("output");
        let displaced_dir = directory.path().join("displaced");
        std::fs::create_dir(&protected_dir).unwrap();
        std::fs::create_dir(&output_dir).unwrap();
        let input_path = protected_dir.join("diagram.mmd");
        let output_path = output_dir.join("diagram.mmd");
        std::fs::write(&input_path, b"flowchart LR\nA-->B\n").unwrap();

        let input = ProtectedInput::inspect(
            "Input file",
            InputRole::Primary,
            &input_path,
            directory.path(),
        )
        .unwrap();
        let mut guards = PublicationGuards::new(Some(directory.path()));
        let target = preflight_file_target(
            &output_path,
            directory.path(),
            std::slice::from_ref(&input),
            MissingParent::Reject,
        )
        .unwrap();
        guards.approve_exact(target).unwrap();
        guards.protect(&[input]);

        std::fs::rename(&output_dir, &displaced_dir).unwrap();
        symlink(&protected_dir, &output_dir).unwrap();

        let error = publish_atomic_file(&output_path, b"replacement", &guards).unwrap_err();

        assert!(matches!(
            error,
            CliError::File {
                operation: FileOperation::VerifyPublication,
                ..
            }
        ));
        assert_eq!(
            std::fs::read(&input_path).unwrap(),
            b"flowchart LR\nA-->B\n",
            "a substituted output parent must not redirect publication onto an input"
        );
        assert!(!displaced_dir.join("diagram.mmd").exists());
    }
}
