//! Shared family selection over classification retained from admitted font bytes.

use std::collections::BTreeMap;
use usvg::fontdb::{Database, FaceInfo, Family, ID, Query, Stretch, Style, Weight};

#[derive(Clone, Copy, Default)]
enum FaceClass {
    Symbolic,
    Serif,
    Sans,
    #[default]
    Unclassified,
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

    fn allows(&self, id: ID) -> bool {
        self.classes
            .get(&id)
            .is_some_and(|class| !matches!(class, FaceClass::Symbolic))
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
            .filter(|id| self.allows(*id))
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
            Family::Monospace => face.monospaced,
            _ => true,
        };
        let face = database
            .faces()
            .find(|face| self.allows(face.id) && compatible(face))
            .or_else(|| database.faces().find(|face| self.allows(face.id)))?;
        face.families
            .first()
            .and_then(|(name, _)| {
                query_font_family(database, Family::Name(name), weight, stretch, style)
                    .filter(|id| self.allows(*id))
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
}

/// Classify only bytes already admitted through the native source registry.
/// Platform family ranking remains owned by Fontique; this gate owns text suitability.
#[cfg(all(feature = "native-fonts", feature = "system-fonts"))]
pub(crate) fn allows_implicit_binary_face(face: &FaceInfo) -> bool {
    let usvg::fontdb::Source::Binary(bytes) = &face.source else {
        return false;
    };
    !matches!(
        classify(bytes.as_ref().as_ref(), face.index),
        FaceClass::Symbolic
    )
}

fn classify(bytes: &[u8], index: u32) -> FaceClass {
    let Ok(face) = ttf_parser::Face::parse(bytes, index) else {
        return FaceClass::Unclassified;
    };
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
