use std::path::PathBuf;

pub(crate) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

pub(crate) fn repo_ref_root() -> PathBuf {
    workspace_root().join("repo-ref")
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
