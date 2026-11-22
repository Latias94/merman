//! Platform-managed discovery with bounded, operation-local binary snapshots.

use super::resolver::{ExpectedFace, FontCandidate};
use super::*;
#[cfg(any(not(target_os = "macos"), test))]
use fontique::CollectionOptions;
use fontique::{
    Collection, FallbackKey, FamilyId, FamilyInfo, FontInfo, FontStyle, FontWeight, FontWidth,
    GenericFamily, SourceKind,
};
use ttf_parser::{RawFace, Tag, name, name_id};
use unicode_script::UnicodeScript;
use usvg::fontdb::{Database, FaceInfo, Family, ID, Source, Stretch, Style, Weight};

type AcceptFallback<'a> = dyn FnMut(FontCandidate) -> Result<Option<ID>, NativeFontError> + 'a;

#[cfg(windows)]
#[allow(unsafe_code)]
mod dwrite;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub(super) mod coretext;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod fontconfig;

pub(super) fn ensure_available(control: &OperationControl) -> Result<(), NativeFontError> {
    control.terminal_checkpoint_at(OperationPhase::Admission)?;
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if !fontconfig::library_available() {
        return Err(NativeFontError::PlatformDiscoveryUnavailable {
            service: "Fontconfig",
        });
    }
    control.terminal_checkpoint_at(OperationPhase::Admission)?;
    Ok(())
}

struct PlatformCandidate {
    font: FontInfo,
    family: String,
}

/// Identity of a selected face as reported by its discovery provider.
pub(super) struct PlatformFace {
    index: u32,
    family: String,
    post_script_name: Option<String>,
    attributes: FaceAttributes,
    coverage_source: Option<FontInfo>,
}

#[derive(Default)]
struct FaceAttributes {
    width: Option<FontWidth>,
    style: Option<FontStyle>,
    weight: Option<FontWeight>,
}

pub(super) struct PlatformCatalog {
    collection: Option<Mutex<Collection>>,
    invalid: std::sync::atomic::AtomicBool,
    #[cfg(windows)]
    character_fallback: OnceLock<Option<dwrite::CharacterFallback>>,
    #[cfg(test)]
    family_queries: std::sync::atomic::AtomicUsize,
}

impl PlatformCatalog {
    pub fn new(control: &OperationControl) -> Result<Self, NativeFontError> {
        ensure_available(control)?;
        // Native discovery may map candidate files internally. This is an explicitly
        // authorized platform policy, not the bounded metadata discovery policy.
        #[cfg(not(target_os = "macos"))]
        let collection = Some(Mutex::new(platform_call(|| {
            Collection::new(CollectionOptions::default())
        })?));
        #[cfg(target_os = "macos")]
        let collection = None;
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        Ok(Self {
            collection,
            invalid: std::sync::atomic::AtomicBool::new(false),
            #[cfg(windows)]
            character_fallback: OnceLock::new(),
            #[cfg(test)]
            family_queries: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    fn call<T>(&self, call: impl FnOnce() -> T) -> Result<T, NativeFontError> {
        if self.invalid.load(std::sync::atomic::Ordering::Acquire) {
            return Err(NativeFontError::Measurement(
                "platform font collection failed previously".to_owned(),
            ));
        }
        let result = platform_call(call);
        if result.is_err() {
            self.invalid
                .store(true, std::sync::atomic::Ordering::Release);
        }
        result
    }

    pub(super) fn default_family(
        &self,
        control: &OperationControl,
    ) -> Result<Option<String>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        #[cfg(target_os = "macos")]
        if self.collection.is_none() {
            let face = self.call(|| {
                coretext::named("Helvetica", Weight::NORMAL, Stretch::Normal, Style::Normal)
            })?;
            control.terminal_checkpoint_at(OperationPhase::Admission)?;
            return Ok(face.map(|face| face.family));
        }
        let mut collection = self
            .collection
            .as_ref()
            .ok_or_else(|| {
                NativeFontError::Measurement("Fontique catalog is not active".to_owned())
            })?
            .lock()
            .map_err(|_| {
                NativeFontError::Measurement("platform font collection is poisoned".to_owned())
            })?;
        // Find a usable default family name for context identity without asking for its faces.
        let name = self.call(|| {
            for generic in [GenericFamily::SansSerif, GenericFamily::Serif] {
                let id = collection.generic_families(generic).next();
                if let Some(name) = id.and_then(|id| collection.family_name(id)) {
                    return Some(name.to_owned());
                }
            }
            None
        })?;
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        Ok(name)
    }

    fn query<T>(
        &self,
        control: &OperationControl,
        query: impl FnOnce(&mut Collection) -> T,
    ) -> Result<T, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let mut collection = self
            .collection
            .as_ref()
            .ok_or_else(|| {
                NativeFontError::Measurement("Fontique catalog is not active".to_owned())
            })?
            .lock()
            .map_err(|_| {
                NativeFontError::Measurement("platform font collection is poisoned".to_owned())
            })?;
        let result = self.call(|| query(&mut collection))?;
        drop(collection);
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        Ok(result)
    }

    fn family(
        &self,
        id: FamilyId,
        control: &OperationControl,
    ) -> Result<Option<FamilyInfo>, NativeFontError> {
        self.query(control, |collection| {
            #[cfg(test)]
            self.family_queries
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            collection.family(id)
        })
    }

    fn candidates<T>(
        &self,
        requested: &usvg::FontFamily,
        weight: Weight,
        stretch: Stretch,
        style: Style,
        control: &OperationControl,
        mut accept: impl FnMut(PlatformCandidate) -> Result<Option<T>, NativeFontError>,
    ) -> Result<Option<T>, NativeFontError> {
        let ids = self.query(control, |collection| match requested {
            usvg::FontFamily::Named(name) if !name.is_empty() => {
                collection.family_id(name).into_iter().collect::<Vec<_>>()
            }
            _ => generic_family(requested)
                .map(|generic| collection.generic_families(generic).collect())
                .unwrap_or_default(),
        })?;
        for id in ids {
            if let Some(family) = self.family(id, control)?
                && let Some(candidate) = matching_candidate(&family, weight, stretch, style)
                && let Some(result) = accept(candidate)?
            {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }

    pub(super) fn fallback(
        &self,
        character: char,
        base: &FaceInfo,
        control: &OperationControl,
        sources: &super::registry::FontSourceRegistry,
        accept: &mut AcceptFallback<'_>,
    ) -> Result<Option<ID>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let _ = sources;
        #[cfg(target_os = "macos")]
        if self.collection.is_none() {
            let origin = sources.native_origin(base)?;
            return self.call(|| {
                coretext::fallback_candidates(
                    character,
                    base,
                    origin.as_ref(),
                    control,
                    &mut |face| accept(FontCandidate::Native(face)),
                )
            })?;
        }
        #[cfg(windows)]
        let prefer_exact = !matches!(
            character.script(),
            unicode_script::Script::Common
                | unicode_script::Script::Inherited
                | unicode_script::Script::Unknown
        );
        #[cfg(windows)]
        if prefer_exact {
            // A script sample can miss the needed subset. Use the exact character
            // before opening the sample's candidate files.
            if let Some(id) = self.exact_fallback(character, base, control, accept)? {
                return Ok(Some(id));
            }
        }
        // Shared symbols and combining characters don't identify a run's script on their own.
        // Keep the established fallback order for them before trying a scalar-only lookup.
        for family_id in self.fallback_families(character, control)? {
            if let Some(family) = self.family(family_id, control)?
                && let Some(candidate) =
                    matching_candidate(&family, base.weight, base.stretch, base.style)
                && let Some(candidate) = self.candidate_file(candidate, control)?
                && let Some(id) = accept(candidate)?
            {
                return Ok(Some(id));
            }
        }
        #[cfg(windows)]
        if !prefer_exact {
            return self.exact_fallback(character, base, control, accept);
        }
        Ok(None)
    }

    #[cfg(all(target_os = "macos", test))]
    fn discovery_data(
        &self,
        font: &FontInfo,
        sources: &super::registry::FontSourceRegistry,
    ) -> Result<Option<fontique::Blob<u8>>, NativeFontError> {
        if let SourceKind::Path(path) = font.source().kind()
            && let Some(data) = sources.snapshot(path)?
        {
            return Ok(Some(fontique::Blob::new(data)));
        }
        self.call(|| font.load(None))
    }

    #[cfg(windows)]
    fn directwrite(&self) -> Option<&dwrite::CharacterFallback> {
        self.character_fallback
            .get_or_init(dwrite::CharacterFallback::new)
            .as_ref()
    }

    #[cfg(windows)]
    fn exact_fallback(
        &self,
        character: char,
        base: &FaceInfo,
        control: &OperationControl,
        accept: &mut AcceptFallback<'_>,
    ) -> Result<Option<ID>, NativeFontError> {
        let exact = self
            .directwrite()
            .and_then(|fallback| fallback.lookup(character, base));
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        match exact {
            Some(candidate) => accept(candidate),
            None => Ok(None),
        }
    }

    fn fallback_families(
        &self,
        character: char,
        control: &OperationControl,
    ) -> Result<Vec<FamilyId>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let mut collection = self
            .collection
            .as_ref()
            .ok_or_else(|| {
                NativeFontError::Measurement("Fontique catalog is not active".to_owned())
            })?
            .lock()
            .map_err(|_| {
                NativeFontError::Measurement("platform font collection is poisoned".to_owned())
            })?;
        let mut ids = Vec::new();
        let scripts =
            std::iter::once(character.script()).chain(character.script_extension().iter());
        for script in scripts {
            let script = fontique::Script::from_bytes(script.as_iso15924_tag().to_be_bytes());
            for id in self.call(|| {
                collection
                    .fallback_families(FallbackKey::new(script, None))
                    .collect::<Vec<_>>()
            })? {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
        }
        for generic in [
            GenericFamily::Emoji,
            GenericFamily::SansSerif,
            GenericFamily::Serif,
        ] {
            for id in self.call(|| collection.generic_families(generic).collect::<Vec<_>>())? {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
        }
        Ok(ids)
    }

    fn candidate_file(
        &self,
        candidate: PlatformCandidate,
        control: &OperationControl,
    ) -> Result<Option<FontCandidate>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let SourceKind::Path(path) = candidate.font.source().kind() else {
            return Err(NativeFontError::Measurement(
                "platform candidate has no snapshot-compatible file path".to_owned(),
            ));
        };
        #[cfg(windows)]
        if !candidate.font.axes().is_empty() {
            let unsupported = self.directwrite().is_some_and(|directwrite| {
                directwrite.is_variable_family_alias(
                    &candidate.family,
                    path,
                    candidate.font.index(),
                )
            });
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            if unsupported {
                // The usvg bridge stores a physical face, which cannot carry the Windows
                // family's named axis settings. Continue the declared font stack.
                return Ok(None);
            }
        }
        let path = path.as_ref().to_owned();
        Ok(Some(candidate.into_face().into_file(path)))
    }
}

fn matching_candidate(
    family: &FamilyInfo,
    weight: Weight,
    stretch: Stretch,
    style: Style,
) -> Option<PlatformCandidate> {
    family
        .match_font(
            width(stretch),
            platform_style(style),
            FontWeight::new(f32::from(weight.0)),
            false,
        )
        .map(|font| PlatformCandidate {
            font: font.clone(),
            family: family.name().to_owned(),
        })
}

fn platform_call<T>(call: impl FnOnce() -> T) -> Result<T, NativeFontError> {
    // Native failures must reach the fallible boundary. Aborting panic profiles
    // and native calls that do not return remain outside this containment.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(call))
        .map_err(|_| NativeFontError::Measurement("platform font discovery panicked".to_owned()))
}

fn width(stretch: Stretch) -> FontWidth {
    match stretch {
        Stretch::UltraCondensed => FontWidth::ULTRA_CONDENSED,
        Stretch::ExtraCondensed => FontWidth::EXTRA_CONDENSED,
        Stretch::Condensed => FontWidth::CONDENSED,
        Stretch::SemiCondensed => FontWidth::SEMI_CONDENSED,
        Stretch::Normal => FontWidth::NORMAL,
        Stretch::SemiExpanded => FontWidth::SEMI_EXPANDED,
        Stretch::Expanded => FontWidth::EXPANDED,
        Stretch::ExtraExpanded => FontWidth::EXTRA_EXPANDED,
        Stretch::UltraExpanded => FontWidth::ULTRA_EXPANDED,
    }
}

fn platform_style(style: Style) -> FontStyle {
    match style {
        Style::Normal => FontStyle::Normal,
        Style::Italic => FontStyle::Italic,
        Style::Oblique => FontStyle::Oblique(None),
    }
}

impl PlatformFace {
    #[cfg(target_os = "macos")]
    pub(super) fn selected_collection_data(
        &self,
        path: &Path,
        limit: usize,
        max_faces: usize,
        control: &OperationControl,
    ) -> Result<Option<Vec<u8>>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let Some(data) = platform_call(|| self.coverage_source.as_ref()?.load(None))? else {
            return Ok(None);
        };
        let Some(count) = ttf_parser::fonts_in_collection(data.as_ref()) else {
            return Ok(None);
        };
        check_limit("font-collection-faces", count as usize, max_faces)?;
        let raw =
            RawFace::parse(data.as_ref(), self.index).map_err(|error| source_error(path, error))?;
        let table = raw
            .table(Tag::from_bytes(b"name"))
            .and_then(name::Table::parse)
            .ok_or_else(|| source_error(path, "selected collection face has no name table"))?;
        let post_script_name = if let Some(expected) = &self.post_script_name {
            if !post_script_name_matches(table, expected) {
                return Err(source_error(
                    path,
                    "font identity changed after platform lookup",
                ));
            }
            Some(expected.clone())
        } else {
            table.names.into_iter().find_map(|record| {
                (record.name_id == name_id::POST_SCRIPT_NAME)
                    .then(|| {
                        record.to_string().or_else(|| {
                            (record.platform_id == ttf_parser::PlatformId::Macintosh
                                && record.encoding_id == 0
                                && record.name.is_ascii())
                            .then(|| String::from_utf8_lossy(record.name).into_owned())
                        })
                    })
                    .flatten()
            })
        };
        let Some(name) = post_script_name else {
            return Ok(None);
        };
        coretext::collection_snapshot(&name, path, &raw, limit, control)
    }

    pub(super) fn discovery_covers(
        &self,
        character: char,
    ) -> Result<Option<bool>, NativeFontError> {
        platform_call(|| {
            let font = self.coverage_source.as_ref()?;
            // Fontique maps path sources without copying the whole binary. Reuse the
            // admission parser and coverage rules, including malformed-table handling.
            let data = font.load(None)?;
            let face = ttf_parser::Face::parse(data.as_ref(), font.index()).ok()?;
            Some(crate::font_selection::covers_character(&face, character))
        })
    }

    fn into_file(self, path: PathBuf) -> FontCandidate {
        FontCandidate::File {
            path,
            index: self.index,
            expected: ExpectedFace::Platform(self),
        }
    }

    pub(super) fn validate(
        &self,
        face: &FaceInfo,
        path: &Path,
        origin_index: u32,
    ) -> Result<(), NativeFontError> {
        if origin_index != self.index
            || !self.attributes.matches(face)
            || !matches_family(face, &self.family)
            || self
                .post_script_name
                .as_ref()
                .is_some_and(|name| !matches_post_script_name(face, name))
        {
            return Err(source_error(
                path,
                "font properties changed after platform lookup; retry the operation",
            ));
        }
        Ok(())
    }
}

fn matches_post_script_name(face: &FaceInfo, expected: &str) -> bool {
    if face.post_script_name == expected {
        return true;
    }
    // fontdb stores the first supported record. The platform can select a different record
    // from the very same face, so consult its admitted name table before rejecting the match.
    face_names(face).is_some_and(|table| post_script_name_matches(table, expected))
}

fn post_script_name_matches(table: name::Table<'_>, expected: &str) -> bool {
    table.names.into_iter().any(|record| {
        record.name_id == name_id::POST_SCRIPT_NAME
            && (record.to_string().is_some_and(|name| name == expected)
                // Some macOS fonts have only Macintosh Roman name records, which
                // ttf-parser's Unicode conversion intentionally excludes. OpenType
                // PostScript names are ASCII, so no legacy text decoder is needed.
                || (record.platform_id == ttf_parser::PlatformId::Macintosh
                    && record.encoding_id == 0
                    && record.name.is_ascii()
                    && record.name == expected.as_bytes()))
    })
}

impl FaceAttributes {
    fn matches(&self, face: &FaceInfo) -> bool {
        self.style.is_none_or(|selected| {
            matches!(
                (face.style, selected),
                (Style::Normal, FontStyle::Normal)
                    | (Style::Italic, FontStyle::Italic)
                    | (Style::Oblique, FontStyle::Oblique(_))
            )
        }) && self
            .width
            .is_none_or(|selected| width(face.stretch) == selected)
            && self
                .weight
                .is_none_or(|selected| f32::from(face.weight.0) == selected.value())
    }
}

fn matches_family(face: &FaceInfo, expected: &str) -> bool {
    if face
        .families
        .iter()
        .any(|(family, _)| unicase::eq(family.as_str(), expected))
    {
        return true;
    }
    // fontdb prefers typographic families, while native providers may use legacy
    // or WWS names. Resolve these aliases from the admitted bytes, never the path.
    face_names(face).is_some_and(|table| family_names_match(table, expected))
}

fn face_names(face: &FaceInfo) -> Option<name::Table<'_>> {
    let Source::Binary(data) = &face.source else {
        return None;
    };
    let raw = RawFace::parse(data.as_ref().as_ref(), face.index).ok()?;
    name::Table::parse(raw.table(Tag::from_bytes(b"name"))?)
}

fn family_names_match(table: name::Table<'_>, expected: &str) -> bool {
    table.names.into_iter().any(|name| {
        matches!(
            name.name_id,
            name_id::FAMILY | name_id::TYPOGRAPHIC_FAMILY | name_id::WWS_FAMILY
        ) && name
            .to_string()
            .is_some_and(|family| unicase::eq(family.as_str(), expected))
    })
}

impl PlatformCandidate {
    fn into_face(self) -> PlatformFace {
        PlatformFace {
            family: self.family,
            index: self.font.index(),
            post_script_name: None,
            attributes: FaceAttributes {
                width: Some(self.font.width()),
                style: Some(self.font.style()),
                weight: Some(self.font.weight()),
            },
            coverage_source: Some(self.font),
        }
    }
}

fn explicit_face(
    database: &Database,
    family: &usvg::FontFamily,
    weight: Weight,
    stretch: Stretch,
    style: Style,
) -> Option<FaceInfo> {
    let id = match family {
        usvg::FontFamily::Named(name) => super::super::query_named_font_family_case_insensitively(
            database, name, weight, stretch, style,
        ),
        _ => super::super::query_font_family(
            database,
            database_family(family)?,
            weight,
            stretch,
            style,
        ),
    }?;
    database.face(id).cloned()
}

fn database_family(family: &usvg::FontFamily) -> Option<Family<'static>> {
    Some(match family {
        usvg::FontFamily::Serif => Family::Serif,
        usvg::FontFamily::SansSerif => Family::SansSerif,
        usvg::FontFamily::Monospace => Family::Monospace,
        usvg::FontFamily::Cursive => Family::Cursive,
        usvg::FontFamily::Fantasy => Family::Fantasy,
        usvg::FontFamily::Named(_) => return None,
    })
}

fn generic_family(family: &usvg::FontFamily) -> Option<GenericFamily> {
    Some(match family {
        usvg::FontFamily::Serif => GenericFamily::Serif,
        usvg::FontFamily::SansSerif => GenericFamily::SansSerif,
        usvg::FontFamily::Monospace => GenericFamily::Monospace,
        usvg::FontFamily::Cursive => GenericFamily::Cursive,
        usvg::FontFamily::Fantasy => GenericFamily::Fantasy,
        usvg::FontFamily::Named(_) => return None,
    })
}

impl PlatformCatalog {
    pub(super) fn select(
        &self,
        font: &usvg::Font,
        explicit: &Database,
        control: &OperationControl,
        accept: &mut dyn FnMut(
            super::resolver::FontSelection,
            bool,
        ) -> Result<Option<usvg::fontdb::ID>, NativeFontError>,
    ) -> Result<Option<usvg::fontdb::ID>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let weight = Weight(font.weight());
        let stretch = font.stretch().into();
        let style = font.style().into();
        for (index, family) in font
            .families()
            .iter()
            .chain(std::iter::once(&usvg::FontFamily::Serif))
            .enumerate()
        {
            let family = if matches!(family, usvg::FontFamily::Named(name) if name.is_empty())
                && explicit.faces().next().is_none()
            {
                &usvg::FontFamily::SansSerif
            } else {
                family
            };
            let family_fallback_used = index == font.families().len();
            let implicit = !matches!(family, usvg::FontFamily::Named(_));
            if let Some(face) = explicit_face(explicit, family, weight, stretch, style)
                && let Some(id) = accept(
                    super::resolver::FontSelection {
                        candidate: FontCandidate::Loaded(face),
                        family_fallback_used,
                    },
                    implicit,
                )?
            {
                return Ok(Some(id));
            }
            #[cfg(target_os = "macos")]
            if self.collection.is_none() {
                // Keep the established generic choices, but ask Core Text for each
                // family only when the preceding candidate has not been accepted.
                let names: &[&str] = match family {
                    usvg::FontFamily::Named(name) => &[name.as_str()],
                    usvg::FontFamily::SansSerif => &["Helvetica"],
                    usvg::FontFamily::Serif => &["Times", "Times New Roman"],
                    usvg::FontFamily::Monospace => &["Courier", "Courier New"],
                    usvg::FontFamily::Cursive => &["Apple Chancery"],
                    usvg::FontFamily::Fantasy => &["Papyrus"],
                };
                for name in names {
                    control.terminal_checkpoint_at(OperationPhase::Layout)?;
                    let selected = self.call(|| coretext::named(name, weight, stretch, style))?;
                    control.terminal_checkpoint_at(OperationPhase::Layout)?;
                    if let Some(face) = selected
                        && let Some(id) = accept(
                            super::resolver::FontSelection {
                                candidate: FontCandidate::Native(face),
                                family_fallback_used,
                            },
                            implicit,
                        )?
                    {
                        return Ok(Some(id));
                    }
                }
                continue;
            }
            if let Some(id) =
                self.candidates(family, weight, stretch, style, control, |selected| {
                    let Some(candidate) = self.candidate_file(selected, control)? else {
                        return Ok(None);
                    };
                    accept(
                        super::resolver::FontSelection {
                            candidate,
                            family_fallback_used,
                        },
                        implicit,
                    )
                })?
            {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_catalog() -> PlatformCatalog {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
        let mut collection = Collection::new(CollectionOptions {
            system_fonts: false,
            ..Default::default()
        });
        collection.load_fonts_from_paths([
            root.join("Cousine-Regular.ttf"),
            root.join("NotoSans-Latin-subset.ttf"),
        ]);
        let first = collection.family_id("Cousine").unwrap();
        let second = collection.family_id("Noto Sans").unwrap();
        collection.set_generic_families(GenericFamily::SansSerif, [first, second].into_iter());
        PlatformCatalog {
            collection: Some(Mutex::new(collection)),
            invalid: AtomicBool::new(false),
            #[cfg(windows)]
            character_fallback: OnceLock::from(None),
            family_queries: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    #[test]
    fn generic_queries_stop_after_the_first_accepted_family() {
        for accepted in ["Cousine", "Noto Sans"] {
            let catalog = test_catalog();
            let mut visited = Vec::new();
            let result = catalog
                .candidates(
                    &usvg::FontFamily::SansSerif,
                    Weight::NORMAL,
                    Stretch::Normal,
                    Style::Normal,
                    &OperationControl::new(),
                    |candidate| {
                        visited.push(candidate.family.clone());
                        Ok((candidate.family == accepted).then_some(candidate.family))
                    },
                )
                .unwrap();
            assert_eq!(result.as_deref(), Some(accepted));
            let expected = if accepted == "Cousine" {
                vec!["Cousine"]
            } else {
                vec!["Cousine", "Noto Sans"]
            };
            assert_eq!(visited, expected);
            assert_eq!(
                catalog.family_queries.load(Ordering::Relaxed),
                expected.len()
            );
        }
    }

    #[test]
    fn generic_query_stops_on_cancellation_or_admission_error() {
        for cancel in [false, true] {
            let catalog = test_catalog();
            let control = OperationControl::new();
            let result = catalog.candidates::<()>(
                &usvg::FontFamily::SansSerif,
                Weight::NORMAL,
                Stretch::Normal,
                Style::Normal,
                &control,
                |_| {
                    if cancel {
                        control.cancel();
                        Ok(None)
                    } else {
                        Err(NativeFontError::Limit {
                            resource: "system-font-bytes",
                            actual: 2,
                            limit: 1,
                        })
                    }
                },
            );
            assert!(if cancel {
                matches!(result, Err(NativeFontError::Operation(_)))
            } else {
                matches!(
                    result,
                    Err(NativeFontError::Limit {
                        resource: "system-font-bytes",
                        ..
                    })
                )
            });
            assert_eq!(catalog.family_queries.load(Ordering::Relaxed), 1);
        }
    }

    fn file_candidate(path: &Path, index: u32) -> FontCandidate {
        let font = FontInfo::from_source(
            fontique::SourceInfo::new(fontique::SourceId::new(), SourceKind::Path(path.into())),
            index,
        )
        .unwrap();
        PlatformCandidate {
            font,
            family: "Cousine".to_owned(),
        }
        .into_face()
        .into_file(path.to_owned())
    }

    #[test]
    fn non_covering_fallback_does_not_consume_snapshot_budget() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf");
        let registry =
            super::super::registry::FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let control = OperationControl::new();
        let limits = NativeFontLimits {
            max_bytes: 1,
            ..Default::default()
        };
        let mut database = Arc::new(Database::new());
        assert!(
            registry
                .materialize(
                    file_candidate(&path, 0),
                    Some('\u{0378}'),
                    &mut database,
                    &limits,
                    &control
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(registry.resident_bytes(), 0);
        assert!(database.is_empty());
        assert!(matches!(
            registry.materialize(
                file_candidate(&path, 0),
                Some('A'),
                &mut database,
                &limits,
                &control
            ),
            Err(NativeFontError::Limit {
                resource: "system-font-bytes",
                limit: 1,
                ..
            })
        ));
    }

    #[test]
    fn fallback_discovery_cannot_override_an_admitted_collection_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "merman-platform-snapshot-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let path = root.join("font.ttc");
        let original = include_bytes!("../../tests/fonts/Cousine-test-collection.ttc");
        fs::write(&path, original).unwrap();
        let candidate = file_candidate(&path, 1);
        let later_candidate = file_candidate(&root.join(".").join("font.ttc"), 1);
        #[cfg(target_os = "macos")]
        let font = match &candidate {
            FontCandidate::File {
                expected: ExpectedFace::Platform(face),
                ..
            } => face.coverage_source.clone().unwrap(),
            _ => unreachable!(),
        };
        let registry =
            super::super::registry::FontSourceRegistry::new(BTreeMap::new(), BTreeMap::new(), 0, 0);
        let control = OperationControl::new();
        let limits = NativeFontLimits::default();
        let mut database = Arc::new(Database::new());
        let selected = registry
            .materialize(candidate, Some('A'), &mut database, &limits, &control)
            .unwrap()
            .unwrap();
        fs::write(
            &path,
            include_bytes!("../../tests/fonts/NotoSansSC-subset.ttf"),
        )
        .unwrap();
        let repeated = registry
            .materialize(later_candidate, Some('A'), &mut database, &limits, &control)
            .unwrap()
            .unwrap();
        assert_eq!(selected, repeated);
        assert_eq!(
            database.face(selected).unwrap().post_script_name,
            "Cousine-Italic"
        );
        assert_eq!(database.face(selected).unwrap().index, 1);
        assert_eq!(registry.resident_bytes(), original.len());
        fs::remove_file(path).unwrap();
        #[cfg(target_os = "macos")]
        {
            let data = test_catalog()
                .discovery_data(&font, &registry)
                .unwrap()
                .unwrap();
            let raw = RawFace::parse(data.as_ref(), font.index()).unwrap();
            let names = name::Table::parse(raw.table(Tag::from_bytes(b"name")).unwrap()).unwrap();
            assert!(post_script_name_matches(names, "Cousine-Italic"));
            assert!(!post_script_name_matches(names, "Cousine-Regular"));
            assert_eq!(data.as_ref(), original);
        }
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn postscript_identity_accepts_macintosh_roman_ascii_records() {
        let mut bytes = Vec::new();
        for value in [0_u16, 1, 18, 1, 0, 0, name_id::POST_SCRIPT_NAME, 12, 0] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        bytes.extend_from_slice(b"LucidaGrande");
        let table = name::Table::parse(&bytes).unwrap();
        assert!(post_script_name_matches(table, "LucidaGrande"));
        assert!(!post_script_name_matches(table, "LucidaGrande-Bold"));
    }

    #[test]
    fn postscript_validation_uses_only_records_from_the_selected_collection_index() {
        let mut database = Database::new();
        database.load_font_data(
            include_bytes!("../../tests/fonts/Cousine-test-collection.ttc").to_vec(),
        );
        let mut face = database
            .faces()
            .find(|face| face.index == 1)
            .unwrap()
            .clone();
        let physical_name = std::mem::replace(&mut face.post_script_name, "AnotherRecord".into());
        assert!(matches_post_script_name(&face, &physical_name));
        assert!(!matches_post_script_name(&face, "Cousine-Regular"));
    }

    fn memory_font(bytes: Vec<u8>) -> FontInfo {
        let source = fontique::SourceInfo::new(
            fontique::SourceId::new(),
            SourceKind::Memory(fontique::Blob::from(bytes)),
        );
        FontInfo::from_source(source, 0).expect("fixture has platform-readable font metadata")
    }

    fn binary_face(bytes: Vec<u8>) -> FaceInfo {
        let mut database = Database::new();
        database.load_font_data(bytes);
        database
            .faces()
            .next()
            .expect("fixture has a font face")
            .clone()
    }

    fn fixture_with_family_aliases() -> Vec<u8> {
        let mut bytes = include_bytes!("../../tests/fonts/Cousine-Regular.ttf").to_vec();
        let table_count = usize::from(u16::from_be_bytes(bytes[4..6].try_into().unwrap()));
        let name_entry = bytes[12..]
            .as_chunks::<16>()
            .0
            .iter()
            .take(table_count)
            .position(|entry| &entry[..4] == b"name")
            .map(|index| 12 + index * 16)
            .expect("fixture has a name table");
        let name_offset = usize::try_from(u32::from_be_bytes(
            bytes[name_entry + 8..name_entry + 12].try_into().unwrap(),
        ))
        .unwrap();
        let count = usize::from(u16::from_be_bytes(
            bytes[name_offset + 2..name_offset + 4].try_into().unwrap(),
        ));
        let storage = name_offset
            + usize::from(u16::from_be_bytes(
                bytes[name_offset + 4..name_offset + 6].try_into().unwrap(),
            ));
        // Retag existing Unicode names without touching glyphs or font properties.
        // The full name and unique ID have enough storage for distinct family aliases.
        for index in 0..count {
            let record = name_offset + 6 + index * 12;
            let platform = u16::from_be_bytes(bytes[record..record + 2].try_into().unwrap());
            if platform != 3 {
                continue;
            }
            let id = u16::from_be_bytes(bytes[record + 6..record + 8].try_into().unwrap());
            let (new_id, family) = match id {
                4 => (
                    rustybuzz::ttf_parser::name_id::TYPOGRAPHIC_FAMILY,
                    "Cousine Text",
                ),
                3 => (rustybuzz::ttf_parser::name_id::WWS_FAMILY, "Cousine WWS"),
                _ => continue,
            };
            let previous_length = usize::from(u16::from_be_bytes(
                bytes[record + 8..record + 10].try_into().unwrap(),
            ));
            let offset = storage
                + usize::from(u16::from_be_bytes(
                    bytes[record + 10..record + 12].try_into().unwrap(),
                ));
            let encoded = family
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>();
            assert!(encoded.len() <= previous_length);
            bytes[record + 6..record + 8].copy_from_slice(&new_id.to_be_bytes());
            bytes[record + 8..record + 10]
                .copy_from_slice(&u16::try_from(encoded.len()).unwrap().to_be_bytes());
            bytes[offset..offset + encoded.len()].copy_from_slice(&encoded);
        }
        bytes
    }

    #[test]
    fn platform_candidate_rejects_another_family_with_identical_attributes() {
        let candidate = PlatformCandidate {
            font: memory_font(include_bytes!("../../tests/fonts/Cousine-Regular.ttf").to_vec()),
            family: "Cousine".to_owned(),
        };
        let replacement =
            binary_face(include_bytes!("../../tests/fonts/NotoSans-Latin-subset.ttf").to_vec());
        assert_eq!(replacement.index, candidate.font.index());
        assert_eq!(
            f32::from(replacement.weight.0),
            candidate.font.weight().value()
        );
        assert_eq!(width(replacement.stretch), candidate.font.width());
        assert_eq!(platform_style(replacement.style), candidate.font.style());
        assert!(
            candidate
                .into_face()
                .validate(
                    &replacement,
                    Path::new("unused-font-path"),
                    replacement.index
                )
                .is_err()
        );
    }

    #[test]
    fn platform_candidate_accepts_binary_legacy_typographic_and_wws_aliases() {
        let bytes = fixture_with_family_aliases();
        let face = binary_face(bytes.clone());
        assert!(
            face.families
                .iter()
                .all(|(family, _)| family == "Cousine Text")
        );
        let font = memory_font(bytes);
        for family in ["cOuSiNe", "COUSINE TEXT", "cousine wws"] {
            let candidate = PlatformCandidate {
                font: font.clone(),
                family: family.to_owned(),
            };
            candidate
                .into_face()
                .validate(&face, Path::new("unused-font-path"), face.index)
                .expect("family aliases come from the same immutable binary");
        }
    }

    fn poisoned_platform_context(
        symbol_fixture: &str,
        include_text: bool,
        limits: NativeFontLimits,
    ) -> NativeFontContext {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
        let mut collection = Collection::new(CollectionOptions {
            system_fonts: false,
            ..Default::default()
        });
        collection.load_fonts_from_paths([root.join(symbol_fixture)]);
        if include_text {
            collection.load_fonts_from_paths([root.join("Cousine-Regular.ttf")]);
        }
        let family = if symbol_fixture.contains("Symbolic") {
            "Merman Symbolic Test"
        } else {
            "Merman Pictorial Test"
        };
        let symbol = collection.family_by_name(family).unwrap().id();
        let mut preferred = vec![symbol];
        if include_text {
            preferred.push(collection.family_by_name("Cousine").unwrap().id());
        }
        for generic in [
            GenericFamily::Serif,
            GenericFamily::SansSerif,
            GenericFamily::Monospace,
            GenericFamily::Cursive,
            GenericFamily::Fantasy,
        ] {
            collection.set_generic_families(generic, preferred.iter().copied());
        }
        NativeFontContext(Arc::new(FontContextInner {
            database: Arc::new(Database::new()),
            selection: Arc::new(FontSelectionPolicy::default()),
            discovery: Some(super::super::resolver::DiscoveryProvider::Platform {
                catalog: Arc::new(PlatformCatalog {
                    collection: Some(Mutex::new(collection)),
                    invalid: AtomicBool::new(false),
                    #[cfg(windows)]
                    character_fallback: OnceLock::from(None),
                    family_queries: std::sync::atomic::AtomicUsize::new(0),
                }),
                explicit: Arc::new(Database::new()),
            }),
            sources: super::super::registry::FontSourceRegistry::new(
                BTreeMap::new(),
                BTreeMap::new(),
                0,
                0,
            ),
            default_family: String::new(),
            identity: "poisoned-platform-fixture".to_owned(),
            limits,
            control: OperationControl::new(),
            state: Mutex::new(MeasurementState::default()),
            fatal: OnceLock::new(),
        }))
    }

    #[test]
    fn platform_queries_retry_missing_coverage_without_negative_caching() {
        let context = poisoned_platform_context(
            "MermanSymbolicTest.ttf",
            true,
            NativeFontLimits {
                max_measurements: 2,
                ..Default::default()
            },
        );
        let style = TextStyle::default();
        for _ in 0..2 {
            assert!(
                context
                    .measure(HostTextMeasurementRequest {
                        operation: TextMeasurementOperation::TitleBBoxX,
                        phase: merman_render::environment::TextMeasurementPhase::SvgBBox,
                        text: "\u{10ffff}",
                        style: &style,
                        max_width: None,
                        wrap_mode: WrapMode::SvgLike,
                    })
                    .unwrap()
                    .is_none()
            );
        }
        let state = context.0.state.lock().unwrap();
        assert_eq!(state.measurements, 2);
        assert!(state.cache.is_empty());
        assert_eq!(state.cache_bytes, 0);
        drop(state);
        context.check().unwrap();
    }

    fn platform_painted_families(
        context: &NativeFontContext,
        family: Option<&str>,
    ) -> BTreeSet<String> {
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options(&mut options);
        let family =
            family.map_or_else(String::new, |family| format!(r#" font-family="{family}""#));
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="80"><text id="probe" x="0" y="40"{family}>amA</text></svg>"#
        );
        let tree = usvg::Tree::from_str(&source, &options).unwrap();
        let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
            return BTreeSet::new();
        };
        text.layouted()
            .iter()
            .flat_map(|span| &span.positioned_glyphs)
            .map(|glyph| {
                tree.fontdb()
                    .face(glyph.font)
                    .unwrap()
                    .post_script_name
                    .clone()
            })
            .collect()
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires the Windows Segoe UI Variable and Arial system fonts"]
    fn platform_named_variable_aliases_continue_the_declared_font_stack() {
        let control = OperationControl::new();
        let context = NativeFontOptions::default()
            .with_system_fonts(true)
            .load_platform(&control)
            .unwrap();
        let Some(super::super::resolver::DiscoveryProvider::Platform { catalog, .. }) =
            &context.0.discovery
        else {
            panic!("platform discovery");
        };
        for family in ["Segoe UI Variable Display", "Segoe UI Variable Text"] {
            let candidate = catalog
                .candidates(
                    &usvg::FontFamily::Named(family.to_owned()),
                    Weight::NORMAL,
                    Stretch::Normal,
                    Style::Normal,
                    &control,
                    |candidate| Ok(Some(candidate)),
                )
                .unwrap()
                .unwrap();
            assert!(!candidate.font.axes().is_empty(), "{family}");
            assert!(
                catalog
                    .candidate_file(candidate, &control)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                platform_painted_families(&context, Some(&format!("{family}, Arial"))),
                BTreeSet::from(["ArialMT".to_owned()]),
                "{family}"
            );
            context.check().unwrap();
        }
    }

    #[test]
    fn platform_family_selection_rejects_symbolic_generics_after_binary_admission() {
        for (fixture, named) in [
            ("MermanSymbolicTest.ttf", "Merman Symbolic Test"),
            ("MermanPictorialTest.ttf", "Merman Pictorial Test"),
        ] {
            let context = poisoned_platform_context(fixture, true, NativeFontLimits::default());
            for family in [
                Some("serif"),
                Some("sans-serif"),
                Some("monospace"),
                Some("__missing__"),
                None,
            ] {
                assert_eq!(
                    platform_painted_families(&context, family),
                    BTreeSet::from(["Cousine-Regular".to_owned()]),
                    "{fixture}, {family:?}"
                );
                context.check().unwrap();
            }
            assert_eq!(
                platform_painted_families(&context, Some(named)),
                BTreeSet::from([named.replace(' ', "")])
            );
            assert_eq!(context.0.sources.snapshots.lock().unwrap().files.len(), 2);
        }
    }

    #[test]
    fn platform_glyph_fallback_rejects_symbolic_faces_after_binary_admission() {
        for fixture in ["MermanSymbolicTest.ttf", "MermanPictorialTest.ttf"] {
            for include_text_fallback in [false, true] {
                let discovered = poisoned_platform_context(
                    fixture,
                    include_text_fallback,
                    NativeFontLimits::default(),
                );
                let mut context = NativeFontOptions::default()
                    .with_font_file(
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("tests/fonts/NotoSans-Latin-subset.ttf"),
                    )
                    .load(&OperationControl::new())
                    .unwrap();
                let Some(super::super::resolver::DiscoveryProvider::Platform { catalog, .. }) =
                    &discovered.0.discovery
                else {
                    panic!("platform discovery");
                };
                let inner = Arc::get_mut(&mut context.0).unwrap();
                inner.discovery = Some(super::super::resolver::DiscoveryProvider::Platform {
                    catalog: Arc::clone(catalog),
                    explicit: Arc::clone(&inner.database),
                });
                let expected = if include_text_fallback {
                    "Cousine-Regular"
                } else {
                    "NotoSans-Regular"
                };
                assert_eq!(
                    platform_painted_families(&context, Some("Noto Sans")),
                    BTreeSet::from([expected.to_owned()]),
                    "{fixture}, include_text_fallback={include_text_fallback}"
                );
                context.check().unwrap();
            }
        }
    }

    #[test]
    fn platform_symbol_only_generics_remain_unresolved_without_blocking_named_symbol() {
        let context =
            poisoned_platform_context("MermanSymbolicTest.ttf", false, NativeFontLimits::default());
        for family in [Some("serif"), Some("__missing__"), None] {
            assert!(
                platform_painted_families(&context, family).is_empty(),
                "{family:?}"
            );
            context.check().unwrap();
        }
        assert_eq!(
            platform_painted_families(&context, Some("Merman Symbolic Test")),
            BTreeSet::from(["MermanSymbolicTest".to_owned()])
        );
    }

    #[test]
    fn platform_symbol_rejection_cannot_bypass_snapshot_admission_limits() {
        let context = poisoned_platform_context(
            "MermanSymbolicTest.ttf",
            true,
            NativeFontLimits {
                max_files: 1,
                ..Default::default()
            },
        );
        assert!(platform_painted_families(&context, Some("serif")).is_empty());
        assert!(matches!(
            context.check(),
            Err(NativeFontError::Limit {
                resource: "system-snapshot-files",
                actual: 2,
                limit: 1
            })
        ));
    }

    #[test]
    fn cancellation_precedes_platform_initialization() {
        let control = OperationControl::new();
        control.cancel();
        assert!(matches!(
            PlatformCatalog::new(&control),
            Err(NativeFontError::Operation(_))
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires macOS Arial and its Core Text registry"]
    fn successful_requests_survive_provider_changes_and_export_but_reset_per_operation() {
        let context = NativeFontOptions::default()
            .with_system_fonts(true)
            .with_limits(NativeFontLimits {
                max_cache_entries: 0,
                ..Default::default()
            })
            .load_platform(&OperationControl::new())
            .unwrap();
        let style = TextStyle {
            font_family: Some("Arial".to_owned()),
            ..Default::default()
        };
        let measure = |context: &NativeFontContext, text: &str| {
            context.measure(HostTextMeasurementRequest {
                operation: TextMeasurementOperation::CanvasMeasureTextWidth,
                phase: merman_render::environment::TextMeasurementPhase::ComputedLength,
                text,
                style: &style,
                max_width: None,
                wrap_mode: WrapMode::SvgLike,
            })
        };
        assert!(measure(&context, "AVA").unwrap().is_some());
        assert!(measure(&context, "A中V").unwrap().is_some());
        let bytes = context.resident_font_bytes();
        let Some(super::super::resolver::DiscoveryProvider::Platform { catalog, .. }) =
            &context.0.discovery
        else {
            unreachable!()
        };
        // Make fresh native queries fail after successful admission. A pinned request
        // must still work, even when its new text cannot hit the measurement cache.
        catalog.invalid.store(true, Ordering::Release);
        assert!(measure(&context, "WWW").unwrap().is_some());
        assert!(measure(&context, "W中").unwrap().is_some());
        let export = context.for_export(&OperationControl::new()).unwrap();
        assert!(measure(&export, "中V").unwrap().is_some());
        assert_eq!(
            platform_painted_families(&export, Some("Arial")),
            BTreeSet::from(["ArialMT".to_owned()])
        );
        export.check().unwrap();
        assert_eq!(export.resident_font_bytes(), bytes);
        assert!(measure(&context.for_operation(&OperationControl::new()), "VVV").is_err());
    }

    #[test]
    fn width_and_style_preserve_requested_attributes() {
        assert_eq!(width(Stretch::Condensed), FontWidth::CONDENSED);
        assert_eq!(width(Stretch::ExtraExpanded), FontWidth::EXTRA_EXPANDED);
        assert_eq!(platform_style(Style::Italic), FontStyle::Italic);
        assert_eq!(platform_style(Style::Oblique), FontStyle::Oblique(None));
    }
}
