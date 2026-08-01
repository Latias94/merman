use merman_core::baseline::{PINNED_MERMAID_BASELINE_TAG, PINNED_MERMAID_BASELINE_VERSION};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

pub(crate) fn repo_ref_root() -> PathBuf {
    workspace_root().join("repo-ref")
}

pub(crate) fn fixtures_root() -> PathBuf {
    workspace_root().join("fixtures")
}

pub(crate) fn target_root() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_TARGET_DIR") {
        let path = PathBuf::from(path);
        return if path.is_absolute() {
            path
        } else {
            workspace_root().join(path)
        };
    }

    workspace_root().join("target")
}

pub(crate) fn wasm_build_target_root() -> PathBuf {
    target_root().join("wasm-build")
}

pub(crate) fn mermaid_repo_root() -> PathBuf {
    repo_ref_root().join("mermaid")
}

pub(crate) fn dompurify_repo_root() -> PathBuf {
    repo_ref_root().join("dompurify")
}

pub(crate) fn mermaid_cli_root() -> PathBuf {
    workspace_root().join("tools").join("mermaid-cli")
}

pub(crate) fn pinned_mermaid_baseline_label(workspace_root: &Path) -> String {
    let lock_path = workspace_root
        .join("tools")
        .join("upstreams")
        .join("REPOS.lock.json");
    let fallback = || format!("@{PINNED_MERMAID_BASELINE_VERSION}");
    let Ok(text) = fs::read_to_string(lock_path) else {
        return fallback();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return fallback();
    };
    let Some(reference) = value
        .get("repos")
        .and_then(|repos| repos.get("mermaid"))
        .and_then(|mermaid| mermaid.get("ref"))
        .and_then(|reference| reference.as_str())
        .filter(|reference| !reference.trim().is_empty())
    else {
        return fallback();
    };

    reference
        .strip_prefix("mermaid")
        .map(str::to_owned)
        .unwrap_or_else(|| {
            PINNED_MERMAID_BASELINE_TAG
                .strip_prefix("mermaid")
                .unwrap_or(PINNED_MERMAID_BASELINE_TAG)
                .to_owned()
        })
}

#[cfg(test)]
mod tests {
    use super::pinned_mermaid_baseline_label;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn pinned_mermaid_baseline_label_reads_lockfile_ref() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "merman-pinned-baseline-{}-{nonce}",
            std::process::id()
        ));
        let lock_dir = root.join("tools").join("upstreams");
        fs::create_dir_all(&lock_dir).expect("lock dir");
        fs::write(
            lock_dir.join("REPOS.lock.json"),
            r#"{"repos":{"mermaid":{"ref":"mermaid@11.16.0"}}}"#,
        )
        .expect("lockfile");

        assert_eq!(pinned_mermaid_baseline_label(&root), "@11.16.0");

        fs::remove_dir_all(root).expect("cleanup");
    }
}
