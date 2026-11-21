use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if !cfg!(feature = "tree-sitter") {
        return;
    }

    // The generated parser sizes the mask that it passes to the same C scanner.
    let parser_path = "../distribution/tree-sitter-mermaid/src/parser.c";
    println!("cargo:rerun-if-changed={parser_path}");
    let parser = File::open(parser_path).expect("open generated Tree-sitter parser");
    let token_count: usize = BufReader::new(parser)
        .lines()
        .find_map(|line| {
            let line = line.expect("read generated Tree-sitter parser");
            line.strip_prefix("#define EXTERNAL_TOKEN_COUNT ")
                .map(|count| count.trim().parse().expect("parse external token count"))
        })
        .expect("generated parser defines EXTERNAL_TOKEN_COUNT");
    assert!(
        token_count <= u32::BITS as usize,
        "the scanner fuzz seed format has a 32-bit token mask"
    );

    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    fs::write(
        output.join("scanner_tokens.rs"),
        format!("const TOKEN_COUNT: usize = {token_count};\n"),
    )
    .expect("write scanner token count");
}
