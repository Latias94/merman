//! The Fontconfig ABI shared by Linux and FreeBSD, loaded through a single handle.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_double, c_int, c_long, c_uint, c_void};
use std::ptr::NonNull;
use std::sync::{Mutex, MutexGuard, OnceLock};

pub(super) enum FcConfig {}
pub(super) enum FcPattern {}
pub(super) enum FcCharSet {}
pub(super) enum FcStrList {}
pub(super) enum FtFace {}
enum FtLibrary {}

pub(super) type FcBool = c_int;
pub(super) type FcResult = c_uint;
pub(super) const FC_RESULT_MATCH: FcResult = 0;
pub(super) const FC_RESULT_NO_MATCH: FcResult = 1;
pub(super) const FC_MATCH_PATTERN: c_uint = 0;
pub(super) const FC_MATCH_SCAN: c_uint = 2;
pub(super) const FC_SET_SYSTEM: c_uint = 0;
pub(super) const FC_SET_APPLICATION: c_uint = 1;
pub(super) const FC_VALUE_BINDING_WEAK: c_uint = 0;

// The pointed-to members of the Fontconfig union all have the same layout.
#[repr(C)]
pub(super) struct FcValue {
    kind: c_int,
    value: FcValueData,
}

#[repr(C)]
union FcValueData {
    pointer: *const c_void,
    number: c_double,
    integer: c_int,
}

impl FcValue {
    pub(super) fn string(value: &std::ffi::CStr) -> Self {
        const FC_TYPE_STRING: c_int = 3;
        Self {
            kind: FC_TYPE_STRING,
            value: FcValueData {
                pointer: value.as_ptr().cast(),
            },
        }
    }
}

#[repr(C)]
pub(super) struct FcFontSet {
    pub(super) nfont: c_int,
    sfont: c_int,
    pub(super) fonts: *mut *mut FcPattern,
}

impl FcFontSet {
    pub(super) fn borrow(fonts: &mut [*mut FcPattern]) -> Option<Self> {
        let nfont = c_int::try_from(fonts.len()).ok()?;
        Some(Self {
            nfont,
            sfont: nfont,
            fonts: fonts.as_mut_ptr(),
        })
    }
}

macro_rules! api {
    ($(fn $name:ident($($arg:ty),*) $(-> $result:ty)?;)*) => {
        pub(super) struct Api {
            $(pub(super) $name: unsafe extern "C" fn($($arg),*) $(-> $result)?,)*
            needs_lock: bool,
            _library: libloading::Library,
        }

        impl Api {
            pub(super) fn get() -> Option<&'static Self> {
                static API: OnceLock<Option<Api>> = OnceLock::new();
                API.get_or_init(|| {
                    // SAFETY: Signatures and layouts follow fontconfig.h, including 2.13.1.
                    // The versioned SONAME is the same on Linux and FreeBSD. Keep the
                    // library loaded for the lifetime of every resolved function pointer.
                    unsafe {
                        let library = libloading::Library::new("libfontconfig.so.1").ok()?;
                        let version = library.get::<unsafe extern "C" fn() -> c_int>(b"FcGetVersion\0").ok()?;
                        Some(Self {
                            $($name: *library.get(concat!(stringify!($name), "\0").as_bytes()).ok()?,)*
                            needs_lock: version() < 21393,
                            _library: library,
                        })
                    }
                }).as_ref()
            }
        }
    };
}

api! {
    fn FcInitLoadConfigAndFonts() -> *mut FcConfig;
    fn FcConfigDestroy(*mut FcConfig);
    fn FcConfigUptoDate(*mut FcConfig) -> FcBool;
    fn FcConfigGetConfigDirs(*mut FcConfig) -> *mut FcStrList;
    fn FcConfigGetConfigFiles(*mut FcConfig) -> *mut FcStrList;
    fn FcConfigGetFontDirs(*mut FcConfig) -> *mut FcStrList;
    fn FcConfigGetFonts(*mut FcConfig, c_uint) -> *mut FcFontSet;
    fn FcConfigGetSysRoot(*const FcConfig) -> *const u8;
    fn FcStrListNext(*mut FcStrList) -> *mut u8;
    fn FcStrListDone(*mut FcStrList);
    fn FcConfigSubstitute(*mut FcConfig, *mut FcPattern, c_uint) -> FcBool;
    fn FcDefaultSubstitute(*mut FcPattern);
    fn FcFontMatch(*mut FcConfig, *mut FcPattern, *mut FcResult) -> *mut FcPattern;
    fn FcFontSort(*mut FcConfig, *mut FcPattern, FcBool, *mut *mut FcCharSet, *mut FcResult) -> *mut FcFontSet;
    fn FcFontSetSort(*mut FcConfig, *mut *mut FcFontSet, c_int, *mut FcPattern, FcBool, *mut *mut FcCharSet, *mut FcResult) -> *mut FcFontSet;
    fn FcFontSetSortDestroy(*mut FcFontSet);
    fn FcFontRenderPrepare(*mut FcConfig, *mut FcPattern, *mut FcPattern) -> *mut FcPattern;
    fn FcFreeTypeQueryFace(*const FtFace, *const u8, c_uint, *mut c_void) -> *mut FcPattern;
    fn FcPatternCreate() -> *mut FcPattern;
    fn FcPatternDuplicate(*const FcPattern) -> *mut FcPattern;
    fn FcPatternDestroy(*mut FcPattern);
    fn FcPatternDel(*mut FcPattern, *const c_char) -> FcBool;
    fn FcPatternAddString(*mut FcPattern, *const c_char, *const u8) -> FcBool;
    fn FcPatternAddWeak(*mut FcPattern, *const c_char, FcValue, FcBool) -> FcBool;
    fn FcPatternAddDouble(*mut FcPattern, *const c_char, c_double) -> FcBool;
    fn FcPatternAddInteger(*mut FcPattern, *const c_char, c_int) -> FcBool;
    fn FcPatternAddCharSet(*mut FcPattern, *const c_char, *const FcCharSet) -> FcBool;
    fn FcPatternGetString(*const FcPattern, *const c_char, c_int, *mut *mut u8) -> FcResult;
    fn FcPatternGetInteger(*const FcPattern, *const c_char, c_int, *mut c_int) -> FcResult;
    fn FcPatternGetBool(*const FcPattern, *const c_char, c_int, *mut FcBool) -> FcResult;
    fn FcPatternGetCharSet(*const FcPattern, *const c_char, c_int, *mut *mut FcCharSet) -> FcResult;
    fn FcPatternGetWithBinding(*const FcPattern, *const c_char, c_int, *mut FcValue, *mut c_uint) -> FcResult;
    fn FcCharSetCreate() -> *mut FcCharSet;
    fn FcCharSetDestroy(*mut FcCharSet);
    fn FcCharSetAddChar(*mut FcCharSet, u32) -> FcBool;
    fn FcCharSetHasChar(*const FcCharSet, u32) -> FcBool;
    fn FcCharSetSubtract(*const FcCharSet, *const FcCharSet) -> *mut FcCharSet;
    fn FcCharSetCount(*const FcCharSet) -> u32;
    fn FcWeightFromOpenTypeDouble(c_double) -> c_double;
}

/// A private FreeType library for scanning resident caller bytes.
pub(super) struct MemoryFaces {
    raw: NonNull<FtLibrary>,
    open_face:
        unsafe extern "C" fn(*mut FtLibrary, *const u8, c_long, c_long, *mut *mut FtFace) -> c_int,
    close_face: unsafe extern "C" fn(*mut FtFace) -> c_int,
    close_library: unsafe extern "C" fn(*mut FtLibrary) -> c_int,
}

impl MemoryFaces {
    pub(super) fn new(api: &Api) -> Option<Self> {
        // Resolve FreeType through Fontconfig's dependencies so a memory face
        // comes from the same implementation that will inspect it.
        let library = &api._library;
        // SAFETY: These signatures follow freetype.h. The API owns a live library.
        unsafe {
            let init = library
                .get::<unsafe extern "C" fn(*mut *mut FtLibrary) -> c_int>(b"FT_Init_FreeType\0")
                .ok()?;
            let open_face = *library.get(b"FT_New_Memory_Face\0").ok()?;
            let close_face = *library.get(b"FT_Done_Face\0").ok()?;
            let close_library = *library.get(b"FT_Done_FreeType\0").ok()?;
            let mut raw = std::ptr::null_mut();
            if init(&mut raw) != 0 {
                return None;
            }
            Some(Self {
                raw: NonNull::new(raw)?,
                open_face,
                close_face,
                close_library,
            })
        }
    }

    pub(super) fn query(&self, api: &Api, data: &[u8], index: u32) -> *mut FcPattern {
        let (Ok(len), Ok(face_index)) = (
            c_long::try_from(data.len()),
            c_long::try_from(u64::from(index)),
        ) else {
            return std::ptr::null_mut();
        };
        let mut face = std::ptr::null_mut();
        // SAFETY: FreeType borrows data until the face is closed below.
        unsafe {
            if (self.open_face)(self.raw.as_ptr(), data.as_ptr(), len, face_index, &mut face) != 0 {
                return std::ptr::null_mut();
            }
            // Fontconfig may open the path to identify a WOFF wrapper. The
            // caller already supplied SFNT bytes, so no caller path is needed.
            let pattern = (api.FcFreeTypeQueryFace)(
                face,
                c"/dev/null".as_ptr().cast(),
                index,
                std::ptr::null_mut(),
            );
            (self.close_face)(face);
            pattern
        }
    }
}

impl Drop for MemoryFaces {
    fn drop(&mut self) {
        // SAFETY: This owned handle has no outstanding faces.
        unsafe {
            (self.close_library)(self.raw.as_ptr());
        }
    }
}

// Match Skia's compatibility lock for Fontconfig releases with known races.
static LEGACY_ABI: Mutex<()> = Mutex::new(());

impl Api {
    pub(super) fn lock(&self) -> CallLock {
        CallLock(self.needs_lock.then(lock_legacy))
    }
}

fn lock_legacy() -> MutexGuard<'static, ()> {
    // Rust callbacks run outside this lock. A panic while holding it does not
    // leave shared Rust state under the lock to repair.
    LEGACY_ABI.lock().unwrap_or_else(|error| error.into_inner())
}

pub(super) struct CallLock(Option<MutexGuard<'static, ()>>);

impl CallLock {
    pub(super) fn outside<T>(&mut self, callback: impl FnOnce() -> T) -> T {
        struct Relock<'a>(&'a mut Option<MutexGuard<'static, ()>>);

        impl Drop for Relock<'_> {
            fn drop(&mut self) {
                *self.0 = Some(lock_legacy());
            }
        }

        if let Some(guard) = self.0.take() {
            drop(guard);
            // Restore the lock before native patterns are used or destroyed, even on unwind.
            let _relock = Relock(&mut self.0);
            callback()
        } else {
            callback()
        }
    }
}
