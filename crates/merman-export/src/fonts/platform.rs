//! Platform-managed discovery with bounded, operation-local binary snapshots.

use super::resolver::{ExpectedFace, FontCandidate};
use super::*;
use fontique::{
    Collection, CollectionOptions, FallbackKey, FontInfo, FontStyle, FontWeight, FontWidth,
    GenericFamily, SourceKind,
};
use unicode_script::UnicodeScript;
use usvg::fontdb::{Database, FaceInfo, Family, Source, Stretch, Style, Weight};

pub(super) struct PlatformCandidate {
    pub(super) font: FontInfo,
    pub(super) family: String,
}

pub(super) struct PlatformCatalog {
    collection: Mutex<Collection>,
    invalid: std::sync::atomic::AtomicBool,
}

impl PlatformCatalog {
    pub fn new(control: &OperationControl) -> Result<Self, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        // Native discovery may map candidate files internally. This is an explicitly
        // authorized platform policy, not the bounded metadata discovery policy.
        let collection = platform_call(|| Collection::new(CollectionOptions::default()))?;
        control.terminal_checkpoint_at(OperationPhase::Admission)?;
        Ok(Self {
            collection: Mutex::new(collection),
            invalid: std::sync::atomic::AtomicBool::new(false),
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

    pub fn default_path(&self, control: &OperationControl) -> Result<PathBuf, NativeFontError> {
        for generic in [GenericFamily::SansSerif, GenericFamily::Serif] {
            let candidates = self.candidates(
                None,
                Some(generic),
                Weight::NORMAL,
                Stretch::Normal,
                Style::Normal,
                control,
            )?;
            if let Some(path) =
                candidates
                    .into_iter()
                    .find_map(|candidate| match candidate.font.source().kind() {
                        SourceKind::Path(path) => Some(path.as_ref().to_owned()),
                        SourceKind::Memory(_) => None,
                    })
            {
                return Ok(path);
            }
        }
        Err(NativeFontError::EmptyFontSet)
    }

    fn candidates(
        &self,
        name: Option<&str>,
        generic: Option<GenericFamily>,
        weight: Weight,
        stretch: Stretch,
        style: Style,
        control: &OperationControl,
    ) -> Result<Vec<PlatformCandidate>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        if name == Some("") {
            return Ok(Vec::new());
        }
        let mut collection = self.collection.lock().map_err(|_| {
            NativeFontError::Measurement("platform font collection is poisoned".to_owned())
        })?;
        let families = if let Some(name) = name {
            let family = self.call(|| collection.family_by_name(name))?;
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            family.into_iter().collect::<Vec<_>>()
        } else if let Some(generic) = generic {
            let ids = self.call(|| collection.generic_families(generic).collect::<Vec<_>>())?;
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            let mut families = Vec::new();
            for id in ids {
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
                if let Some(family) = self.call(|| collection.family(id))? {
                    families.push(family);
                }
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
            }
            families
        } else {
            Vec::new()
        };
        let result = families
            .into_iter()
            .filter_map(|family| {
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
            })
            .collect();
        drop(collection);
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        Ok(result)
    }

    pub(super) fn fallback_candidates(
        &self,
        character: char,
        base: &FaceInfo,
        control: &OperationControl,
    ) -> Result<Vec<PlatformCandidate>, NativeFontError> {
        control.terminal_checkpoint_at(OperationPhase::Layout)?;
        let mut collection = self.collection.lock().map_err(|_| {
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
        let mut candidates = Vec::new();
        for id in ids {
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
            let family = self.call(|| collection.family(id))?;
            if let Some(family) = family {
                let font = family.match_font(
                    width(base.stretch),
                    platform_style(base.style),
                    FontWeight::new(f32::from(base.weight.0)),
                    false,
                );
                if let Some(font) = font {
                    candidates.push(PlatformCandidate {
                        font: font.clone(),
                        family: family.name().to_owned(),
                    });
                }
            }
            control.terminal_checkpoint_at(OperationPhase::Layout)?;
        }
        Ok(candidates)
    }
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

pub(super) fn validate_candidate(
    face: &FaceInfo,
    candidate: &PlatformCandidate,
    path: &Path,
) -> Result<(), NativeFontError> {
    let style_matches = matches!(
        (face.style, candidate.font.style()),
        (Style::Normal, FontStyle::Normal)
            | (Style::Italic, FontStyle::Italic)
            | (Style::Oblique, FontStyle::Oblique(_))
    );
    if face.index != candidate.font.index()
        || f32::from(face.weight.0) != candidate.font.weight().value()
        || width(face.stretch) != candidate.font.width()
        || !style_matches
        || !matches_family(face, &candidate.family)
    {
        return Err(source_error(
            path,
            "font properties changed after platform lookup; retry the operation",
        ));
    }
    Ok(())
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
    let Source::Binary(data) = &face.source else {
        return false;
    };
    let Ok(parsed) = rustybuzz::ttf_parser::Face::parse(data.as_ref().as_ref(), face.index) else {
        return false;
    };
    parsed.names().into_iter().any(|name| {
        matches!(
            name.name_id,
            rustybuzz::ttf_parser::name_id::FAMILY
                | rustybuzz::ttf_parser::name_id::TYPOGRAPHIC_FAMILY
                | rustybuzz::ttf_parser::name_id::WWS_FAMILY
        ) && name
            .to_string()
            .is_some_and(|family| unicase::eq(family.as_str(), expected))
    })
}

pub(super) fn candidate(candidate: PlatformCandidate) -> Result<FontCandidate, NativeFontError> {
    let SourceKind::Path(path) = candidate.font.source().kind() else {
        return Err(NativeFontError::Measurement(
            "platform candidate has no snapshot-compatible file path".to_owned(),
        ));
    };
    Ok(FontCandidate::File {
        path: path.as_ref().to_owned(),
        index: candidate.font.index(),
        expected: ExpectedFace::Platform(candidate),
    })
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
            let name = match family {
                usvg::FontFamily::Named(name) => Some(name.as_str()),
                _ => None,
            };
            for selected in self.candidates(
                name,
                generic_family(family),
                weight,
                stretch,
                style,
                control,
            )? {
                control.terminal_checkpoint_at(OperationPhase::Layout)?;
                if let Some(id) = accept(
                    super::resolver::FontSelection {
                        candidate: candidate(selected)?,
                        family_fallback_used,
                    },
                    implicit,
                )? {
                    return Ok(Some(id));
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .chunks_exact(16)
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
            validate_candidate(&replacement, &candidate, Path::new("unused-font-path")).is_err()
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
            validate_candidate(&face, &candidate, Path::new("unused-font-path"))
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
            initial_system_faces: BTreeSet::new(),
            discovery: Some(super::super::resolver::DiscoveryProvider::Platform {
                catalog: Arc::new(PlatformCatalog {
                    collection: Mutex::new(collection),
                    invalid: AtomicBool::new(false),
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

    #[test]
    fn width_and_style_preserve_requested_attributes() {
        assert_eq!(width(Stretch::Condensed), FontWidth::CONDENSED);
        assert_eq!(width(Stretch::ExtraExpanded), FontWidth::EXTRA_EXPANDED);
        assert_eq!(platform_style(Style::Italic), FontStyle::Italic);
        assert_eq!(platform_style(Style::Oblique), FontStyle::Oblique(None));
    }
}
