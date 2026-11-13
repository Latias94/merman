//! Shared family and glyph selection over classification retained from admitted font bytes.

use std::collections::BTreeMap;
use usvg::fontdb::{Database, FaceInfo, Family, ID, Query, Stretch, Style, Weight};

#[derive(Clone, Copy, Default)]
enum FaceClass {
    LastResort,
    Symbolic,
    Monospace,
    Serif,
    Sans,
    #[default]
    Unclassified,
}

impl FaceClass {
    fn allows_implicit_text(self) -> bool {
        !matches!(self, Self::Symbolic | Self::LastResort)
    }
}

#[derive(Clone, Default)]
pub(crate) struct FontSelectionPolicy {
    classes: BTreeMap<ID, FaceClass>,
}

impl FontSelectionPolicy {
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(crate) fn from_database(database: &Database) -> Self {
        let mut policy = Self::default();
        for face in database.faces() {
            policy.admit(face.id, database, face.id);
        }
        policy
    }

    /// Capture classification before bounded discovery releases the source bytes.
    pub(crate) fn admit(&mut self, id: ID, database: &Database, source_id: ID) {
        let class = database
            .with_face_data(source_id, classify)
            .unwrap_or_default();
        self.classes.insert(id, class);
    }

    pub(crate) fn allows_implicit(&self, id: ID) -> bool {
        self.classes
            .get(&id)
            .is_some_and(|class| class.allows_implicit_text())
    }

    pub(crate) fn configure_generics(&self, database: &mut Database) {
        for family in [
            Family::SansSerif,
            Family::Serif,
            Family::Monospace,
            Family::Cursive,
            Family::Fantasy,
        ] {
            let selected = self
                .query_generic(
                    database,
                    family,
                    Weight::NORMAL,
                    Stretch::Normal,
                    Style::Normal,
                )
                .and_then(|id| database.face(id))
                .and_then(face_family_name)
                .unwrap_or_else(|| "__merman_no_implicit_text_font__".to_owned());
            match family {
                Family::SansSerif => database.set_sans_serif_family(selected),
                Family::Serif => database.set_serif_family(selected),
                Family::Monospace => database.set_monospace_family(selected),
                Family::Cursive => database.set_cursive_family(selected),
                Family::Fantasy => database.set_fantasy_family(selected),
                Family::Name(_) => unreachable!(),
            }
        }
    }

    pub(crate) fn default_family(&self, database: &Database) -> Option<String> {
        [Family::SansSerif, Family::Serif]
            .into_iter()
            .find_map(|family| {
                self.query_generic(
                    database,
                    family,
                    Weight::NORMAL,
                    Stretch::Normal,
                    Style::Normal,
                )
            })
            .and_then(|id| database.face(id))
            .and_then(face_family_name)
    }

    fn query_generic(
        &self,
        database: &Database,
        family: Family<'_>,
        weight: Weight,
        stretch: Stretch,
        style: Style,
    ) -> Option<ID> {
        if let Some(id) = query_font_family(database, family, weight, stretch, style)
            .filter(|id| self.allows_implicit(*id))
        {
            return Some(id);
        }
        let compatible = |face: &FaceInfo| match family {
            Family::Serif => {
                !face.monospaced && matches!(self.classes.get(&face.id), Some(FaceClass::Serif))
            }
            Family::SansSerif => {
                !face.monospaced && matches!(self.classes.get(&face.id), Some(FaceClass::Sans))
            }
            Family::Monospace => {
                face.monospaced || matches!(self.classes.get(&face.id), Some(FaceClass::Monospace))
            }
            _ => true,
        };
        let face = database
            .faces()
            .find(|face| self.allows_implicit(face.id) && compatible(face))
            .or_else(|| {
                if matches!(family, Family::Serif | Family::SansSerif) {
                    database.faces().find(|face| {
                        self.allows_implicit(face.id)
                            && !face.monospaced
                            && matches!(self.classes.get(&face.id), Some(FaceClass::Unclassified))
                    })
                } else {
                    None
                }
            })
            .or_else(|| database.faces().find(|face| self.allows_implicit(face.id)))?;
        face.families
            .first()
            .and_then(|(name, _)| {
                query_font_family(database, Family::Name(name), weight, stretch, style)
                    .filter(|id| self.allows_implicit(*id))
            })
            .or(Some(face.id))
    }

    pub(crate) fn requested(&self, font: &usvg::Font, database: &Database) -> Option<ID> {
        let weight = Weight(font.weight());
        let stretch = font.stretch().into();
        let style = font.style().into();
        font.families().iter().find_map(|family| match family {
            usvg::FontFamily::Named(name) => {
                query_named_font_family_case_insensitively(database, name, weight, stretch, style)
            }
            usvg::FontFamily::Serif => {
                self.query_generic(database, Family::Serif, weight, stretch, style)
            }
            usvg::FontFamily::SansSerif => {
                self.query_generic(database, Family::SansSerif, weight, stretch, style)
            }
            usvg::FontFamily::Cursive => {
                self.query_generic(database, Family::Cursive, weight, stretch, style)
            }
            usvg::FontFamily::Fantasy => {
                self.query_generic(database, Family::Fantasy, weight, stretch, style)
            }
            usvg::FontFamily::Monospace => {
                self.query_generic(database, Family::Monospace, weight, stretch, style)
            }
        })
    }

    pub(crate) fn fallback(&self, font: &usvg::Font, database: &Database) -> Option<ID> {
        let weight = Weight(font.weight());
        let stretch = font.stretch().into();
        let style = font.style().into();
        // Preserve usvg's serif fallback after exhausting the authored family stack.
        self.query_generic(database, Family::Serif, weight, stretch, style)
    }

    pub(crate) fn glyph_fallback(
        &self,
        character: char,
        excluded: &[ID],
        database: &Database,
    ) -> Option<ID> {
        let base = database.face(*excluded.first()?)?;
        database.faces().find_map(|face| {
            // Preserve usvg's ordering and style matching, with the same text-suitability
            // gate as implicit family selection. Named symbol requests bypass this path.
            if excluded.contains(&face.id)
                || !self.allows_implicit(face.id)
                || (base.style != face.style
                    && base.weight != face.weight
                    && base.stretch != face.stretch)
            {
                return None;
            }
            database
                .with_face_data(face.id, |bytes, index| {
                    ttf_parser::Face::parse(bytes, index)
                        .ok()
                        .is_some_and(|face| face.glyph_index(character).is_some())
                })
                .unwrap_or(false)
                .then_some(face.id)
        })
    }
}

/// Classify only bytes already admitted through the native source registry.
/// Platform family ranking remains owned by Fontique; this gate owns text suitability.
#[cfg(all(
    feature = "native-fonts",
    feature = "system-fonts",
    not(target_family = "wasm")
))]
pub(crate) fn allows_implicit_binary_face(face: &FaceInfo) -> bool {
    let usvg::fontdb::Source::Binary(bytes) = &face.source else {
        return false;
    };
    !ttf_parser::Face::parse(bytes.as_ref().as_ref(), face.index)
        .ok()
        .is_some_and(|face| !classify_family(&face).allows_implicit_text())
}

fn classify(bytes: &[u8], index: u32) -> FaceClass {
    let Ok(face) = ttf_parser::Face::parse(bytes, index) else {
        return FaceClass::Unclassified;
    };
    let class = classify_family(&face);
    if class.allows_implicit_text() && !face.is_monospaced() && has_fixed_latin_advances(&face) {
        FaceClass::Monospace
    } else {
        class
    }
}

fn has_fixed_latin_advances(face: &ttf_parser::Face<'_>) -> bool {
    let mut expected = None;
    for character in b' '..=b'~' {
        let Some(advance) = face
            .glyph_index(char::from(character))
            .filter(|glyph| glyph.0 != 0)
            .and_then(|glyph| face.glyph_hor_advance(glyph))
            .filter(|advance| *advance != 0)
        else {
            return false;
        };
        if expected.is_some_and(|expected| expected != advance) {
            return false;
        }
        expected = Some(advance);
    }
    true
}

fn classify_family(face: &ttf_parser::Face<'_>) -> FaceClass {
    // OpenType head.flags bit 14 marks generic code-point-range placeholders,
    // whose nonzero cmap glyphs do not establish readable character coverage.
    // https://learn.microsoft.com/en-us/typography/opentype/spec/head
    if face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"head"))
        .and_then(|head| head.get(16..18))
        .is_some_and(|flags| u16::from_be_bytes([flags[0], flags[1]]) & (1 << 14) != 0)
    {
        return FaceClass::LastResort;
    }
    if face.tables().cmap.is_some_and(|cmap| {
        cmap.subtables.into_iter().any(|subtable| {
            subtable.platform_id == ttf_parser::PlatformId::Windows && subtable.encoding_id == 0
        })
    }) {
        return FaceClass::Symbolic;
    }
    let Some(os2) = face.raw_face().table(ttf_parser::Tag::from_bytes(b"OS/2")) else {
        return FaceClass::Unclassified;
    };
    // OpenType OS/2 sFamilyClass and PANOSE bFamilyType/bSerifStyle have fixed offsets.
    let family = os2.get(30).copied().unwrap_or_default();
    let panose = os2.get(32).copied().unwrap_or_default();
    if family == 12 || panose == 5 {
        return FaceClass::Symbolic;
    }
    let serif = os2.get(33).copied().unwrap_or_default();
    if (1..=7).contains(&family) || (panose == 2 && (2..=10).contains(&serif)) {
        FaceClass::Serif
    } else if family == 8 || (panose == 2 && (11..=15).contains(&serif)) {
        FaceClass::Sans
    } else {
        FaceClass::Unclassified
    }
}

pub(crate) fn query_named_font_family_case_insensitively(
    database: &Database,
    name: &str,
    weight: Weight,
    stretch: Stretch,
    style: Style,
) -> Option<ID> {
    // usvg represents an unresolved default as an empty Named family.
    if name.is_empty() {
        return None;
    }
    query_font_family(database, Family::Name(name), weight, stretch, style).or_else(|| {
        let canonical = database
            .faces()
            .flat_map(|face| &face.families)
            .map(|(name, _)| name)
            .find(|family| unicase::eq(family.as_str(), name))?;
        query_font_family(database, Family::Name(canonical), weight, stretch, style)
    })
}

pub(crate) fn query_font_family(
    database: &Database,
    family: Family<'_>,
    weight: Weight,
    stretch: Stretch,
    style: Style,
) -> Option<ID> {
    database.query(&Query {
        families: &[family],
        weight,
        stretch,
        style,
    })
}

pub(crate) fn face_family_name(face: &FaceInfo) -> Option<String> {
    face.families
        .iter()
        .find(|(_, language)| *language == usvg::fontdb::Language::English_UnitedStates)
        .or_else(|| face.families.first())
        .map(|(family, _)| family.clone())
}
