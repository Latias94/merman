//! Shared family and glyph selection over classification retained from admitted font bytes.

use std::collections::BTreeMap;
use usvg::fontdb::{Database, FaceInfo, Family, ID, Query, Stretch, Style, Weight};

#[derive(Clone, Copy, Default)]
enum FaceClass {
    LastResort,
    SymbolEncoding,
    Symbolic,
    Monospace,
    Serif,
    Sans,
    #[default]
    Unclassified,
}

impl FaceClass {
    fn allows_implicit_text(self) -> bool {
        !matches!(
            self,
            Self::SymbolEncoding | Self::Symbolic | Self::LastResort
        )
    }
}

#[derive(Clone, Default)]
pub(crate) struct FontSelectionPolicy {
    classes: BTreeMap<ID, FaceClass>,
}

impl FontSelectionPolicy {
    #[cfg(all(
        any(feature = "png", feature = "jpeg", feature = "pdf"),
        any(test, all(feature = "system-fonts", not(target_family = "wasm")))
    ))]
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
                || !fallback_style_matches(base, face)
            {
                return None;
            }
            database
                .with_face_data(face.id, |bytes, index| {
                    ttf_parser::Face::parse(bytes, index)
                        .ok()
                        .is_some_and(|face| covers_character(&face, character))
                })
                .unwrap_or(false)
                .then_some(face.id)
        })
    }
}

pub(crate) fn fallback_style_matches(base: &FaceInfo, face: &FaceInfo) -> bool {
    base.style == face.style || base.weight == face.weight || base.stretch == face.stretch
}

pub(crate) fn covers_character(face: &ttf_parser::Face<'_>, character: char) -> bool {
    face.tables()
        .cmap
        .is_some_and(|cmap| cmap_covers_character(cmap, face.number_of_glyphs(), character))
}

pub(crate) fn cmap_covers_character(
    cmap: ttf_parser::cmap::Table<'_>,
    num_glyphs: u16,
    character: char,
) -> bool {
    cmap.subtables.into_iter().any(|table| {
        table.is_unicode()
            && table
                .glyph_index(u32::from(character))
                .is_some_and(|glyph| glyph.0 != 0 && glyph.0 < num_glyphs)
    })
}

/// The same SFNT cmap subtable selection used by HarfRust.
#[cfg(all(
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
pub(crate) struct PlatformCmap<'a> {
    cmap: read_fonts::tables::cmap::Cmap<'a>,
    nominal: Option<(
        read_fonts::tables::cmap::EncodingRecord,
        read_fonts::tables::cmap::CmapSubtable<'a>,
    )>,
    glyph_count: u32,
    legacy_symbol: Option<harfrust::Font>,
}

#[cfg(all(
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
impl<'a> PlatformCmap<'a> {
    pub(crate) fn parse(data: &'a [u8], glyph_count: u16) -> Option<Self> {
        Self::parse_with_os2(data, None, glyph_count)
    }

    pub(crate) fn parse_with_os2(
        data: &'a [u8],
        os2: Option<&[u8]>,
        glyph_count: u16,
    ) -> Option<Self> {
        use read_fonts::{FontData, FontRead, tables::cmap::Cmap};

        let cmap = Cmap::read(FontData::new(data)).ok()?;
        let nominal = cmap
            .best_subtable()
            .map(|(_, record, table)| (record, table));
        let legacy_symbol = os2
            .filter(|os2| Self::is_legacy_arabic_os2(os2))
            .filter(|_| {
                nominal
                    .as_ref()
                    .is_some_and(|(record, _)| record.is_symbol())
            })
            .and_then(|os2| {
                use harfrust::font::{Blob, TableFunction};
                // Reuse HarfRust's legacy PUA maps, which depend on the OS/2
                // encoding page and are not part of the Unicode cmap.
                let cmap = Blob::from(data.to_vec());
                let os2 = Blob::from(os2.to_vec());
                let tables =
                    TableFunction::new(std::sync::Arc::new(move |tag| match &tag.to_be_bytes() {
                        b"cmap" => Some(cmap.clone()),
                        b"OS/2" => Some(os2.clone()),
                        _ => None,
                    }));
                harfrust::Font::new(tables, 0)
            });
        Some(Self {
            cmap,
            nominal,
            glyph_count: u32::from(glyph_count),
            legacy_symbol,
        })
    }

    fn is_legacy_arabic_os2(os2: &[u8]) -> bool {
        // OS/2 version 0 stores the legacy symbol encoding in fsSelection.
        os2.starts_with(&[0, 0]) && matches!(os2.get(62), Some(0xb2 | 0xb3))
    }

    pub(crate) fn is_symbol(&self) -> bool {
        self.nominal
            .as_ref()
            .is_some_and(|(record, _)| record.is_symbol())
    }

    pub(crate) fn may_use_arabic_symbol_page(ch: char) -> bool {
        matches!(ch, '\u{600}'..='\u{8ff}' | '\u{fb50}'..='\u{fdff}' | '\u{fe70}'..='\u{fefc}')
    }

    pub(crate) fn maps(&self, ch: char) -> bool {
        self.nominal_glyph(ch)
            .is_some_and(|glyph| self.valid_glyph(glyph))
    }

    fn nominal_glyph(&self, ch: char) -> Option<read_fonts::types::GlyphId> {
        let (record, table) = self.nominal.as_ref()?;
        let codepoint = if record.is_mac_roman() && !ch.is_ascii() {
            u32::from(read_fonts::tables::name::MacRomanMapping.encode(ch)?)
        } else {
            u32::from(ch)
        };
        let map = |codepoint| {
            table
                .map_codepoint(codepoint)
                .filter(|glyph| glyph.to_u32() != 0)
        };
        map(codepoint)
            .or_else(|| {
                if record.is_symbol() && codepoint <= 0xff {
                    map(0xf000 + codepoint)
                } else {
                    None
                }
            })
            .or_else(|| {
                self.legacy_symbol.as_ref().and_then(|font| {
                    harfrust::ShaperFont::new(font).default_nominal_glyph(codepoint)
                })
            })
    }

    fn directional_base(&self, ch: char, rtl: bool) -> char {
        // HarfRust mirrors before looking up variations, provided the mirror
        // has a nominal glyph in the preferred subtable.
        if rtl
            && let Some(mirror) = unicode_bidi_mirroring::get_mirrored(ch)
            && self.nominal_glyph(mirror).is_some()
        {
            return mirror;
        }
        ch
    }

    pub(crate) fn maps_directional(&self, ch: char, rtl: bool) -> bool {
        self.maps(self.directional_base(ch, rtl))
    }

    pub(crate) fn maps_variation(
        &self,
        rtl: bool,
        allow_default: bool,
    ) -> impl Fn((char, char)) -> bool + '_ {
        use read_fonts::tables::cmap::MapVariant;

        let table = self.cmap.uvs_subtable().map(|(_, subtable)| subtable);
        move |(base, selector)| {
            table.as_ref().is_some_and(|table| {
                let base = self.directional_base(base, rtl);
                match table.map_variant(base, selector) {
                    Some(MapVariant::Variant(glyph)) => self.valid_glyph(glyph),
                    Some(MapVariant::UseDefault) => allow_default && self.maps(base),
                    None => false,
                }
            })
        }
    }

    fn valid_glyph(&self, glyph: read_fonts::types::GlyphId) -> bool {
        (1..self.glyph_count).contains(&glyph.to_u32())
    }
}

#[cfg(test)]
#[test]
fn cmap_notdef_is_not_fallback_coverage() {
    let face =
        ttf_parser::Face::parse(include_bytes!("../tests/fonts/Cousine-Regular.ttf"), 0).unwrap();
    // The format 4 sentinel maps to glyph zero, which is not a real fallback glyph.
    assert_eq!(face.glyph_index('\u{ffff}'), Some(ttf_parser::GlyphId(0)));
    assert!(!covers_character(&face, '\u{ffff}'));
    assert!(covers_character(&face, 'A'));
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
    let table = |tag| face.raw_face().table(ttf_parser::Tag::from_bytes(tag));
    classify_tables(table(b"head"), table(b"OS/2"), face.tables().cmap)
}

fn classify_tables(
    head: Option<&[u8]>,
    os2: Option<&[u8]>,
    cmap: Option<ttf_parser::cmap::Table<'_>>,
) -> FaceClass {
    // OpenType head.flags bit 14 marks generic code-point-range placeholders,
    // whose nonzero cmap glyphs do not establish readable character coverage.
    // https://learn.microsoft.com/en-us/typography/opentype/spec/head
    if head.is_some_and(is_last_resort_head) {
        return FaceClass::LastResort;
    }
    if cmap.is_some_and(|cmap| {
        cmap.subtables.into_iter().any(|subtable| {
            subtable.platform_id == ttf_parser::PlatformId::Windows && subtable.encoding_id == 0
        }) && !cmap
            .subtables
            .into_iter()
            .any(|subtable| subtable.is_unicode())
    }) {
        return FaceClass::SymbolEncoding;
    }
    let Some(os2) = os2 else {
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

pub(crate) fn is_last_resort_head(head: &[u8]) -> bool {
    head.get(16).is_some_and(|flags| flags & 0x40 != 0)
}

/// Eligibility for an implicit platform match, before acquiring its outlines.
#[cfg(all(
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
pub(crate) fn allows_font_tables(
    head: &[u8],
    os2: Option<&[u8]>,
    cmap_data: &[u8],
    maxp: &[u8],
    fallback: Option<(&str, bool)>,
) -> bool {
    use ttf_parser::{cmap, maxp};
    use unicode_general_category::{GeneralCategory, get_general_category};

    let Some(cmap) = cmap::Table::parse(cmap_data) else {
        return false;
    };
    if is_last_resort_head(head) {
        return false;
    }
    if os2.is_some_and(PlatformCmap::is_legacy_arabic_os2)
        && fallback.is_some_and(|(text, rtl)| {
            let Some(glyphs) = maxp::Table::parse(maxp) else {
                return false;
            };
            PlatformCmap::parse_with_os2(cmap_data, os2, glyphs.number_of_glyphs.get()).is_some_and(
                |cmap| {
                    cmap.is_symbol()
                        && text.chars().any(|ch| {
                            PlatformCmap::may_use_arabic_symbol_page(ch)
                                && cmap.maps_directional(ch, rtl)
                        })
                },
            )
        })
    {
        return true;
    }
    if !cmap
        .subtables
        .into_iter()
        .any(|subtable| subtable.is_unicode())
    {
        return false;
    }
    match classify_tables(Some(head), os2, Some(cmap)) {
        FaceClass::LastResort | FaceClass::SymbolEncoding => false,
        FaceClass::Symbolic => fallback.is_some_and(|(text, rtl)| {
            let Some(maxp) = maxp::Table::parse(maxp) else {
                return false;
            };
            let Some(cmap) = PlatformCmap::parse(cmap_data, maxp.number_of_glyphs.get()) else {
                return false;
            };
            let maps_variant = cmap.maps_variation(rtl, false);
            let mut symbol = false;
            let keycap = text.ends_with('\u{20e3}');
            let mut chars = text.chars().peekable();
            while let Some(ch) = chars.next() {
                match get_general_category(ch) {
                    GeneralCategory::MathSymbol
                    | GeneralCategory::CurrencySymbol
                    | GeneralCategory::ModifierSymbol
                    | GeneralCategory::OtherSymbol => {}
                    _ if keycap && matches!(ch, '0'..='9' | '#' | '*') => {}
                    GeneralCategory::NonspacingMark
                    | GeneralCategory::SpacingMark
                    | GeneralCategory::EnclosingMark
                    | GeneralCategory::Format => continue,
                    _ => return false,
                }
                symbol |= cmap.maps(cmap.directional_base(ch, rtl))
                    || chars
                        .peek()
                        .is_some_and(|&selector| maps_variant((ch, selector)));
            }
            symbol
        }),
        _ => true,
    }
}

#[cfg(all(
    test,
    feature = "experimental-system-font-query",
    not(target_family = "wasm")
))]
#[test]
fn pictorial_face_is_eligible_for_its_unicode_dingbat_only() {
    use ttf_parser::{RawFace, Tag};

    let bytes = include_bytes!("../tests/fonts/MermanProbeFull.ttf");
    let face = RawFace::parse(bytes, 0).unwrap();
    let table = |tag| face.table(Tag::from_bytes(tag)).unwrap();
    let mut os2 = table(b"OS/2").to_vec();
    os2[30] = 12;
    let mut head = table(b"head").to_vec();
    let cmap = table(b"cmap");
    let maxp = table(b"maxp");
    for text in [None, Some(("f", false)), Some(("∑", false))] {
        assert!(!allows_font_tables(&head, Some(&os2), cmap, maxp, text));
    }
    assert!(allows_font_tables(
        &head,
        Some(&os2),
        cmap,
        maxp,
        Some(("✓", false))
    ));
    head[16] |= 0x40;
    assert!(!allows_font_tables(
        &head,
        Some(&os2),
        cmap,
        maxp,
        Some(("✓", false))
    ));
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
