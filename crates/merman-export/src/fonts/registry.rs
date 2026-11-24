//! One operation-local admission boundary for all discovered font sources.

#[cfg(test)]
use super::resolver::GlyphFallback;
#[cfg(feature = "experimental-system-font-query")]
use super::resolver::TriedFont;
use super::resolver::{AdmittedFont, CandidateRequest, ExpectedFace, FontCandidate};
use super::*;
use usvg::fontdb::{Database, FaceInfo, ID, Source};

#[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
mod shared_collection;

#[cfg(feature = "experimental-system-font-query")]
mod platform_file;
#[cfg(feature = "experimental-system-font-query")]
use platform_file::OpenPlatformFont;

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
    native_origins: std::collections::HashMap<AttemptKey, Option<platform::coretext::NativeFace>>,
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    anonymous_native_files: usize,
    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    native_face_count: usize,
    #[cfg(feature = "experimental-system-font-query")]
    selections: std::collections::HashMap<SelectionRequest, SelectedFont<Arc<FaceInfo>>>,
    #[cfg(feature = "experimental-system-font-query")]
    pinned_faces: std::collections::HashMap<AdmittedFaceKey, Arc<FaceInfo>>,
    #[cfg(feature = "experimental-system-font-query")]
    authored_stacks: std::collections::HashSet<Arc<SelectionStyle>>,
    #[cfg(feature = "experimental-system-font-query")]
    misses: std::collections::HashSet<SelectionRequest>,
    #[cfg(feature = "experimental-system-font-query")]
    miss_bytes: usize,
    #[cfg(feature = "experimental-system-font-query")]
    scan_bytes: usize,
    #[cfg(feature = "experimental-system-font-query")]
    selection_bytes: usize,
    aliases: BTreeMap<PathBuf, PathBuf>,
    faces: BTreeMap<(PathBuf, u32), FaceInfo>,
    cmap_bytes: usize,
    bytes: usize,
    #[cfg(feature = "experimental-system-font-query")]
    collection: Option<CollectionMetadata>,
}

#[cfg(feature = "experimental-system-font-query")]
#[derive(Clone, Hash, Eq, PartialEq)]
pub(super) enum SelectionRequest {
    Primary(Arc<SelectionStyle>),
    Fallback {
        style: Option<Arc<SelectionStyle>>,
        text: String,
        language: Option<String>,
        rtl: bool,
        excluded: Vec<AttemptKey>,
    },
}

#[cfg(feature = "experimental-system-font-query")]
#[derive(Hash, Eq, PartialEq)]
pub(super) struct SelectionStyle {
    families: Box<[usvg::FontFamily]>,
    weight: u16,
    stretch: usvg::FontStretch,
    style: usvg::FontStyle,
    language: Option<String>,
}

#[cfg(feature = "experimental-system-font-query")]
impl SelectionStyle {
    fn new(font: &usvg::Font) -> Arc<Self> {
        Arc::new(Self {
            families: font.families().into(),
            weight: font.weight(),
            stretch: font.stretch(),
            style: font.style(),
            language: font.selection_language().map(str::to_owned),
        })
    }

    fn heap_bytes(&self) -> usize {
        size_of::<Self>()
            + std::mem::size_of_val(self.families.as_ref())
            + self.language.as_ref().map_or(0, String::len)
            + self
                .families
                .iter()
                .map(|family| match family {
                    usvg::FontFamily::Named(name) => name.len(),
                    _ => 0,
                })
                .sum::<usize>()
    }
}

/// Retain the source so pointer identity cannot be recycled while a choice is pinned.
#[cfg(feature = "experimental-system-font-query")]
#[derive(Clone)]
struct AdmittedFaceKey {
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    index: u32,
}

#[cfg(feature = "experimental-system-font-query")]
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
#[derive(Clone, Hash, Eq, PartialEq)]
pub(super) struct AttemptKey {
    face: AdmittedFaceKey,
    scale: u32,
    variations: Box<[usvg::FontVariation]>,
}

#[cfg(feature = "experimental-system-font-query")]
impl AttemptKey {
    fn new(face: &FaceInfo, scale: f32, variations: &[usvg::FontVariation]) -> Option<Self> {
        Some(Self {
            face: AdmittedFaceKey::of(face)?,
            scale: scale.to_bits(),
            variations: variations.into(),
        })
    }
}

#[cfg(feature = "experimental-system-font-query")]
impl SelectionRequest {
    pub(super) fn primary(font: &usvg::Font) -> Self {
        Self::Primary(SelectionStyle::new(font))
    }

    pub(super) fn fallback(
        request: super::resolver::GlyphFallback<'_>,
        excluded: &[TriedFont],
    ) -> Option<Self> {
        Some(Self::Fallback {
            style: request.font.map(SelectionStyle::new),
            text: request.text.to_owned(),
            language: request.language.map(str::to_owned),
            rtl: request.rtl,
            excluded: excluded
                .iter()
                .map(|font| AttemptKey::new(&font.face, font.scale, &font.variations))
                .collect::<Option<_>>()?,
        })
    }

    fn style(&mut self) -> Option<&mut Arc<SelectionStyle>> {
        match self {
            Self::Primary(style) => Some(style),
            Self::Fallback { style, .. } => style.as_mut(),
        }
    }

    fn heap_bytes(&self) -> usize {
        match self {
            Self::Primary(_) => 0,
            Self::Fallback {
                text,
                language,
                excluded,
                ..
            } => {
                text.len()
                    + language.as_ref().map_or(0, String::len)
                    + std::mem::size_of_val(excluded.as_slice())
                    + excluded
                        .iter()
                        .map(|key| std::mem::size_of_val(key.variations.as_ref()))
                        .sum::<usize>()
            }
        }
    }
}

#[cfg(feature = "experimental-system-font-query")]
#[derive(Clone)]
pub(super) struct SelectedFont<F = FaceInfo> {
    pub(super) face: F,
    pub(super) source: NativeExportFontSource,
    pub(super) scale: f32,
    pub(super) variations: Vec<usvg::FontVariation>,
    pub(super) family_fallback_used: bool,
}

impl SnapshotState {
    fn face_count(&self) -> usize {
        let count = self.faces.len();
        #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
        let count = count.saturating_add(self.native_face_count);
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
    fn validated_platform_face(
        &mut self,
        source: Source,
        index: u32,
        count: u32,
        path: &Path,
        validate: impl FnOnce(&FaceInfo) -> Result<(), NativeFontError>,
    ) -> Result<FaceInfo, NativeFontError> {
        let missing = || source_error(path, "selected face cannot be parsed from snapshot");
        if let Some(collection) = &self.collection
            && same_source(&collection.source, &source)
        {
            let position = collection
                .faces
                .binary_search_by_key(&index, |face| face.index)
                .map_err(|_| missing())?;
            let face = &collection.faces[position];
            validate(face)?;
            return Ok(face.clone());
        }

        let mut parsed = Database::new();
        parsed.load_font_source(source.clone());
        let selected = parsed
            .faces()
            .find(|face| face.index == index)
            .ok_or_else(missing)?;
        let mut next_collection = None;
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
                next_collection = Some(CollectionMetadata {
                    source,
                    faces: faces.into(),
                });
            }
        }
        // Cached members own the full source. Publish them only after the final admission check.
        validate(selected)?;
        if let Some(collection) = next_collection {
            self.collection = Some(collection);
        }
        Ok(selected.clone())
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
        base: &TriedFont,
        control: &OperationControl,
    ) -> Result<Option<platform::coretext::NativeFace>, NativeFontError> {
        let mut state = self.snapshots.lock().map_err(|_| snapshot_poisoned())?;
        let Some(key) = AttemptKey::new(&base.face, base.scale, &base.variations) else {
            return Ok(None);
        };
        if let Some(origin) = state.native_origins.get(&key) {
            return Ok(origin.clone());
        }
        let origin = platform::coretext::registered_base(&base.face, control)?;
        // The source registry bounds the number of admitted physical faces.
        if state.native_origins.len() < 1024 {
            state.native_origins.insert(key, origin.clone());
        }
        Ok(origin)
    }

    #[cfg(feature = "experimental-system-font-query")]
    pub(super) fn selected(
        &self,
        request: &SelectionRequest,
        database: &mut Arc<Database>,
    ) -> Result<Option<SelectedFont<ID>>, NativeFontError> {
        let state = self.snapshots.lock().map_err(|_| snapshot_poisoned())?;
        Ok(state.selections.get(request).map(|selected| SelectedFont {
            face: install(selected.face.as_ref().clone(), database),
            source: selected.source,
            family_fallback_used: selected.family_fallback_used,
            scale: selected.scale,
            variations: selected.variations.clone(),
        }))
    }

    #[cfg(feature = "experimental-system-font-query")]
    pub(super) fn selection_missed(
        &self,
        request: &SelectionRequest,
    ) -> Result<bool, NativeFontError> {
        let state = self.snapshots.lock().map_err(|_| snapshot_poisoned())?;
        Ok(state.misses.contains(request))
    }

    #[cfg(feature = "experimental-system-font-query")]
    pub(super) fn remember_miss(
        &self,
        mut request: SelectionRequest,
        limits: &NativeFontLimits,
    ) -> Result<(), NativeFontError> {
        let mut state = self.snapshots.lock().map_err(|_| snapshot_poisoned())?;
        let mut bytes = size_of::<SelectionRequest>().saturating_add(request.heap_bytes());
        if let Some(style) = request.style() {
            if let Some(shared) = state.authored_stacks.get(style) {
                *style = shared.clone();
            } else {
                bytes = bytes.saturating_add(style.heap_bytes());
            }
        }
        if state.misses.len() < limits.max_cache_entries
            && bytes <= limits.max_cache_bytes.saturating_sub(state.miss_bytes)
            && state.misses.insert(request)
        {
            state.miss_bytes += bytes;
        }
        Ok(())
    }

    #[cfg(feature = "experimental-system-font-query")]
    pub(super) fn remember_selection(
        &self,
        mut request: SelectionRequest,
        selected: SelectedFont,
        #[cfg(target_os = "macos")] origin: Option<platform::coretext::NativeFace>,
        limits: &NativeFontLimits,
    ) -> Result<(), NativeFontError> {
        let mut state = self.snapshots.lock().map_err(|_| snapshot_poisoned())?;
        check_limit(
            "font-selections",
            state.selections.len().saturating_add(1),
            limits.max_selection_entries,
        )?;
        let mut bytes = size_of::<(SelectionRequest, SelectedFont<Arc<FaceInfo>>)>()
            .saturating_add(request.heap_bytes())
            .saturating_add(std::mem::size_of_val(selected.variations.as_slice()));
        let face_key = AdmittedFaceKey::of(&selected.face);
        let pinned = face_key
            .as_ref()
            .and_then(|key| state.pinned_faces.get(key))
            .cloned();
        if pinned.is_none() {
            bytes = bytes
                .saturating_add(size_of::<(AdmittedFaceKey, FaceInfo)>())
                .saturating_add(selected.face.post_script_name.len())
                .saturating_add(
                    selected
                        .face
                        .families
                        .iter()
                        .map(|(name, _)| size_of::<(String, usvg::fontdb::Language)>() + name.len())
                        .sum(),
                );
        }
        let mut new_style = None;
        if let Some(style) = request.style() {
            if let Some(shared) = state.authored_stacks.get(style) {
                *style = shared.clone();
            } else {
                bytes = bytes.saturating_add(style.heap_bytes());
                new_style = Some(style.clone());
            }
        }
        #[cfg(target_os = "macos")]
        let origin = origin
            .zip(AttemptKey::new(
                &selected.face,
                selected.scale,
                &selected.variations,
            ))
            .filter(|(_, key)| !state.native_origins.contains_key(key));
        #[cfg(target_os = "macos")]
        if let Some((origin, _)) = &origin {
            bytes = bytes
                .saturating_add(size_of::<(AttemptKey, platform::coretext::NativeFace)>())
                .saturating_add(origin.diagnostic_path().as_os_str().len())
                .saturating_add(origin.family.len())
                .saturating_add(origin.post_script_name.len())
                .saturating_add(std::mem::size_of_val(origin.variations.as_slice()));
        }
        let total = state.selection_bytes.saturating_add(bytes);
        check_limit("font-selection-bytes", total, limits.max_selection_bytes)?;
        state.selection_bytes = total;
        if let Some(style) = new_style {
            state.authored_stacks.insert(style);
        }
        #[cfg(target_os = "macos")]
        if let Some((origin, key)) = origin {
            state.native_origins.insert(key, Some(origin));
        }
        let face = pinned.unwrap_or_else(|| {
            let face = Arc::new(selected.face);
            if let Some(key) = face_key {
                state.pinned_faces.insert(key, face.clone());
            }
            face
        });
        state.selections.insert(
            request,
            SelectedFont {
                face,
                source: selected.source,
                scale: selected.scale,
                variations: selected.variations,
                family_fallback_used: selected.family_fallback_used,
            },
        );
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
        key: Option<&Path>,
        limits: &NativeFontLimits,
    ) -> Result<(), NativeFontError> {
        let files = self
            .initial
            .acquired_files
            .saturating_add(state.files.len());
        #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
        let files = files.saturating_add(state.anonymous_native_files);
        check_limit(
            "system-snapshot-files",
            files.saturating_add(usize::from(
                key.is_none_or(|key| !state.files.contains_key(key)),
            )),
            limits.max_files,
        )
    }

    #[cfg(test)]
    pub(super) fn materialize(
        &self,
        candidate: FontCandidate,
        fallback: Option<char>,
        database: &mut Arc<Database>,
        limits: &NativeFontLimits,
        control: &OperationControl,
    ) -> Result<Option<ID>, NativeFontError> {
        let mut utf8 = [0; 4];
        let text = fallback.map(|ch| &*ch.encode_utf8(&mut utf8));
        self.admit(
            candidate,
            text.map(GlyphFallback::from),
            database,
            limits,
            control,
        )
    }

    #[cfg(test)]
    pub(super) fn admit(
        &self,
        candidate: FontCandidate,
        fallback: Option<GlyphFallback<'_>>,
        database: &mut Arc<Database>,
        limits: &NativeFontLimits,
        control: &OperationControl,
    ) -> Result<Option<ID>, NativeFontError> {
        self.admit_choice(
            candidate,
            CandidateRequest {
                fallback,
                implicit: false,
                tried: &[],
            },
            database,
            limits,
            control,
        )
        .map(|choice| choice.map(|choice| choice.id))
    }

    pub(super) fn admit_choice(
        &self,
        candidate: FontCandidate,
        request: CandidateRequest<'_>,
        database: &mut Arc<Database>,
        limits: &NativeFontLimits,
        control: &OperationControl,
    ) -> Result<Option<AdmittedFont>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        #[cfg(not(feature = "experimental-system-font-query"))]
        let _ = (request.fallback, request.implicit);
        let (path, index, expected) = match candidate {
            FontCandidate::Loaded(face) => {
                if tried_face(&request, &face.source, face.index, 1.0, &[]) {
                    return Ok(None);
                }
                #[cfg(feature = "experimental-system-font-query")]
                if let Source::Binary(data) = &face.source
                    && (request.implicit || request.fallback.is_some())
                    && !eligible_face(data.as_ref().as_ref(), face.index, &request)
                {
                    return Ok(None);
                }
                let id = install(face, database);
                return Ok(Some(AdmittedFont {
                    id,
                    scale: 1.0,
                    variations: Vec::new(),
                }));
            }
            #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
            FontCandidate::Native(face) => {
                return self.materialize_native(face, request, database, limits, control);
            }
            FontCandidate::File {
                path,
                index,
                expected,
            } => (path, index, expected),
        };
        let mut state = self.snapshots.lock().map_err(|_| snapshot_poisoned())?;
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
            let occupied = self.initial.bytes.saturating_add(state.bytes);
            let budget = FontByteBudget::new("system-font-bytes", occupied, limits.max_bytes);
            let mut bytes = Vec::new();
            let digest = match &expected {
                ExpectedFace::Bounded { digest, .. } => {
                    self.check_file_limit(&state, Some(&key_path), limits)?;
                    read_bounded_into(&path, budget, control, OperationPhase::Layout, &mut bytes)?;
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
                ExpectedFace::Platform(face) => {
                    let mut opened = OpenPlatformFont::new(&key_path, control)?;
                    let Some(preflight_axes) =
                        opened.probe(face, index, &request, limits, &mut state.scan_bytes)?
                    else {
                        return Ok(None);
                    };
                    self.check_file_limit(&state, Some(&key_path), limits)?;
                    opened.rewind()?;
                    read_bounded_file(
                        &path,
                        &mut opened.file,
                        opened.len,
                        budget,
                        control,
                        OperationPhase::Layout,
                        &mut bytes,
                    )?;
                    let actual_axes = file_axes(face, &bytes, index, &path)?;
                    if actual_axes != preflight_axes {
                        return Err(source_error(
                            &path,
                            "font instance changed during snapshot acquisition",
                        ));
                    }
                    None
                }
            };
            (Arc::new(bytes), digest, true)
        };
        let source = Source::Binary(data.clone());
        let (scale, variations) = match &expected {
            ExpectedFace::Bounded { .. } => (1.0, Vec::new()),
            #[cfg(feature = "experimental-system-font-query")]
            ExpectedFace::Platform(face) => {
                let variations = file_axes(face, &data, index, &path)?;
                if !eligible_face(&data, index, &request) {
                    return Ok(None);
                }
                (face.fallback_scale, variations)
            }
        };
        if tried_face(&request, &source, index, scale, &variations) {
            return Ok(None);
        }
        let choice = |id| AdmittedFont {
            id,
            scale,
            variations,
        };
        let admitted = state.faces.get(&key).or_else(|| {
            database
                .faces()
                .find(|face| face.index == index && same_source(&face.source, &source))
        });
        if let Some(face) = admitted.cloned() {
            expected.validate(&face, &path)?;
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            state.aliases.insert(path, key_path);
            return Ok(Some(choice(install(face, database))));
        }
        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        check_limit(
            "font-collection-faces",
            usize::try_from(count).unwrap_or(usize::MAX),
            limits.max_faces,
        )?;
        let parsed =
            ttf_parser::Face::parse(&data, index).map_err(|error| source_error(&path, error))?;
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
            .table(ttf_parser::Tag::from_bytes(b"cmap"))
            .unwrap_or_default();
        let cmap_bytes = checked_cmap_bytes(table, control, state.cmap_bytes, limits)?;
        let validate = |face: &FaceInfo| -> Result<(), NativeFontError> {
            expected.validate(face, &path)?;
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            Ok(())
        };
        let face = match &expected {
            ExpectedFace::Bounded { metadata, digest } => {
                let face = if discovery_digest
                    .as_ref()
                    .is_some_and(|actual| actual.as_slice() == digest)
                {
                    // Discovery parsed these bytes. Rebind its metadata to the snapshot.
                    FaceInfo {
                        source,
                        ..metadata.clone()
                    }
                } else {
                    let mut parsed = Database::new();
                    parsed.load_font_source(source);
                    parsed
                        .faces()
                        .find(|face| face.index == index)
                        .cloned()
                        .ok_or_else(|| {
                            source_error(&path, "selected face cannot be parsed from snapshot")
                        })?
                };
                validate(&face)?;
                face
            }
            #[cfg(feature = "experimental-system-font-query")]
            ExpectedFace::Platform(_) => {
                state.validated_platform_face(source, index, count, &path, validate)?
            }
        };
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
        Ok(Some(choice(install(face, database))))
    }

    #[cfg(all(feature = "experimental-system-font-query", target_os = "macos"))]
    fn materialize_native(
        &self,
        candidate: platform::coretext::NativeFace,
        request: CandidateRequest<'_>,
        database: &mut Arc<Database>,
        limits: &NativeFontLimits,
        control: &OperationControl,
    ) -> Result<Option<AdmittedFont>, NativeFontError> {
        let mut state = self.snapshots.lock().map_err(|_| snapshot_poisoned())?;
        let variations = candidate.variations.clone();
        let known_choice = |face: &FaceInfo, database: &mut Arc<Database>| {
            if tried_face(&request, &face.source, face.index, 1.0, &variations) {
                return None;
            }
            Some(AdmittedFont {
                id: install(face.clone(), database),
                scale: 1.0,
                variations: variations.clone(),
            })
        };
        if let Some(face) = state.native_faces.get(&candidate) {
            let Source::Binary(data) = &face.source else {
                return Ok(None);
            };
            return if eligible_face(data.as_ref().as_ref(), face.index, &request) {
                Ok(known_choice(face, database))
            } else {
                Ok(None)
            };
        }
        if !candidate.check_eligibility(request.fallback, request.implicit, control)? {
            return Ok(None);
        }
        let path = candidate.diagnostic_path();
        let key = candidate
            .path
            .as_ref()
            .and_then(|path| self.source_key(&state, path).ok());
        let existing = key.as_ref().and_then(|key| {
            self.initial
                .files
                .get(key)
                .cloned()
                .or_else(|| match state.files.get(key) {
                    Some(FontSnapshot::Whole { data, .. }) => Some(Arc::clone(data)),
                    _ => None,
                })
        });
        // Distinct native handles and design coordinates can share one physical face.
        let mut equivalent = None;
        for (origin, face) in &state.native_faces {
            if origin.post_script_name == candidate.post_script_name
                && origin
                    .path
                    .as_ref()
                    .and_then(|path| state.aliases.get(path))
                    == key.as_ref()
                && let Source::Binary(data) = &face.source
                && candidate.matches_data(data.as_ref().as_ref(), face.index, control)?
            {
                equivalent = Some(face.clone());
                break;
            }
        }
        if let Some(face) = equivalent {
            let choice = known_choice(&face, database);
            if state.native_faces.len() < limits.max_faces.saturating_mul(2) {
                state.native_faces.insert(candidate, face);
            }
            return Ok(choice);
        }
        let next_face = self
            .initial
            .faces
            .saturating_add(state.face_count())
            .saturating_add(1);
        let (data, index, new_bytes) = if let Some(data) = existing {
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
                return Ok(known_choice(&face, database));
            }
            check_limit("font-faces", next_face, limits.max_faces)?;
            (data, index, 0)
        } else {
            self.check_file_limit(&state, key.as_deref(), limits)?;
            check_limit("font-faces", next_face, limits.max_faces)?;
            let occupied = self.initial.bytes.saturating_add(state.bytes);
            let budget = FontByteBudget::new("system-font-bytes", occupied, limits.max_bytes);
            let shared =
                if let Some(key) = key.as_ref().filter(|key| !state.files.contains_key(*key)) {
                    shared_collection::shared_collection(
                        &candidate,
                        key,
                        budget,
                        limits.max_faces,
                        control,
                    )?
                } else {
                    None
                };
            let (bytes, index) = match shared {
                Some(collection) => collection,
                None => (candidate.snapshot(budget, control)?, 0),
            };
            let length = bytes.len();
            (Arc::new(bytes), index, length)
        };
        let parsed =
            ttf_parser::Face::parse(&data, index).map_err(|error| source_error(path, error))?;
        if !eligible_face(&data, index, &request) {
            return Ok(None);
        }
        let source = Source::Binary(data.clone());
        if tried_face(&request, &source, index, 1.0, &variations) {
            return Ok(None);
        }
        let table = parsed
            .raw_face()
            .table(ttf_parser::Tag::from_bytes(b"cmap"))
            .unwrap_or_default();
        let cmap_bytes = checked_cmap_bytes(table, control, state.cmap_bytes, limits)?;
        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        let face = state.validated_platform_face(source, index, count, path, |face| {
            candidate.validate(face)?;
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            Ok(())
        })?;
        state.bytes = state.bytes.saturating_add(new_bytes);
        state.cmap_bytes = cmap_bytes;
        if let Some(key) = key {
            if let Some(path) = &candidate.path {
                state.aliases.insert(path.clone(), key.clone());
            }
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
        } else {
            state.anonymous_native_files += 1;
        }
        state.native_face_count += 1;
        state.native_faces.insert(candidate, face.clone());
        drop(state);
        Ok(Some(AdmittedFont {
            id: install(face, database),
            scale: 1.0,
            variations,
        }))
    }
}

fn snapshot_poisoned() -> NativeFontError {
    NativeFontError::Measurement("font snapshot state is poisoned".to_owned())
}

fn tried_face(
    request: &CandidateRequest<'_>,
    source: &Source,
    index: u32,
    scale: f32,
    variations: &[usvg::FontVariation],
) -> bool {
    request.tried.iter().any(|used| {
        used.face.index == index
            && same_source(&used.face.source, source)
            && used.scale == scale
            && used.variations == variations
    })
}

#[cfg(feature = "experimental-system-font-query")]
fn file_axes(
    expected: &platform::PlatformFace,
    data: &[u8],
    index: u32,
    path: &Path,
) -> Result<Vec<usvg::FontVariation>, NativeFontError> {
    let table = if expected.has_variations() {
        ttf_parser::RawFace::parse(data, index)
            .ok()
            .and_then(|raw| raw.table(ttf_parser::Tag::from_bytes(b"fvar")))
    } else {
        None
    };
    platform_file::axes(expected, table).map_err(|reason| source_error(path, reason))
}

#[cfg(feature = "experimental-system-font-query")]
fn eligible_face(data: &[u8], index: u32, request: &CandidateRequest<'_>) -> bool {
    let Ok(face) = ttf_parser::Face::parse(data, index) else {
        return false;
    };
    let table = |tag| face.raw_face().table(ttf_parser::Tag::from_bytes(tag));
    if request.implicit
        && !crate::font_selection::allows_font_tables(
            table(b"head").unwrap_or_default(),
            table(b"OS/2"),
            table(b"cmap").unwrap_or_default(),
            table(b"maxp").unwrap_or_default(),
            request.fallback.map(|fallback| fallback.text),
        )
    {
        return false;
    }
    request
        .fallback
        .is_none_or(|fallback| platform::cluster::may_shape(face, fallback))
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
    budget: FontByteBudget,
    control: &OperationControl,
    phase: OperationPhase,
    bytes: &mut Vec<u8>,
) -> Result<(), NativeFontError> {
    let metadata = verify_font_file(path, control, phase)?;
    let mut file = fs::File::open(path).map_err(|error| source_error(path, error))?;
    read_bounded_file(
        path,
        &mut file,
        metadata.len(),
        budget,
        control,
        phase,
        bytes,
    )
}

fn read_bounded_file(
    path: &Path,
    file: &mut fs::File,
    length: u64,
    budget: FontByteBudget,
    control: &OperationControl,
    phase: OperationPhase,
    bytes: &mut Vec<u8>,
) -> Result<(), NativeFontError> {
    budget.check(length)?;
    let size = length as usize;
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
        budget.check((bytes.len() as u64).saturating_add(count as u64))?;
        bytes
            .try_reserve_exact(count)
            .map_err(|error| source_error(path, error))?;
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
        let path = Path::new("Cousine-test-collection.ttc");
        let select = |state: &mut SnapshotState, source: &Source, index| {
            state.validated_platform_face(source.clone(), index, 2, path, |_| Ok(()))
        };
        let regular = select(&mut state, &source, 0).unwrap();
        let cached = Arc::clone(&state.collection.as_ref().unwrap().faces);
        assert_eq!(cached.len(), 2);
        let italic = select(&mut state, &source, 1).unwrap();
        assert_eq!(regular.post_script_name, "Cousine-Regular");
        assert_eq!(italic.post_script_name, "Cousine-Italic");
        assert!(same_source(&italic.source, &source));
        assert!(Arc::ptr_eq(
            &cached,
            &state.collection.as_ref().unwrap().faces
        ));
        assert!(select(&mut state, &source, 2).is_err());

        let mut export = state.clone();
        assert!(Arc::ptr_eq(
            &cached,
            &export.collection.as_ref().unwrap().faces
        ));
        let another = Source::Binary(Arc::new(bytes.to_vec()));
        assert!(
            export
                .validated_platform_face(another.clone(), 1, 2, path, |_| {
                    Err(source_error(path, "font identity changed"))
                })
                .is_err()
        );
        assert!(Arc::ptr_eq(
            &cached,
            &export.collection.as_ref().unwrap().faces
        ));
        assert!(same_source(
            &select(&mut export, &another, 0).unwrap().source,
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
            read_bounded_into(
                &path,
                FontByteBudget::new("system-font-bytes", 0, limit),
                &control,
                OperationPhase::Admission,
                bytes,
            )
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
