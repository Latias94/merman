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
            "cdylib not found at {}. Build an explicit merman-uniffi artifact profile first, or pass --cdylib.",
            cdylib.display()
        )
        .into());
    }

    let module_dir = args.package_dir.join("src").join("merman");
    fs::create_dir_all(&module_dir)?;

    uniffi::generate(uniffi::GenerateOptions {
        languages: vec![uniffi::TargetLanguage::Python],
        source: utf8_path(&cdylib).into(),
        out_dir: utf8_path(&module_dir).into(),
        config_override: None,
        format: false,
        crate_filter: Some("merman_uniffi".to_string()),
        metadata_no_deps: false,
    })?;

    require_stable_python_surface(&module_dir)?;
    ensure_init_file(&module_dir)?;

    let cdylib_name = cdylib
        .file_name()
        .ok_or_else(|| format!("cdylib path has no file name: {}", cdylib.display()))?;
    let copied_cdylib = module_dir.join(cdylib_name);
    fs::copy(&cdylib, &copied_cdylib)?;

    println!("generated Python UniFFI module in {}", module_dir.display());
    println!("copied native library to {}", copied_cdylib.display());
    Ok(())
}

fn require_stable_python_surface(module_dir: &Path) -> io::Result<()> {
    let bindings = fs::read_to_string(module_dir.join(PYTHON_BINDINGS_MODULE))?;
    validate_stable_python_surface(&bindings)
}

fn validate_stable_python_surface(bindings: &str) -> io::Result<()> {
    for required in [
        "class MermanTextMeasurer(",
        "class MermanOperationRequest:",
        "class MermanResourceErrorDetails:",
        "class MermanResourceOverrideId(",
        "id:MermanResourceOverrideId",
        "self.operation_id = operation_id",
        "self.options_json = options_json",
        "UNKNOWN_OPERATION",
        "def execute(self, request: MermanOperationRequest) -> MermanOperationResult:",
        "def render_svg(self, source: str,options_json: typing.Optional[str]) -> str:",
        "def reusable_engine_with_text_measurer(",
    ] {
        if !bindings.contains(required) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("generated Python UniFFI binding is missing stable API `{required}`"),
            ));
        }
    }
    if !bindings.lines().any(|line| {
        line.trim()
            .strip_prefix("def presentation_catalog_json(")
            .and_then(|signature| signature.strip_suffix(") -> str:"))
            .is_some_and(|parameters| matches!(parameters.trim(), "self" | "self,"))
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "generated Python UniFFI binding is missing stable API `presentation_catalog_json(self) -> str`",
        ));
    }
    for removed in [
        "request: MermanOperationRequest,options_json",
        "def render_svg(self, source: str) -> str:",
        "self.output_id = output_id",
        "UNKNOWN_OUTPUT",
        "def set_text_measurer(",
        "def clear_text_measurer(",
        "def supported_host_theme_presets(",
        "class MermanResourceLimitId(",
        "id:MermanResourceLimitId",
    ] {
        if bindings.contains(removed) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("generated Python UniFFI binding retains obsolete API `{removed}`"),
            ));
        }
    }
    Ok(())
}

fn ensure_init_file(module_dir: &Path) -> io::Result<()> {
    fs::write(
        module_dir.join("_binding_contract.py"),
        PYTHON_BINDING_CONTRACT_MODULE,
    )?;
    fs::write(
        module_dir.join("_runtime_catalog.py"),
        PYTHON_RUNTIME_CATALOG_MODULE,
    )?;
    let legacy_runtime_contract = module_dir.join("_runtime_contract.py");
    if legacy_runtime_contract.exists() {
        fs::remove_file(legacy_runtime_contract)?;
    }
    fs::write(
        module_dir.join("_resource_options.py"),
        PYTHON_RESOURCE_OPTIONS_MODULE,
    )?;
    fs::write(
        module_dir.join("_text_measurement_protocol.py"),
        merman_uniffi::MERMAN_UNIFFI_PYTHON_TEXT_MEASUREMENT_PROTOCOL_MODULE,
    )?;

    let expected_init = python_package_init();
    let init = module_dir.join("__init__.py");
    if !init.exists() {
        fs::write(init, expected_init)?;
        return Ok(());
    }

    let current = fs::read_to_string(&init)?;
    if is_managed_init_file(&current) && current != expected_init {
        fs::write(init, expected_init)?;
    }

    Ok(())
}

const PYTHON_PACKAGE_INIT_MARKER: &str = "# Generated by `cargo run -p merman-uniffi --example generate_python_package`; do not edit by hand.";
const PYTHON_BINDING_CONTRACT_MODULE: &str = include_str!("../python/binding_contract.py");
const PYTHON_RUNTIME_CATALOG_MODULE: &str = include_str!("../python/runtime_catalog.py");
const PYTHON_RESOURCE_OPTIONS_MODULE: &str = include_str!("../python/resource_options.py");
const PYTHON_BINDINGS_MODULE: &str = "merman_uniffi.py";

const PYTHON_PACKAGE_INIT_PREFIX: &str = concat!(
    "\"\"\"Python package shim for generated merman UniFFI bindings.\"\"\"\n",
    "# Generated by `cargo run -p merman-uniffi --example generate_python_package`; do not edit by hand.",
    "\n\n"
);

const PYTHON_PACKAGE_INIT_TEXT_MEASUREMENT_HELPERS: &str = concat!(
    "from ._text_measurement_protocol import (\n",
    "    TEXT_MEASUREMENT_PROTOCOL_VERSION,\n",
    "    TEXT_MEASUREMENT_OPERATIONS,\n",
    "    TEXT_MEASUREMENT_RESULT_KINDS,\n",
    "    TextMeasurementProtocolVersionMismatch,\n",
    "    require_text_measurement_protocol_version,\n",
    ")\n\n"
);

const PYTHON_PACKAGE_INIT_BASE_IMPORTS: &str = concat!(
    "from ._runtime_catalog import (\n",
    "    MermanRuntimeCatalog,\n",
    "    MermanRuntimeCatalogError,\n",
    "    get_runtime_catalog,\n",
    ")\n\n",
    "from ._resource_options import (\n",
    "    ResourceLimitId,\n",
    "    ResourceOverrideId,\n",
    "    ResourceOptions,\n",
    "    ResourceOptionsBuilder,\n",
    "    ResourceProfile,\n",
    ")\n\n",
    "try:\n",
    "    from .merman_uniffi import (\n",
    "        MermanAsciiCapability,\n",
    "        MermanAsciiCapabilityEvidence,\n",
    "        MermanDiagramFamilyCapability,\n",
    "        MermanEngine,\n",
    "        MermanError,\n",
    "        MermanErrorKind,\n",
    "        MermanLintRuleCatalogEntry,\n",
    "        MermanOperationRequest,\n",
    "        MermanOperationResult,\n",
    "        MermanResourceErrorDetails,\n",
    "        MermanReusableEngine,\n",
);

const PYTHON_PACKAGE_INIT_TEXT_MEASUREMENT_IMPORTS: &str = concat!(
    "        MermanTextDirection,\n",
    "        MermanTextMeasureRequest,\n",
    "        MermanTextMeasureResult,\n",
    "        MermanTextMeasurementOperation,\n",
    "        MermanTextMeasurementPhase,\n",
    "        MermanTextMeasurementResultKind,\n",
    "        MermanTextMeasurer,\n",
    "        MermanTextWhiteSpace,\n",
    "        MermanTextWrapMode,\n",
);

const PYTHON_PACKAGE_INIT_IMPORT_SUFFIX: &str = concat!(
    "        MermanValidationResult,\n",
    "    )\n",
    "except ModuleNotFoundError as exc:\n",
    "    if exc.name == f\"{__name__}.merman_uniffi\":\n",
    "        raise ImportError(\n",
    "            \"Generated merman UniFFI bindings are missing. \"\n",
    "            \"Build an explicit merman-uniffi artifact profile, then run \"\n",
    "            \"`cargo run -p merman-uniffi --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' --example \"\n",
    "            \"generate_python_package -- --package-dir platforms/python/merman`.\"\n",
    "        ) from exc\n",
    "    raise\n\n",
    "__all__ = [\n",
    "    \"MermanAsciiCapability\",\n",
    "    \"MermanAsciiCapabilityEvidence\",\n",
    "    \"MermanDiagramFamilyCapability\",\n",
    "    \"MermanEngine\",\n",
    "    \"MermanError\",\n",
    "    \"MermanErrorKind\",\n",
    "    \"MermanLintRuleCatalogEntry\",\n",
    "    \"MermanOperationRequest\",\n",
    "    \"MermanOperationResult\",\n",
    "    \"MermanResourceErrorDetails\",\n",
    "    \"MermanReusableEngine\",\n",
    "    \"ResourceLimitId\",\n",
    "    \"ResourceOverrideId\",\n",
    "    \"ResourceOptions\",\n",
    "    \"ResourceOptionsBuilder\",\n",
    "    \"ResourceProfile\",\n",
    "    \"MermanRuntimeCatalog\",\n",
    "    \"MermanRuntimeCatalogError\",\n",
    "    \"MermanValidationResult\",\n",
    "    \"get_runtime_catalog\",\n",
);

const PYTHON_PACKAGE_INIT_TEXT_MEASUREMENT_EXPORTS: &str = concat!(
    "    \"TEXT_MEASUREMENT_PROTOCOL_VERSION\",\n",
    "    \"TextMeasurementProtocolVersionMismatch\",\n",
    "    \"TEXT_MEASUREMENT_OPERATIONS\",\n",
    "    \"TEXT_MEASUREMENT_RESULT_KINDS\",\n",
    "    \"MermanTextDirection\",\n",
    "    \"MermanTextMeasureRequest\",\n",
    "    \"MermanTextMeasureResult\",\n",
    "    \"MermanTextMeasurementOperation\",\n",
    "    \"MermanTextMeasurementPhase\",\n",
    "    \"MermanTextMeasurementResultKind\",\n",
    "    \"MermanTextMeasurer\",\n",
    "    \"MermanTextWhiteSpace\",\n",
    "    \"MermanTextWrapMode\",\n",
    "    \"require_text_measurement_protocol_version\",\n",
);

const PYTHON_PACKAGE_INIT_SUFFIX: &str = "]\n";

fn python_package_init() -> String {
    let mut init = String::from(PYTHON_PACKAGE_INIT_PREFIX);
    init.push_str(PYTHON_PACKAGE_INIT_TEXT_MEASUREMENT_HELPERS);
    init.push_str(PYTHON_PACKAGE_INIT_BASE_IMPORTS);
    init.push_str(PYTHON_PACKAGE_INIT_TEXT_MEASUREMENT_IMPORTS);
    init.push_str(PYTHON_PACKAGE_INIT_IMPORT_SUFFIX);
    init.push_str(PYTHON_PACKAGE_INIT_TEXT_MEASUREMENT_EXPORTS);
    init.push_str(PYTHON_PACKAGE_INIT_SUFFIX);
    init
}

fn is_managed_init_file(contents: &str) -> bool {
    contents.contains(PYTHON_PACKAGE_INIT_MARKER)
        || contents.starts_with("\"\"\"Generated merman UniFFI package shim.\"\"\"\n")
        || contents.starts_with(
            "\"\"\"Python package shim for generated merman UniFFI bindings.\"\"\"\n\n",
        )
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
        "usage: cargo run -p merman-uniffi --no-default-features --features 'svg,analysis,ascii,png,jpeg,pdf,layout-cytoscape,layout-elk,math,system-clock,system-timezone,system-random,bindgen-smoke' --example generate_python_package -- [--cdylib PATH] [--package-dir PATH]"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_text_measurement_protocol_exports(init: &str) {
        for name in [
            "MermanTextDirection",
            "MermanTextMeasureRequest",
            "MermanTextMeasureResult",
            "MermanTextMeasurementOperation",
            "MermanTextMeasurementPhase",
            "MermanTextMeasurementResultKind",
            "MermanTextMeasurer",
            "MermanTextWhiteSpace",
            "MermanTextWrapMode",
        ] {
            assert!(
                init.contains(&format!("        {name},\n")),
                "generated package shim must import {name}"
            );
            assert!(
                init.contains(&format!("    \"{name}\",\n")),
                "generated package shim must include {name} in __all__"
            );
        }
        for name in [
            "TEXT_MEASUREMENT_PROTOCOL_VERSION",
            "TextMeasurementProtocolVersionMismatch",
            "TEXT_MEASUREMENT_OPERATIONS",
            "TEXT_MEASUREMENT_RESULT_KINDS",
            "require_text_measurement_protocol_version",
        ] {
            assert!(
                init.contains(name),
                "generated SVG package shim must expose {name}"
            );
        }
    }

    fn assert_error_contract_exports(init: &str) {
        for name in [
            "MermanError",
            "MermanErrorKind",
            "MermanResourceErrorDetails",
        ] {
            assert!(
                init.contains(&format!("        {name},\n")),
                "generated package shim must import {name}"
            );
            assert!(
                init.contains(&format!("    \"{name}\",\n")),
                "generated package shim must include {name} in __all__"
            );
        }
    }

    #[test]
    fn generated_binding_surface_requires_stable_public_api() {
        let incomplete = validate_stable_python_surface("class MermanEngine:\n    pass\n")
            .expect_err("feature-slim bindings must not omit the stable callback API");
        assert!(incomplete.to_string().contains("MermanTextMeasurer"));

        let stable_surface = "class MermanTextMeasurer(abc.ABC):\n\
             class MermanOperationRequest:\n    pass\n\
             class MermanResourceErrorDetails:\n    pass\n\
             class MermanResourceOverrideId(str):\n    pass\n\
             id:MermanResourceOverrideId\n\
             self.operation_id = operation_id\n\
             self.options_json = options_json\n\
             UNKNOWN_OPERATION\n\
             def execute(self, request: MermanOperationRequest) -> MermanOperationResult:\n    pass\n\
             def presentation_catalog_json(self, ) -> str:\n    pass\n\
             def render_svg(self, source: str,options_json: typing.Optional[str]) -> str:\n    pass\n\
             def reusable_engine_with_text_measurer(self):\n    pass\n";
        validate_stable_python_surface(stable_surface).expect("stable callback API");
        validate_stable_python_surface(&stable_surface.replace(
            "def presentation_catalog_json(self, ) -> str:",
            "def presentation_catalog_json(self) -> str:",
        ))
        .expect("formatted zero-argument presentation API");

        for invalid_signature in [
            "def presentation_catalog_json(self, options_json: str) -> str:",
            "def presentation_catalog_json(self, ) -> bytes:",
        ] {
            let invalid = stable_surface.replace(
                "def presentation_catalog_json(self, ) -> str:",
                invalid_signature,
            );
            let error = validate_stable_python_surface(&invalid)
                .expect_err("presentation catalog signature drift must fail closed");
            assert!(error.to_string().contains("presentation_catalog_json"));
        }

        let obsolete_surface = format!(
            "{stable_surface}def execute(self, request: MermanOperationRequest,options_json: typing.Optional[str]):\n    pass\n"
        );
        let obsolete = validate_stable_python_surface(&obsolete_surface)
            .expect_err("generic options must not remain a parallel execute argument");
        assert!(obsolete.to_string().contains("obsolete API"));
    }

    #[test]
    fn ensure_init_file_creates_stable_managed_shim() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let module_dir = temp.path().join("merman");
        fs::create_dir_all(&module_dir).expect("create module dir");
        fs::write(module_dir.join("_runtime_contract.py"), "legacy helper")
            .expect("write legacy runtime helper");

        ensure_init_file(&module_dir).expect("create init file");

        let init = fs::read_to_string(module_dir.join("__init__.py")).expect("read init file");
        assert!(init.contains(PYTHON_PACKAGE_INIT_MARKER));
        assert!(init.contains("MermanTextMeasurer"));
        assert!(init.contains("MermanLintRuleCatalogEntry"));
        assert!(init.contains("ResourceOptionsBuilder"));
        assert!(init.contains("get_runtime_catalog"));
        assert!(!init.contains("get_runtime_contract"));
        assert!(!init.contains("MermanRuntimeContract,"));
        assert!(!init.contains("\"MermanTextMeasurementCapabilities\""));
        assert!(
            !module_dir.join("_runtime_contract.py").exists(),
            "generation must remove the obsolete split runtime helper"
        );
        assert_eq!(
            fs::read_to_string(module_dir.join("_binding_contract.py"))
                .expect("read binding contract helper"),
            PYTHON_BINDING_CONTRACT_MODULE
        );
        assert_eq!(
            fs::read_to_string(module_dir.join("_runtime_catalog.py"))
                .expect("read runtime catalog helper"),
            PYTHON_RUNTIME_CATALOG_MODULE
        );
        assert!(
            !module_dir.join("_runtime_contract.py").exists(),
            "the checked-in Python package must not retain the split runtime helper"
        );
        assert_eq!(
            fs::read_to_string(module_dir.join("_resource_options.py"))
                .expect("read resource options helper"),
            PYTHON_RESOURCE_OPTIONS_MODULE
        );
        assert_eq!(
            fs::read_to_string(module_dir.join("_text_measurement_protocol.py"))
                .expect("read text measurement helper"),
            merman_uniffi::MERMAN_UNIFFI_PYTHON_TEXT_MEASUREMENT_PROTOCOL_MODULE
        );
        assert_text_measurement_protocol_exports(&init);
        assert_error_contract_exports(&init);
    }

    #[test]
    fn ensure_init_file_keeps_text_measurement_exports_for_feature_slim_artifacts() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let module_dir = temp.path().join("merman");
        fs::create_dir_all(&module_dir).expect("create module dir");
        fs::write(
            module_dir.join("_text_measurement_protocol.py"),
            "stale feature-conditional helper",
        )
        .expect("write stale text measurement helper");

        ensure_init_file(&module_dir).expect("create feature-stable init file");

        let init = fs::read_to_string(module_dir.join("__init__.py")).expect("read package shim");
        assert!(init.contains("        MermanEngine,\n"));
        assert!(init.contains("        MermanOperationRequest,\n"));
        assert!(init.contains("        MermanOperationResult,\n"));
        assert!(init.contains("    \"ResourceOptionsBuilder\",\n"));
        assert!(init.contains("    \"ResourceProfile\",\n"));
        assert!(init.contains("    \"get_runtime_catalog\",\n"));
        assert_text_measurement_protocol_exports(&init);
        assert_eq!(
            fs::read_to_string(module_dir.join("_text_measurement_protocol.py"))
                .expect("read stable text measurement helper"),
            merman_uniffi::MERMAN_UNIFFI_PYTHON_TEXT_MEASUREMENT_PROTOCOL_MODULE
        );
    }

    #[test]
    fn ensure_init_file_refreshes_stale_generated_shim() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let module_dir = temp.path().join("merman");
        fs::create_dir_all(&module_dir).expect("create module dir");
        fs::write(
            module_dir.join("__init__.py"),
            concat!(
                "\"\"\"Generated merman UniFFI package shim.\"\"\"\n",
                "from .merman_uniffi import (\n",
                "    MermanEngine,\n",
                ")\n\n",
                "__all__ = [\n",
                "    \"MermanEngine\",\n",
                "]\n",
            ),
        )
        .expect("write stale init");

        ensure_init_file(&module_dir).expect("refresh init file");

        let init = fs::read_to_string(module_dir.join("__init__.py")).expect("read init file");
        assert_eq!(init, python_package_init());
        assert!(init.contains("MermanTextMeasureRequest"));
        assert!(init.contains("MermanTextMeasureResult"));
        assert!(init.contains("MermanRuntimeCatalog"));
        assert!(init.contains("ResourceOptionsBuilder"));
        assert_text_measurement_protocol_exports(&init);
        assert_error_contract_exports(&init);
    }

    #[test]
    fn ensure_init_file_preserves_custom_unmarked_shim() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let module_dir = temp.path().join("merman");
        fs::create_dir_all(&module_dir).expect("create module dir");
        let custom = "\"\"\"Custom package code.\"\"\"\nCUSTOM = True\n";
        fs::write(module_dir.join("__init__.py"), custom).expect("write custom init");

        ensure_init_file(&module_dir).expect("preserve custom init file");

        let init = fs::read_to_string(module_dir.join("__init__.py")).expect("read init file");
        assert_eq!(init, custom);
    }

    #[test]
    fn checked_in_python_support_files_match_the_generator() {
        let module_dir = workspace_root()
            .join("platforms")
            .join("python")
            .join("merman")
            .join("src")
            .join("merman");
        let init = fs::read_to_string(module_dir.join("__init__.py")).expect("read package shim");
        assert_eq!(init, python_package_init());
        assert_eq!(
            fs::read_to_string(module_dir.join("_binding_contract.py"))
                .expect("read binding contract helper"),
            PYTHON_BINDING_CONTRACT_MODULE
        );
        assert_eq!(
            fs::read_to_string(module_dir.join("_runtime_catalog.py"))
                .expect("read runtime catalog helper"),
            PYTHON_RUNTIME_CATALOG_MODULE
        );
        assert_eq!(
            fs::read_to_string(module_dir.join("_resource_options.py"))
                .expect("read resource options helper"),
            PYTHON_RESOURCE_OPTIONS_MODULE
        );
        assert_eq!(
            fs::read_to_string(module_dir.join("_text_measurement_protocol.py"))
                .expect("read text measurement helper"),
            merman_uniffi::MERMAN_UNIFFI_PYTHON_TEXT_MEASUREMENT_PROTOCOL_MODULE
        );
        assert_text_measurement_protocol_exports(&init);
        assert_error_contract_exports(&init);
    }
}
