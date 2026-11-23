//! One operation-local admission boundary for all discovered font sources.

use super::resolver::{ExpectedFace, FontCandidate};
use super::*;
use usvg::fontdb::{Database, FaceInfo, ID, Source};

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
mod shared_collection;

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
    native_faces: std::collections::HashMap<platform::coretext::NativeFace, FaceInfo>,
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    native_origins: std::collections::HashMap<AdmittedFaceKey, platform::coretext::NativeFace>,
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    selections: std::collections::HashMap<SelectionRequest, SelectedFont>,
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    selection_bytes: usize,
    aliases: BTreeMap<PathBuf, PathBuf>,
    faces: BTreeMap<(PathBuf, u32), FaceInfo>,
    cmap_bytes: usize,
    bytes: usize,
    #[cfg(feature = "experimental-system-font-query")]
    collection: Option<CollectionMetadata>,
}

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
#[derive(Clone, Hash, Eq, PartialEq)]
pub(super) enum SelectionRequest {
    /// usvg applies variation coordinates during shaping, after selecting the physical face.
    Primary {
        families: Box<[usvg::FontFamily]>,
        weight: u16,
        stretch: usvg::FontStretch,
        style: usvg::FontStyle,
    },
    Fallback(char, Vec<AdmittedFaceKey>),
}

/// Retain the source so pointer identity cannot be recycled while a request is pinned.
#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
#[derive(Clone)]
pub(super) struct AdmittedFaceKey {
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    index: u32,
}

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
impl AdmittedFaceKey {
    fn of(face: &FaceInfo) -> Option<Self> {
        let Source::Binary(data) = &face.source else {
            return None;
        };
        Some(Self {
            data: Arc::clone(data),
            index: face.index,
        })
    }
}

/// Distinguishes two primary native handles in fontdb while sharing the physical bytes.
#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
struct NativeSourceView(Arc<dyn AsRef<[u8]> + Send + Sync>);

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
impl AsRef<[u8]> for NativeSourceView {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref().as_ref()
    }
}

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
impl PartialEq for AdmittedFaceKey {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Arc::ptr_eq(&self.data, &other.data)
    }
}

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
impl Eq for AdmittedFaceKey {}

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
impl std::hash::Hash for AdmittedFaceKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&(Arc::as_ptr(&self.data) as *const ()), state);
        std::hash::Hash::hash(&self.index, state);
    }
}

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
impl SelectionRequest {
    pub(super) fn primary(font: &usvg::Font) -> Self {
        Self::Primary {
            families: font.families().into(),
            weight: font.weight(),
            stretch: font.stretch(),
            style: font.style(),
        }
    }

    pub(super) fn fallback(character: char, excluded: &[FaceInfo]) -> Option<Self> {
        let keys = excluded
            .iter()
            .map(AdmittedFaceKey::of)
            .collect::<Option<Vec<_>>>()?;
        Some(Self::Fallback(character, keys))
    }

    fn heap_bytes(&self) -> usize {
        match self {
            Self::Primary { families, .. } => {
                let family_bytes = std::mem::size_of_val(families.as_ref());
                families
                    .iter()
                    .fold(family_bytes, |bytes, family| match family {
                        usvg::FontFamily::Named(name) => bytes.saturating_add(name.len()),
                        _ => bytes,
                    })
            }
            Self::Fallback(_, keys) => keys.len().saturating_mul(size_of::<AdmittedFaceKey>()),
        }
    }
}

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
#[derive(Clone)]
pub(super) struct SelectedFont {
    pub(super) face: FaceInfo,
    pub(super) source: NativeExportFontSource,
    pub(super) family_fallback_used: bool,
}

impl SnapshotState {
    fn face_count(&self) -> usize {
        let count = self.faces.len();
        #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
        let count = count.saturating_add(self.native_faces.len());
        count
    }
}

#[derive(Clone)]
pub(super) enum FontSnapshot {
    Whole {
        data: Arc<Vec<u8>>,
        /// The digest checked against bounded discovery on first read.
        discovery_digest: Option<[u8; 32]>,
    },
    /// Counts the source path once. Core Text owns face identity within the collection.
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    NativeFaces,
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
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
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
            #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
            selection_gate: Mutex::new(()),
        }
    }

    pub(super) fn for_operation(&self) -> Self {
        Self {
            initial: Arc::clone(&self.initial),
            snapshots: Mutex::new(SnapshotState::default()),
            #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
            selection_gate: Mutex::new(()),
        }
    }

    pub(super) fn for_export(&self) -> Result<Self, NativeFontError> {
        #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
        let _selection = self.lock_selection()?;
        let snapshots = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
        Ok(Self {
            initial: Arc::clone(&self.initial),
            snapshots: Mutex::new(snapshots.clone()),
            #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
            selection_gate: Mutex::new(()),
        })
    }

    /// Serialize cold selection through admission and pinning. Otherwise a second
    /// query can fail after another thread has selected the same request successfully.
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
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
        Ok(AdmittedFaceKey::of(face).and_then(|key| state.native_origins.get(&key).cloned()))
    }

    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    pub(super) fn selected(
        &self,
        request: &SelectionRequest,
        database: &mut Arc<Database>,
    ) -> Result<Option<(ID, NativeExportFontSource, bool)>, NativeFontError> {
        let state = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
        Ok(state.selections.get(request).map(|selected| {
            (
                install(selected.face.clone(), database),
                selected.source,
                selected.family_fallback_used,
            )
        }))
    }

    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    pub(super) fn remember_selection(
        &self,
        request: SelectionRequest,
        selected: SelectedFont,
        origin: Option<platform::coretext::NativeFace>,
        limits: &NativeFontLimits,
    ) -> Result<(), NativeFontError> {
        let mut state = self.snapshots.lock().map_err(|_| {
            NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
        })?;
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
        let origin = origin
            .zip(AdmittedFaceKey::of(&selected.face))
            .filter(|(_, key)| !state.native_origins.contains_key(key));
        let origin_bytes = origin.as_ref().map_or(0, |(origin, _)| {
            size_of::<(AdmittedFaceKey, platform::coretext::NativeFace)>()
                .saturating_add(origin.path.as_os_str().len())
                .saturating_add(origin.family.len())
                .saturating_add(origin.post_script_name.len())
        });
        let total = state
            .selection_bytes
            .saturating_add(bytes)
            .saturating_add(origin_bytes);
        check_limit("font-selection-bytes", total, limits.max_selection_bytes)?;
        state.selection_bytes = total;
        if let Some((origin, key)) = origin {
            state.native_origins.insert(key, origin);
        }
        // The resolver holds the selection gate from lookup through this insertion.
        state.selections.insert(request, selected);
        Ok(())
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

    fn check_file_limit(
        &self,
        state: &SnapshotState,
        key: &Path,
        limits: &NativeFontLimits,
    ) -> Result<(), NativeFontError> {
        let files = self
            .initial
            .acquired_files
            .saturating_add(state.files.len());
        check_limit(
            "system-snapshot-files",
            files.saturating_add(usize::from(!state.files.contains_key(key))),
            limits.max_files,
        )
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
        let (data, discovery_digest, is_new) = if let Some(data) = self.initial.files.get(&key_path)
        {
            (Arc::clone(data), None, false)
        } else if let Some(FontSnapshot::Whole {
            data,
            discovery_digest,
        }) = state.files.get(&key_path)
        {
            (Arc::clone(data), *discovery_digest, false)
        } else {
            // Discovery may reject a missing glyph before acquiring its entire source.
            // Existing snapshots always win: a changed file cannot invalidate admitted bytes.
            #[cfg(feature = "experimental-system-font-query")]
            if let (ExpectedFace::Platform(face), Some(character)) = (&expected, fallback) {
                // Fontique may open the path while probing its cmap. Apply the same
                // source policy here that protects the subsequent snapshot read.
                verify_font_file(&path, control, OperationPhase::Layout)?;
                let covered = face.discovery_covers(character)?;
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
                if covered == Some(false) {
                    return Ok(None);
                }
            }
            self.check_file_limit(&state, &key_path, limits)?;
            let occupied = self.initial.bytes.saturating_add(state.bytes);
            let remaining = limits.max_bytes.saturating_sub(occupied);
            let mut bytes = Vec::new();
            read_bounded_into(
                &path,
                remaining,
                control,
                OperationPhase::Layout,
                &mut bytes,
            )?;
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
            (Arc::new(bytes), discovery_digest, true)
        };
        let source = Source::Binary(data.clone());
        let admitted = state.faces.get(&key).or_else(|| {
            database
                .faces()
                .find(|face| face.index == index && same_source(&face.source, &source))
        });
        if let Some(face) = admitted.cloned() {
            expected.validate(&face, &path)?;
            state.aliases.insert(path, key_path);
            return Ok(Some(install(face, database)));
        }
        let count = rustybuzz::ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        check_limit(
            "font-collection-faces",
            usize::try_from(count).unwrap_or(usize::MAX),
            limits.max_faces,
        )?;
        let parsed = rustybuzz::ttf_parser::Face::parse(&data, index)
            .map_err(|error| source_error(&path, error))?;
        #[cfg(feature = "experimental-system-font-query")]
        if matches!(&expected, ExpectedFace::Platform(_))
            && fallback.is_some_and(|ch| !crate::font_selection::covers_character(&parsed, ch))
        {
            return Ok(None);
        }
        check_limit(
            "font-faces",
            self.initial
                .faces
                .saturating_add(state.face_count())
                .saturating_add(1),
            limits.max_faces,
        )?;
        let table = parsed
            .raw_face()
            .table(rustybuzz::ttf_parser::Tag::from_bytes(b"cmap"))
            .unwrap_or_default();
        let cmap_bytes = checked_cmap_bytes(table, control, state.cmap_bytes, limits)?;
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
                parsed.faces().find(|face| face.index == index).cloned()
            }
            #[cfg(feature = "experimental-system-font-query")]
            ExpectedFace::Platform(_) => state.platform_face(source, index, count),
        };
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let face = face
            .ok_or_else(|| source_error(&path, "selected face cannot be parsed from snapshot"))?;
        expected.validate(&face, &path)?;
        if is_new {
            state.bytes = state.bytes.saturating_add(data.len());
            state.files.insert(
                key_path.clone(),
                FontSnapshot::Whole {
                    data,
                    discovery_digest,
                },
            );
        }
        state.aliases.insert(path, key_path);
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
        if let Some(face) = state.native_faces.get(&candidate) {
            return Ok(Some(install_native(
                &state, &candidate, face, fallback, database,
            )));
        }
        if fallback.is_some_and(|character| !candidate.covers(character)) {
            return Ok(None);
        }
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let path = &candidate.path;
        let key = self
            .source_key(&state, path)
            .map_err(|error| source_error(path, error))?;
        let existing =
            self.initial
                .files
                .get(&key)
                .cloned()
                .or_else(|| match state.files.get(&key) {
                    Some(FontSnapshot::Whole { data, .. }) => Some(Arc::clone(data)),
                    _ => None,
                });
        // A CSS query and a character fallback can return unequal CTFonts for
        // exactly the same physical face. Names and paths only narrow the search.
        if existing.is_some() || state.files.contains_key(&key) {
            for (origin, face) in &state.native_faces {
                if origin.post_script_name == candidate.post_script_name
                    && state.aliases.get(&origin.path).unwrap_or(&origin.path) == &key
                    && let Source::Binary(data) = &face.source
                    && candidate.matches_data(data.as_ref().as_ref(), face.index, control)?
                {
                    return Ok(Some(install_native(
                        &state, &candidate, face, fallback, database,
                    )));
                }
            }
        }
        let next_face = self
            .initial
            .faces
            .saturating_add(state.face_count())
            .saturating_add(1);
        let (data, index, new_bytes) = if let Some(data) = existing {
            // Reuse immutable explicit or shared collection bytes, including their physical index.
            let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
            check_limit("font-collection-faces", count as usize, limits.max_faces)?;
            let mut selected = None;
            for index in 0..count {
                if candidate.matches_data(&data, index, control)? {
                    selected = Some(index);
                    break;
                }
            }
            let index = selected.ok_or_else(|| {
                source_error(path, "native selection differs from the admitted font")
            })?;
            let source = Source::Binary(data.clone());
            let installed = database
                .faces()
                .find(|face| face.index == index && same_source(&face.source, &source))
                .cloned();
            if let Some(face) = installed {
                return Ok(Some(install_native(
                    &state, &candidate, &face, fallback, database,
                )));
            }
            // Explicit source deduplication can retain a file while omitting this face.
            check_limit("font-faces", next_face, limits.max_faces)?;
            (data, index, 0)
        } else {
            self.check_file_limit(&state, &key, limits)?;
            check_limit("font-faces", next_face, limits.max_faces)?;
            let file = verify_font_file(path, control, OperationPhase::Layout)?;
            let remaining = limits
                .max_bytes
                .saturating_sub(self.initial.bytes.saturating_add(state.bytes));
            let shared = if state.files.contains_key(&key) {
                None
            } else {
                shared_collection::shared_collection(
                    &candidate,
                    file.len(),
                    remaining,
                    limits.max_faces,
                    control,
                )?
            };
            let (bytes, index) = match shared {
                Some(collection) => collection,
                None => (candidate.snapshot(remaining, control)?, 0),
            };
            let length = bytes.len();
            (Arc::new(bytes), index, length)
        };
        let parsed =
            ttf_parser::Face::parse(&data, index).map_err(|error| source_error(path, error))?;
        if fallback.is_some_and(|ch| !crate::font_selection::covers_character(&parsed, ch)) {
            return Ok(None);
        }
        let table = parsed
            .raw_face()
            .table(ttf_parser::Tag::from_bytes(b"cmap"))
            .unwrap_or_default();
        let cmap_bytes = checked_cmap_bytes(table, control, state.cmap_bytes, limits)?;
        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        let face = state
            .platform_face(Source::Binary(data.clone()), index, count)
            .ok_or_else(|| source_error(path, "selected native face cannot be parsed"))?;
        candidate.validate(&face)?;
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        state.bytes = state.bytes.saturating_add(new_bytes);
        state.cmap_bytes = cmap_bytes;
        state.aliases.insert(path.clone(), key.clone());
        if new_bytes > 0 {
            let snapshot = if count > 1 {
                FontSnapshot::Whole {
                    data,
                    discovery_digest: None,
                }
            } else {
                FontSnapshot::NativeFaces
            };
            state.files.entry(key).or_insert(snapshot);
        }
        state.native_faces.insert(candidate, face.clone());
        drop(state);
        Ok(Some(install(face, database)))
    }
}

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
fn install_native(
    state: &SnapshotState,
    origin: &platform::coretext::NativeFace,
    face: &FaceInfo,
    fallback: Option<char>,
    database: &mut Arc<Database>,
) -> ID {
    let mut face = face.clone();
    // usvg starts the cascade from the primary face. Give distinct primary
    // origins separate database identities while keeping the same byte allocation.
    if fallback.is_none()
        && let Some(key) = AdmittedFaceKey::of(&face)
        && state
            .native_origins
            .get(&key)
            .is_some_and(|pinned| pinned != origin)
    {
        face.source = Source::Binary(Arc::new(NativeSourceView(key.data)));
    }
    install(face, database)
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

pub(super) fn checked_cmap_bytes(
    table: &[u8],
    control: &OperationControl,
    metadata_bytes: usize,
    limits: &NativeFontLimits,
) -> Result<usize, NativeFontError> {
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
    Ok(actual)
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

    #[cfg(unix)]
    #[test]
    fn materialization_prefers_explicit_bytes_after_symlink_replacement() {
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
        let source = Source::Binary(original.clone());
        let mut explicit = Database::new();
        explicit.load_font_source(source.clone());
        let metadata = explicit.faces().next().unwrap().clone();
        let mut database = Arc::new(explicit);
        let id = registry
            .materialize(
                FontCandidate::File {
                    path: path.clone(),
                    index: metadata.index,
                    expected: ExpectedFace::Bounded {
                        metadata,
                        digest: Sha256::digest(&*original).to_vec(),
                    },
                },
                None,
                &mut database,
                &NativeFontLimits::default(),
                &OperationControl::new(),
            )
            .unwrap()
            .unwrap();
        assert!(same_source(&database.face(id).unwrap().source, &source));
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
