//! One operation-local admission boundary for all discovered font sources.

use super::resolver::{ExpectedFace, FontCandidate};
use super::*;
use usvg::fontdb::{Database, FaceInfo, ID, Source};

struct InitialSources {
    files: BTreeMap<PathBuf, Arc<Vec<u8>>>,
    aliases: BTreeMap<PathBuf, PathBuf>,
    acquired_files: usize,
    faces: usize,
    bytes: usize,
}

#[derive(Clone, Default)]
pub(super) struct SnapshotState {
    pub(super) files: BTreeMap<PathBuf, FontSnapshot>,
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    selected_faces: BTreeMap<(PathBuf, u32), Arc<Vec<u8>>>,
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    native_faces: std::collections::HashMap<platform::coretext::NativeFace, NativeSnapshot>,
    #[cfg(feature = "experimental-system-font-query")]
    selections: std::collections::HashMap<SelectionRequest, SelectedFont>,
    #[cfg(feature = "experimental-system-font-query")]
    selection_bytes: usize,
    aliases: BTreeMap<PathBuf, PathBuf>,
    faces: BTreeMap<(PathBuf, u32), FaceInfo>,
    cmap_bytes: usize,
    bytes: usize,
    #[cfg(feature = "experimental-system-font-query")]
    collection: Option<CollectionMetadata>,
}

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
#[derive(Clone)]
struct NativeSnapshot {
    path: PathBuf,
    face: FaceInfo,
}

#[cfg(feature = "experimental-system-font-query")]
#[derive(Clone, Hash, Eq, PartialEq)]
pub(super) enum SelectionRequest {
    Font(usvg::Font),
    Fallback {
        font: Option<usvg::Font>,
        text: String,
        language: Option<String>,
        rtl: bool,
        excluded: Vec<AdmittedFaceKey>,
    },
}

/// Retain the source so pointer identity cannot be recycled while a request is pinned.
#[cfg(feature = "experimental-system-font-query")]
#[derive(Clone)]
pub(super) struct AdmittedFaceKey {
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    index: u32,
}

#[cfg(feature = "experimental-system-font-query")]
impl PartialEq for AdmittedFaceKey {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Arc::ptr_eq(&self.data, &other.data)
    }
}

#[cfg(feature = "experimental-system-font-query")]
impl Eq for AdmittedFaceKey {}

#[cfg(feature = "experimental-system-font-query")]
impl std::hash::Hash for AdmittedFaceKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&(Arc::as_ptr(&self.data) as *const ()), state);
        std::hash::Hash::hash(&self.index, state);
    }
}

#[cfg(feature = "experimental-system-font-query")]
impl SelectionRequest {
    pub(super) fn fallback(
        request: super::resolver::GlyphFallback<'_>,
        excluded: &[FaceInfo],
    ) -> Option<Self> {
        let keys = excluded
            .iter()
            .map(|face| {
                let Source::Binary(data) = &face.source else {
                    return None;
                };
                Some(AdmittedFaceKey {
                    data: Arc::clone(data),
                    index: face.index,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self::Fallback {
            font: request.font.cloned(),
            text: request.text.to_owned(),
            language: request.language.map(str::to_owned),
            rtl: request.rtl,
            excluded: keys,
        })
    }

    fn heap_bytes(&self) -> usize {
        let font_bytes = |font: &usvg::Font| {
            font.families().iter().fold(
                font.variations()
                    .len()
                    .saturating_mul(size_of::<usvg::FontVariation>()),
                |bytes, family| {
                    bytes
                        .saturating_add(size_of::<usvg::FontFamily>())
                        .saturating_add(match family {
                            usvg::FontFamily::Named(name) => name.len(),
                            _ => 0,
                        })
                },
            )
        };
        match self {
            Self::Font(font) => font_bytes(font),
            Self::Fallback {
                font,
                text,
                language,
                excluded,
                ..
            } => excluded
                .len()
                .saturating_mul(size_of::<AdmittedFaceKey>())
                .saturating_add(font.as_ref().map_or(0, font_bytes))
                .saturating_add(text.len())
                .saturating_add(language.as_ref().map_or(0, String::len)),
        }
    }
}

#[cfg(feature = "experimental-system-font-query")]
#[derive(Clone)]
pub(super) struct SelectedFont {
    pub(super) face: FaceInfo,
    pub(super) source: NativeExportFontSource,
    pub(super) scale: f32,
    pub(super) family_fallback_used: bool,
}

impl SnapshotState {
    fn face_count(&self) -> usize {
        let count = self.faces.len();
        #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
        let count = count.saturating_add(self.native_faces.len());
        count
    }

    fn source_paths(&self) -> BTreeSet<&PathBuf> {
        let paths: BTreeSet<_> = self.files.keys().collect();
        #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
        let paths = {
            let mut paths = paths;
            paths.extend(self.selected_faces.keys().map(|(path, _)| path));
            paths.extend(self.native_faces.values().map(|snapshot| &snapshot.path));
            paths
        };
        paths
    }
}

#[derive(Clone)]
pub(super) struct FontSnapshot {
    data: Arc<Vec<u8>>,
    /// The digest checked against bounded discovery when these bytes were first read.
    discovery_digest: Option<[u8; 32]>,
}

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos", test))]
pub(super) struct FaceData {
    pub(super) data: Arc<Vec<u8>>,
    pub(super) index: u32,
}

#[cfg(feature = "experimental-system-font-query")]
#[derive(Clone)]
struct CollectionMetadata {
    source: Source,
    faces: Arc<[FaceInfo]>,
}

#[cfg(feature = "experimental-system-font-query")]
impl SnapshotState {
    fn platform_face(&mut self, source: Source, index: u32, count: u32) -> Option<FaceInfo> {
        if let Some(collection) = &self.collection
            && same_source(&collection.source, &source)
        {
            let position = collection
                .faces
                .binary_search_by_key(&index, |face| face.index)
                .ok()?;
            return Some(collection.faces[position].clone());
        }

        let mut parsed = Database::new();
        parsed.load_font_source(source.clone());
        let selected = parsed.faces().find(|face| face.index == index)?.clone();
        if count > 1 {
            // fontdb loads every collection member. Retain at most one bounded metadata
            // table per operation, using the same immutable bytes as the selected faces.
            const MAX_COLLECTION_METADATA_BYTES: usize = 64 * 1024;
            let bytes = parsed.faces().fold(0_usize, |sum, face| {
                sum.saturating_add(size_of::<FaceInfo>())
                    .saturating_add(face.post_script_name.len())
                    .saturating_add(
                        face.families
                            .len()
                            .saturating_mul(size_of::<(String, usvg::fontdb::Language)>()),
                    )
                    .saturating_add(face.families.iter().map(|(name, _)| name.len()).sum())
            });
            if bytes <= MAX_COLLECTION_METADATA_BYTES {
                let mut faces: Vec<_> = parsed.faces().cloned().collect();
                faces.sort_unstable_by_key(|face| face.index);
                self.collection = Some(CollectionMetadata {
                    source,
                    faces: faces.into(),
                });
            }
        }
        Some(selected)
    }
}

/// Initial bytes are immutable and shared; later admissions belong to one operation.
pub(super) struct FontSourceRegistry {
    initial: Arc<InitialSources>,
    pub(super) snapshots: Mutex<SnapshotState>,
    #[cfg(feature = "experimental-system-font-query")]
    selection_gate: Mutex<()>,
}

impl FontSourceRegistry {
    pub(super) fn new(
        files: BTreeMap<PathBuf, Arc<Vec<u8>>>,
        aliases: BTreeMap<PathBuf, PathBuf>,
        acquired_files: usize,
        faces: usize,
    ) -> Self {
        let mut identities = BTreeSet::new();
        let bytes = files
            .values()
            .filter(|data| identities.insert(Arc::as_ptr(data)))
            .map(|data| data.len())
            .sum();
        Self {
            initial: Arc::new(InitialSources {
                files,
                aliases,
                acquired_files,
                faces,
                bytes,
            }),
            snapshots: Mutex::new(SnapshotState::default()),
            #[cfg(feature = "experimental-system-font-query")]
            selection_gate: Mutex::new(()),
        }
    }

    pub(super) fn for_operation(&self) -> Self {
        Self {
            initial: Arc::clone(&self.initial),
            snapshots: Mutex::new(SnapshotState::default()),
            #[cfg(feature = "experimental-system-font-query")]
            selection_gate: Mutex::new(()),
        }
    }

    pub(super) fn for_export(&self) -> Result<Self, NativeFontError> {
        #[cfg(feature = "experimental-system-font-query")]
        let _selection = self.lock_selection()?;
        let snapshots = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
        Ok(Self {
            initial: Arc::clone(&self.initial),
            snapshots: Mutex::new(snapshots.clone()),
            #[cfg(feature = "experimental-system-font-query")]
            selection_gate: Mutex::new(()),
        })
    }

    /// Serialize cold selection through admission and pinning. Otherwise a second
    /// query can fail after another thread has selected the same request successfully.
    #[cfg(feature = "experimental-system-font-query")]
    pub(super) fn lock_selection(&self) -> Result<std::sync::MutexGuard<'_, ()>, NativeFontError> {
        self.selection_gate.lock().map_err(|_| {
            NativeFontError::Measurement("font selection state is poisoned".to_owned())
        })
    }

    pub(super) fn resident_bytes(&self) -> usize {
        self.initial.bytes.saturating_add(
            self.snapshots
                .lock()
                .map(|state| state.bytes)
                .unwrap_or_default(),
        )
    }

    pub(super) fn admitted_faces(&self) -> usize {
        self.initial.faces.saturating_add(
            self.snapshots
                .lock()
                .map(|state| state.face_count())
                .unwrap_or_default(),
        )
    }

    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    pub(super) fn native_origin(
        &self,
        face: &FaceInfo,
    ) -> Result<Option<platform::coretext::NativeFace>, NativeFontError> {
        let state = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
        Ok(state.native_faces.iter().find_map(|(origin, snapshot)| {
            (face.index == snapshot.face.index && same_source(&face.source, &snapshot.face.source))
                .then(|| origin.clone())
        }))
    }

    #[cfg(feature = "experimental-system-font-query")]
    pub(super) fn selected(
        &self,
        request: &SelectionRequest,
        database: &mut Arc<Database>,
    ) -> Result<Option<(ID, SelectedFont)>, NativeFontError> {
        let state = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
        Ok(state
            .selections
            .get(request)
            .map(|selected| (install(selected.face.clone(), database), selected.clone())))
    }

    #[cfg(feature = "experimental-system-font-query")]
    pub(super) fn remember_selection(
        &self,
        request: SelectionRequest,
        selected: SelectedFont,
        limits: &NativeFontLimits,
        database: &mut Arc<Database>,
    ) -> Result<(ID, SelectedFont), NativeFontError> {
        let mut state = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
        if let Some(existing) = state.selections.get(&request) {
            return Ok((install(existing.face.clone(), database), existing.clone()));
        }
        check_limit(
            "font-selections",
            state.selections.len().saturating_add(1),
            limits.max_selection_entries,
        )?;
        let bytes = size_of::<(SelectionRequest, SelectedFont)>()
            .saturating_add(request.heap_bytes())
            .saturating_add(selected.face.post_script_name.len())
            .saturating_add(
                selected
                    .face
                    .families
                    .iter()
                    .fold(0_usize, |bytes, (name, _)| {
                        bytes
                            .saturating_add(size_of::<(String, usvg::fontdb::Language)>())
                            .saturating_add(name.len())
                    }),
            );
        let total = state.selection_bytes.saturating_add(bytes);
        check_limit("font-selection-bytes", total, limits.max_selection_bytes)?;
        state.selection_bytes = total;
        let id = install(selected.face.clone(), database);
        state.selections.insert(request, selected.clone());
        Ok((id, selected))
    }

    fn source_key(&self, state: &SnapshotState, path: &Path) -> std::io::Result<PathBuf> {
        // Admitted aliases retain their identity even after deletion or replacement.
        if let Some(key) = self
            .initial
            .aliases
            .get(path)
            .or_else(|| state.aliases.get(path))
        {
            Ok(key.clone())
        } else if self.initial.files.contains_key(path) || state.files.contains_key(path) {
            Ok(path.to_owned())
        } else {
            fs::canonicalize(path)
        }
    }

    /// Identity probes must prefer admitted bytes over a file that may have changed.
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos", test))]
    pub(super) fn snapshot(&self, path: &Path) -> Result<Option<Arc<Vec<u8>>>, NativeFontError> {
        let state = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
        let key = self.source_key(&state, path).ok();
        let key = key.as_deref().unwrap_or(path);
        Ok(self.initial.files.get(key).cloned().or_else(|| {
            state
                .files
                .get(key)
                .map(|snapshot| Arc::clone(&snapshot.data))
        }))
    }

    /// A standalone snapshot keeps the original index in the registry key, not in its data.
    /// Explicit initial bytes win; a selected face precedes any later whole-file snapshot.
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos", test))]
    pub(super) fn face_snapshot(
        &self,
        path: &Path,
        origin_index: u32,
    ) -> Result<Option<FaceData>, NativeFontError> {
        let state = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
        let key = self.source_key(&state, path).ok();
        let key = key.as_deref().unwrap_or(path);
        if let Some(data) = self.initial.files.get(key) {
            return Ok(Some(FaceData {
                data: Arc::clone(data),
                index: origin_index,
            }));
        }
        if let Some(data) = state.selected_faces.get(&(key.to_owned(), origin_index)) {
            return Ok(Some(FaceData {
                data: Arc::clone(data),
                index: 0,
            }));
        }
        Ok(state.files.get(key).map(|snapshot| FaceData {
            data: Arc::clone(&snapshot.data),
            index: origin_index,
        }))
    }

    pub(super) fn materialize(
        &self,
        candidate: FontCandidate,
        fallback: Option<char>,
        database: &mut Arc<Database>,
        limits: &NativeFontLimits,
        control: &OperationControl,
    ) -> Result<Option<ID>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        #[cfg(not(feature = "experimental-system-font-query"))]
        let _ = fallback;
        let (path, index, expected) = match candidate {
            FontCandidate::Loaded(face) => return Ok(Some(install(face, database))),
            #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
            FontCandidate::Native(face) => {
                return self.materialize_native(face, fallback, database, limits, control);
            }
            FontCandidate::File {
                path,
                index,
                expected,
            } => (path, index, expected),
        };
        let mut state = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
        // Already admitted bytes remain usable even if their file disappears later.
        let key_path = self
            .source_key(&state, &path)
            .map_err(|error| source_error(&path, error))?;
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let key = (key_path.clone(), index);
        #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
        let selected = state.selected_faces.get(&key).cloned();
        #[cfg(not(all(feature = "experimental-system-font-query", target_os = "macos")))]
        let selected: Option<Arc<Vec<u8>>> = None;
        let mut data_index = index;
        // Keep this precedence aligned with face_snapshot's identity probes.
        let (data, discovery_digest) = if let Some(data) = self.initial.files.get(&key_path) {
            (Arc::clone(data), None)
        } else if let Some(data) = selected {
            data_index = 0;
            (data, None)
        } else if let Some(snapshot) = state.files.get(&key_path) {
            (Arc::clone(&snapshot.data), snapshot.discovery_digest)
        } else {
            // Discovery may reject a missing glyph before acquiring its entire source.
            // Existing snapshots always win: a changed file cannot invalidate admitted bytes.
            #[cfg(feature = "experimental-system-font-query")]
            if let (ExpectedFace::Platform(face), Some(character)) = (&expected, fallback) {
                let covered = face.discovery_covers(character)?;
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
                if covered == Some(false) {
                    return Ok(None);
                }
            }
            let mut source_paths = state.source_paths();
            source_paths.insert(&key_path);
            check_limit(
                "system-snapshot-files",
                self.initial
                    .acquired_files
                    .saturating_add(source_paths.len()),
                limits.max_files,
            )?;
            let occupied = self.initial.bytes.saturating_add(state.bytes);
            let remaining = limits.max_bytes.saturating_sub(occupied);
            #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
            let normalized = match &expected {
                ExpectedFace::Platform(face) => {
                    face.selected_collection_data(&path, remaining, limits.max_faces, control)?
                }
                ExpectedFace::Bounded { .. } => None,
            };
            #[cfg(not(all(feature = "experimental-system-font-query", target_os = "macos")))]
            let normalized: Option<Vec<u8>> = None;
            let is_normalized = normalized.is_some();
            let bytes = if let Some(bytes) = normalized {
                data_index = 0;
                bytes
            } else {
                let mut bytes = Vec::new();
                read_bounded_into(
                    &path,
                    remaining,
                    control,
                    OperationPhase::Layout,
                    &mut bytes,
                )?;
                bytes
            };
            let discovery_digest = match &expected {
                ExpectedFace::Bounded { digest, .. } => {
                    let actual: [u8; 32] = Sha256::digest(&bytes).into();
                    if actual.as_slice() != digest {
                        return Err(source_error(
                            &path,
                            "font changed after metadata discovery; retry the operation",
                        ));
                    }
                    Some(actual)
                }
                #[cfg(feature = "experimental-system-font-query")]
                ExpectedFace::Platform(_) => None,
            };
            let data = Arc::new(bytes);
            state.bytes = state.bytes.saturating_add(data.len());
            if is_normalized {
                #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
                state.selected_faces.insert(key.clone(), Arc::clone(&data));
            } else {
                state.files.insert(
                    key_path.clone(),
                    FontSnapshot {
                        data: Arc::clone(&data),
                        discovery_digest,
                    },
                );
            }
            (data, discovery_digest)
        };
        state.aliases.insert(path.clone(), key_path.clone());
        state.aliases.insert(key_path.clone(), key_path);
        let source = Source::Binary(data.clone());
        if let Some(face) = state.faces.get(&key)
            && face.index == data_index
            && same_source(&face.source, &source)
        {
            expected.validate(face, &path, index)?;
            return Ok(Some(install(face.clone(), database)));
        }
        if let Some(face) = database
            .faces()
            .find(|face| face.index == data_index && same_source(&face.source, &source))
        {
            expected.validate(face, &path, index)?;
            return Ok(Some(face.id));
        }
        let count = rustybuzz::ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        check_limit(
            "font-collection-faces",
            usize::try_from(count).unwrap_or(usize::MAX),
            limits.max_faces,
        )?;
        check_limit(
            "font-faces",
            self.initial
                .faces
                .saturating_add(state.face_count())
                .saturating_add(1),
            limits.max_faces,
        )?;
        let parsed = rustybuzz::ttf_parser::Face::parse(&data, data_index)
            .map_err(|error| source_error(&path, error))?;
        let table = parsed
            .raw_face()
            .table(rustybuzz::ttf_parser::Tag::from_bytes(b"cmap"))
            .unwrap_or_default();
        let mut cmap_bytes = state.cmap_bytes;
        let _ = snapshot_cmap(table, control, &mut cmap_bytes, limits)?;
        let face = match &expected {
            // The catalog parsed this face from the same bytes and the first snapshot
            // verified the digest. Rebind its metadata to the admitted binary source.
            ExpectedFace::Bounded { metadata, digest }
                if discovery_digest
                    .as_ref()
                    .is_some_and(|actual| actual.as_slice() == digest) =>
            {
                Some(FaceInfo {
                    source,
                    ..metadata.clone()
                })
            }
            ExpectedFace::Bounded { .. } => {
                // The snapshot might predate this discovery of the same path.
                let mut parsed = Database::new();
                parsed.load_font_source(source);
                parsed
                    .faces()
                    .find(|face| face.index == data_index)
                    .cloned()
            }
            #[cfg(feature = "experimental-system-font-query")]
            ExpectedFace::Platform(_) => state.platform_face(source, data_index, count),
        };
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let face = face
            .ok_or_else(|| source_error(&path, "selected face cannot be parsed from snapshot"))?;
        expected.validate(&face, &path, index)?;
        state.cmap_bytes = cmap_bytes;
        state.faces.insert(key, face.clone());
        drop(state);
        Ok(Some(install(face, database)))
    }

    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    fn materialize_native(
        &self,
        candidate: platform::coretext::NativeFace,
        fallback: Option<char>,
        database: &mut Arc<Database>,
        limits: &NativeFontLimits,
        control: &OperationControl,
    ) -> Result<Option<ID>, NativeFontError> {
        let mut state = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
        if let Some(snapshot) = state.native_faces.get(&candidate) {
            // The retained native origin is an identity, never a fresh path lookup.
            return Ok(Some(install(snapshot.face.clone(), database)));
        }
        if fallback.is_some_and(|character| !candidate.covers(character)) {
            return Ok(None);
        }
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let path = &candidate.path;
        let key = self
            .source_key(&state, path)
            .map_err(|error| source_error(path, error))?;
        // Explicitly supplied immutable bytes have precedence over later system data.
        if let Some(data) = self.initial.files.get(&key) {
            let count = ttf_parser::fonts_in_collection(data).unwrap_or(1);
            check_limit("font-collection-faces", count as usize, limits.max_faces)?;
            for index in 0..count {
                if candidate.matches_data(data, index, control)? {
                    let source = Source::Binary(data.clone());
                    if let Some(face) = database
                        .faces()
                        .find(|face| face.index == index && same_source(&face.source, &source))
                    {
                        return Ok(Some(face.id));
                    }
                    let mut parsed = Database::new();
                    parsed.load_font_source(source);
                    let face = parsed
                        .faces()
                        .find(|face| face.index == index)
                        .cloned()
                        .ok_or_else(|| {
                            source_error(path, "selected explicit face cannot be parsed")
                        })?;
                    candidate.validate(&face)?;
                    return Ok(Some(install(face, database)));
                }
            }
            return Err(source_error(
                path,
                "native selection differs from the admitted explicit font",
            ));
        }
        let mut paths = state.source_paths();
        paths.insert(&key);
        check_limit(
            "system-snapshot-files",
            self.initial.acquired_files.saturating_add(paths.len()),
            limits.max_files,
        )?;
        check_limit(
            "font-faces",
            self.initial
                .faces
                .saturating_add(state.face_count())
                .saturating_add(1),
            limits.max_faces,
        )?;
        verify_font_file(path, control, OperationPhase::Layout)?;
        let remaining = limits
            .max_bytes
            .saturating_sub(self.initial.bytes.saturating_add(state.bytes));
        let data = Arc::new(candidate.snapshot(remaining, control)?);
        let parsed = rustybuzz::ttf_parser::Face::parse(&data, 0)
            .map_err(|error| source_error(path, error))?;
        let table = parsed
            .raw_face()
            .table(rustybuzz::ttf_parser::Tag::from_bytes(b"cmap"))
            .unwrap_or_default();
        let mut cmap_bytes = state.cmap_bytes;
        let _ = snapshot_cmap(table, control, &mut cmap_bytes, limits)?;
        let mut parsed = Database::new();
        parsed.load_font_source(Source::Binary(data.clone()));
        let face = parsed
            .faces()
            .next()
            .cloned()
            .ok_or_else(|| source_error(path, "selected native face cannot be parsed"))?;
        candidate.validate(&face)?;
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        state.bytes = state.bytes.saturating_add(data.len());
        state.cmap_bytes = cmap_bytes;
        state.aliases.insert(path.clone(), key.clone());
        state.aliases.insert(key.clone(), key.clone());
        state.native_faces.insert(
            candidate,
            NativeSnapshot {
                path: key,
                face: face.clone(),
            },
        );
        drop(state);
        Ok(Some(install(face, database)))
    }
}

fn same_source(left: &Source, right: &Source) -> bool {
    match (left, right) {
        (Source::Binary(left), Source::Binary(right)) => Arc::ptr_eq(left, right),
        _ => false,
    }
}

fn install(face: FaceInfo, database: &mut Arc<Database>) -> ID {
    if let Some(existing) = database.faces().find(|existing| {
        existing.index == face.index && same_source(&existing.source, &face.source)
    }) {
        return existing.id;
    }
    Arc::make_mut(database).push_face_info(face)
}

pub(super) fn snapshot_cmap(
    table: &[u8],
    control: &OperationControl,
    metadata_bytes: &mut usize,
    limits: &NativeFontLimits,
) -> Result<Vec<u8>, NativeFontError> {
    control.terminal_checkpoint_at(OperationPhase::Admission)?;
    check_limit(
        "system-cmap-table-bytes",
        table.len(),
        limits.max_system_cmap_table_bytes,
    )?;
    if let Some(parsed) = rustybuzz::ttf_parser::cmap::Table::parse(table) {
        // Bound repeated encoding records too: an input can alias one large subtable many times.
        check_limit(
            "system-cmap-subtables",
            usize::from(parsed.subtables.len()),
            32,
        )?;
    }
    let actual = metadata_bytes.saturating_add(table.len());
    check_limit("system-cmap-bytes", actual, limits.max_system_cmap_bytes)?;
    // Keep bounded cmap bytes, never expand declared ranges using an unstoppable callback.
    let result = table.to_vec();
    *metadata_bytes = actual;
    control.terminal_checkpoint_at(OperationPhase::Admission)?;
    Ok(result)
}

pub(super) fn read_bounded_into(
    path: &Path,
    limit: usize,
    control: &OperationControl,
    phase: OperationPhase,
    bytes: &mut Vec<u8>,
) -> Result<(), NativeFontError> {
    let metadata = verify_font_file(path, control, phase)?;
    let size = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    check_limit("system-font-bytes", size, limit)?;
    let mut file = fs::File::open(path).map_err(|error| source_error(path, error))?;
    // Keep at most one file length of spare capacity. Release an unsuitable buffer before
    // reserving so growth never needs to copy the previous font or keep two fonts live.
    if bytes.capacity() < size || bytes.capacity() > size.saturating_mul(2) {
        *bytes = Vec::new();
    } else {
        bytes.clear();
    }
    bytes
        .try_reserve_exact(size)
        .map_err(|error| source_error(path, error))?;
    let mut chunk = [0; 64 * 1024];
    loop {
        control.terminal_checkpoint_at(phase)?;
        let count = file
            .read(&mut chunk)
            .map_err(|error| source_error(path, error))?;
        if count == 0 {
            break;
        }
        check_limit(
            "system-font-bytes",
            bytes.len().saturating_add(count),
            limit,
        )?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(())
}

pub(super) fn verify_font_file(
    path: &Path,
    control: &OperationControl,
    phase: OperationPhase,
) -> Result<fs::Metadata, NativeFontError> {
    control.terminal_checkpoint_at(phase)?;
    // Recheck ancestors as well as the leaf when delayed materialization opens a source.
    for ancestor in path.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        if fs::symlink_metadata(ancestor)
            .map_err(|error| source_error(ancestor, error))?
            .is_symlink()
        {
            return Err(source_error(path, "font source or ancestor is a symlink"));
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| source_error(path, error))?;
    if !metadata.is_file() {
        return Err(source_error(path, "expected a regular font file"));
    }
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    mod selected_snapshots {
        use super::*;

        fn parsed_face(data: &Arc<Vec<u8>>, index: u32) -> FaceInfo {
            let mut database = Database::new();
            database.load_font_source(Source::Binary(data.clone()));
            database
                .faces()
                .find(|face| face.index == index)
                .unwrap()
                .clone()
        }

        fn candidate(path: &Path, face: &FaceInfo, data: &[u8]) -> FontCandidate {
            FontCandidate::File {
                path: path.to_owned(),
                index: face.index,
                expected: ExpectedFace::Bounded {
                    metadata: face.clone(),
                    digest: Sha256::digest(data).to_vec(),
                },
            }
        }

        #[test]
        fn selected_face_survives_replacement_and_later_whole_snapshot() {
            let root = std::env::temp_dir().join(format!(
                "merman-selected-whole-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            fs::create_dir(&root).unwrap();
            let root = root.canonicalize().unwrap();
            let path = root.join("font.ttc");
            let selected =
                Arc::new(include_bytes!("../../tests/fonts/NotoSans-Latin-subset.ttf").to_vec());
            let whole =
                Arc::new(include_bytes!("../../tests/fonts/Cousine-test-collection.ttc").to_vec());
            let old_face = parsed_face(&selected, 0);
            let other_face = parsed_face(&whole, 1);
            fs::write(&path, selected.as_slice()).unwrap();
            let registry = FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
            {
                // Model a normalized admission without requiring a registered host font.
                let mut state = registry.snapshots.lock().unwrap();
                state
                    .selected_faces
                    .insert((path.clone(), 0), selected.clone());
                state.bytes = selected.len();
            }
            let limits = NativeFontLimits {
                max_files: 1,
                ..Default::default()
            };
            let control = OperationControl::new();
            let mut database = Arc::new(Database::new());
            registry
                .materialize(
                    candidate(&path, &old_face, &selected),
                    None,
                    &mut database,
                    &limits,
                    &control,
                )
                .unwrap()
                .unwrap();

            fs::write(&path, whole.as_slice()).unwrap();
            registry
                .materialize(
                    candidate(&path, &other_face, &whole),
                    None,
                    &mut database,
                    &limits,
                    &control,
                )
                .unwrap()
                .unwrap();
            let resident = selected.len() + whole.len();
            assert_eq!(registry.resident_bytes(), resident);
            let FaceData { data, index } = registry.face_snapshot(&path, 0).unwrap().unwrap();
            assert!(Arc::ptr_eq(&data, &selected));
            assert_eq!(index, 0);
            let FaceData { data, index } = registry.face_snapshot(&path, 1).unwrap().unwrap();
            assert_eq!(data.as_slice(), whole.as_slice());
            assert_eq!(index, 1);

            // Both cached metadata and reparsing the selected bytes must retain the old face.
            for cached in [true, false] {
                let export = registry.for_export().unwrap();
                if !cached {
                    export
                        .snapshots
                        .lock()
                        .unwrap()
                        .faces
                        .remove(&(path.clone(), 0));
                }
                let mut database = Arc::new(Database::new());
                let id = export
                    .materialize(
                        candidate(&path, &old_face, &selected),
                        None,
                        &mut database,
                        &limits,
                        &control,
                    )
                    .unwrap()
                    .unwrap();
                assert!(same_source(
                    &database.face(id).unwrap().source,
                    &old_face.source
                ));
                assert_eq!(export.resident_bytes(), resident);
                assert_eq!(export.admitted_faces(), 2);
            }
            fs::remove_file(path).unwrap();
            fs::remove_dir(root).unwrap();
        }

        #[test]
        fn initial_whole_snapshot_precedes_selected_face_and_cached_metadata() {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fonts/Cousine-test-collection.ttc")
                .canonicalize()
                .unwrap();
            let whole =
                Arc::new(include_bytes!("../../tests/fonts/Cousine-test-collection.ttc").to_vec());
            let selected =
                Arc::new(include_bytes!("../../tests/fonts/NotoSans-Latin-subset.ttf").to_vec());
            let face = parsed_face(&whole, 1);
            let registry = FontSourceRegistry::new(
                BTreeMap::from([(path.clone(), whole.clone())]),
                BTreeMap::new(),
                1,
                2,
            );
            {
                // Even a conflicting dynamic cache cannot supersede explicit input bytes.
                let mut state = registry.snapshots.lock().unwrap();
                state
                    .faces
                    .insert((path.clone(), 1), parsed_face(&selected, 0));
                state
                    .selected_faces
                    .insert((path.clone(), 1), selected.clone());
                state.bytes = selected.len();
            }
            let FaceData { data, index } = registry.face_snapshot(&path, 1).unwrap().unwrap();
            assert!(Arc::ptr_eq(&data, &whole));
            assert_eq!(index, 1);
            let mut database = Arc::new(Database::new());
            let id = registry
                .materialize(
                    candidate(&path, &face, &whole),
                    None,
                    &mut database,
                    &NativeFontLimits::default(),
                    &OperationControl::new(),
                )
                .unwrap()
                .unwrap();
            let installed = database.face(id).unwrap();
            assert_eq!(installed.index, 1);
            assert!(same_source(&installed.source, &face.source));
        }

        #[test]
        fn canonical_selected_face_survives_removal_and_replacement() {
            let root = std::env::temp_dir().join(format!(
                "merman-selected-alias-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            fs::create_dir(&root).unwrap();
            let root = root.canonicalize().unwrap();
            let child = root.join("child");
            fs::create_dir(&child).unwrap();
            let path = root.join("font.ttc");
            let alias = child.join("..").join("font.ttc");
            assert_ne!(alias, path);
            let selected =
                Arc::new(include_bytes!("../../tests/fonts/Cousine-Regular.ttf").to_vec());
            let face = parsed_face(&selected, 0);
            fs::write(&path, selected.as_slice()).unwrap();
            let registry = FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
            {
                let mut state = registry.snapshots.lock().unwrap();
                state
                    .selected_faces
                    .insert((path.clone(), 0), selected.clone());
                state.bytes = selected.len();
            }
            let limits = NativeFontLimits::default();
            let control = OperationControl::new();
            registry
                .materialize(
                    candidate(&alias, &face, &selected),
                    None,
                    &mut Arc::new(Database::new()),
                    &limits,
                    &control,
                )
                .unwrap()
                .unwrap();
            fs::remove_file(&path).unwrap();
            let replacement = root.join("replacement.ttf");
            fs::write(
                &replacement,
                include_bytes!("../../tests/fonts/NotoSans-Latin-subset.ttf"),
            )
            .unwrap();
            for replaced in [false, true] {
                if replaced {
                    std::os::unix::fs::symlink(&replacement, &path).unwrap();
                    assert_eq!(path.canonicalize().unwrap(), replacement);
                }
                let export = registry.for_export().unwrap();
                for source in [&registry, &export] {
                    let FaceData { data, index } = source.face_snapshot(&path, 0).unwrap().unwrap();
                    assert!(Arc::ptr_eq(&data, &selected));
                    assert_eq!(index, 0);
                    let mut database = Arc::new(Database::new());
                    let id = source
                        .materialize(
                            candidate(&path, &face, &selected),
                            None,
                            &mut database,
                            &limits,
                            &control,
                        )
                        .unwrap()
                        .unwrap();
                    assert!(same_source(
                        &database.face(id).unwrap().source,
                        &face.source
                    ));
                    assert_eq!(source.resident_bytes(), selected.len());
                    assert_eq!(source.admitted_faces(), 1);
                }
            }
            fs::remove_file(path).unwrap();
            fs::remove_file(replacement).unwrap();
            fs::remove_dir(child).unwrap();
            fs::remove_dir(root).unwrap();
        }
    }

    #[cfg(feature = "experimental-system-font-query")]
    #[test]
    fn platform_collection_shares_metadata_across_face_indices_and_export() {
        let bytes = include_bytes!("../../tests/fonts/Cousine-test-collection.ttc");
        let source = Source::Binary(Arc::new(bytes.to_vec()));
        let mut state = SnapshotState::default();
        let regular = state.platform_face(source.clone(), 0, 2).unwrap();
        let cached = Arc::clone(&state.collection.as_ref().unwrap().faces);
        assert_eq!(cached.len(), 2);
        let italic = state.platform_face(source.clone(), 1, 2).unwrap();
        assert_eq!(regular.post_script_name, "Cousine-Regular");
        assert_eq!(italic.post_script_name, "Cousine-Italic");
        assert!(same_source(&italic.source, &source));
        assert!(Arc::ptr_eq(
            &cached,
            &state.collection.as_ref().unwrap().faces
        ));
        assert!(state.platform_face(source.clone(), 2, 2).is_none());

        let mut export = state.clone();
        assert!(Arc::ptr_eq(
            &cached,
            &export.collection.as_ref().unwrap().faces
        ));
        let another = Source::Binary(Arc::new(bytes.to_vec()));
        assert!(same_source(
            &export.platform_face(another.clone(), 0, 2).unwrap().source,
            &another
        ));
        assert!(!Arc::ptr_eq(
            &cached,
            &export.collection.as_ref().unwrap().faces
        ));
    }

    #[test]
    fn bounded_discovery_metadata_does_not_override_an_older_explicit_snapshot() {
        let explicit_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf");
        let explicit_bytes = fs::read(&explicit_path).unwrap();
        let discovered_bytes =
            include_bytes!("../../tests/fonts/NotoSans-Latin-subset.ttf").to_vec();
        let digest = Sha256::digest(&discovered_bytes).to_vec();
        let mut catalog = Database::new();
        catalog.load_font_data(discovered_bytes);
        let metadata = catalog.faces().next().unwrap().clone();
        let registry = FontSourceRegistry::new(
            BTreeMap::from([(explicit_path.clone(), Arc::new(explicit_bytes))]),
            BTreeMap::new(),
            1,
            1,
        );
        let error = registry
            .materialize(
                FontCandidate::File {
                    path: explicit_path,
                    index: metadata.index,
                    expected: ExpectedFace::Bounded { metadata, digest },
                },
                None,
                &mut Arc::new(Database::new()),
                &NativeFontLimits::default(),
                &OperationControl::new(),
            )
            .unwrap_err();
        assert!(
            matches!(error, NativeFontError::Source { message, .. } if message.contains("font properties changed"))
        );
    }

    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    #[test]
    fn identity_probe_prefers_admitted_path_after_symlink_replacement() {
        let root = std::env::temp_dir().join(format!(
            "merman-font-probe-alias-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let path = root.join("font.ttf");
        let replacement = root.join("replacement.ttf");
        let original = Arc::new(include_bytes!("../../tests/fonts/Cousine-Regular.ttf").to_vec());
        fs::write(
            &replacement,
            include_bytes!("../../tests/fonts/NotoSansSC-subset.ttf"),
        )
        .unwrap();
        let registry = FontSourceRegistry::new(
            BTreeMap::from([(path.clone(), Arc::clone(&original))]),
            BTreeMap::new(),
            1,
            1,
        );
        std::os::unix::fs::symlink(&replacement, &path).unwrap();
        assert_eq!(path.canonicalize().unwrap(), replacement);
        assert!(Arc::ptr_eq(
            &registry.snapshot(&path).unwrap().unwrap(),
            &original
        ));
        fs::remove_file(path).unwrap();
        fs::remove_file(replacement).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn bounded_snapshot_reuses_metadata_only_for_its_verified_discovery_digest() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-test-collection.ttc");
        let mut catalog = Database::new();
        let collection = include_bytes!("../../tests/fonts/Cousine-test-collection.ttc");
        catalog.load_font_data(collection.to_vec());
        let mut new_catalog = Database::new();
        let replacement = include_bytes!("../../tests/fonts/NotoSans-Latin-subset.ttf");
        new_catalog.load_font_data(replacement.to_vec());

        let registry = FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let mut database = Arc::new(Database::new());
        let limits = NativeFontLimits::default();
        let control = OperationControl::new();
        let italic = catalog.faces().find(|face| face.index == 1).unwrap();
        let id = registry
            .materialize(
                FontCandidate::File {
                    path: path.clone(),
                    index: italic.index,
                    expected: ExpectedFace::Bounded {
                        metadata: italic.clone(),
                        digest: Sha256::digest(collection).to_vec(),
                    },
                },
                None,
                &mut database,
                &limits,
                &control,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            database.face(id).unwrap().post_script_name,
            "Cousine-Italic"
        );

        let changed = new_catalog.faces().next().unwrap();
        let error = registry
            .materialize(
                FontCandidate::File {
                    path,
                    index: changed.index,
                    expected: ExpectedFace::Bounded {
                        metadata: changed.clone(),
                        digest: Sha256::digest(replacement).to_vec(),
                    },
                },
                None,
                &mut database,
                &limits,
                &control,
            )
            .unwrap_err();
        assert!(
            matches!(error, NativeFontError::Source { message, .. } if message.contains("font properties changed"))
        );
        assert_eq!(database.len(), 1);
    }

    #[test]
    fn reused_read_buffer_charges_only_the_new_file_and_discards_its_old_tail() {
        let expected = include_bytes!("../../tests/fonts/Cousine-Regular.ttf");
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fonts/Cousine-Regular.ttf")
            .canonicalize()
            .unwrap();
        let mut bytes = vec![0xff; expected.len() + 1];
        let control = OperationControl::new();
        let read = |limit, bytes: &mut Vec<u8>| {
            read_bounded_into(&path, limit, &control, OperationPhase::Admission, bytes)
        };
        read(expected.len(), &mut bytes).unwrap();
        assert_eq!(bytes.as_slice(), expected);
        assert!(matches!(
            read(expected.len() - 1, &mut bytes),
            Err(NativeFontError::Limit { resource: "system-font-bytes", actual, limit })
                if actual == expected.len() && limit == expected.len() - 1
        ));
    }

    #[test]
    fn admitted_original_path_survives_removal_but_operation_fork_reopens_dynamic_sources() {
        let root = std::env::temp_dir().join(format!(
            "merman-font-alias-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir(&root).unwrap();
        // Resolve host temp-directory aliases before constructing a regular font source.
        let root = fs::canonicalize(root).unwrap();
        // Dot segments keep the candidate path distinct from its canonical registry key.
        let path = root.join(".").join("font.ttf");
        let bytes = fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
        )
        .unwrap();
        fs::write(&path, &bytes).unwrap();
        let mut metadata = Database::new();
        metadata.load_font_data(bytes.clone());
        let face = metadata.faces().next().unwrap().clone();
        let make_candidate = || FontCandidate::File {
            path: path.clone(),
            index: face.index,
            expected: ExpectedFace::Bounded {
                metadata: face.clone(),
                digest: Sha256::digest(&bytes).to_vec(),
            },
        };
        let registry = FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let control = OperationControl::new();
        registry
            .materialize(
                make_candidate(),
                None,
                &mut Arc::new(Database::new()),
                &NativeFontLimits::default(),
                &control,
            )
            .unwrap();
        fs::remove_file(&path).unwrap();
        let resident = registry.resident_bytes();
        registry
            .materialize(
                make_candidate(),
                None,
                &mut Arc::new(Database::new()),
                &NativeFontLimits::default(),
                &control,
            )
            .unwrap();
        assert_eq!(registry.resident_bytes(), resident);
        let export = registry.for_export().unwrap();
        export
            .materialize(
                make_candidate(),
                None,
                &mut Arc::new(Database::new()),
                &NativeFontLimits::default(),
                &OperationControl::new(),
            )
            .expect("export must retain the admitted bytes after the source disappears");
        assert_eq!(export.resident_bytes(), resident);
        let extra_path = root.join("extra.ttf");
        fs::write(&extra_path, &bytes).unwrap();
        for (resource, limits) in [
            (
                "system-snapshot-files",
                NativeFontLimits {
                    max_files: 1,
                    ..Default::default()
                },
            ),
            (
                "system-font-bytes",
                NativeFontLimits {
                    max_bytes: bytes.len(),
                    ..Default::default()
                },
            ),
            (
                "font-faces",
                NativeFontLimits {
                    max_faces: 1,
                    ..Default::default()
                },
            ),
        ] {
            let export = registry.for_export().unwrap();
            let mut extra = make_candidate();
            let FontCandidate::File { path, .. } = &mut extra else {
                unreachable!()
            };
            *path = extra_path.clone();
            let error = export
                .materialize(
                    extra,
                    None,
                    &mut Arc::new(Database::new()),
                    &limits,
                    &OperationControl::new(),
                )
                .unwrap_err();
            assert!(
                matches!(error, NativeFontError::Limit { resource: actual, .. } if actual == resource),
                "{error}"
            );
            assert_eq!(
                registry.resident_bytes(),
                resident,
                "export admission must not modify the source snapshots"
            );
        }
        fs::remove_file(extra_path).unwrap();
        let next = registry.for_operation();
        assert_eq!(next.resident_bytes(), 0);
        assert!(
            next.materialize(
                make_candidate(),
                None,
                &mut Arc::new(Database::new()),
                &NativeFontLimits::default(),
                &control
            )
            .is_err()
        );
        fs::remove_dir(&root).unwrap();
    }
}
