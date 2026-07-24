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
        "public var operationId: String",
        "public var optionsJson: String?",
        "func execute(request: MermanOperationRequest) throws",
        "func renderSvg(source: String, optionsJson: String?) throws",
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
