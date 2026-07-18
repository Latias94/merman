use merman_fixture_render_context::{MANIFEST_RELATIVE_PATH, RenderContextCatalog};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn fixture_paths(root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                if !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('_'))
                {
                    directories.push(path);
                }
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("mmd") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path
        .parent()
        .ok_or("render context manifest has no parent")?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("render context manifest name is not UTF-8")?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let backup = parent.join(format!(".{file_name}.{}.backup", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);

    let had_original = path.is_file();
    if had_original {
        fs::rename(path, &backup)?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if had_original {
            fs::rename(&backup, path)?;
        }
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    if had_original {
        fs::remove_file(backup)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures_root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("fixtures")
        });
    let mut catalog = RenderContextCatalog::rebuild(&fixtures_root)?;
    for path in fixture_paths(&fixtures_root)? {
        let relative = path
            .strip_prefix(&fixtures_root)?
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read(&path)?;
        catalog.upsert_from_source(&relative, &source)?;
    }
    let manifest_path = fixtures_root.join(MANIFEST_RELATIVE_PATH);
    atomic_write(&manifest_path, catalog.to_json()?.as_bytes())?;
    println!(
        "wrote {} fixture render contexts to {}",
        catalog.contexts().count(),
        manifest_path.display()
    );
    Ok(())
}
