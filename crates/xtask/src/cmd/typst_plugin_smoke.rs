use crate::XtaskError;
use serde_json::Value as JsonValue;
use std::path::{Path, PathBuf};
use wasmi::{Caller, Config, Engine, ExternType, Linker, Module, Store, Val, ValType};

const DEFAULT_SOURCE: &[u8] = b"flowchart TD\nA[Hello] --> B[World]";
const DEFAULT_OPTIONS_JSON: &[u8] =
    br#"{"fixed_today":"2026-06-10","fixed_local_offset_minutes":480}"#;

#[derive(Debug)]
struct TypstPluginSmokeOptions {
    wasm_file: PathBuf,
    source: Vec<u8>,
    options_json: Vec<u8>,
}

#[derive(Debug, Default)]
struct CallData {
    args: Vec<Vec<u8>>,
    output: Vec<u8>,
    memory_error: Option<MemoryError>,
}

#[derive(Debug, Clone, Copy)]
struct MemoryError {
    offset: u32,
    length: u32,
    write: bool,
}

pub(crate) fn typst_plugin_smoke(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let output = call_render_svg_json(&options)?;
    let payload: JsonValue = serde_json::from_slice(&output).map_err(|source| {
        XtaskError::TypstPluginSmokeFailed(format!(
            "render_svg_json returned non-JSON bytes: {source}"
        ))
    })?;

    assert_success_payload(&payload)?;

    let svg_len = payload["svg"].as_str().map(str::len).unwrap_or_default();
    println!(
        "typst-plugin-smoke OK wasm={} output_bytes={} svg_bytes={svg_len}",
        options.wasm_file.display(),
        output.len()
    );
    Ok(())
}

fn parse_options(args: Vec<String>) -> Result<TypstPluginSmokeOptions, XtaskError> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_usage();
        return Err(XtaskError::Usage);
    }

    let mut wasm_file = None;
    let mut source = DEFAULT_SOURCE.to_vec();
    let mut options_json = DEFAULT_OPTIONS_JSON.to_vec();

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--wasm" => {
                wasm_file = Some(PathBuf::from(iter.next().ok_or(XtaskError::Usage)?));
            }
            "--source" => {
                source = iter.next().ok_or(XtaskError::Usage)?.into_bytes();
            }
            "--source-file" => {
                let path = PathBuf::from(iter.next().ok_or(XtaskError::Usage)?);
                source = std::fs::read(&path).map_err(|source| XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                })?;
            }
            "--options-json" => {
                options_json = iter.next().ok_or(XtaskError::Usage)?.into_bytes();
            }
            "--options-json-file" => {
                let path = PathBuf::from(iter.next().ok_or(XtaskError::Usage)?);
                options_json = std::fs::read(&path).map_err(|source| XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                })?;
            }
            _ => {
                print_usage();
                return Err(XtaskError::Usage);
            }
        }
    }

    let wasm_file = wasm_file.ok_or_else(|| {
        print_usage();
        XtaskError::Usage
    })?;

    Ok(TypstPluginSmokeOptions {
        wasm_file,
        source,
        options_json,
    })
}

fn print_usage() {
    println!("usage: xtask typst-plugin-smoke --wasm <plugin.wasm> [options]");
    println!();
    println!("Options:");
    println!("  --source <text>              Mermaid source bytes to pass to render_svg_json");
    println!("  --source-file <path>         Read Mermaid source bytes from a file");
    println!("  --options-json <json>        Options JSON bytes to pass to render_svg_json");
    println!("  --options-json-file <path>   Read options JSON bytes from a file");
}

fn call_render_svg_json(options: &TypstPluginSmokeOptions) -> Result<Vec<u8>, XtaskError> {
    let mut instance = PluginInstance::new(&options.wasm_file)?;
    instance.call(
        "render_svg_json",
        vec![options.source.clone(), options.options_json.clone()],
    )
}

fn assert_success_payload(payload: &JsonValue) -> Result<(), XtaskError> {
    let ok = payload["ok"].as_bool() == Some(true);
    let code_name = payload["code_name"].as_str() == Some("MERMAN_OK");
    let svg = payload["svg"].as_str().unwrap_or_default();
    let has_svg = svg.contains("<svg");

    if ok && code_name && has_svg {
        return Ok(());
    }

    Err(XtaskError::TypstPluginSmokeFailed(format!(
        "render_svg_json returned an unexpected payload: {payload}"
    )))
}

struct PluginInstance {
    instance: wasmi::Instance,
    store: Store<CallData>,
}

impl PluginInstance {
    fn new(wasm_file: &Path) -> Result<Self, XtaskError> {
        let bytes = std::fs::read(wasm_file).map_err(|source| XtaskError::ReadFile {
            path: wasm_file.display().to_string(),
            source,
        })?;

        let mut config = Config::default();
        config.wasm_relaxed_simd(false);

        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes.as_slice()).map_err(|source| {
            XtaskError::TypstPluginSmokeFailed(format!(
                "failed to load WebAssembly module {}: {source}",
                wasm_file.display()
            ))
        })?;

        if !matches!(module.get_export("memory"), Some(ExternType::Memory(_))) {
            return Err(XtaskError::TypstPluginSmokeFailed(
                "plugin does not export memory".to_string(),
            ));
        }

        let mut linker = Linker::new(&engine);
        linker
            .func_wrap(
                "typst_env",
                "wasm_minimal_protocol_send_result_to_host",
                wasm_minimal_protocol_send_result_to_host,
            )
            .map_err(|source| {
                XtaskError::TypstPluginSmokeFailed(format!(
                    "failed to link send_result_to_host: {source}"
                ))
            })?;
        linker
            .func_wrap(
                "typst_env",
                "wasm_minimal_protocol_write_args_to_buffer",
                wasm_minimal_protocol_write_args_to_buffer,
            )
            .map_err(|source| {
                XtaskError::TypstPluginSmokeFailed(format!(
                    "failed to link write_args_to_buffer: {source}"
                ))
            })?;

        let mut store = Store::new(linker.engine(), CallData::default());
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|source| {
                XtaskError::TypstPluginSmokeFailed(format!(
                    "failed to instantiate WebAssembly module: {source}"
                ))
            })?;

        Ok(Self { instance, store })
    }

    fn call(&mut self, name: &str, args: Vec<Vec<u8>>) -> Result<Vec<u8>, XtaskError> {
        let handle = self
            .instance
            .get_export(&self.store, name)
            .ok_or_else(|| {
                XtaskError::TypstPluginSmokeFailed(format!("missing exported function `{name}`"))
            })?
            .into_func()
            .ok_or_else(|| {
                XtaskError::TypstPluginSmokeFailed(format!("export `{name}` is not a function"))
            })?;

        let ty = handle.ty(&self.store);
        if ty.params().iter().any(|&val| val != ValType::I32) {
            return Err(XtaskError::TypstPluginSmokeFailed(format!(
                "plugin function `{name}` has a non-i32 parameter"
            )));
        }
        if ty.results() != [ValType::I32] {
            return Err(XtaskError::TypstPluginSmokeFailed(format!(
                "plugin function `{name}` does not return exactly one i32"
            )));
        }
        if ty.params().len() != args.len() {
            return Err(XtaskError::TypstPluginSmokeFailed(format!(
                "plugin function `{name}` expects {} arguments, got {}",
                ty.params().len(),
                args.len()
            )));
        }

        let lengths = args
            .iter()
            .map(|arg| Val::I32(arg.len() as i32))
            .collect::<Vec<_>>();
        self.store.data_mut().args = args;
        self.store.data_mut().output.clear();
        self.store.data_mut().memory_error = None;

        let mut code = Val::I32(-1);
        handle
            .call(&mut self.store, &lengths, std::slice::from_mut(&mut code))
            .map_err(|source| {
                XtaskError::TypstPluginSmokeFailed(format!("plugin panicked: {source}"))
            })?;

        if let Some(error) = self.store.data_mut().memory_error.take() {
            return Err(XtaskError::TypstPluginSmokeFailed(format!(
                "plugin tried to {} out of bounds at pointer {:#x} with length {}",
                if error.write { "write" } else { "read" },
                error.offset,
                error.length
            )));
        }

        let output = std::mem::take(&mut self.store.data_mut().output);
        match code {
            Val::I32(0) => Ok(output),
            Val::I32(1) => {
                let message = String::from_utf8_lossy(&output);
                Err(XtaskError::TypstPluginSmokeFailed(format!(
                    "plugin returned an error: {message}"
                )))
            }
            _ => Err(XtaskError::TypstPluginSmokeFailed(
                "plugin did not respect the wasm-minimal-protocol return code".to_string(),
            )),
        }
    }
}

fn wasm_minimal_protocol_write_args_to_buffer(mut caller: Caller<CallData>, ptr: u32) {
    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        caller.data_mut().memory_error = Some(MemoryError {
            offset: ptr,
            length: 0,
            write: true,
        });
        return;
    };

    let args = std::mem::take(&mut caller.data_mut().args);
    let mut offset = ptr as usize;
    for arg in args {
        if memory.write(&mut caller, offset, arg.as_slice()).is_err() {
            caller.data_mut().memory_error = Some(MemoryError {
                offset: offset as u32,
                length: arg.len() as u32,
                write: true,
            });
            return;
        }
        offset += arg.len();
    }
}

fn wasm_minimal_protocol_send_result_to_host(mut caller: Caller<CallData>, ptr: u32, len: u32) {
    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        caller.data_mut().memory_error = Some(MemoryError {
            offset: ptr,
            length: len,
            write: false,
        });
        return;
    };

    let mut output = std::mem::take(&mut caller.data_mut().output);
    output.resize(len as usize, 0);
    if memory.read(&caller, ptr as usize, &mut output).is_err() {
        caller.data_mut().memory_error = Some(MemoryError {
            offset: ptr,
            length: len,
            write: false,
        });
        return;
    }
    caller.data_mut().output = output;
}
