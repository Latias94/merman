fn main() {
    let source_directory = std::path::Path::new("src");
    let parser = source_directory.join("parser.c");
    let scanner = source_directory.join("scanner.c");

    let mut build = cc::Build::new();
    build
        .std("c11")
        .include(source_directory)
        .file(&parser)
        .flag_if_supported("-Wno-unused-value");

    if scanner.exists() {
        build.file(&scanner);
    }

    #[cfg(target_env = "msvc")]
    build.flag("-utf-8");

    println!("cargo:rerun-if-changed={}", parser.display());
    println!("cargo:rerun-if-changed={}", scanner.display());
    build.compile("tree-sitter-mermaid");
}
