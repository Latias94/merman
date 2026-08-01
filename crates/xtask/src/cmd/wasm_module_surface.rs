use std::{io, path::PathBuf};
use wasmi::{Config, Engine, ExternType, Module, Mutability, ValType};

const TYPST_PROTOCOL_IMPORTS: &[FunctionContract] = &[
    FunctionContract::import("typst_env", "wasm_minimal_protocol_send_result_to_host", 2),
    FunctionContract::import("typst_env", "wasm_minimal_protocol_write_args_to_buffer", 1),
];
const TYPST_ABI_FUNCTIONS: &[FunctionContract] = &[
    FunctionContract::export("abi_version", 0),
    FunctionContract::export("package_version", 0),
    FunctionContract::export("capabilities_json", 0),
    FunctionContract::export("render_svg_json", 2),
    FunctionContract::export("analyze_json", 2),
];
const TYPST_LINKER_METADATA_EXPORTS: &[&str] = &["__data_end", "__heap_base"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WasmSurfaceProfile {
    PureWasm,
    Typst,
}

#[derive(Debug)]
pub(crate) enum WasmModuleLoadError {
    Read { path: PathBuf, source: io::Error },
    Compile { path: PathBuf, message: String },
}

pub(crate) struct LoadedWasmModule {
    engine: Engine,
    module: Module,
    surface: WasmModuleSurface,
}

#[derive(Debug, Clone)]
pub(crate) struct WasmModuleSurface {
    imports: Vec<WasmImport>,
    exports: Vec<WasmExport>,
}

#[derive(Debug, Clone)]
pub(crate) struct WasmImport {
    module: String,
    name: String,
    ty: ExternType,
}

#[derive(Debug, Clone)]
pub(crate) struct WasmExport {
    name: String,
    ty: ExternType,
}

#[derive(Debug, Clone, Copy)]
struct FunctionContract {
    module: Option<&'static str>,
    name: &'static str,
    parameter_count: usize,
    result_count: usize,
}

impl FunctionContract {
    const fn import(module: &'static str, name: &'static str, parameter_count: usize) -> Self {
        Self {
            module: Some(module),
            name,
            parameter_count,
            result_count: 0,
        }
    }

    const fn export(name: &'static str, parameter_count: usize) -> Self {
        Self {
            module: None,
            name,
            parameter_count,
            result_count: 1,
        }
    }
}

impl LoadedWasmModule {
    pub(crate) fn from_file(path: impl Into<PathBuf>) -> Result<Self, WasmModuleLoadError> {
        let path = path.into();
        let bytes = std::fs::read(&path).map_err(|source| WasmModuleLoadError::Read {
            path: path.clone(),
            source,
        })?;
        let mut config = Config::default();
        config.wasm_relaxed_simd(false);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes.as_slice()).map_err(|source| {
            WasmModuleLoadError::Compile {
                path: path.clone(),
                message: source.to_string(),
            }
        })?;
        let surface = WasmModuleSurface::from_module(&module);
        Ok(Self {
            engine,
            module,
            surface,
        })
    }

    pub(crate) fn engine(&self) -> &Engine {
        &self.engine
    }

    pub(crate) fn module(&self) -> &Module {
        &self.module
    }

    pub(crate) fn surface(&self) -> &WasmModuleSurface {
        &self.surface
    }
}

impl WasmModuleSurface {
    fn from_module(module: &Module) -> Self {
        Self {
            imports: module
                .imports()
                .map(|import| WasmImport {
                    module: import.module().to_string(),
                    name: import.name().to_string(),
                    ty: import.ty().clone(),
                })
                .collect(),
            exports: module
                .exports()
                .map(|export| WasmExport {
                    name: export.name().to_string(),
                    ty: export.ty().clone(),
                })
                .collect(),
        }
    }

    pub(crate) fn imports(&self) -> &[WasmImport] {
        &self.imports
    }

    pub(crate) fn exports(&self) -> &[WasmExport] {
        &self.exports
    }

    pub(crate) fn validate_imports(&self, profile: WasmSurfaceProfile) -> Vec<String> {
        match profile {
            WasmSurfaceProfile::PureWasm => self
                .imports
                .iter()
                .map(|import| {
                    format!(
                        "pure-wasm profile forbids import {}::{} ({:?})",
                        import.module, import.name, import.ty
                    )
                })
                .collect(),
            WasmSurfaceProfile::Typst => self.validate_typst_imports(),
        }
    }

    pub(crate) fn validate_exports(&self, profile: WasmSurfaceProfile) -> Vec<String> {
        match profile {
            WasmSurfaceProfile::PureWasm => Vec::new(),
            WasmSurfaceProfile::Typst => self.validate_typst_exports(),
        }
    }

    fn validate_typst_imports(&self) -> Vec<String> {
        let mut failures = Vec::new();
        for contract in TYPST_PROTOCOL_IMPORTS {
            let module = contract
                .module
                .expect("Typst protocol import contracts declare a module");
            let matching = self
                .imports
                .iter()
                .filter(|import| import.module == module && import.name == contract.name)
                .collect::<Vec<_>>();
            match matching.as_slice() {
                [] => failures.push(format!("missing import {module}::{}", contract.name)),
                [import] => check_i32_function_type(
                    &import.ty,
                    contract.parameter_count,
                    contract.result_count,
                    &format!("import {module}::{}", contract.name),
                    &mut failures,
                ),
                imports => failures.push(format!(
                    "expected exactly one import {module}::{}, found {}",
                    contract.name,
                    imports.len()
                )),
            }
        }
        for import in self.imports.iter().filter(|import| {
            !TYPST_PROTOCOL_IMPORTS.iter().any(|contract| {
                contract.module == Some(import.module.as_str()) && contract.name == import.name
            })
        }) {
            failures.push(format!(
                "forbidden extra import {}::{} ({:?})",
                import.module, import.name, import.ty
            ));
        }
        failures
    }

    fn validate_typst_exports(&self) -> Vec<String> {
        let mut failures = Vec::new();
        let memory = self
            .exports
            .iter()
            .filter(|export| export.name == "memory")
            .collect::<Vec<_>>();
        match memory.as_slice() {
            [] => failures.push("missing memory export `memory`".to_string()),
            [export] if matches!(export.ty, ExternType::Memory(_)) => {}
            [export] => failures.push(format!(
                "export `memory` must be a memory, found {:?}",
                export.ty
            )),
            exports => failures.push(format!(
                "expected exactly one memory export `memory`, found {}",
                exports.len()
            )),
        }

        for contract in TYPST_ABI_FUNCTIONS {
            let matching = self
                .exports
                .iter()
                .filter(|export| export.name == contract.name)
                .collect::<Vec<_>>();
            match matching.as_slice() {
                [] => failures.push(format!(
                    "missing Typst ABI function export `{}`",
                    contract.name
                )),
                [export] => check_i32_function_type(
                    &export.ty,
                    contract.parameter_count,
                    contract.result_count,
                    &format!("export `{}`", contract.name),
                    &mut failures,
                ),
                exports => failures.push(format!(
                    "expected exactly one Typst ABI function export `{}`, found {}",
                    contract.name,
                    exports.len()
                )),
            }
        }

        for name in TYPST_LINKER_METADATA_EXPORTS {
            let matching = self
                .exports
                .iter()
                .filter(|export| export.name == *name)
                .collect::<Vec<_>>();
            match matching.as_slice() {
                [] => failures.push(format!("missing linker metadata export `{name}`")),
                [export] => check_linker_metadata_global_type(
                    &export.ty,
                    &format!("export `{name}`"),
                    &mut failures,
                ),
                exports => failures.push(format!(
                    "expected exactly one linker metadata export `{name}`, found {}",
                    exports.len()
                )),
            }
        }

        for export in self.exports.iter().filter(|export| {
            export.name != "memory"
                && !TYPST_ABI_FUNCTIONS
                    .iter()
                    .any(|contract| export.name == contract.name)
                && !TYPST_LINKER_METADATA_EXPORTS.contains(&export.name.as_str())
        }) {
            failures.push(format!(
                "forbidden extra export `{}` ({:?})",
                export.name, export.ty
            ));
        }
        failures
    }
}

impl WasmImport {
    pub(crate) fn module(&self) -> &str {
        &self.module
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

impl WasmExport {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn ty(&self) -> &ExternType {
        &self.ty
    }
}

fn check_i32_function_type(
    ty: &ExternType,
    parameter_count: usize,
    result_count: usize,
    context: &str,
    failures: &mut Vec<String>,
) {
    let Some(function) = ty.func() else {
        failures.push(format!("{context} must be a function, found {ty:?}"));
        return;
    };
    if function.params().len() != parameter_count
        || function.params().iter().any(|ty| *ty != ValType::I32)
        || function.results().len() != result_count
        || function.results().iter().any(|ty| *ty != ValType::I32)
    {
        failures.push(format!(
            "{context} must have signature ({}) -> ({}), found {:?} -> {:?}",
            vec!["i32"; parameter_count].join(", "),
            vec!["i32"; result_count].join(", "),
            function.params(),
            function.results()
        ));
    }
}

fn check_linker_metadata_global_type(ty: &ExternType, context: &str, failures: &mut Vec<String>) {
    let Some(global) = ty.global() else {
        failures.push(format!(
            "{context} must be an immutable i32 global, found {ty:?}"
        ));
        return;
    };
    if global.content() != ValType::I32 || global.mutability() != Mutability::Const {
        failures.push(format!(
            "{context} must be an immutable i32 global, found {:?} {:?}",
            global.mutability(),
            global.content()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmi::{FuncType, GlobalType, MemoryType};

    #[test]
    fn typst_profile_accepts_the_closed_typed_surface() {
        let surface = valid_typst_surface();

        assert!(
            surface
                .validate_imports(WasmSurfaceProfile::Typst)
                .is_empty()
        );
        assert!(
            surface
                .validate_exports(WasmSurfaceProfile::Typst)
                .is_empty()
        );
    }

    #[test]
    fn pure_profile_rejects_every_import() {
        let surface = WasmModuleSurface {
            imports: vec![WasmImport {
                module: "env".to_string(),
                name: "callback".to_string(),
                ty: function_type(0, 0),
            }],
            exports: Vec::new(),
        };

        let failures = surface.validate_imports(WasmSurfaceProfile::PureWasm);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("env::callback"));
    }

    #[test]
    fn pure_profile_accepts_an_import_free_surface() {
        let surface = WasmModuleSurface {
            imports: Vec::new(),
            exports: Vec::new(),
        };

        assert!(
            surface
                .validate_imports(WasmSurfaceProfile::PureWasm)
                .is_empty()
        );
    }

    #[test]
    fn typst_profile_rejects_missing_extra_and_mistyped_items() {
        let mut surface = valid_typst_surface();
        surface.imports.remove(0);
        surface.imports.push(WasmImport {
            module: "wasi_snapshot_preview1".to_string(),
            name: "fd_write".to_string(),
            ty: function_type(0, 0),
        });
        surface
            .exports
            .iter_mut()
            .find(|export| export.name == "analyze_json")
            .expect("analyze_json export")
            .ty = function_type(1, 0);
        surface.exports.push(WasmExport {
            name: "browser_render".to_string(),
            ty: function_type(0, 1),
        });
        surface
            .exports
            .iter_mut()
            .find(|export| export.name == "__heap_base")
            .expect("heap base export")
            .ty = ExternType::Global(GlobalType::new(ValType::I32, Mutability::Var));

        let import_failures = surface.validate_imports(WasmSurfaceProfile::Typst);
        let export_failures = surface.validate_exports(WasmSurfaceProfile::Typst);
        assert!(
            import_failures
                .iter()
                .any(|failure| failure.contains("send_result_to_host"))
        );
        assert!(
            import_failures
                .iter()
                .any(|failure| failure.contains("wasi_snapshot_preview1"))
        );
        assert!(
            export_failures
                .iter()
                .any(|failure| failure.contains("analyze_json") && failure.contains("signature"))
        );
        assert!(
            export_failures
                .iter()
                .any(|failure| failure.contains("browser_render"))
        );
        assert!(
            export_failures
                .iter()
                .any(|failure| failure.contains("__heap_base") && failure.contains("immutable"))
        );
    }

    fn valid_typst_surface() -> WasmModuleSurface {
        WasmModuleSurface {
            imports: vec![
                WasmImport {
                    module: "typst_env".to_string(),
                    name: "wasm_minimal_protocol_send_result_to_host".to_string(),
                    ty: function_type(2, 0),
                },
                WasmImport {
                    module: "typst_env".to_string(),
                    name: "wasm_minimal_protocol_write_args_to_buffer".to_string(),
                    ty: function_type(1, 0),
                },
            ],
            exports: vec![
                WasmExport {
                    name: "memory".to_string(),
                    ty: ExternType::Memory(MemoryType::new(1, None)),
                },
                WasmExport {
                    name: "abi_version".to_string(),
                    ty: function_type(0, 1),
                },
                WasmExport {
                    name: "package_version".to_string(),
                    ty: function_type(0, 1),
                },
                WasmExport {
                    name: "capabilities_json".to_string(),
                    ty: function_type(0, 1),
                },
                WasmExport {
                    name: "render_svg_json".to_string(),
                    ty: function_type(2, 1),
                },
                WasmExport {
                    name: "analyze_json".to_string(),
                    ty: function_type(2, 1),
                },
                WasmExport {
                    name: "__data_end".to_string(),
                    ty: immutable_i32_global(),
                },
                WasmExport {
                    name: "__heap_base".to_string(),
                    ty: immutable_i32_global(),
                },
            ],
        }
    }

    fn function_type(parameters: usize, results: usize) -> ExternType {
        ExternType::Func(FuncType::new(
            vec![ValType::I32; parameters],
            vec![ValType::I32; results],
        ))
    }

    fn immutable_i32_global() -> ExternType {
        ExternType::Global(GlobalType::new(ValType::I32, Mutability::Const))
    }
}
