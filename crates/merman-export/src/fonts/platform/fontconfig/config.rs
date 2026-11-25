use std::ffi::{CStr, OsStr};
use std::io::ErrorKind;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use super::ffi::{Api, FcStrList};
use super::{Catalog, NativeFontError, failed};

// FcConfigUptoDate also writes the last rescan timestamp. Serialize it and
// initialization even on ABIs that support concurrent matching.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

impl Catalog {
    pub(in super::super) fn new() -> Result<Self, NativeFontError> {
        let api = Api::get().ok_or(NativeFontError::PlatformDiscoveryUnavailable {
            service: "Fontconfig",
        })?;
        let _serial = CONFIG_LOCK
            .lock()
            .map_err(|_| failed("locking configuration initialization"))?;
        Self::load(api)
    }

    pub(in super::super) fn refreshed(self: &Arc<Self>) -> Result<Arc<Self>, NativeFontError> {
        let _serial = CONFIG_LOCK
            .lock()
            .map_err(|_| failed("locking configuration refresh"))?;
        if self.paths.iter().all(Watch::unchanged) {
            let _guard = self.api.lock();
            // SAFETY: This config is live, and the rescan time has one writer.
            if unsafe { (self.api.FcConfigUptoDate)(self.config.as_ptr()) } != 0 {
                return Ok(Arc::clone(self));
            }
        }
        Self::load(self.api).map(Arc::new)
    }

    fn load(api: &'static Api) -> Result<Self, NativeFontError> {
        let config = {
            let _guard = api.lock();
            // SAFETY: This returns an owned configuration without setting the current one.
            NonNull::new(unsafe { (api.FcInitLoadConfigAndFonts)() })
                .ok_or_else(|| failed("initializing the catalog"))?
        };
        let mut catalog = Self {
            api,
            config,
            sysroot: None,
            paths: Vec::new(),
        };
        let mut paths = Vec::new();
        {
            let _guard = api.lock();
            // SAFETY: A non-null root is borrowed from the live configuration or process environment.
            let sysroot = unsafe { (api.FcConfigGetSysRoot)(config.as_ptr()) };
            if !sysroot.is_null() {
                let root = unsafe { CStr::from_ptr(sysroot.cast()) };
                catalog.sysroot = Some(PathBuf::from(OsStr::from_bytes(root.to_bytes())));
            }
            for list in [
                api.FcConfigGetConfigFiles,
                api.FcConfigGetConfigDirs,
                api.FcConfigGetFontDirs,
            ] {
                // SAFETY: The list and the strings it visits belong to this configuration.
                unsafe { catalog.collect_paths(list(config.as_ptr()), &mut paths)? };
            }
        }
        paths.sort_unstable();
        paths.dedup();
        catalog.paths = paths.into_iter().map(Watch::new).collect();
        Ok(catalog)
    }

    unsafe fn collect_paths(
        &self,
        list: *mut FcStrList,
        paths: &mut Vec<PathBuf>,
    ) -> Result<(), NativeFontError> {
        let list = NonNull::new(list).ok_or_else(|| failed("reading tracked paths"))?;
        // SAFETY: Every non-null string stays live until this iterator is destroyed.
        unsafe {
            loop {
                let raw = (self.api.FcStrListNext)(list.as_ptr());
                if raw.is_null() {
                    break;
                }
                let path = Path::new(OsStr::from_bytes(CStr::from_ptr(raw.cast()).to_bytes()));
                paths.push(self.resolve_path(path));
            }
            (self.api.FcStrListDone)(list.as_ptr());
        }
        Ok(())
    }

    pub(super) fn resolve_path(&self, path: &Path) -> PathBuf {
        if let Some(root) = &self.sysroot
            && !path.starts_with(root)
        {
            return root.join(path.strip_prefix("/").unwrap_or(path));
        }
        path.to_owned()
    }
}

impl Drop for Catalog {
    fn drop(&mut self) {
        let _guard = self.api.lock();
        // SAFETY: This drops the reference returned by FcInitLoadConfigAndFonts.
        unsafe { (self.api.FcConfigDestroy)(self.config.as_ptr()) };
    }
}

pub(super) struct Watch {
    path: PathBuf,
    stamp: Option<Stamp>,
}

#[derive(PartialEq, Eq)]
enum Stamp {
    Missing,
    Present {
        device: u64,
        inode: u64,
        length: u64,
        modified: (i64, i64),
        changed: (i64, i64),
    },
}

impl Watch {
    fn new(path: PathBuf) -> Self {
        let stamp = Self::stamp(&path);
        Self { path, stamp }
    }

    fn unchanged(&self) -> bool {
        self.stamp.is_some() && self.stamp == Self::stamp(&self.path)
    }

    fn stamp(path: &Path) -> Option<Stamp> {
        // Follow links so replacement of a linked config file or font directory is visible.
        match path.metadata() {
            Ok(info) => Some(Stamp::Present {
                device: info.dev(),
                inode: info.ino(),
                length: info.size(),
                modified: (info.mtime(), info.mtime_nsec()),
                changed: (info.ctime(), info.ctime_nsec()),
            }),
            Err(error)
                if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) =>
            {
                Some(Stamp::Missing)
            }
            Err(_) => None,
        }
    }
}
