use crate::XtaskError;
use crate::cmd::{UpstreamSvgFamilyLock, UpstreamSvgToolchainLock};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::LocalKey;

thread_local! {
    static BORROWED_FAMILY_LOCK_TARGETS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    static BORROWED_TOOLCHAIN_LOCK_TARGETS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
}

type BorrowedLockTargets = LocalKey<RefCell<Vec<PathBuf>>>;

struct BorrowedLockScope {
    targets: &'static BorrowedLockTargets,
    canonical_target: PathBuf,
    active: bool,
}

#[derive(Debug)]
pub(crate) struct UpstreamSvgToolchainReadGuard {
    tools_root: PathBuf,
    _owned_lock: Option<UpstreamSvgToolchainLock>,
}

impl Drop for BorrowedLockScope {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        self.targets.with(|targets| {
            let popped = targets.borrow_mut().pop();
            debug_assert_eq!(popped.as_deref(), Some(self.canonical_target.as_path()));
        });
    }
}

fn canonical_lock_target(target: &Path) -> Result<PathBuf, XtaskError> {
    fs::canonicalize(target).map_err(|source| XtaskError::ReadFile {
        path: target.display().to_string(),
        source,
    })
}

fn borrowed_lock_target(targets: &'static BorrowedLockTargets) -> Option<PathBuf> {
    targets.with(|targets| targets.borrow().last().cloned())
}

fn with_borrowed_lock<T>(
    targets: &'static BorrowedLockTargets,
    canonical_target: PathBuf,
    description: &str,
    operation: impl FnOnce() -> Result<T, XtaskError>,
) -> Result<T, XtaskError> {
    if let Some(active_target) = borrowed_lock_target(targets)
        && active_target != canonical_target
    {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "cannot nest borrowed upstream SVG {description} locks for {} and {}",
            active_target.display(),
            canonical_target.display()
        )));
    }

    let mut scope = BorrowedLockScope {
        targets,
        canonical_target: canonical_target.clone(),
        active: false,
    };
    targets.with(|targets| {
        targets.borrow_mut().push(canonical_target);
    });
    scope.active = true;
    operation()
}

fn borrowed_family_lock_target() -> Option<PathBuf> {
    borrowed_lock_target(&BORROWED_FAMILY_LOCK_TARGETS)
}

fn borrowed_toolchain_lock_target() -> Option<PathBuf> {
    borrowed_lock_target(&BORROWED_TOOLCHAIN_LOCK_TARGETS)
}

pub(crate) fn with_borrowed_upstream_svg_family_lock<T>(
    family_lock: &UpstreamSvgFamilyLock,
    target: &Path,
    operation: impl FnOnce() -> Result<T, XtaskError>,
) -> Result<T, XtaskError> {
    family_lock.validate_target(target)?;
    let canonical_target = canonical_lock_target(target)?;

    // The function borrow keeps the OS lock alive; TLS carries only its validated target through
    // the legacy diagram adapter function pointers.
    with_borrowed_lock(
        &BORROWED_FAMILY_LOCK_TARGETS,
        canonical_target,
        "family",
        operation,
    )
}

pub(crate) fn with_borrowed_upstream_svg_toolchain_lock<T>(
    toolchain_lock: &UpstreamSvgToolchainLock,
    target: &Path,
    operation: impl FnOnce() -> Result<T, XtaskError>,
) -> Result<T, XtaskError> {
    toolchain_lock.validate_target(target)?;
    let canonical_target = canonical_lock_target(target)?;
    with_borrowed_lock(
        &BORROWED_TOOLCHAIN_LOCK_TARGETS,
        canonical_target,
        "toolchain",
        operation,
    )
}

pub(crate) fn with_borrowed_upstream_svg_transaction_locks<T>(
    toolchain_lock: &UpstreamSvgToolchainLock,
    tools_root: &Path,
    family_lock: &UpstreamSvgFamilyLock,
    family_target: &Path,
    operation: impl FnOnce() -> Result<T, XtaskError>,
) -> Result<T, XtaskError> {
    // Writers use this same global order. Keep the toolchain scope outside the family scope so a
    // compare adapter can safely reuse both guards without introducing a lock-order inversion.
    with_borrowed_upstream_svg_toolchain_lock(toolchain_lock, tools_root, || {
        with_borrowed_upstream_svg_family_lock(family_lock, family_target, operation)
    })
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
    let canonical_target = canonical_lock_target(target)?;
    if borrowed_target == canonical_target {
        return Ok(None);
    }

    Err(XtaskError::UpstreamSvgFailed(format!(
        "borrowed upstream SVG family lock protects {}, not {}",
        borrowed_target.display(),
        canonical_target.display()
    )))
}

pub(crate) fn acquire_upstream_svg_toolchain_read_guard(
    tools_root: &Path,
) -> Result<UpstreamSvgToolchainReadGuard, XtaskError> {
    let owned_lock = match borrowed_toolchain_lock_target() {
        None => Some(crate::cmd::acquire_upstream_svg_toolchain_lock(tools_root)?),
        Some(borrowed_target) => {
            let canonical_target = canonical_lock_target(tools_root)?;
            if borrowed_target != canonical_target {
                return Err(XtaskError::UpstreamSvgFailed(format!(
                    "borrowed upstream SVG toolchain lock protects {}, not {}",
                    borrowed_target.display(),
                    canonical_target.display()
                )));
            }
            None
        }
    };

    Ok(UpstreamSvgToolchainReadGuard {
        tools_root: tools_root.to_path_buf(),
        _owned_lock: owned_lock,
    })
}

impl UpstreamSvgToolchainReadGuard {
    pub(crate) fn tools_root(&self) -> &Path {
        &self.tools_root
    }

    pub(crate) fn node_katex_math_renderer(
        &self,
    ) -> Option<Arc<dyn merman_render::math::MathRenderer + Send + Sync>> {
        if !self.tools_root.join("package.json").is_file()
            || !self.tools_root.join("node_modules").is_dir()
        {
            return None;
        }

        Some(Arc::new(merman_render::math::NodeKatexMathRenderer::new(
            self.tools_root.clone(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

    #[test]
    fn toolchain_read_guard_serializes_node_modules_reader_with_writer() {
        let tools_root = unique_family_dir("toolchain-reader");
        fs::create_dir_all(&tools_root).expect("create toolchain directory");
        let read_guard = acquire_upstream_svg_toolchain_read_guard(&tools_root)
            .expect("acquire toolchain read guard");

        let (started_tx, started_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let writer_root = tools_root.clone();
        let writer = thread::spawn(move || {
            started_tx.send(()).expect("report writer start");
            let lock = crate::cmd::acquire_upstream_svg_toolchain_lock(&writer_root)
                .expect("writer should acquire released toolchain lock");
            acquired_tx.send(()).expect("report writer acquisition");
            drop(lock);
        });

        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("writer should start its lock attempt");
        assert!(
            acquired_rx.recv_timeout(Duration::from_millis(75)).is_err(),
            "a node_modules writer must wait for the reader guard"
        );
        drop(read_guard);
        acquired_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("writer should proceed after the reader guard is dropped");
        writer.join().expect("writer thread should finish");
        fs::remove_dir(&tools_root).expect("remove toolchain directory");
    }

    #[test]
    fn toolchain_read_guard_keeps_missing_node_modules_unavailable() {
        let tools_root = unique_family_dir("missing-node-modules");
        fs::create_dir_all(&tools_root).expect("create toolchain directory");
        fs::write(tools_root.join("package.json"), "{}\n").expect("write package metadata");

        let read_guard = acquire_upstream_svg_toolchain_read_guard(&tools_root)
            .expect("acquire toolchain read guard");
        assert!(
            read_guard.node_katex_math_renderer().is_none(),
            "a stable missing install must not construct a Node KaTeX renderer"
        );

        drop(read_guard);
        fs::remove_file(tools_root.join("package.json")).expect("remove package metadata");
        fs::remove_dir(&tools_root).expect("remove toolchain directory");
    }

    #[test]
    fn borrowed_transaction_scope_reuses_toolchain_then_family_locks() {
        let tools_root = unique_family_dir("borrowed-toolchain");
        let family_target = unique_family_dir("borrowed-transaction-family");
        fs::create_dir_all(&tools_root).expect("create toolchain directory");
        fs::create_dir_all(&family_target).expect("create family directory");

        let toolchain_lock = crate::cmd::acquire_upstream_svg_toolchain_lock(&tools_root)
            .expect("acquire toolchain lock first");
        let family_lock = crate::cmd::acquire_upstream_svg_family_lock(&family_target)
            .expect("acquire family lock second");

        let result = with_borrowed_upstream_svg_transaction_locks(
            &toolchain_lock,
            &tools_root,
            &family_lock,
            &family_target,
            || {
                assert!(borrowed_toolchain_lock_target().is_some());
                assert!(borrowed_family_lock_target().is_some());

                let read_guard = acquire_upstream_svg_toolchain_read_guard(&tools_root)?;
                assert!(
                    read_guard._owned_lock.is_none(),
                    "the compare reader must reuse the borrowed toolchain lock"
                );
                assert!(
                    acquire_upstream_svg_family_lock_for_compare(&family_target, true)?.is_none(),
                    "the compare harness must reuse the borrowed family lock"
                );
                Err::<(), _>(XtaskError::SvgCompareFailed("expected failure".to_string()))
            },
        );

        assert!(matches!(result, Err(XtaskError::SvgCompareFailed(_))));
        assert!(borrowed_toolchain_lock_target().is_none());
        assert!(borrowed_family_lock_target().is_none());
        drop(family_lock);
        drop(toolchain_lock);
        fs::remove_dir(&family_target).expect("remove family directory");
        fs::remove_dir(&tools_root).expect("remove toolchain directory");
    }
}
