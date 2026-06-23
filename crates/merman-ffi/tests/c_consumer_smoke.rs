use libloading::Library;
use merman_ffi::{MermanBuffer, MermanResult};
use std::ffi::c_char;
use std::path::{Path, PathBuf};
use std::process::Command;

#[repr(C)]
struct MermanApi {
    render_enabled: i32,
    ascii_enabled: i32,
    abi_version: extern "C" fn() -> u32,
    package_version: extern "C" fn() -> *const c_char,
    buffer_struct_size: extern "C" fn() -> usize,
    result_struct_size: extern "C" fn() -> usize,
    engine_result_struct_size: extern "C" fn() -> usize,
    host_text_measure_request_struct_size: extern "C" fn() -> usize,
    host_text_measure_result_struct_size: extern "C" fn() -> usize,
    engine_new: unsafe extern "C" fn(*const u8, usize) -> merman_ffi::MermanEngineResult,
    engine_free: unsafe extern "C" fn(*mut merman_ffi::MermanEngine),
    engine_set_text_measure_callback: unsafe extern "C" fn(
        *mut merman_ffi::MermanEngine,
        Option<merman_ffi::MermanHostTextMeasureCallback>,
        *mut std::ffi::c_void,
    ) -> MermanResult,
    engine_render_svg:
        unsafe extern "C" fn(*const merman_ffi::MermanEngine, *const u8, usize) -> MermanResult,
    engine_render_ascii:
        unsafe extern "C" fn(*const merman_ffi::MermanEngine, *const u8, usize) -> MermanResult,
    engine_analyze_json:
        unsafe extern "C" fn(*const merman_ffi::MermanEngine, *const u8, usize) -> MermanResult,
    engine_parse_json:
        unsafe extern "C" fn(*const merman_ffi::MermanEngine, *const u8, usize) -> MermanResult,
    engine_layout_json:
        unsafe extern "C" fn(*const merman_ffi::MermanEngine, *const u8, usize) -> MermanResult,
    engine_validate_json:
        unsafe extern "C" fn(*const merman_ffi::MermanEngine, *const u8, usize) -> MermanResult,
    render_svg: unsafe extern "C" fn(*const u8, usize, *const u8, usize) -> MermanResult,
    render_ascii: unsafe extern "C" fn(*const u8, usize, *const u8, usize) -> MermanResult,
    parse_json: unsafe extern "C" fn(*const u8, usize, *const u8, usize) -> MermanResult,
    layout_json: unsafe extern "C" fn(*const u8, usize, *const u8, usize) -> MermanResult,
    validate_json: unsafe extern "C" fn(*const u8, usize, *const u8, usize) -> MermanResult,
    supported_diagrams_json: extern "C" fn() -> MermanResult,
    ascii_supported_diagrams_json: extern "C" fn() -> MermanResult,
    diagram_family_capabilities_json: extern "C" fn() -> MermanResult,
    supported_themes_json: extern "C" fn() -> MermanResult,
    supported_host_theme_presets_json: extern "C" fn() -> MermanResult,
    buffer_free: unsafe extern "C" fn(MermanBuffer),
}

#[test]
fn c_consumer_smoke() {
    let library_path = compile_c_consumer();

    unsafe {
        let library = Library::new(&library_path).unwrap_or_else(|err| {
            panic!(
                "failed to load C consumer smoke library {}: {err}",
                library_path.display()
            )
        });
        let smoke: libloading::Symbol<unsafe extern "C" fn(MermanApi) -> i32> = library
            .get(b"merman_c_consumer_smoke")
            .expect("load merman_c_consumer_smoke symbol");

        let rc = smoke(MermanApi {
            render_enabled: i32::from(cfg!(feature = "render")),
            ascii_enabled: i32::from(cfg!(feature = "ascii")),
            abi_version: merman_ffi::merman_abi_version,
            package_version: merman_ffi::merman_package_version,
            buffer_struct_size: merman_ffi::merman_buffer_struct_size,
            result_struct_size: merman_ffi::merman_result_struct_size,
            engine_result_struct_size: merman_ffi::merman_engine_result_struct_size,
            host_text_measure_request_struct_size:
                merman_ffi::merman_host_text_measure_request_struct_size,
            host_text_measure_result_struct_size:
                merman_ffi::merman_host_text_measure_result_struct_size,
            engine_new: merman_ffi::merman_engine_new,
            engine_free: merman_ffi::merman_engine_free,
            engine_set_text_measure_callback: merman_ffi::merman_engine_set_text_measure_callback,
            engine_render_svg: merman_ffi::merman_engine_render_svg,
            engine_render_ascii: merman_ffi::merman_engine_render_ascii,
            engine_analyze_json: merman_ffi::merman_engine_analyze_json,
            engine_parse_json: merman_ffi::merman_engine_parse_json,
            engine_layout_json: merman_ffi::merman_engine_layout_json,
            engine_validate_json: merman_ffi::merman_engine_validate_json,
            render_svg: merman_ffi::merman_render_svg,
            render_ascii: merman_ffi::merman_render_ascii,
            analyze_json: merman_ffi::merman_analyze_json,
            parse_json: merman_ffi::merman_parse_json,
            layout_json: merman_ffi::merman_layout_json,
            validate_json: merman_ffi::merman_validate_json,
            supported_diagrams_json: merman_ffi::merman_supported_diagrams_json,
            ascii_supported_diagrams_json: merman_ffi::merman_ascii_supported_diagrams_json,
            diagram_family_capabilities_json: merman_ffi::merman_diagram_family_capabilities_json,
            supported_themes_json: merman_ffi::merman_supported_themes_json,
            supported_host_theme_presets_json: merman_ffi::merman_supported_host_theme_presets_json,
            buffer_free: merman_ffi::merman_buffer_free,
        });
        assert_eq!(rc, 0, "C consumer smoke returned {rc}");
    }
}

fn compile_c_consumer() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out_dir = std::env::temp_dir().join(format!(
        "merman-ffi-c-consumer-smoke-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&out_dir).expect("create C consumer smoke output directory");

    let source = manifest_dir.join("tests/c_consumer_smoke.c");
    let include_dir = manifest_dir.join("include");
    let library_path = out_dir.join(shared_library_name("merman_c_consumer_smoke"));
    let mut build = cc::Build::new();
    let target = current_target();
    build.opt_level(0).target(target).host(target);
    let compiler = build.get_compiler();
    let mut command = compiler.to_command();

    if compiler.is_like_msvc() {
        command
            .arg("/LD")
            .arg("/nologo")
            .arg(format!("/I{}", include_dir.display()))
            .arg(format!("/Fe:{}", library_path.display()))
            .arg(format!(
                "/Fo:{}",
                out_dir.join("merman_c_consumer_smoke.obj").display()
            ))
            .arg(&source);
    } else {
        command
            .arg("-shared")
            .arg("-fPIC")
            .arg("-I")
            .arg(&include_dir)
            .arg(&source)
            .arg("-o")
            .arg(&library_path);
    }

    run_compile_command(command, &library_path);
    library_path
}

fn shared_library_name(stem: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

fn current_target() -> &'static str {
    if cfg!(all(
        target_arch = "x86_64",
        target_os = "windows",
        target_env = "msvc"
    )) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(
        target_arch = "x86_64",
        target_os = "windows",
        target_env = "gnu"
    )) {
        "x86_64-pc-windows-gnu"
    } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
        "aarch64-apple-darwin"
    } else {
        panic!("unsupported C consumer smoke target");
    }
}

fn run_compile_command(mut command: Command, library_path: &Path) {
    let output = command
        .output()
        .unwrap_or_else(|err| panic!("failed to run C compiler: {err}"));
    if !output.status.success() {
        panic!(
            "failed to compile C consumer smoke library {}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            library_path.display(),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
