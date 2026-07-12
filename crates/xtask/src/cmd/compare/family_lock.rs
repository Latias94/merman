use crate::XtaskError;
use crate::cmd::UpstreamSvgFamilyLock;
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

thread_local! {
    static BORROWED_FAMILY_LOCK_TARGETS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

struct BorrowedFamilyLockScope {
    canonical_target: PathBuf,
    active: bool,
}

impl Drop for BorrowedFamilyLockScope {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        BORROWED_FAMILY_LOCK_TARGETS.with(|targets| {
            let popped = targets.borrow_mut().pop();
            debug_assert_eq!(popped.as_deref(), Some(self.canonical_target.as_path()));
        });
    }
}

fn canonical_family_target(target: &Path) -> Result<PathBuf, XtaskError> {
    fs::canonicalize(target).map_err(|source| XtaskError::ReadFile {
        path: target.display().to_string(),
        source,
    })
}

fn borrowed_family_lock_target() -> Option<PathBuf> {
    BORROWED_FAMILY_LOCK_TARGETS.with(|targets| targets.borrow().last().cloned())
}

pub(crate) fn with_borrowed_upstream_svg_family_lock<T>(
    family_lock: &UpstreamSvgFamilyLock,
    target: &Path,
    operation: impl FnOnce() -> Result<T, XtaskError>,
) -> Result<T, XtaskError> {
    family_lock.validate_target(target)?;
    let canonical_target = canonical_family_target(target)?;

    if let Some(active_target) = borrowed_family_lock_target()
        && active_target != canonical_target
    {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "cannot nest borrowed upstream SVG family locks for {} and {}",
            active_target.display(),
            canonical_target.display()
        )));
    }

    // The function borrow keeps the OS lock alive; TLS carries only its validated target through
    // the legacy diagram adapter function pointers.
    let mut scope = BorrowedFamilyLockScope {
        canonical_target: canonical_target.clone(),
        active: false,
    };
    BORROWED_FAMILY_LOCK_TARGETS.with(|targets| {
        targets.borrow_mut().push(canonical_target.clone());
    });
    scope.active = true;
    operation()
}

pub(crate) fn acquire_upstream_svg_family_lock_for_compare(
    target: &Path,
    acquire_when_unborrowed: bool,
) -> Result<Option<UpstreamSvgFamilyLock>, XtaskError> {
    let Some(borrowed_target) = borrowed_family_lock_target() else {
        return if acquire_when_unborrowed {
            crate::cmd::acquire_upstream_svg_family_lock(target).map(Some)
        } else {
            Ok(None)
        };
    };
    let canonical_target = canonical_family_target(target)?;
    if borrowed_target == canonical_target {
        return Ok(None);
    }

    Err(XtaskError::UpstreamSvgFailed(format!(
        "borrowed upstream SVG family lock protects {}, not {}",
        borrowed_target.display(),
        canonical_target.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_family_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        crate::cmd::target_root()
            .join("compare")
            .join("family-lock-tests")
            .join(format!("{name}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn borrowed_scope_reuses_the_held_lock_and_clears_after_error() {
        let target = unique_family_dir("borrowed");
        fs::create_dir_all(&target).expect("create family directory");
        let family_lock = crate::cmd::acquire_upstream_svg_family_lock(&target)
            .expect("acquire external family lock");

        let result = with_borrowed_upstream_svg_family_lock(&family_lock, &target, || {
            assert!(
                acquire_upstream_svg_family_lock_for_compare(&target, true)?.is_none(),
                "the compare path must reuse the borrowed family lock"
            );
            Err::<(), _>(XtaskError::SvgCompareFailed("expected failure".to_string()))
        });

        assert!(matches!(result, Err(XtaskError::SvgCompareFailed(_))));
        assert!(borrowed_family_lock_target().is_none());
        drop(family_lock);
        fs::remove_dir(&target).expect("remove family directory");
    }

    #[test]
    fn borrowed_scope_rejects_a_different_compare_target() {
        let target = unique_family_dir("borrowed-target");
        let other = unique_family_dir("other-target");
        fs::create_dir_all(&target).expect("create borrowed family directory");
        fs::create_dir_all(&other).expect("create other family directory");
        let family_lock = crate::cmd::acquire_upstream_svg_family_lock(&target)
            .expect("acquire external family lock");

        let error = with_borrowed_upstream_svg_family_lock(&family_lock, &target, || {
            acquire_upstream_svg_family_lock_for_compare(&other, false).map(|_| ())
        })
        .expect_err("a different compare target must be rejected");

        assert!(matches!(error, XtaskError::UpstreamSvgFailed(_)));
        assert!(borrowed_family_lock_target().is_none());
        drop(family_lock);
        fs::remove_dir(&target).expect("remove borrowed family directory");
        fs::remove_dir(&other).expect("remove other family directory");
    }

    #[test]
    fn borrowed_scope_rejects_nested_different_family_locks() {
        let target = unique_family_dir("outer");
        let other = unique_family_dir("nested");
        fs::create_dir_all(&target).expect("create outer family directory");
        fs::create_dir_all(&other).expect("create nested family directory");
        let family_lock = crate::cmd::acquire_upstream_svg_family_lock(&target)
            .expect("acquire outer family lock");
        let other_lock = crate::cmd::acquire_upstream_svg_family_lock(&other)
            .expect("acquire nested family lock");

        let error = with_borrowed_upstream_svg_family_lock(&family_lock, &target, || {
            with_borrowed_upstream_svg_family_lock(&other_lock, &other, || Ok(()))
        })
        .expect_err("different borrowed family locks must not be nested");

        assert!(matches!(error, XtaskError::UpstreamSvgFailed(_)));
        assert!(borrowed_family_lock_target().is_none());
        drop(other_lock);
        drop(family_lock);
        fs::remove_dir(&target).expect("remove outer family directory");
        fs::remove_dir(&other).expect("remove nested family directory");
    }

    #[test]
    fn borrowed_scope_clears_after_panic() {
        let target = unique_family_dir("panic");
        fs::create_dir_all(&target).expect("create family directory");
        let family_lock = crate::cmd::acquire_upstream_svg_family_lock(&target)
            .expect("acquire external family lock");

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = with_borrowed_upstream_svg_family_lock(
                &family_lock,
                &target,
                || -> Result<(), XtaskError> { panic!("expected panic") },
            );
        }));

        assert!(panic.is_err());
        assert!(borrowed_family_lock_target().is_none());
        drop(family_lock);
        fs::remove_dir(&target).expect("remove family directory");
    }
}
