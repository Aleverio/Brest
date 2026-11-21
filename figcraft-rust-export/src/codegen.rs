//! Turn the IR into a small Rust Cargo project (or a single Rust file).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use figcraft_plugin_sdk::Value;

use crate::model::{Scene, num, rust_string};
use crate::zip::Entry;

const CARGO: &str = include_str!("../templates/Cargo.toml.txt");
const PROJECT_README: &str = include_str!("../templates/README.md.txt");
const MAIN: &str = include_str!("../templates/src_main.rs.txt");
const COMPONENT_HEADER: &str = include_str!("../templates/src_component_header.rs.txt");
const COMPONENT_FOOTER: &str = include_str!("../templates/src_component_footer.rs.txt");
const CALLER: &str = include_str!("../templates/src_caller.rs.txt");
const RENDERER_MOD: &str = include_str!("../templates/src_renderer_mod.rs.txt");
const RENDERER_PIXELS: &str = include_str!("../templates/src_renderer_pixels.rs.txt");
const RENDERER_FONT: &str = include_str!("../templates/src_renderer_font.rs.txt");
const RENDERER_CPU: &str = include_str!("../templates/src_renderer_cpu.rs.txt");
const RENDERER_SKIA: &str = include_str!("../templates/src_renderer_skia.rs.txt");
const RENDERER_GPU: &str = include_str!("../templates/src_renderer_gpu.rs.txt");

/// Export options coming from the plugin's parameters or panel.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub crate_name: String,
    pub backend: String,
}

impl Options {
    pub fn from_params(params: &Value) -> Self {
        Self {
            crate_name: crate_name(params.get("crate").and_then(Value::as_str).unwrap_or("figcraft_design")),
            backend: backend(params.get("backend").and_then(Value::as_str).unwrap_or("Skia")),
        }
    }
}

/// Accept the three names used in the UI, plus their lowercase ids.
pub fn backend(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "cpu" | "software" => "cpu".to_string(),
        "gpu" | "wgpu" => "gpu".to_string(),
        _ => "skia".to_string(),
    }
}

/// Turn any user text into a safe Rust crate name.
pub fn crate_name(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    while out.starts_with('_') || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.remove(0);
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("figcraft_design");
    }
    out
}

fn inject(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut text = template.to_string();
    for (key, value) in pairs {
        text = text.replace(key, value);
    }
    text
}

fn nodes_source(scene: &Scene) -> String {
    let mut out = String::new();
    for (i, node) in scene.roots.iter().enumerate() {
        node.literal("        ", &mut out);
        if i + 1 < scene.roots.len() {
            out.push_str(",\n");
        }
    }
    out
}

/// All three backends are always selectable in the generated project; only the
/// default changes with the export. Optional dependencies are fetched only when
/// their feature is enabled.
fn extra_deps() -> (&'static str, &'static str) {
    (
        "skia = [\"dep:skia-safe\"]\ngpu = [\"dep:wgpu\", \"dep:pollster\"]\n",
        "\n[dependencies]\nskia-safe = { version = \"0.81\", optional = true }\nwgpu = { version = \"0.19\", optional = true }\npollster = { version = \"0.3\", optional = true }\n",
    )
}

/// All files of the generated Cargo project, as ZIP entries.
pub fn project(scene: &Scene, options: &Options) -> Result<Vec<Entry>, String> {
    if scene.roots.is_empty() {
        return Err("the design has no layers to export".to_string());
    }
    let backend = options.backend.as_str();
    if scene.count > crate::model::MAX_NODES {
        return Err(format!("the design has more than {} layers", crate::model::MAX_NODES));
    }
    let (features, deps) = extra_deps();
    let name = &options.crate_name;
    let nodes = nodes_source(scene);
    let component = format!(
        "{}{}",
        COMPONENT_HEADER,
        inject(
            COMPONENT_FOOTER,
            &[
                ("{{SCENE_NAME}}", &rust_string(&scene.name)),
                ("{{WIDTH}}", &num(scene.width)),
                ("{{HEIGHT}}", &num(scene.height)),
                ("{{NODES}}", &nodes),
            ]
        )
    );

    let cargo = inject(CARGO, &[("{{CRATE}}", name), ("{{BACKEND}}", backend), ("{{EXTRA_FEATURES}}", features), ("{{EXTRA_DEPS}}", deps)]);
    let readme = inject(
        PROJECT_README,
        &[
            ("{{CRATE}}", name),
            ("{{BACKEND}}", backend),
            ("{{SCENE_NAME}}", &scene.name),
            ("{{NODE_COUNT}}", &scene.count.to_string()),
        ],
    );
    let main = inject(MAIN, &[("{{BACKEND}}", backend), ("{{CRATE}}", name)]);

    Ok(vec![
        Entry::new(&format!("{name}/Cargo.toml"), cargo.into_bytes()),
        Entry::new(&format!("{name}/README.md"), readme.into_bytes()),
        Entry::new(&format!("{name}/src/main.rs"), main.into_bytes()),
        Entry::new(&format!("{name}/src/component.rs"), component.into_bytes()),
        Entry::new(&format!("{name}/src/caller.rs"), CALLER.to_string().into_bytes()),
        Entry::new(&format!("{name}/src/renderer/mod.rs"), RENDERER_MOD.to_string().into_bytes()),
        Entry::new(&format!("{name}/src/renderer/pixels.rs"), RENDERER_PIXELS.to_string().into_bytes()),
        Entry::new(&format!("{name}/src/renderer/font.rs"), RENDERER_FONT.to_string().into_bytes()),
        Entry::new(&format!("{name}/src/renderer/cpu.rs"), RENDERER_CPU.to_string().into_bytes()),
        Entry::new(&format!("{name}/src/renderer/skia.rs"), RENDERER_SKIA.to_string().into_bytes()),
        Entry::new(&format!("{name}/src/renderer/gpu.rs"), RENDERER_GPU.to_string().into_bytes()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, Node, Rect, Style};

    fn sample() -> Scene {
        Scene {
            name: "Login screen".to_string(),
            width: 320.0,
            height: 200.0,
            count: 2,
            bindings: vec!["title".to_string()],
            roots: vec![Node {
                id: 1,
                name: "Card".to_string(),
                rect: Rect { x: 0.0, y: 0.0, width: 320.0, height: 200.0 },
                style: Style { radius: 12.0, ..Style::default() },
                kind: Kind::Frame { layout: crate::model::Axis::Vertical, gap: 8.0, padding: 16.0 },
                visible: true,
                children: vec![Node {
                    id: 2,
                    name: "Title".to_string(),
                    rect: Rect { x: 16.0, y: 16.0, width: 120.0, height: 20.0 },
                    style: Style::default(),
                    kind: Kind::Text {
                        text: "Hello {{title}}".to_string(),
                        font_size: 16.0,
                        bold: true,
                        color: crate::model::Rgba { r: 0, g: 0, b: 0, a: 255 },
                        align: crate::model::Align::Start,
                    },
                    visible: true,
                    children: Vec::new(),
                }],
            }],
        }
    }

    #[test]
    fn crate_names_are_sanitized() {
        assert_eq!(crate_name("My Cool-Design!"), "my_cool_design");
        assert_eq!(crate_name("9lives"), "lives");
        assert_eq!(crate_name(""), "figcraft_design");
        assert_eq!(backend("GPU"), "gpu");
    }

    #[test]
    fn project_has_all_three_modules() {
        let files = project(&sample(), &Options { crate_name: "demo".into(), backend: "cpu".into() }).unwrap_or_default();
        assert_eq!(files.len(), 11);
        let component = files.iter().find(|f| f.path.ends_with("component.rs")).map(|f| String::from_utf8_lossy(&f.data).into_owned()).unwrap_or_default();
        assert!(component.contains("Hello {{title}}"));
        assert!(component.contains("Kind::Frame"));
        assert!(files.iter().any(|f| f.path.ends_with("caller.rs")));
        assert!(files.iter().any(|f| f.path.ends_with("renderer/gpu.rs")));
    }
}
