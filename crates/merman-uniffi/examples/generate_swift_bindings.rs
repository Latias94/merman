use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process,
};

fn main() {
    let args = match Args::parse(env::args_os().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            print_usage();
            process::exit(2);
        }
    };

    if args.help {
        print_usage();
        return;
    }

    if let Err(error) = generate(&args) {
        eprintln!("failed to generate Swift UniFFI bindings: {error}");
        process::exit(1);
    }
}

#[derive(Debug)]
struct Args {
    library: Option<PathBuf>,
    output_dir: PathBuf,
    help: bool,
}

impl Args {
    fn parse(values: impl Iterator<Item = OsString>) -> Result<Self, String> {
        let mut library = None;
        let mut output_dir = default_output_dir();
        let mut help = false;
        let mut values = values.peekable();

        while let Some(value) = values.next() {
            let Some(value) = value.to_str() else {
                return Err("arguments must be valid Unicode paths".to_string());
            };

            match value {
                "--library" => library = Some(next_path(&mut values, "--library")?),
                "--output-dir" => output_dir = next_path(&mut values, "--output-dir")?,
                "-h" | "--help" => help = true,
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(Self {
            library,
            output_dir,
            help,
        })
    }
}

fn next_path(
    values: &mut std::iter::Peekable<impl Iterator<Item = OsString>>,
    flag: &str,
) -> Result<PathBuf, String> {
    values
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("{flag} requires a path"))
}

fn generate(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let library = args.library.clone().unwrap_or_else(default_library_path);
    if !library.is_file() {
        return Err(format!(
            "UniFFI library not found at {}. Build merman-uniffi with the intended artifact profile, or pass --library.",
            library.display()
        )
        .into());
    }

    std::fs::create_dir_all(&args.output_dir)?;
    uniffi::generate(uniffi::GenerateOptions {
        languages: vec![uniffi::TargetLanguage::Swift],
        source: utf8_path(&library).into(),
        out_dir: utf8_path(&args.output_dir).into(),
        // The source is normally a compiled library in target/. The bindgen-smoke
        // feature enables UniFFI's Cargo metadata layer so it can still locate
        // crates/merman-uniffi/uniffi.toml from the compiled crate name.
        config_override: None,
        format: false,
        crate_filter: Some("merman_uniffi".to_string()),
        metadata_no_deps: false,
    })?;

    apply_swift_5_9_compatibility(&args.output_dir.join("Merman.swift"))?;

    for output in ["Merman.swift", "MermanFFI.h", "MermanFFI.modulemap"] {
        let path = args.output_dir.join(output);
        if !path.is_file() {
            return Err(format!(
                "UniFFI did not generate required Swift artifact: {}",
                path.display()
            )
            .into());
        }
        normalize_generated_text(&path)?;
    }
    require_stable_swift_surface(&args.output_dir.join("Merman.swift"))?;

    println!(
        "generated Swift UniFFI bindings in {} from {}",
        args.output_dir.display(),
        library.display()
    );
    Ok(())
}

fn apply_swift_5_9_compatibility(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;
    let compatible = swift_5_9_compatible_source(&source).map_err(|message| {
        format!(
            "unable to apply the pinned Swift 5.9 compatibility transform to {}: {message}",
            path.display()
        )
    })?;
    std::fs::write(path, compatible)?;
    Ok(())
}

fn swift_5_9_compatible_source(source: &str) -> Result<String, String> {
    const GENERATED_BLOCK: &str = r#"    // `nonisolated(unsafe)` is needed under Swift 6 strict concurrency.
    // This is safe because the pointee is initialized once during static init
    // and never mutated by either side of the FFI.  Its fields are C function pointers.
    nonisolated(unsafe) static let vtablePtr: UnsafePointer<UniffiVTableCallbackInterfaceMermanTextMeasurer> = {
        let ptr = UnsafeMutablePointer<UniffiVTableCallbackInterfaceMermanTextMeasurer>.allocate(capacity: 1)
        ptr.initialize(to: vtable)
        return UnsafePointer(ptr)
    }()"#;
    const COMPATIBLE_BLOCK: &str = r#"    // Swift 5.9 does not support `nonisolated(unsafe)`. The storage is initialized
    // once and never mutated by either side of the FFI, so its pointer can safely
    // cross concurrency domains.
    private final class VTableStorage: @unchecked Sendable {
        let pointer: UnsafePointer<UniffiVTableCallbackInterfaceMermanTextMeasurer>

        init(_ value: UniffiVTableCallbackInterfaceMermanTextMeasurer) {
            let pointer =
                UnsafeMutablePointer<UniffiVTableCallbackInterfaceMermanTextMeasurer>.allocate(
                    capacity: 1
                )
            pointer.initialize(to: value)
            self.pointer = UnsafePointer(pointer)
        }
    }

    private static let vtableStorage = VTableStorage(vtable)

    static var vtablePtr: UnsafePointer<UniffiVTableCallbackInterfaceMermanTextMeasurer> {
        vtableStorage.pointer
    }"#;

    let generated_count = source.matches(GENERATED_BLOCK).count();
    let compatible_count = source.matches(COMPATIBLE_BLOCK).count();
    match (generated_count, compatible_count) {
        (1, 0) => Ok(source.replacen(GENERATED_BLOCK, COMPATIBLE_BLOCK, 1)),
        (0, 1) => Ok(source.to_string()),
        _ => Err(format!(
            "expected exactly one generated or compatible callback vtable block, found generated={generated_count} compatible={compatible_count}"
        )),
    }
}

fn normalize_generated_text(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;
    let mut normalized = String::with_capacity(source.len());
    for line in source.lines() {
        normalized.push_str(line.trim_end());
        normalized.push('\n');
    }
    while normalized.ends_with("\n\n") {
        normalized.pop();
    }
    std::fs::write(path, normalized)?;
    Ok(())
}

fn require_stable_swift_surface(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(path)?;
    for required in [
        "enum MermanErrorKind",
        "case unknownOperation",
        "kind: MermanErrorKind",
        "capabilityId: String?",
        "struct MermanResourceErrorDetails",
        "public var cause: String",
        "enum MermanResourceOverrideId",
        "public var id: MermanResourceOverrideId",
        "func resourceOptionsJson(profile: MermanResourceProfile?",
        "resource: MermanResourceErrorDetails?",
        "public var operationId: String",
        "public var optionsJson: String?",
        "func execute(request: MermanOperationRequest) throws",
        "func presentationCatalogJson() throws",
        "func renderSvg(source: String, optionsJson: String?) throws",
        "func reusableEngineWithTextMeasurer(",
        "private final class VTableStorage: @unchecked Sendable",
    ] {
        if !source.contains(required) {
            return Err(format!(
                "generated Swift binding is missing stable contract `{required}`: {}",
                path.display()
            )
            .into());
        }
    }
    for removed in [
        "func execute(request: MermanOperationRequest, optionsJson:",
        "func renderSvg(source: String) throws",
        "public var outputId: String",
        "case unknownOutput",
        "nonisolated(unsafe) static let vtablePtr",
        "func setTextMeasurer(",
        "func clearTextMeasurer(",
        "func supportedHostThemePresets()",
        "enum MermanResourceLimitId",
        "public var id: MermanResourceLimitId",
        "func resourceOptionsJson(profile: MermanResourceProfile,",
    ] {
        if source.contains(removed) {
            return Err(format!(
                "generated Swift binding retains obsolete API `{removed}`: {}",
                path.display()
            )
            .into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::swift_5_9_compatible_source;

    const GENERATED: &str = r#"before
    // `nonisolated(unsafe)` is needed under Swift 6 strict concurrency.
    // This is safe because the pointee is initialized once during static init
    // and never mutated by either side of the FFI.  Its fields are C function pointers.
    nonisolated(unsafe) static let vtablePtr: UnsafePointer<UniffiVTableCallbackInterfaceMermanTextMeasurer> = {
        let ptr = UnsafeMutablePointer<UniffiVTableCallbackInterfaceMermanTextMeasurer>.allocate(capacity: 1)
        ptr.initialize(to: vtable)
        return UnsafePointer(ptr)
    }()
after"#;

    #[test]
    fn swift_5_9_transform_replaces_the_generated_concurrency_annotation() {
        let compatible = swift_5_9_compatible_source(GENERATED).unwrap();

        assert!(compatible.contains("private final class VTableStorage: @unchecked Sendable"));
        assert!(!compatible.contains("nonisolated(unsafe) static let vtablePtr"));
    }

    #[test]
    fn swift_5_9_transform_is_idempotent_and_fails_closed_on_generator_drift() {
        let compatible = swift_5_9_compatible_source(GENERATED).unwrap();

        assert_eq!(
            swift_5_9_compatible_source(&compatible).unwrap(),
            compatible
        );
        assert!(swift_5_9_compatible_source("unrelated output").is_err());
    }
}

fn default_output_dir() -> PathBuf {
    workspace_root()
        .join("platforms")
        .join("apple")
        .join("Sources")
        .join("Merman")
        .join("Generated")
}

fn default_library_path() -> PathBuf {
    let target_dir = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"));
    target_dir.join("debug").join(static_library_filename())
}

fn static_library_filename() -> &'static str {
    if cfg!(target_os = "windows") {
        "merman_uniffi.lib"
    } else {
        "libmerman_uniffi.a"
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("merman-uniffi should live under workspace/crates")
        .to_path_buf()
}

fn utf8_path(path: &Path) -> String {
    path.to_str()
        .unwrap_or_else(|| panic!("path is not valid UTF-8: {}", path.display()))
        .to_string()
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -p merman-uniffi --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' --example generate_swift_bindings -- [--library PATH] [--output-dir PATH]"
    );
}
