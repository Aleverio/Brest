//! Host-side helper: write a sample exported project to `dist/sample` so the
//! generated Rust can be compiled and run without opening FigCraft.
//!
//! ```sh
//! cargo run -p figcraft-rust-export --example generate_sample
//! cargo run --manifest-path figcraft-rust-export/dist/sample/figcraft_sample/Cargo.toml
//! ```

use std::{fs, path::PathBuf};

use figcraft_emit::{Backend, EmitOptions};
use figcraft_import::ImportContext;

const FIXTURE: &str = include_str!("../../fixtures/figcraft-page.json");

fn main() -> Result<(), String> {
    let context = ImportContext {
        page_name: Some("Halaman Contoh".into()),
        selection: Vec::new(),
        code_reference: true,
        assets: Vec::new(),
        limits: figcraft_import::ImportLimits::default(),
    };
    let design = figcraft_import::import(FIXTURE, &context).map_err(|error| error.to_string())?;
    let options = EmitOptions::new("figcraft_sample".into(), Backend::Cpu);
    let project = figcraft_emit::emit(&design, &options).map_err(|error| error.to_string())?;

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist").join("sample");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|error| format!("clean {}: {error}", out.display()))?;
    }
    for entry in &project.files {
        let path = out.join(&entry.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        fs::write(&path, &entry.data).map_err(|error| format!("write {}: {error}", path.display()))?;
    }
    println!("generated {} file(s) in {}", project.files.len(), out.display());
    Ok(())
}
