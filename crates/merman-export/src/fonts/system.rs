//! Bounded metadata discovery and operation-local binary snapshots.

use super::registry::{read_bounded, snapshot_cmap};
use super::resolver::{ExpectedFace, FontCandidate};
use super::*;
use usvg::fontdb::{Database, FaceInfo, ID, Source};

pub(super) struct SystemCatalog {
    pub database: Arc<Database>,
    pub identity: Vec<u8>,
    faces: BTreeMap<ID, SystemFace>,
}

struct SystemFace {
    path: PathBuf,
    digest: Vec<u8>,
    coverage: Vec<u8>,
}

struct Discovery<'a> {
    catalog: SystemCatalog,
    limits: &'a NativeFontLimits,
    control: &'a OperationControl,
    files: usize,
    bytes: usize,
    entries: usize,
    cmap_bytes: usize,
    descriptors: BTreeSet<String>,
    hashes: BTreeSet<Vec<u8>>,
    identity: Sha256,
}

impl SystemCatalog {
    pub fn discover(
        loader: &FontLoader,
        control: &OperationControl,
    ) -> Result<Self, NativeFontError> {
        Self::from_roots(loader, control, &platform_roots())
    }

    fn from_roots(
        loader: &FontLoader,
        control: &OperationControl,
        roots: &[PathBuf],
    ) -> Result<Self, NativeFontError> {
        let mut discovery = Discovery {
            catalog: Self {
                database: Arc::new(loader.database.clone()),
                identity: Vec::new(),
                faces: BTreeMap::new(),
            },
            limits: &loader.limits,
            control,
            files: 0,
            bytes: 0,
            entries: 0,
            cmap_bytes: 0,
            descriptors: loader.descriptors.clone(),
            hashes: loader.hashes.clone(),
            identity: Sha256::new(),
        };
        for root in roots {
            control.terminal_checkpoint_at(OperationPhase::Admission)?;
            match fs::symlink_metadata(root) {
                Ok(_) => discovery.directory(root, 0)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(source_error(root, error)),
            }
        }
        super::super::configure_fontdb_generic_families(Arc::make_mut(
            &mut discovery.catalog.database,
        ));
        discovery.catalog.identity = discovery.identity.finalize().to_vec();
        Ok(discovery.catalog)
    }

    fn candidate(&self, face: &FaceInfo) -> FontCandidate {
        match self.faces.get(&face.id) {
            Some(record) => FontCandidate::File {
                path: record.path.clone(),
                index: face.index,
                expected: ExpectedFace::Bounded {
                    metadata: face.clone(),
                    digest: record.digest.clone(),
                },
            },
            None => FontCandidate::Loaded(face.clone()),
        }
    }

    pub(super) fn select(
        &self,
        font: &usvg::Font,
        control: &OperationControl,
    ) -> Result<Option<FontCandidate>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let selector = super::super::browser_like_font_resolver().select_font;
        let id = selector(font, &mut Arc::clone(&self.database));
        Ok(id
            .and_then(|id| self.database.face(id))
            .map(|face| self.candidate(face)))
    }

    pub(super) fn fallback(
        &self,
        character: char,
        base: &FaceInfo,
        excluded: &[FaceInfo],
        control: &OperationControl,
        accept: &mut dyn FnMut(FontCandidate) -> Result<Option<ID>, NativeFontError>,
    ) -> Result<Option<ID>, NativeFontError> {
        for face in self.database.faces() {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            if excluded.iter().any(|used| same_face(used, face))
                || (base.style != face.style
                    && base.weight != face.weight
                    && base.stretch != face.stretch)
                || !self.supports(face, character, control)?
            {
                continue;
            }
            if let Some(id) = accept(self.candidate(face))? {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    fn supports(
        &self,
        face: &FaceInfo,
        character: char,
        control: &OperationControl,
    ) -> Result<bool, NativeFontError> {
        if let Some(record) = self.faces.get(&face.id) {
            return cmap_supports(&record.coverage, character, control);
        }
        Ok(self
            .database
            .with_face_data(face.id, |bytes, index| {
                rustybuzz::ttf_parser::Face::parse(bytes, index)
                    .ok()
                    .is_some_and(|face| face.glyph_index(character).is_some())
            })
            .unwrap_or(false))
    }
}

pub(super) fn same_face(left: &FaceInfo, right: &FaceInfo) -> bool {
    left.post_script_name == right.post_script_name
        && left.index == right.index
        && left.families == right.families
        && left.style == right.style
        && left.weight == right.weight
        && left.stretch == right.stretch
}

impl Discovery<'_> {
    fn directory(&mut self, path: &Path, depth: usize) -> Result<(), NativeFontError> {
        self.control
            .terminal_checkpoint_at(OperationPhase::Admission)?;
        check_limit(
            "system-directory-depth",
            depth,
            self.limits.max_directory_depth,
        )?;
        let metadata = fs::symlink_metadata(path).map_err(|error| source_error(path, error))?;
        if metadata.is_symlink() || !metadata.is_dir() {
            return Err(source_error(path, "expected a directory, not a symlink"));
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(path).map_err(|error| source_error(path, error))? {
            self.control
                .terminal_checkpoint_at(OperationPhase::Admission)?;
            self.entries = self.entries.saturating_add(1);
            check_limit(
                "system-directory-entries",
                self.entries,
                self.limits.max_directory_entries,
            )?;
            entries.push(entry.map_err(|error| source_error(path, error))?);
        }
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|error| source_error(&path, error))?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                self.directory(&path, depth.saturating_add(1))?;
            } else if kind.is_file() && is_font_path(&path) {
                self.file(&path)?;
            }
        }
        Ok(())
    }

    fn file(&mut self, path: &Path) -> Result<(), NativeFontError> {
        self.files = self.files.saturating_add(1);
        check_limit("system-files", self.files, self.limits.max_system_files)?;
        // Discovery buffers are temporary; selected snapshots enforce the residency limit.
        let remaining = self.limits.max_system_scan_bytes.saturating_sub(self.bytes);
        let bytes = read_bounded(path, remaining, self.control, OperationPhase::Admission)?;
        self.bytes = self.bytes.saturating_add(bytes.len());
        let digest = Sha256::digest(&bytes).to_vec();
        if !self.hashes.insert(digest.clone()) {
            return Ok(());
        }
        let count = rustybuzz::ttf_parser::fonts_in_collection(&bytes).unwrap_or(1) as usize;
        check_limit(
            "system-font-faces",
            self.catalog.database.faces().count().saturating_add(count),
            self.limits.max_system_faces,
        )?;
        let mut candidate = Database::new();
        candidate.load_font_data(bytes);
        for face in candidate.faces() {
            self.control
                .terminal_checkpoint_at(OperationPhase::Admission)?;
            if !self.descriptors.insert(face_descriptor(face)) {
                continue;
            }
            let coverage = candidate
                .with_face_data(face.id, |bytes, index| {
                    coverage(
                        bytes,
                        index,
                        self.control,
                        &mut self.cmap_bytes,
                        self.limits,
                    )
                })
                .transpose()?
                .unwrap_or_default();
            let mut metadata = face.clone();
            metadata.source = Source::Binary(Arc::new(Vec::<u8>::new()));
            let id = Arc::make_mut(&mut self.catalog.database).push_face_info(metadata);
            self.catalog.faces.insert(
                id,
                SystemFace {
                    path: fs::canonicalize(path).map_err(|error| source_error(path, error))?,
                    digest: digest.clone(),
                    coverage,
                },
            );
            self.identity.update(&digest);
            self.identity.update(face.index.to_be_bytes());
        }
        Ok(())
    }
}

fn coverage(
    bytes: &[u8],
    index: u32,
    control: &OperationControl,
    metadata_bytes: &mut usize,
    limits: &NativeFontLimits,
) -> Result<Vec<u8>, NativeFontError> {
    control.terminal_checkpoint_at(OperationPhase::Admission)?;
    let Ok(face) = rustybuzz::ttf_parser::Face::parse(bytes, index) else {
        return Ok(Vec::new());
    };
    let table = face
        .raw_face()
        .table(rustybuzz::ttf_parser::Tag::from_bytes(b"cmap"))
        .unwrap_or_default();
    snapshot_cmap(table, control, metadata_bytes, limits)
}

fn cmap_supports(
    bytes: &[u8],
    character: char,
    control: &OperationControl,
) -> Result<bool, NativeFontError> {
    let Some(table) = rustybuzz::ttf_parser::cmap::Table::parse(bytes) else {
        return Ok(false);
    };
    for subtable in table
        .subtables
        .into_iter()
        .filter(|table| table.is_unicode())
    {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        if subtable
            .glyph_index(character as u32)
            .is_some_and(|glyph| glyph.0 != 0)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn platform_roots() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut roots = Vec::new();
        if let Some(root) = std::env::var_os("WINDIR") {
            roots.push(PathBuf::from(root).join("Fonts"));
        }
        if let Some(root) = std::env::var_os("LOCALAPPDATA") {
            roots.push(PathBuf::from(root).join("Microsoft/Windows/Fonts"));
        }
        roots
    }
    #[cfg(target_os = "macos")]
    {
        let mut roots = vec![
            PathBuf::from("/System/Library/Fonts"),
            PathBuf::from("/Library/Fonts"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Library/Fonts"));
        }
        roots
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Custom fontconfig search paths are not interpreted; use explicit font directories.
        let mut roots = vec![
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            roots.push(home.join(".fonts"));
            let data = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share"));
            roots.push(data.join("fonts"));
        }
        roots
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::registry::FontSourceRegistry;
    use super::super::resolver::DiscoveryProvider;
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn oversized_range_cmap(format: u16) -> Vec<u8> {
        let mut bytes = Vec::new();
        for number in [0_u16, 1, 0, 4] {
            bytes.extend_from_slice(&number.to_be_bytes());
        }
        bytes.extend_from_slice(&12_u32.to_be_bytes());
        bytes.extend_from_slice(&format.to_be_bytes());
        bytes.extend_from_slice(&0_u16.to_be_bytes());
        for number in [28_u32, 0, 1, 0, u32::MAX, 1] {
            bytes.extend_from_slice(&number.to_be_bytes());
        }
        bytes
    }

    #[test]
    fn huge_declared_cmap_ranges_are_never_enumerated() {
        for format in [12, 13] {
            let bytes = oversized_range_cmap(format);
            let control = OperationControl::new().with_deadline(std::time::Duration::from_secs(1));
            let started = std::time::Instant::now();
            let mut consumed = 0;
            let snapshot = snapshot_cmap(
                &bytes,
                &control,
                &mut consumed,
                &NativeFontLimits::default(),
            )
            .unwrap();
            assert_eq!(snapshot.len(), 40);
            assert_eq!(consumed, 40);
            assert!(cmap_supports(&snapshot, 'A', &control).unwrap());
            assert!(started.elapsed() < std::time::Duration::from_secs(1));
            control
                .terminal_checkpoint_at(OperationPhase::Admission)
                .unwrap();
            let mut limited = 0;
            assert!(matches!(
                snapshot_cmap(
                    &bytes,
                    &OperationControl::new(),
                    &mut limited,
                    &NativeFontLimits {
                        max_system_cmap_bytes: 39,
                        ..Default::default()
                    }
                ),
                Err(NativeFontError::Limit {
                    resource: "system-cmap-bytes",
                    ..
                })
            ));
            assert_eq!(limited, 0);
            let cancelled = OperationControl::new();
            cancelled.cancel();
            assert!(
                snapshot_cmap(
                    &bytes,
                    &cancelled,
                    &mut limited,
                    &NativeFontLimits::default()
                )
                .is_err()
            );
            assert_eq!(limited, 0);
        }
    }

    #[test]
    fn aliased_encoding_records_have_an_independent_bound() {
        let count = 33_u16;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0_u16.to_be_bytes());
        bytes.extend_from_slice(&count.to_be_bytes());
        let offset = 4 + u32::from(count) * 8;
        for _ in 0..count {
            bytes.extend_from_slice(&0_u16.to_be_bytes());
            bytes.extend_from_slice(&4_u16.to_be_bytes());
            bytes.extend_from_slice(&offset.to_be_bytes());
        }
        bytes.extend_from_slice(&oversized_range_cmap(13)[12..]);
        let mut consumed = 0;
        assert!(matches!(
            snapshot_cmap(
                &bytes,
                &OperationControl::new(),
                &mut consumed,
                &NativeFontLimits::default()
            ),
            Err(NativeFontError::Limit {
                resource: "system-cmap-subtables",
                ..
            })
        ));
        assert_eq!(consumed, 0);
    }

    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "merman-system-fonts-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
        fn copy(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fonts")
                    .join(name),
                &path,
            )
            .unwrap();
            path
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn context(
        root: &Root,
        limits: NativeFontLimits,
        control: &OperationControl,
    ) -> Result<NativeFontContext, NativeFontError> {
        context_from_loader(root, FontLoader::new(limits, control.clone()), control)
    }

    fn context_from_loader(
        root: &Root,
        loader: FontLoader,
        control: &OperationControl,
    ) -> Result<NativeFontContext, NativeFontError> {
        let catalog = SystemCatalog::from_roots(&loader, control, std::slice::from_ref(&root.0))?;
        let default_family = super::super::super::raster_default_font_family(&catalog.database)
            .ok_or(NativeFontError::EmptyFontSet)?;
        let sources = FontSourceRegistry::new(
            loader.initial_sources,
            loader.source_aliases,
            loader.files,
            loader.database.faces().count(),
        );
        Ok(NativeFontContext(Arc::new(FontContextInner {
            database: Arc::new(loader.database),
            discovery: Some(DiscoveryProvider::Bounded(Arc::new(catalog))),
            sources,
            default_family,
            identity: "fixture-system".to_owned(),
            limits: loader.limits,
            control: control.clone(),
            state: Mutex::new(MeasurementState::default()),
            fatal: OnceLock::new(),
        })))
    }

    fn probe(context: &NativeFontContext, label: &str) -> HostMeasurementResult {
        probe_weight(context, label, "400")
    }

    fn probe_weight(
        context: &NativeFontContext,
        label: &str,
        weight: &str,
    ) -> HostMeasurementResult {
        let style = TextStyle {
            font_family: Some("Cousine".to_owned()),
            font_size: 20.0,
            font_weight: Some(weight.to_owned()),
            ..TextStyle::default()
        };
        context.measure(HostTextMeasurementRequest {
            operation: TextMeasurementOperation::SimpleBBoxWidth,
            phase: merman_render::environment::TextMeasurementPhase::SvgBBox,
            text: label,
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        })
    }

    #[test]
    fn explicit_and_discovered_files_share_one_admission_limit() {
        let root = Root::new();
        let regular = root.copy("Cousine-Regular.ttf");
        root.copy("Cousine-Bold.ttf");
        let control = OperationControl::new();
        let mut loader = FontLoader::new(
            NativeFontLimits {
                max_files: 1,
                ..Default::default()
            },
            control.clone(),
        );
        loader.file(&regular).unwrap();
        let context = context_from_loader(&root, loader, &control).unwrap();
        let resident = context.resident_font_bytes();
        assert!(probe(&context, "explicit").unwrap().is_some());
        assert!(probe_weight(&context, "discovered", "700").is_err());
        assert!(matches!(
            context.check(),
            Err(NativeFontError::Limit {
                resource: "system-snapshot-files",
                actual: 2,
                limit: 1
            })
        ));
        assert_eq!(context.resident_font_bytes(), resident);
    }

    #[test]
    fn separate_measurement_databases_cannot_reset_admitted_face_limits() {
        let root = Root::new();
        root.copy("Cousine-Regular.ttf");
        root.copy("Cousine-Bold.ttf");
        let context = context(
            &root,
            NativeFontLimits {
                max_faces: 1,
                ..Default::default()
            },
            &OperationControl::new(),
        )
        .unwrap();
        assert!(probe(&context, "first face").unwrap().is_some());
        assert!(probe_weight(&context, "second face", "700").is_err());
        assert!(matches!(
            context.check(),
            Err(NativeFontError::Limit {
                resource: "font-faces",
                actual: 2,
                limit: 1
            })
        ));
    }

    #[test]
    fn initial_and_snapshot_bytes_share_one_residency_limit() {
        let root = Root::new();
        let regular = root.copy("Cousine-Regular.ttf");
        let bold = root.copy("Cousine-Bold.ttf");
        let max_bytes = usize::try_from(
            fs::metadata(&regular).unwrap().len() + fs::metadata(&bold).unwrap().len() - 1,
        )
        .unwrap();
        let control = OperationControl::new();
        let mut loader = FontLoader::new(
            NativeFontLimits {
                max_bytes,
                ..Default::default()
            },
            control.clone(),
        );
        loader.file(&regular).unwrap();
        let context = context_from_loader(&root, loader, &control).unwrap();
        let resident = context.resident_font_bytes();
        assert!(probe_weight(&context, "bold", "700").is_err());
        assert!(matches!(
            context.check(),
            Err(NativeFontError::Limit {
                resource: "system-font-bytes",
                ..
            })
        ));
        assert_eq!(context.resident_font_bytes(), resident);
    }

    #[test]
    fn operation_forks_share_initial_bytes_and_reset_only_dynamic_snapshots() {
        let root = Root::new();
        let regular = root.copy("Cousine-Regular.ttf");
        let bold = root.copy("Cousine-Bold.ttf");
        let control = OperationControl::new();
        let mut loader = FontLoader::new(NativeFontLimits::default(), control.clone());
        loader.file(&regular).unwrap();
        let context = context_from_loader(&root, loader, &control).unwrap();
        let initial_bytes = context.resident_font_bytes();
        assert!(probe_weight(&context, "bold", "700").unwrap().is_some());
        assert!(context.resident_font_bytes() > initial_bytes);
        let next = context.for_operation(&OperationControl::new());
        assert_eq!(next.resident_font_bytes(), initial_bytes);
        assert!(next.selected_faces().is_empty());
        fs::remove_file(regular).unwrap();
        fs::remove_file(bold).unwrap();
        assert!(probe(&next, "initial bytes survive").unwrap().is_some());
        assert_eq!(next.resident_font_bytes(), initial_bytes);
        assert!(
            probe_weight(&context, "admitted bold survives", "700")
                .unwrap()
                .is_some()
        );
        assert!(probe_weight(&next, "unadmitted bold fails", "700").is_err());
        assert!(context.check().is_ok());
    }

    #[test]
    fn catalog_has_no_file_sources_or_resident_system_bytes() {
        let root = Root::new();
        root.copy("Cousine-Regular.ttf");
        root.copy("Cousine-Bold.ttf");
        let context =
            context(&root, NativeFontLimits::default(), &OperationControl::new()).unwrap();
        assert!(context.uses_system_fonts());
        assert_eq!(context.face_count(), 2);
        assert_eq!(context.resident_font_bytes(), 0);
        let Some(DiscoveryProvider::Bounded(catalog)) = &context.0.discovery else {
            panic!("fixture uses bounded discovery")
        };
        assert!(
            catalog
                .database
                .faces()
                .all(|face| matches!(face.source, Source::Binary(_)))
        );
        assert!(probe(&context, "AV measurement").unwrap().is_some());
        assert!(context.resident_font_bytes() > 0);
        assert_eq!(context.0.sources.snapshots.lock().unwrap().files.len(), 1);
    }

    #[test]
    fn snapshot_stays_stable_after_source_is_changed_or_removed() {
        let root = Root::new();
        let path = root.copy("Cousine-Regular.ttf");
        let context =
            context(&root, NativeFontLimits::default(), &OperationControl::new()).unwrap();
        let Some(HostTextMeasurement::Length(before)) = probe(&context, "AV").unwrap() else {
            panic!("expected scalar width")
        };
        fs::write(&path, b"changed").unwrap();
        assert!(probe(&context, "VA").unwrap().is_some());
        fs::remove_file(&path).unwrap();
        let Some(HostTextMeasurement::Length(after)) = probe(&context, "AV").unwrap() else {
            panic!("expected scalar width")
        };
        assert_eq!(after, before);
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options(&mut options);
        let tree = usvg::Tree::from_str(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="30"><text x="0" y="20" font-family="Cousine">Paint</text></svg>"#, &options).unwrap();
        assert!(!tree.root().children().is_empty());
        assert_eq!(tree.fontdb().faces().count(), 1);
        assert!(
            tree.fontdb()
                .faces()
                .all(|face| matches!(face.source, Source::Binary(_)))
        );
        assert!(context.check().is_ok());
    }

    #[test]
    fn changed_or_unreadable_unsnapshotted_sources_fail_closed() {
        for remove in [false, true] {
            let root = Root::new();
            let path = root.copy("Cousine-Regular.ttf");
            let context =
                context(&root, NativeFontLimits::default(), &OperationControl::new()).unwrap();
            if remove {
                fs::remove_file(&path).unwrap();
            } else {
                fs::write(&path, b"changed").unwrap();
            }
            assert!(probe(&context, "AV").is_err());
            assert!(context.check().is_err());
        }
    }

    #[test]
    fn discovery_uses_scan_budget_while_selected_bytes_use_residency_budget() {
        let root = Root::new();
        let font = root.copy("Cousine-Regular.ttf");
        let byte_count = usize::try_from(fs::metadata(&font).unwrap().len()).unwrap();
        let limits = NativeFontLimits {
            max_bytes: byte_count - 1,
            max_system_scan_bytes: byte_count,
            ..Default::default()
        };
        let fonts = context(&root, limits.clone(), &OperationControl::new()).unwrap();
        assert_eq!(fonts.resident_font_bytes(), 0);
        fonts.check().unwrap();
        assert!(probe(&fonts, "AV").is_err());
        assert!(matches!(
            fonts.check(),
            Err(NativeFontError::Limit {
                resource: "system-font-bytes",
                actual,
                limit,
            }) if actual == byte_count && limit == byte_count - 1
        ));
        assert_eq!(fonts.resident_font_bytes(), 0);
        assert!(matches!(
            context(
                &root,
                NativeFontLimits {
                    max_system_scan_bytes: byte_count - 1,
                    ..limits
                },
                &OperationControl::new(),
            ),
            Err(NativeFontError::Limit {
                resource: "system-font-bytes",
                actual,
                limit,
            }) if actual == byte_count && limit == byte_count - 1
        ));
    }

    #[test]
    fn discovery_and_snapshot_limits_are_independent() {
        let root = Root::new();
        root.copy("Cousine-Regular.ttf");
        root.copy("Cousine-Bold.ttf");
        assert!(matches!(
            context(
                &root,
                NativeFontLimits {
                    max_system_files: 1,
                    ..Default::default()
                },
                &OperationControl::new()
            ),
            Err(NativeFontError::Limit {
                resource: "system-files",
                ..
            })
        ));
        assert!(
            context(
                &root,
                NativeFontLimits {
                    max_system_scan_bytes: 1,
                    ..Default::default()
                },
                &OperationControl::new()
            )
            .is_err()
        );
        assert!(
            context(
                &root,
                NativeFontLimits {
                    max_system_faces: 1,
                    ..Default::default()
                },
                &OperationControl::new()
            )
            .is_err()
        );
        assert!(
            context(
                &root,
                NativeFontLimits {
                    max_directory_entries: 1,
                    ..Default::default()
                },
                &OperationControl::new()
            )
            .is_err()
        );
        let context = context(
            &root,
            NativeFontLimits {
                max_files: 0,
                ..Default::default()
            },
            &OperationControl::new(),
        )
        .unwrap();
        assert!(probe(&context, "AV").is_err());
    }

    #[test]
    fn cancellation_and_fatal_state_are_operation_local() {
        let root = Root::new();
        root.copy("Cousine-Regular.ttf");
        let cancelled = OperationControl::new();
        cancelled.cancel();
        assert!(matches!(
            context(&root, NativeFontLimits::default(), &cancelled),
            Err(NativeFontError::Operation(_))
        ));
        let control = OperationControl::new();
        let context = context(&root, NativeFontLimits::default(), &control).unwrap();
        let next = context.for_operation(&OperationControl::new());
        control.cancel();
        assert!(probe(&context, "AV").is_err());
        assert!(probe(&next, "AV").unwrap().is_some());
    }

    #[test]
    fn export_resolver_observes_active_control_without_resetting_shared_snapshots() {
        let root = Root::new();
        root.copy("Cousine-Regular.ttf");
        let context =
            context(&root, NativeFontLimits::default(), &OperationControl::new()).unwrap();
        assert!(probe(&context, "AV").unwrap().is_some());
        let resident = context.resident_font_bytes();
        let active = OperationControl::new();
        active.cancel();
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options_controlled(&mut options, &active);
        let _ = usvg::Tree::from_str(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="30"><text x="0" y="20" font-family="Cousine">Paint</text></svg>"#,
            &options,
        );
        assert!(context.check().is_err());
        assert_eq!(context.resident_font_bytes(), resident);
    }

    #[test]
    fn missing_coverage_does_not_load_every_font() {
        let root = Root::new();
        root.copy("Cousine-Regular.ttf");
        root.copy("Cousine-Bold.ttf");
        let context =
            context(&root, NativeFontLimits::default(), &OperationControl::new()).unwrap();
        assert!(probe(&context, "\u{10ffff}").unwrap().is_none());
        assert_eq!(context.0.sources.snapshots.lock().unwrap().files.len(), 1);
    }

    #[test]
    fn mixed_script_fallback_materializes_only_actual_faces() {
        let root = Root::new();
        root.copy("Cousine-Regular.ttf");
        root.copy("Cousine-Bold.ttf");
        root.copy("NotoSansSC-subset.ttf");
        let context =
            context(&root, NativeFontLimits::default(), &OperationControl::new()).unwrap();
        assert!(probe(&context, "A\u{4e2d}V").unwrap().is_some());
        assert_eq!(context.0.sources.snapshots.lock().unwrap().files.len(), 2);
        assert_eq!(context.selected_faces().len(), 2);
        assert!(context.check().is_ok());
    }

    #[test]
    fn fallback_selection_matches_measurement_independently_of_paint_order() {
        let root = Root::new();
        root.copy("Cousine-Regular.ttf");
        root.copy("NotoSans-Latin-subset.ttf");
        root.copy("NotoSansSC-subset.ttf");
        let shared = context(&root, NativeFontLimits::default(), &OperationControl::new()).unwrap();
        let style = TextStyle {
            font_family: Some("Noto Sans SC".to_owned()),
            font_size: 20.0,
            font_weight: Some("400".to_owned()),
            ..TextStyle::default()
        };
        let other = r#"<text font-family="Noto Sans" x="0" y="20">A</text>"#;
        for (before, after) in [(other, ""), ("", other), ("", "")] {
            let active = shared.for_operation(&OperationControl::new());
            let measured = active
                .measure(HostTextMeasurementRequest {
                    operation: TextMeasurementOperation::CanvasMeasureTextWidth,
                    phase: merman_render::environment::TextMeasurementPhase::ComputedLength,
                    text: "A",
                    style: &style,
                    max_width: None,
                    wrap_mode: WrapMode::SvgLike,
                })
                .unwrap()
                .unwrap();
            let HostTextMeasurement::Length(expected) = measured else {
                panic!("expected computed length, got {measured:?}");
            };
            assert_eq!(active.selected_faces(), ["Cousine-Regular#0"]);
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="60" font-size="20">{before}<text id="left" font-family="Noto Sans SC" x="100" y="40">A</text><text id="right" font-family="Noto Sans SC" text-anchor="end" x="100" y="40">A</text>{after}</svg>"#,
            );
            let mut options = usvg::Options::default();
            active.apply_to_usvg_options(&mut options);
            let tree = usvg::Tree::from_str(&source, &options).unwrap();
            let Some(usvg::Node::Text(left)) = tree.node_by_id("left") else {
                panic!("missing left text");
            };
            let Some(usvg::Node::Text(right)) = tree.node_by_id("right") else {
                panic!("missing right text");
            };
            let painted = f64::from(left.bounding_box().x() - right.bounding_box().x()).abs();
            assert!(
                (expected - painted).abs() < 0.001,
                "measurement={expected}, paint={painted}, before={before:?}, after={after:?}"
            );
            for text in [left, right] {
                for glyph in text
                    .layouted()
                    .iter()
                    .flat_map(|span| &span.positioned_glyphs)
                {
                    let face = tree.fontdb().face(glyph.font).unwrap();
                    assert_eq!(face.post_script_name, "Cousine-Regular");
                }
            }
            active.check().unwrap();
        }
    }

    #[test]
    fn explicit_faces_precede_system_duplicates() {
        let root = Root::new();
        let path = root.copy("Cousine-Regular.ttf");
        let control = OperationControl::new();
        let mut loader = FontLoader::new(NativeFontLimits::default(), control.clone());
        loader.file(&path).unwrap();
        let catalog =
            SystemCatalog::from_roots(&loader, &control, std::slice::from_ref(&root.0)).unwrap();
        assert_eq!(catalog.database.faces().count(), 1);
        assert!(catalog.faces.is_empty());
        assert!(binary_bytes(&catalog.database) > 0);
    }

    #[cfg(unix)]
    #[test]
    fn discovered_symlinks_are_skipped_and_replaced_sources_rejected() {
        let root = Root::new();
        let path = root.copy("Cousine-Regular.ttf");
        std::os::unix::fs::symlink(&path, root.0.join("linked.ttf")).unwrap();
        let context =
            context(&root, NativeFontLimits::default(), &OperationControl::new()).unwrap();
        assert_eq!(context.face_count(), 1);
        let target = root.0.join("font-copy.ttf");
        fs::rename(&path, &target).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(probe(&context, "AV").is_err());
    }
}
