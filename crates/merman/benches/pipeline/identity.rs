use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OutputIdentity {
    output_kind: &'static str,
    output_bytes: usize,
    output_sha256: String,
    svg_elements: Option<usize>,
}

fn sha256_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

pub(super) fn output_identity(
    output_kind: &'static str,
    output: &[u8],
    svg_elements: Option<usize>,
) -> OutputIdentity {
    assert!(!output.is_empty(), "benchmark produced empty output");
    OutputIdentity {
        output_kind,
        output_bytes: output.len(),
        output_sha256: sha256_bytes(output),
        svg_elements,
    }
}

pub(super) fn emit_preflight(group: &str, name: &str, identity: &OutputIdentity) {
    let receipt = json!({
        "schema_version": 1,
        "benchmark": format!("{group}/{name}"),
        "output_kind": identity.output_kind,
        "output_bytes": identity.output_bytes,
        "output_sha256": identity.output_sha256,
        "svg_elements": identity.svg_elements,
    });
    eprintln!(
        "[bench][preflight] {}",
        serde_json::to_string(&receipt).expect("preflight receipt must serialize")
    );
}

fn exact_benchmark() -> Option<&'static str> {
    static EXACT: OnceLock<Option<String>> = OnceLock::new();
    EXACT
        .get_or_init(|| {
            let arguments = std::env::args().collect::<Vec<_>>();
            arguments
                .windows(2)
                .find(|pair| pair[0] == "--exact")
                .map(|pair| pair[1].clone())
        })
        .as_deref()
}

pub(super) fn benchmark_selected(group: &str, name: &str) -> bool {
    exact_benchmark().is_none_or(|exact| {
        exact
            .strip_prefix(group)
            .and_then(|value| value.strip_prefix('/'))
            == Some(name)
    })
}

pub(super) fn verify_postflight(
    group: &str,
    name: &str,
    preflight: &OutputIdentity,
    postflight: impl FnOnce() -> OutputIdentity,
) {
    let Some(exact) = exact_benchmark() else {
        return;
    };
    let Some(suffix) = exact
        .strip_prefix(group)
        .and_then(|value| value.strip_prefix('/'))
    else {
        return;
    };
    if suffix != name {
        return;
    }
    assert_eq!(
        &postflight(),
        preflight,
        "{group}/{name} output identity changed after timed iterations"
    );
    eprintln!("[bench][postflight] {group}/{name}");
}
