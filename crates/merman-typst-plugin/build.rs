fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("TARGET").as_deref() == Ok("wasm32-unknown-unknown") {
        println!("cargo:rustc-link-arg-cdylib=--export=__data_end");
        println!("cargo:rustc-link-arg-cdylib=--export=__heap_base");
    }
}
