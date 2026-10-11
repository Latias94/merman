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

#[derive(Default)]
pub(super) struct SnapshotState {
    pub(super) files: BTreeMap<PathBuf, Arc<Vec<u8>>>,
    aliases: BTreeMap<PathBuf, PathBuf>,
    faces: BTreeMap<(PathBuf, u32), FaceInfo>,
    cmap_bytes: usize,
    bytes: usize,
}

/// Initial bytes are immutable and shared; later admissions belong to one operation.
pub(super) struct FontSourceRegistry {
    initial: Arc<InitialSources>,
    pub(super) snapshots: Mutex<SnapshotState>,
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
        }
    }

    pub(super) fn for_operation(&self) -> Self {
        Self {
            initial: Arc::clone(&self.initial),
            snapshots: Mutex::new(SnapshotState::default()),
        }
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
                .map(|state| state.faces.len())
                .unwrap_or_default(),
        )
    }

    pub(super) fn materialize(
        &self,
        candidate: FontCandidate,
        database: &mut Arc<Database>,
        limits: &NativeFontLimits,
        control: &OperationControl,
    ) -> Result<ID, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let (path, index, expected) = match candidate {
            FontCandidate::Loaded(face) => return Ok(install(face, database)),
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
        let key_path = if let Some(key) = self
            .initial
            .aliases
            .get(&path)
            .or_else(|| state.aliases.get(&path))
        {
            key.clone()
        } else if self.initial.files.contains_key(&path) || state.files.contains_key(&path) {
            path.clone()
        } else {
            fs::canonicalize(&path).map_err(|error| source_error(&path, error))?
        };
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let data = if let Some(data) = self
            .initial
            .files
            .get(&key_path)
            .or_else(|| state.files.get(&key_path))
        {
            Arc::clone(data)
        } else {
            check_limit(
                "system-snapshot-files",
                self.initial
                    .acquired_files
                    .saturating_add(state.files.len())
                    .saturating_add(1),
                limits.max_files,
            )?;
            let occupied = self.initial.bytes.saturating_add(state.bytes);
            let bytes = read_bounded(
                &path,
                limits.max_bytes.saturating_sub(occupied),
                control,
                OperationPhase::Layout,
            )?;
            match &expected {
                ExpectedFace::Bounded { digest, .. } => {
                    if Sha256::digest(&bytes)[..] != *digest {
                        return Err(source_error(
                            &path,
                            "font changed after metadata discovery; retry the operation",
                        ));
                    }
                }
                #[cfg(feature = "experimental-system-font-query")]
                ExpectedFace::Platform(_) => {}
            }
            let data = Arc::new(bytes);
            state.bytes = state.bytes.saturating_add(data.len());
            state.files.insert(key_path.clone(), Arc::clone(&data));
            data
        };
        state.aliases.insert(path.clone(), key_path.clone());
        let source = Source::Binary(data.clone());
        if let Some(face) = database
            .faces()
            .find(|face| face.index == index && same_source(&face.source, &source))
        {
            expected.validate(face, &path)?;
            return Ok(face.id);
        }
        let key = (key_path, index);
        if let Some(face) = state.faces.get(&key).cloned() {
            expected.validate(&face, &path)?;
            return Ok(install(face, database));
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
                .saturating_add(state.faces.len())
                .saturating_add(1),
            limits.max_faces,
        )?;
        let parsed = rustybuzz::ttf_parser::Face::parse(&data, index)
            .map_err(|error| source_error(&path, error))?;
        let table = parsed
            .raw_face()
            .table(rustybuzz::ttf_parser::Tag::from_bytes(b"cmap"))
            .unwrap_or_default();
        let mut cmap_bytes = state.cmap_bytes;
        let _ = snapshot_cmap(table, control, &mut cmap_bytes, limits)?;
        let mut parsed_database = Database::new();
        parsed_database.load_font_source(source);
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let face = parsed_database
            .faces()
            .find(|face| face.index == index)
            .cloned()
            .ok_or_else(|| source_error(&path, "selected face cannot be parsed from snapshot"))?;
        expected.validate(&face, &path)?;
        state.cmap_bytes = cmap_bytes;
        state.faces.insert(key, face.clone());
        drop(state);
        Ok(install(face, database))
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

pub(super) fn read_bounded(
    path: &Path,
    limit: usize,
    control: &OperationControl,
    phase: OperationPhase,
) -> Result<Vec<u8>, NativeFontError> {
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
    check_limit(
        "system-font-bytes",
        usize::try_from(metadata.len()).unwrap_or(usize::MAX),
        limit,
    )?;
    let mut file = fs::File::open(path).map_err(|error| source_error(path, error))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(usize::try_from(metadata.len()).unwrap_or(usize::MAX))
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
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admitted_original_path_survives_removal_but_operation_fork_reopens_dynamic_sources() {
        let root = std::env::temp_dir().join(format!(
            "merman-font-alias-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&root).unwrap();
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
                &mut Arc::new(Database::new()),
                &NativeFontLimits::default(),
                &control,
            )
            .unwrap();
        assert_eq!(registry.resident_bytes(), resident);
        let next = registry.for_operation();
        assert_eq!(next.resident_bytes(), 0);
        assert!(
            next.materialize(
                make_candidate(),
                &mut Arc::new(Database::new()),
                &NativeFontLimits::default(),
                &control
            )
            .is_err()
        );
        fs::remove_dir(&root).unwrap();
    }
}
