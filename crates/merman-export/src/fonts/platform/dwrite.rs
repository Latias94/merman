//! Exact-character system fallback. DirectWrite returns a file descriptor; usvg shapes the text.

use super::{FaceAttributes, FontCandidate, FontStyle, FontWeight, FontWidth, PlatformFace};
use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf, ptr};
use usvg::fontdb::{FaceInfo, Style};
use windows::Win32::Foundation::E_POINTER;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_SIMULATIONS_BOLD, DWRITE_FONT_SIMULATIONS_OBLIQUE,
    DWRITE_FONT_STRETCH, DWRITE_FONT_STRETCH_CONDENSED, DWRITE_FONT_STRETCH_EXPANDED,
    DWRITE_FONT_STRETCH_EXTRA_CONDENSED, DWRITE_FONT_STRETCH_EXTRA_EXPANDED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STRETCH_SEMI_CONDENSED,
    DWRITE_FONT_STRETCH_SEMI_EXPANDED, DWRITE_FONT_STRETCH_ULTRA_CONDENSED,
    DWRITE_FONT_STRETCH_ULTRA_EXPANDED, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_STYLE_OBLIQUE, DWRITE_FONT_WEIGHT, DWRITE_INFORMATIONAL_STRING_POSTSCRIPT_NAME,
    DWRITE_READING_DIRECTION, DWRITE_READING_DIRECTION_LEFT_TO_RIGHT, DWriteCreateFactory,
    IDWriteFactory, IDWriteFactory2, IDWriteFont, IDWriteFontCollection, IDWriteFontFace5,
    IDWriteFontFallback, IDWriteLocalFontFileLoader, IDWriteLocalizedStrings,
    IDWriteNumberSubstitution, IDWriteTextAnalysisSource, IDWriteTextAnalysisSource_Impl,
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

    pub(super) fn lookup(&self, character: char, base: &FaceInfo) -> Option<FontCandidate> {
        let mut text = [0; 2];
        let len = character.encode_utf16(&mut text).len();
        let source: IDWriteTextAnalysisSource = ScalarSource { text, len }.into();
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
                    len as u32,
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
        (mapped_len as usize == len)
            .then_some(mapped_font)
            .flatten()
            .and_then(file_candidate)
    }
}

fn file_candidate(font: IDWriteFont) -> Option<FontCandidate> {
    // SAFETY: COM objects stay live through the calls, including the file owning the
    // borrowed reference key. Both buffers include the terminating UTF-16 NUL.
    unsafe {
        let face = font.CreateFontFace().ok()?;
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
        let path = PathBuf::from(OsString::from_wide(&path[..len]));

        let family = localized_name(&font.GetFontFamily().ok()?.GetFamilyNames().ok()?)?;
        let mut postscripts = None;
        let mut exists = Default::default();
        let post_script_name = font
            .GetInformationalStrings(
                DWRITE_INFORMATIONAL_STRING_POSTSCRIPT_NAME,
                &mut postscripts,
                &mut exists,
            )
            .ok()
            .filter(|_| exists.as_bool())
            .and_then(|_| localized_name(&postscripts?));
        let simulations = face.GetSimulations();
        let variable = face
            .cast::<IDWriteFontFace5>()
            .is_ok_and(|face| face.HasVariations().as_bool());
        // A simulated or variable instance can have different CSS attributes from its binary.
        // Check the physical name and any attributes reported without those adjustments.
        let attributes = if variable {
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
            }
            .into_file(path),
        )
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
struct ScalarSource {
    text: [u16; 2],
    len: usize,
}

impl IDWriteTextAnalysisSource_Impl for ScalarSource_Impl {
    fn GetTextAtPosition(&self, position: u32, text: *mut *mut u16, len: *mut u32) -> Result<()> {
        report_text(
            self.text[..self.len]
                .get(position as usize..)
                .unwrap_or_default(),
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
            self.text[..self.len]
                .get(..position as usize)
                .unwrap_or_default(),
            text,
            len,
        )
    }

    fn GetParagraphReadingDirection(&self) -> DWRITE_READING_DIRECTION {
        DWRITE_READING_DIRECTION_LEFT_TO_RIGHT
    }

    fn GetLocaleName(&self, position: u32, len: *mut u32, locale: *mut *mut u16) -> Result<()> {
        if len.is_null() || locale.is_null() {
            return Err(Error::from_hresult(E_POINTER));
        }
        // SAFETY: COM supplies writable output pointers. The neutral locale is static.
        unsafe {
            len.write((self.len as u32).saturating_sub(position));
            locale.write(w!("").as_ptr().cast_mut());
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
        unsafe { len.write((self.len as u32).saturating_sub(position)) };
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
            .lookup('中', base.faces().next().unwrap())
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

        let mut incorrect_name = regular.clone();
        incorrect_name.post_script_name = bold.post_script_name.clone();
        assert!(expected.validate(&incorrect_name, &path).is_err());
        let mut incorrect_weight = regular.clone();
        incorrect_weight.weight = bold.weight;
        assert!(expected.validate(&incorrect_weight, &path).is_err());
    }
}
