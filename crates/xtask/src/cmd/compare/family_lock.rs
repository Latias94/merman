use crate::XtaskError;
use crate::cmd::{UpstreamSvgFamilyLock, UpstreamSvgToolchainLock};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) fn acquire_upstream_svg_family_lock_for_compare(
    target: &Path,
    acquire: bool,
) -> Result<Option<UpstreamSvgFamilyLock>, XtaskError> {
    if acquire {
        crate::cmd::acquire_upstream_svg_family_lock(target).map(Some)
    } else {
        Ok(None)
    }
}

#[derive(Debug)]
pub(crate) struct UpstreamSvgToolchainReadGuard {
    tools_root: PathBuf,
    _lock: UpstreamSvgToolchainLock,
}

pub(crate) fn acquire_upstream_svg_toolchain_read_guard(
    tools_root: &Path,
) -> Result<UpstreamSvgToolchainReadGuard, XtaskError> {
    Ok(UpstreamSvgToolchainReadGuard {
        tools_root: tools_root.to_path_buf(),
        _lock: crate::cmd::acquire_upstream_svg_toolchain_lock(tools_root)?,
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
    use std::fs;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn unique_toolchain_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        crate::cmd::target_root()
            .join("compare")
            .join("toolchain-lock-tests")
            .join(format!("{name}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn toolchain_read_guard_serializes_node_modules_reader_with_writer() {
        let tools_root = unique_toolchain_dir("reader");
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
        let tools_root = unique_toolchain_dir("missing-node-modules");
        fs::create_dir_all(&tools_root).expect("create toolchain directory");
        fs::write(tools_root.join("package.json"), "{}\n").expect("write package metadata");

        let read_guard = acquire_upstream_svg_toolchain_read_guard(&tools_root)
            .expect("acquire toolchain read guard");
        assert!(read_guard.node_katex_math_renderer().is_none());

        drop(read_guard);
        fs::remove_file(tools_root.join("package.json")).expect("remove package metadata");
        fs::remove_dir(&tools_root).expect("remove toolchain directory");
    }
}
