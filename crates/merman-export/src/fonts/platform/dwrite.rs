//! DirectWrite font identity and cluster fallback. usvg shapes the selected faces.

use super::{
    FaceAttributes, FontCandidate, FontStyle, FontWeight, FontWidth, GlyphFallback, PlatformFace,
    family_names_match,
};
use std::{
    ffi::{OsString, c_void},
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    ptr,
};
use ttf_parser::{fvar, name, name_id};
use usvg::fontdb::{FaceInfo, Style};
use windows::Win32::Foundation::E_POINTER;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_AXIS_VALUE, DWRITE_FONT_SIMULATIONS_BOLD,
    DWRITE_FONT_SIMULATIONS_OBLIQUE, DWRITE_FONT_STRETCH, DWRITE_FONT_STRETCH_CONDENSED,
    DWRITE_FONT_STRETCH_EXPANDED, DWRITE_FONT_STRETCH_EXTRA_CONDENSED,
    DWRITE_FONT_STRETCH_EXTRA_EXPANDED, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STRETCH_SEMI_CONDENSED, DWRITE_FONT_STRETCH_SEMI_EXPANDED,
    DWRITE_FONT_STRETCH_ULTRA_CONDENSED, DWRITE_FONT_STRETCH_ULTRA_EXPANDED,
    DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STYLE_OBLIQUE,
    DWRITE_FONT_WEIGHT, DWRITE_INFORMATIONAL_STRING_POSTSCRIPT_NAME, DWRITE_READING_DIRECTION,
    DWRITE_READING_DIRECTION_LEFT_TO_RIGHT, DWRITE_READING_DIRECTION_RIGHT_TO_LEFT,
    DWriteCreateFactory, IDWriteFactory, IDWriteFactory2, IDWriteFont, IDWriteFontCollection,
    IDWriteFontFace, IDWriteFontFace5, IDWriteFontFallback, IDWriteLocalFontFileLoader,
    IDWriteLocalizedStrings, IDWriteNumberSubstitution, IDWriteTextAnalysisSource,
    IDWriteTextAnalysisSource_Impl,
};
use windows_core::{Error, Interface, OutRef, PCWSTR, Result, implement, w};

pub(super) struct CharacterFallback {
    collection: IDWriteFontCollection,
    fallback: IDWriteFontFallback,
}

impl CharacterFallback {
    pub(super) fn new() -> Option<Self> {
        // SAFETY: The shared factory returns owned COM interfaces and the output is a valid slot.
        unsafe {
            let factory = DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED).ok()?;
            let mut collection = None;
            factory
                .GetSystemFontCollection(&mut collection, false)
                .ok()?;
            let fallback = factory
                .cast::<IDWriteFactory2>()
                .ok()?
                .GetSystemFontFallback()
                .ok()?;
            Some(Self {
                collection: collection?,
                fallback,
            })
        }
    }

    pub(super) fn is_variable_family_alias(&self, name: &str, path: &Path, index: u32) -> bool {
        let name_utf16: Vec<_> = name.encode_utf16().chain([0]).collect();
        let mut family_index = 0;
        let mut exists = Default::default();
        // SAFETY: The collection owns the returned COM objects and name_utf16 is NUL terminated.
        unsafe {
            if self
                .collection
                .FindFamilyName(
                    PCWSTR::from_raw(name_utf16.as_ptr()),
                    &mut family_index,
                    &mut exists,
                )
                .is_err()
                || !exists.as_bool()
            {
                return false;
            }
            let Ok(family) = self.collection.GetFontFamily(family_index) else {
                return false;
            };
            (0..family.GetFontCount())
                .filter_map(|i| family.GetFont(i).ok()?.CreateFontFace().ok())
                .any(|face| {
                    face.GetIndex() == index
                        && variable_face(&face).is_some()
                        && file_path(&face).as_deref() == Some(path)
                        && with_font_table(&face, b"name", |bytes| {
                            Some(!family_names_match(name::Table::parse(bytes)?, name))
                        }) == Some(true)
                })
        }
    }

    pub(super) fn lookup(
        &self,
        request: GlyphFallback<'_>,
        base: &FaceInfo,
    ) -> Option<FontCandidate> {
        let text: Vec<_> = request.text.encode_utf16().collect();
        let len = u32::try_from(text.len()).ok()?;
        let locale: Vec<_> = request
            .language
            .unwrap_or_default()
            .encode_utf16()
            .chain([0])
            .collect();
        let source: IDWriteTextAnalysisSource = TextSource {
            text,
            locale,
            rtl: request.rtl,
        }
        .into();
        let family = crate::font_selection::face_family_name(base)?;
        let family: Vec<_> = family.encode_utf16().chain([0]).collect();
        let style = match base.style {
            Style::Normal => DWRITE_FONT_STYLE_NORMAL,
            Style::Italic => DWRITE_FONT_STYLE_ITALIC,
            Style::Oblique => DWRITE_FONT_STYLE_OBLIQUE,
        };
        let mut mapped_len = 0;
        let mut mapped_font = None;
        let mut scale = 1.0;
        // SAFETY: The source owns its UTF-16 buffer, family is NUL terminated and all
        // output pointers remain valid until MapCharacters returns.
        unsafe {
            self.fallback
                .MapCharacters(
                    &source,
                    0,
                    len,
                    &self.collection,
                    PCWSTR::from_raw(family.as_ptr()),
                    DWRITE_FONT_WEIGHT(i32::from(base.weight.0)),
                    style,
                    DWRITE_FONT_STRETCH(i32::from(base.stretch.to_number())),
                    &mut mapped_len,
                    &mut mapped_font,
                    &mut scale,
                )
                .ok()?;
        }
        if mapped_len != len || !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        file_candidate(mapped_font?, scale)
    }
}

fn file_candidate(font: IDWriteFont, fallback_scale: f32) -> Option<FontCandidate> {
    // SAFETY: COM objects stay live through the calls and own their output strings.
    unsafe {
        let face = font.CreateFontFace().ok()?;
        let path = file_path(&face)?;
        let family = localized_name(&font.GetFontFamily().ok()?.GetFamilyNames().ok()?)?;
        let variable = variable_face(&face);
        let post_script_name = if let Some(instance) = &variable {
            if !uses_physical_axis_defaults(instance)? {
                // The resolver admits a file and collection index. These cannot carry the
                // selected instance's coordinates, even when it shares the physical family.
                return None;
            }
            // DirectWrite can name even a default instance differently from the file.
            Some(with_font_table(&face, b"name", |bytes| {
                let table = name::Table::parse(bytes)?;
                if !family_names_match(table, &family) {
                    return None;
                }
                table
                    .names
                    .into_iter()
                    .filter(|name| name.name_id == name_id::POST_SCRIPT_NAME)
                    .find_map(|name| name.to_string())
            })?)
        } else {
            post_script_name(&font)
        };
        let simulations = face.GetSimulations();
        // CSS properties can be varied or simulated independently of the physical face.
        let attributes = if variable.is_some() {
            FaceAttributes::default()
        } else {
            FaceAttributes {
                width: physical_width(font.GetStretch()),
                style: if simulations.0 & DWRITE_FONT_SIMULATIONS_OBLIQUE.0 == 0 {
                    match font.GetStyle() {
                        DWRITE_FONT_STYLE_NORMAL => Some(FontStyle::Normal),
                        DWRITE_FONT_STYLE_OBLIQUE => Some(FontStyle::Oblique(None)),
                        DWRITE_FONT_STYLE_ITALIC => Some(FontStyle::Italic),
                        _ => None,
                    }
                } else {
                    None
                },
                weight: (simulations.0 & DWRITE_FONT_SIMULATIONS_BOLD.0 == 0)
                    .then(|| FontWeight::new(font.GetWeight().0 as f32)),
            }
        };
        Some(
            PlatformFace {
                index: face.GetIndex(),
                family,
                post_script_name,
                attributes,
                coverage_source: None,
                fallback_scale,
            }
            .into_file(path),
        )
    }
}

fn file_path(face: &IDWriteFontFace) -> Option<PathBuf> {
    // SAFETY: The file owns the borrowed reference key and the path buffer includes a NUL slot.
    unsafe {
        let mut count = 0;
        face.GetFiles(&mut count, None).ok()?;
        if count != 1 {
            return None;
        }
        let mut file = None;
        face.GetFiles(&mut count, Some(&mut file)).ok()?;
        let file = file?;
        let loader = file
            .GetLoader()
            .ok()?
            .cast::<IDWriteLocalFontFileLoader>()
            .ok()?;
        let mut key = ptr::null_mut();
        let mut key_len = 0;
        file.GetReferenceKey(&mut key, &mut key_len).ok()?;
        let len = loader.GetFilePathLengthFromKey(key, key_len).ok()? as usize;
        let mut path = vec![0; len.checked_add(1)?];
        loader.GetFilePathFromKey(key, key_len, &mut path).ok()?;
        Some(PathBuf::from(OsString::from_wide(&path[..len])))
    }
}

fn variable_face(face: &IDWriteFontFace) -> Option<IDWriteFontFace5> {
    // SAFETY: HasVariations has no inputs beyond the live COM interface.
    unsafe {
        face.cast::<IDWriteFontFace5>()
            .ok()
            .filter(|face| face.HasVariations().as_bool())
    }
}

fn uses_physical_axis_defaults(instance: &IDWriteFontFace5) -> Option<bool> {
    with_font_table(instance, b"fvar", |bytes| {
        let table = fvar::Table::parse(bytes)?;
        // SAFETY: The slice has the exact number of writable slots reported by this face.
        let mut values = unsafe {
            let count = instance.GetFontAxisValueCount() as usize;
            let mut values = Vec::new();
            values.try_reserve_exact(count).ok()?;
            values.resize(count, DWRITE_FONT_AXIS_VALUE::default());
            instance.GetFontAxisValues(&mut values).ok()?;
            values
        };
        Some(axis_values_match_defaults(table, &mut values))
    })
}

fn axis_values_match_defaults(
    table: fvar::Table<'_>,
    values: &mut [DWRITE_FONT_AXIS_VALUE],
) -> bool {
    values.sort_unstable_by_key(|value| value.axisTag.0);
    // DirectWrite also reports static CSS axes. Only axes declared by the file need a match.
    table.axes.into_iter().all(|axis| {
        let tag = u32::from_le_bytes(axis.tag.to_bytes());
        values
            .binary_search_by_key(&tag, |value| value.axisTag.0)
            .is_ok_and(|index| values[index].value == axis.def_value)
    })
}

struct FontTableLease<'a> {
    face: &'a IDWriteFontFace,
    context: *mut c_void,
}

impl Drop for FontTableLease<'_> {
    fn drop(&mut self) {
        // SAFETY: This face returned the context for an existing table, possibly NULL.
        unsafe { self.face.ReleaseFontTable(self.context) };
    }
}

fn with_font_table<T>(
    face: &IDWriteFontFace,
    tag: &[u8; 4],
    read: impl FnOnce(&[u8]) -> Option<T>,
) -> Option<T> {
    let mut data = ptr::null_mut();
    let mut size = 0;
    let mut context = ptr::null_mut();
    let mut exists = Default::default();
    // SAFETY: The output slots are writable. DirectWrite owns `size` bytes until the lease
    // releases the table. The slice is scoped to the callback and cannot be returned.
    unsafe {
        face.TryGetFontTable(
            u32::from_le_bytes(*tag),
            &mut data,
            &mut size,
            &mut context,
            &mut exists,
        )
        .ok()?;
        if !exists.as_bool() {
            return None;
        }
        let _lease = FontTableLease { face, context };
        let size = size as usize;
        if data.is_null() || size > isize::MAX as usize {
            return None;
        }
        read(std::slice::from_raw_parts(data.cast::<u8>(), size))
    }
}

fn post_script_name(font: &IDWriteFont) -> Option<String> {
    // SAFETY: DirectWrite owns the returned names and out parameters are writable.
    unsafe {
        let mut postscripts = None;
        let mut exists = Default::default();
        font.GetInformationalStrings(
            DWRITE_INFORMATIONAL_STRING_POSTSCRIPT_NAME,
            &mut postscripts,
            &mut exists,
        )
        .ok()?;
        if !exists.as_bool() {
            return None;
        }
        localized_name(&postscripts?)
    }
}

fn localized_name(names: &IDWriteLocalizedStrings) -> Option<String> {
    // SAFETY: The COM object owns its strings and each output buffer includes the NUL.
    unsafe {
        let mut english = 0;
        let mut exists = Default::default();
        names
            .FindLocaleName(w!("en-us"), &mut english, &mut exists)
            .ok()?;
        let index = if exists.as_bool() { english } else { 0 };
        let len = names.GetStringLength(index).ok()? as usize;
        let mut name = vec![0; len.checked_add(1)?];
        names.GetString(index, &mut name).ok()?;
        String::from_utf16(&name[..len])
            .ok()
            .filter(|name| !name.is_empty())
    }
}

fn physical_width(stretch: DWRITE_FONT_STRETCH) -> Option<FontWidth> {
    match stretch {
        DWRITE_FONT_STRETCH_ULTRA_CONDENSED => Some(FontWidth::ULTRA_CONDENSED),
        DWRITE_FONT_STRETCH_EXTRA_CONDENSED => Some(FontWidth::EXTRA_CONDENSED),
        DWRITE_FONT_STRETCH_CONDENSED => Some(FontWidth::CONDENSED),
        DWRITE_FONT_STRETCH_SEMI_CONDENSED => Some(FontWidth::SEMI_CONDENSED),
        DWRITE_FONT_STRETCH_NORMAL => Some(FontWidth::NORMAL),
        DWRITE_FONT_STRETCH_SEMI_EXPANDED => Some(FontWidth::SEMI_EXPANDED),
        DWRITE_FONT_STRETCH_EXPANDED => Some(FontWidth::EXPANDED),
        DWRITE_FONT_STRETCH_EXTRA_EXPANDED => Some(FontWidth::EXTRA_EXPANDED),
        DWRITE_FONT_STRETCH_ULTRA_EXPANDED => Some(FontWidth::ULTRA_EXPANDED),
        _ => None,
    }
}

#[implement(IDWriteTextAnalysisSource)]
struct TextSource {
    text: Vec<u16>,
    locale: Vec<u16>,
    rtl: bool,
}

impl IDWriteTextAnalysisSource_Impl for TextSource_Impl {
    fn GetTextAtPosition(&self, position: u32, text: *mut *mut u16, len: *mut u32) -> Result<()> {
        report_text(
            self.text.get(position as usize..).unwrap_or_default(),
            text,
            len,
        )
    }

    fn GetTextBeforePosition(
        &self,
        position: u32,
        text: *mut *mut u16,
        len: *mut u32,
    ) -> Result<()> {
        report_text(
            self.text.get(..position as usize).unwrap_or_default(),
            text,
            len,
        )
    }

    fn GetParagraphReadingDirection(&self) -> DWRITE_READING_DIRECTION {
        if self.rtl {
            DWRITE_READING_DIRECTION_RIGHT_TO_LEFT
        } else {
            DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
        }
    }

    fn GetLocaleName(&self, position: u32, len: *mut u32, locale: *mut *mut u16) -> Result<()> {
        if len.is_null() || locale.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }
        // SAFETY: COM supplies writable output pointers. The source owns the NUL-terminated locale.
        unsafe {
            len.write((self.text.len() as u32).saturating_sub(position));
            locale.write(self.locale.as_ptr().cast_mut());
        }
        Ok(())
    }

    fn GetNumberSubstitution(
        &self,
        position: u32,
        len: *mut u32,
        substitution: OutRef<'_, IDWriteNumberSubstitution>,
    ) -> Result<()> {
        if len.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }
        substitution.write(None)?;
        // SAFETY: COM supplies a writable output pointer.
        unsafe { len.write((self.text.len() as u32).saturating_sub(position)) };
        Ok(())
    }
}

fn report_text(run: &[u16], text: *mut *mut u16, len: *mut u32) -> Result<()> {
    if text.is_null() || len.is_null() {
        return Err(Error::from_hresult(E_POINTER));
    }
    let start = if run.is_empty() {
        ptr::null_mut()
    } else {
        run.as_ptr().cast_mut()
    };
    // SAFETY: COM supplies writable output pointers. DirectWrite reads the run while
    // the analysis source is live and never writes through the returned text pointer.
    unsafe {
        text.write(start);
        len.write(run.len() as u32);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use usvg::fontdb::Database;

    fn han() -> GlyphFallback<'static> {
        GlyphFallback {
            text: "中",
            font: None,
            language: None,
            rtl: false,
        }
    }

    #[test]
    fn directwrite_requires_a_default_value_for_each_physical_axis() {
        use windows::Win32::Graphics::DirectWrite::{
            DWRITE_FONT_AXIS_TAG_WEIGHT, DWRITE_FONT_AXIS_TAG_WIDTH,
        };

        let bytes = include_bytes!("../../../tests/fonts/MermanVariableWeight.ttf");
        let table = ttf_parser::Face::parse(bytes, 0)
            .unwrap()
            .tables()
            .fvar
            .unwrap();
        let width = DWRITE_FONT_AXIS_VALUE {
            axisTag: DWRITE_FONT_AXIS_TAG_WIDTH,
            value: 100.0,
        };
        let weight = |value| DWRITE_FONT_AXIS_VALUE {
            axisTag: DWRITE_FONT_AXIS_TAG_WEIGHT,
            value,
        };
        assert!(axis_values_match_defaults(
            table,
            &mut [weight(100.0), width]
        ));
        assert!(!axis_values_match_defaults(table, &mut [width]));
        for value in [400.0, 1000.0, f32::NAN] {
            assert!(!axis_values_match_defaults(table, &mut [weight(value)]));
        }
    }

    #[test]
    fn directwrite_maps_fallback_to_the_selected_collection_member() {
        use windows::Win32::Graphics::DirectWrite::{DWRITE_UNICODE_RANGE, IDWriteFactory5};

        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-test-collection.ttc");
        // SAFETY: DirectWrite reads the fixture path and mapping arrays during each call.
        // The resulting collection and fallback own their font references.
        let fallback = unsafe {
            let factory =
                DWriteCreateFactory::<IDWriteFactory5>(DWRITE_FACTORY_TYPE_SHARED).unwrap();
            let path = windows_core::HSTRING::from(fixture.as_path());
            let file = factory.CreateFontFileReference(&path, None).unwrap();
            let fonts = factory.CreateFontSetBuilder().unwrap();
            fonts.AddFontFile(&file).unwrap();
            let collection: IDWriteFontCollection = factory
                .CreateFontCollectionFromFontSet(&fonts.CreateFontSet().unwrap())
                .unwrap()
                .into();
            let mappings = factory.CreateFontFallbackBuilder().unwrap();
            let range = DWRITE_UNICODE_RANGE {
                first: 'A' as u32,
                last: 'A' as u32,
            };
            mappings
                .AddMapping(
                    &[range],
                    &[w!("Cousine").as_ptr()],
                    &collection,
                    None,
                    None,
                    1.5,
                )
                .unwrap();
            CharacterFallback {
                collection,
                fallback: mappings.CreateFontFallback().unwrap(),
            }
        };

        let mut database = Database::new();
        database
            .load_font_data(include_bytes!("../../../tests/fonts/MermanProbeBase.ttf").to_vec());
        let mut base = database.faces().next().unwrap().clone();
        base.style = Style::Italic;
        let selection = fallback.lookup(GlyphFallback::from("AA"), &base).unwrap();
        let FontCandidate::File {
            path,
            index,
            expected: super::super::ExpectedFace::Platform(expected),
        } = selection
        else {
            panic!("the private fallback mapping must select a file");
        };
        assert_eq!(expected.fallback_scale, 1.5);
        assert_eq!(
            path.canonicalize().unwrap(),
            fixture.canonicalize().unwrap()
        );
        assert_eq!(index, 1);
        let mut physical = Database::new();
        physical.load_font_data(std::fs::read(&path).unwrap());
        let italic = physical.faces().find(|face| face.index == index).unwrap();
        assert_eq!(italic.post_script_name, "Cousine-Italic");
        expected.validate(italic, &path).unwrap();
        let mut wrong_index = italic.clone();
        wrong_index.index = 0;
        assert!(expected.validate(&wrong_index, &path).is_err());
    }

    #[test]
    #[ignore = "requires Windows Noto Sans SC with a physical weight of 100"]
    fn directwrite_variable_fallback_requires_physical_axis_defaults() {
        let mut database = Database::new();
        database
            .load_font_data(include_bytes!("../../../tests/fonts/NotoSansSC-subset.ttf").to_vec());
        let mut base = database.faces().next().unwrap().clone();
        let fallback = CharacterFallback::new().unwrap();
        for weight in [400, 700] {
            base.weight = usvg::fontdb::Weight(weight);
            assert!(fallback.lookup(han(), &base).is_none(), "weight {weight}");
        }

        base.weight = usvg::fontdb::Weight(100);
        let Some(FontCandidate::File {
            path,
            index,
            expected,
        }) = fallback.lookup(han(), &base)
        else {
            panic!("the default instance remains representable");
        };
        let mut physical = Database::new();
        physical.load_font_data(std::fs::read(&path).unwrap());
        let face = physical.faces().find(|face| face.index == index).unwrap();
        assert_eq!(face.post_script_name, "NotoSansSC-Thin");
        assert_eq!(face.weight, base.weight);
        expected.validate(face, &path).unwrap();
    }

    #[test]
    #[ignore = "requires the Windows Trebuchet MS and Microsoft YaHei UI font files"]
    fn directwrite_fallback_validates_the_physical_face_and_its_weight() {
        let directory = PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("Fonts");
        let load = |path| {
            let mut database = Database::new();
            database.load_font_data(std::fs::read(path).unwrap());
            database
        };
        let base = load(directory.join("trebuc.ttf"));
        let FontCandidate::File {
            path,
            index,
            expected,
        } = CharacterFallback::new()
            .unwrap()
            .lookup(han(), base.faces().next().unwrap())
            .unwrap()
        else {
            panic!("DirectWrite must provide a file-backed face");
        };
        let regular = load(path.clone());
        let regular = regular.faces().find(|face| face.index == index).unwrap();
        assert_eq!(regular.post_script_name, "MicrosoftYaHeiUI");
        expected.validate(regular, &path).unwrap();

        let bold = load(directory.join("msyhbd.ttc"));
        let bold = bold.faces().find(|face| face.index == index).unwrap();
        assert_eq!(bold.post_script_name, "MicrosoftYaHeiUI-Bold");
        assert!(expected.validate(bold, &path).is_err());

        let mut incorrect_weight = regular.clone();
        incorrect_weight.weight = bold.weight;
        assert!(expected.validate(&incorrect_weight, &path).is_err());

        let super::super::ExpectedFace::Platform(mut descriptor) = expected else {
            panic!("DirectWrite must provide a platform descriptor");
        };
        descriptor.post_script_name = Some(bold.post_script_name.clone());
        assert!(descriptor.validate(regular, &path).is_err());
    }
}
