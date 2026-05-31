use serde_json::Value;
use std::{
    env,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::{self, Command},
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

    if let Err(error) = generate(args) {
        eprintln!("failed to generate Python UniFFI package: {error}");
        process::exit(1);
    }
}

#[derive(Debug)]
struct Args {
    cdylib: Option<PathBuf>,
    package_dir: PathBuf,
    help: bool,
}

impl Args {
    fn parse(values: impl Iterator<Item = OsString>) -> Result<Self, String> {
        let mut cdylib = None;
        let mut package_dir = default_package_dir();
        let mut help = false;
        let mut values = values.peekable();

        while let Some(value) = values.next() {
            let Some(value) = value.to_str() else {
                return Err("arguments must be valid Unicode paths".to_string());
            };

            match value {
                "--cdylib" => {
                    cdylib = Some(next_path(&mut values, "--cdylib")?);
                }
                "--package-dir" => {
                    package_dir = next_path(&mut values, "--package-dir")?;
                }
                "-h" | "--help" => {
                    help = true;
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(Self {
            cdylib,
            package_dir,
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

fn generate(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let cdylib = args.cdylib.unwrap_or_else(default_cdylib_path);
    if !cdylib.is_file() {
        return Err(format!(
            "cdylib not found at {}. Run `cargo build -p merman-uniffi --features bindgen-smoke` first, or pass --cdylib.",
            cdylib.display()
        )
        .into());
    }

    let module_dir = args.package_dir.join("src").join("merman");
    fs::create_dir_all(&module_dir)?;
    ensure_init_file(&module_dir)?;

    uniffi::generate(uniffi::GenerateOptions {
        languages: vec![uniffi::TargetLanguage::Python],
        source: utf8_path(&cdylib).into(),
        out_dir: utf8_path(&module_dir).into(),
        config_override: None,
        format: false,
        crate_filter: Some("merman_uniffi".to_string()),
        metadata_no_deps: false,
    })?;

    let cdylib_name = cdylib
        .file_name()
        .ok_or_else(|| format!("cdylib path has no file name: {}", cdylib.display()))?;
    let copied_cdylib = module_dir.join(cdylib_name);
    fs::copy(&cdylib, &copied_cdylib)?;

    println!("generated Python UniFFI module in {}", module_dir.display());
    println!("copied native library to {}", copied_cdylib.display());
    Ok(())
}

fn ensure_init_file(module_dir: &Path) -> io::Result<()> {
    let init = module_dir.join("__init__.py");
    if !init.exists() {
        fs::write(
            init,
            concat!(
                "\"\"\"Generated merman UniFFI package shim.\"\"\"\n",
                "from .merman_uniffi import MermanEngine, MermanError\n\n",
                "__all__ = [\"MermanEngine\", \"MermanError\"]\n",
            ),
        )?;
    }
    Ok(())
}

fn default_package_dir() -> PathBuf {
    workspace_root()
        .join("platforms")
        .join("python")
        .join("merman")
}

fn default_cdylib_path() -> PathBuf {
    cargo_target_dir()
        .unwrap_or_else(|_| workspace_root().join("target"))
        .join("debug")
        .join(cdylib_filename())
}

fn cargo_target_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .current_dir(workspace_root())
        .args(["metadata", "--format-version=1", "--no-deps"])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let target_directory = metadata
        .get("target_directory")
        .and_then(Value::as_str)
        .ok_or("cargo metadata target_directory missing")?;
    Ok(PathBuf::from(target_directory))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("merman-uniffi should live under workspace/crates")
        .to_path_buf()
}

fn cdylib_filename() -> &'static str {
    if cfg!(windows) {
        "merman_uniffi.dll"
    } else if cfg!(target_os = "macos") {
        "libmerman_uniffi.dylib"
    } else {
        "libmerman_uniffi.so"
    }
}

fn utf8_path(path: &Path) -> String {
    path.to_str()
        .unwrap_or_else(|| panic!("path is not valid UTF-8: {}", path.display()))
        .to_string()
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -p merman-uniffi --features bindgen-smoke --example generate_python_package -- [--cdylib PATH] [--package-dir PATH]"
    );
}
