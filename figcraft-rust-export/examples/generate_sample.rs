//! Host-side helper: write a sample exported project to `dist/sample` so the
//! generated Rust can be compiled and run without opening FigCraft.
//!
//! ```sh
//! cargo run -p figcraft-rust-export --example generate_sample
//! cargo run --manifest-path figcraft-rust-export/dist/sample/Cargo.toml
//! ```

use std::{fs, path::PathBuf};

use figcraft_rust_export::{
    codegen::{self, Options},
    model::{Align, Axis, Kind, Node, Rect, Rgba, Style, scene},
};

fn main() -> Result<(), String> {
    let card = Node {
        id: 1,
        name: "Card".to_string(),
        rect: Rect { x: 0.0, y: 0.0, width: 320.0, height: 180.0 },
        style: Style {
            fill: Some(Rgba { r: 245, g: 247, b: 250, a: 255 }),
            stroke: Some(Rgba { r: 200, g: 210, b: 220, a: 255 }),
            stroke_width: 1.0,
            radius: 16.0,
            opacity: 1.0,
        },
        kind: Kind::Frame { layout: Axis::Vertical, gap: 12.0, padding: 20.0 },
        visible: true,
        children: vec![
            Node {
                id: 2,
                name: "Title".to_string(),
                rect: Rect { x: 20.0, y: 20.0, width: 240.0, height: 28.0 },
                style: Style::default(),
                kind: Kind::Text {
                    text: "Halo {{name}}".to_string(),
                    font_size: 20.0,
                    bold: true,
                    color: Rgba { r: 20, g: 24, b: 30, a: 255 },
                    align: Align::Start,
                },
                visible: true,
                children: Vec::new(),
            },
            Node {
                id: 3,
                name: "Badge".to_string(),
                rect: Rect { x: 20.0, y: 60.0, width: 96.0, height: 32.0 },
                style: Style {
                    fill: Some(Rgba { r: 40, g: 110, b: 240, a: 255 }),
                    radius: 16.0,
                    ..Style::default()
                },
                kind: Kind::Ellipse,
                visible: true,
                children: Vec::new(),
            },
        ],
    };

    let scene = scene("Contoh Kartu".to_string(), vec![card]);
    let files = codegen::project(
        &scene,
        &Options { crate_name: "figcraft_sample".to_string(), backend: "cpu".to_string() },
    )?;

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("dist").join("sample");
    for entry in files {
        let path = out.join(&entry.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        fs::write(&path, entry.data).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    println!("generated {}", out.display());
    Ok(())
}
