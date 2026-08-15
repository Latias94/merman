use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn generate_query_profiles(manifest_directory: &Path) {
    let query_directory = manifest_directory.join("queries");
    let mut profiles = Vec::new();

    for profile_entry in std::fs::read_dir(&query_directory).expect("read query profiles") {
        let profile_entry = profile_entry.expect("inspect query profile");
        if !profile_entry.path().is_dir() {
            continue;
        }
        let profile = profile_entry.file_name().to_string_lossy().into_owned();
        for surface_entry in std::fs::read_dir(profile_entry.path()).expect("read query surfaces") {
            let surface_entry = surface_entry.expect("inspect query surface");
            let path = surface_entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("scm") {
                continue;
            }
            let surface = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("query surface name must be UTF-8")
                .to_string();
            let relative = PathBuf::from("queries")
                .join(&profile)
                .join(format!("{surface}.scm"));
            profiles.push((profile.clone(), surface, relative));
        }
    }
    profiles.sort();
    assert!(!profiles.is_empty(), "package must contain query profiles");

    let mut generated = String::from("pub static QUERY_PROFILES: &[QueryProfile] = &[\n");
    for (profile, surface, relative) in profiles {
        let relative = relative.to_string_lossy().replace('\\', "/");
        writeln!(
            generated,
            "    QueryProfile {{ profile: {profile:?}, surface: {surface:?}, path: {relative:?}, source: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{relative}\")) }},"
        )
        .expect("write generated query profile");
    }
    generated.push_str("];\n");

    let output =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("query_profiles.rs");
    std::fs::write(output, generated).expect("write generated query profiles");
    println!("cargo:rerun-if-changed={}", query_directory.display());
}

fn main() {
    let manifest_directory = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_directory = manifest_directory.join("src");
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
    generate_query_profiles(manifest_directory);
}
