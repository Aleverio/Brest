//! Emit a standalone Rust Cargo project from the versioned IR.
//!
//! Every generated file comes from a checked-in template or from a small,
//! typed formatter in this crate. The plugin never writes Rust source by hand.

#![cfg_attr(test, allow(clippy::expect_used))]

use figcraft_ir::{
    Align, Asset, Axis, Color, Design, FrameSpec, Kind, Node, Paint, Rect, ShapeKind, Style,
    TextAlignVertical, TextSpec, WidgetKind, WidgetSpec,
};
use std::fmt;

const CARGO: &str = include_str!("../../../figcraft-rust-export/templates/v2/Cargo.toml.txt");
const README: &str = include_str!("../../../figcraft-rust-export/templates/v2/README.md.txt");
const GENERATED: &str = include_str!("../../../figcraft-rust-export/templates/v2/GENERATED.md.txt");
const SYNC: &str = include_str!("../../../figcraft-rust-export/templates/v2/sync.ps1.txt");
const MAIN: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_main.rs.txt");
const CORE: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_core.rs.txt");
const APP: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_app.rs.txt");
const COMPONENT_MOD: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_component_mod.rs.txt");
const COMPONENT_STATE: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_component_state.rs.txt");
const COMPONENT_TREE: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_component_tree.rs.txt");
const CALLER_MOD: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_caller_mod.rs.txt");
const CALLER_LAYOUT: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_caller_layout.rs.txt");
const CALLER_RESOLVE: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_caller_resolve.rs.txt");
const RENDERER_MOD: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_renderer_mod.rs.txt");
const RENDERER_PIXELS: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_renderer_pixels.rs.txt");
const RENDERER_FONT: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_renderer_font.rs.txt");
const RENDERER_PNG: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_renderer_png.rs.txt");
const RENDERER_CPU: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_renderer_cpu.rs.txt");
const RENDERER_SKIA: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_renderer_skia.rs.txt");
const RENDERER_GPU: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_renderer_gpu.rs.txt");
const USER_MOD: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_user_mod.rs.txt");
const USER_HANDLERS: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_user_handlers.rs.txt");
const TESTS_MOD: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_tests_mod.rs.txt");
const TESTS_LAYOUT: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_tests_layout.rs.txt");
const TESTS_RENDER: &str = include_str!("../../../figcraft-rust-export/templates/v2/src_tests_render.rs.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Cpu,
    Skia,
    Gpu,
}

impl Backend {
    pub fn parse(value: &str) -> Result<Self, EmitError> {
        match value.to_ascii_lowercase().as_str() {
            "cpu" | "software" => Ok(Self::Cpu),
            "skia" => Ok(Self::Skia),
            "gpu" | "wgpu" => Ok(Self::Gpu),
            other => Err(EmitError::InvalidOption(format!("unknown backend '{other}'"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Skia => "skia",
            Self::Gpu => "gpu",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmitOptions {
    pub crate_name: String,
    pub default_backend: Backend,
    pub include_reference_css: bool,
}

impl EmitOptions {
    pub fn new(crate_name: String, default_backend: Backend) -> Self {
        Self {
            crate_name: sanitize_crate_name(&crate_name),
            default_backend,
            include_reference_css: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileOwner {
    Generated,
    User,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedFile {
    pub path: String,
    pub data: Vec<u8>,
    pub owner: FileOwner,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectManifest {
    pub generator: String,
    pub generator_version: String,
    pub ir_version: u16,
    pub crate_name: String,
    pub default_backend: String,
    pub file_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedProject {
    pub files: Vec<GeneratedFile>,
    pub manifest: ProjectManifest,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EmitError {
    EmptyDesign,
    InvalidOption(String),
    VersionMismatch { found: u16, expected: u16 },
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDesign => write!(f, "the design has no layers to export"),
            Self::InvalidOption(message) => write!(f, "invalid export option: {message}"),
            Self::VersionMismatch { found, expected } => {
                write!(f, "IR version {found} is not supported (expected {expected})")
            }
        }
    }
}

impl std::error::Error for EmitError {}

/// Turn any user text into a safe Rust crate name.
pub fn sanitize_crate_name(value: &str) -> String {
    let mut out = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            out.push(character.to_ascii_lowercase());
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

/// Emit the full project. Deterministic: the same design and options always
/// produce byte-identical files in the same order.
pub fn emit(design: &Design, options: &EmitOptions) -> Result<GeneratedProject, EmitError> {
    if design.version != figcraft_ir::IR_VERSION {
        return Err(EmitError::VersionMismatch {
            found: design.version,
            expected: figcraft_ir::IR_VERSION,
        });
    }
    if design.roots.is_empty() {
        return Err(EmitError::EmptyDesign);
    }
    if options.crate_name.is_empty() || options.crate_name != sanitize_crate_name(&options.crate_name) {
        return Err(EmitError::InvalidOption(format!(
            "crate name '{}' is not a valid Rust identifier",
            options.crate_name
        )));
    }

    let root = format!("{}/", options.crate_name);
    let values = [
        ("{{CRATE}}", options.crate_name.as_str()),
        ("{{DEFAULT_BACKEND}}", options.default_backend.as_str()),
        ("{{GENERATOR}}", figcraft_ir::GENERATOR_NAME),
        ("{{GENERATOR_VERSION}}", figcraft_ir::GENERATOR_VERSION),
        ("{{IR_VERSION}}", &figcraft_ir::IR_VERSION.to_string()),
        ("{{SCENE_NAME}}", design.name.as_str()),
        ("{{NODE_COUNT}}", &design.node_count().to_string()),
    ];

    let mut files = Vec::new();
    let mut add = |path: &str, template: &str, owner: FileOwner| {
        files.push(GeneratedFile {
            path: format!("{root}{path}"),
            data: inject(template, &values).into_bytes(),
            owner,
        });
    };

    add("Cargo.toml", CARGO, FileOwner::Generated);
    add("README.md", README, FileOwner::Generated);
    add("GENERATED.md", GENERATED, FileOwner::Generated);
    add("sync.ps1", SYNC, FileOwner::Generated);
    add("src/main.rs", MAIN, FileOwner::Generated);
    add("src/core.rs", CORE, FileOwner::Generated);
    add("src/app.rs", APP, FileOwner::Generated);
    add("src/component/mod.rs", COMPONENT_MOD, FileOwner::Generated);
    add("src/component/state.rs", COMPONENT_STATE, FileOwner::Generated);
    add("src/component/tree.rs", COMPONENT_TREE, FileOwner::Generated);
    add("src/caller/mod.rs", CALLER_MOD, FileOwner::Generated);
    add("src/caller/layout.rs", CALLER_LAYOUT, FileOwner::Generated);
    add("src/caller/resolve.rs", CALLER_RESOLVE, FileOwner::Generated);
    add("src/renderer/mod.rs", RENDERER_MOD, FileOwner::Generated);
    add("src/renderer/pixels.rs", RENDERER_PIXELS, FileOwner::Generated);
    add("src/renderer/font.rs", RENDERER_FONT, FileOwner::Generated);
    add("src/renderer/png.rs", RENDERER_PNG, FileOwner::Generated);
    add("src/renderer/cpu.rs", RENDERER_CPU, FileOwner::Generated);
    add("src/renderer/skia.rs", RENDERER_SKIA, FileOwner::Generated);
    add("src/renderer/gpu.rs", RENDERER_GPU, FileOwner::Generated);
    add("src/user/mod.rs", USER_MOD, FileOwner::User);
    add("src/user/handlers.rs", USER_HANDLERS, FileOwner::User);
    add("src/tests/mod.rs", TESTS_MOD, FileOwner::Generated);
    add("src/tests/layout.rs", TESTS_LAYOUT, FileOwner::Generated);
    add("src/tests/render.rs", TESTS_RENDER, FileOwner::Generated);

    files.push(GeneratedFile {
        path: format!("{root}src/design/mod.rs"),
        data: design_source(design).into_bytes(),
        owner: FileOwner::Generated,
    });
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let manifest = ProjectManifest {
        generator: figcraft_ir::GENERATOR_NAME.to_string(),
        generator_version: figcraft_ir::GENERATOR_VERSION.to_string(),
        ir_version: design.version,
        crate_name: options.crate_name.clone(),
        default_backend: options.default_backend.as_str().to_string(),
        file_count: files.len(),
    };
    Ok(GeneratedProject { files, manifest })
}

fn inject(template: &str, values: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, value) in values {
        result = result.replace(key, value);
    }
    result
}

fn rust_string(value: &str) -> String {
    let mut out = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn num(value: f32) -> String {
    if value == 0.0 {
        "0.0".to_string()
    } else {
        format!("{value:?}")
    }
}

fn rect(rect: Rect) -> String {
    format!(
        "Rect::new({}, {}, {}, {})",
        num(rect.origin.x),
        num(rect.origin.y),
        num(rect.size.width),
        num(rect.size.height)
    )
}

fn color(color: Color) -> String {
    format!(
        "Color::new({}, {}, {}, {})",
        color.r, color.g, color.b, color.a
    )
}

fn paint(paint: Option<&Paint>) -> String {
    match paint {
        None => "None".to_string(),
        Some(Paint::Solid(value)) => format!("Some(Paint::Solid({}))", color(*value)),
        Some(Paint::Image(id)) => format!("Some(Paint::Image({id}))"),
        Some(Paint::Unsupported(value)) => format!(
            "Some(Paint::Unsupported(UnsupportedPaint {{ kind: {}.into(), fallback: {} }}))",
            rust_string(&value.kind),
            color(value.fallback)
        ),
    }
}

fn style(style: &Style) -> String {
    format!(
        "Style {{ fill: {}, stroke: {}, stroke_width: {}, corner_radius: {}, opacity: {}, clips_content: {} }}",
        paint(style.fill.as_ref()),
        paint(style.stroke.as_ref()),
        num(style.stroke_width),
        num(style.corner_radius),
        num(style.opacity),
        style.clips_content
    )
}

fn align(value: Align) -> &'static str {
    match value {
        Align::Start => "Align::Start",
        Align::Center => "Align::Center",
        Align::End => "Align::End",
        Align::SpaceBetween => "Align::Start",
    }
}

fn axis(value: Axis) -> &'static str {
    match value {
        Axis::None => "Axis::None",
        Axis::Horizontal => "Axis::Horizontal",
        Axis::Vertical => "Axis::Vertical",
    }
}

fn widget_kind(kind: &WidgetKind) -> String {
    match kind {
        WidgetKind::Custom(name) => format!("WidgetKind::Custom({}.into())", rust_string(name)),
        other => format!("WidgetKind::{}", other.as_str()),
    }
}

fn kind_literal(kind: &Kind) -> String {
    match kind {
        Kind::Frame(spec) => {
            let FrameSpec { layout, gap, padding, align } = spec;
            format!(
                "Kind::Frame(FrameSpec {{ layout: {}, gap: {}, padding: Padding {{ top: {}, right: {}, bottom: {}, left: {} }}, align: {} }})",
                axis(*layout),
                num(*gap),
                num(padding.top),
                num(padding.right),
                num(padding.bottom),
                num(padding.left),
                self::align(*align)
            )
        }
        Kind::Shape(shape) => {
            let name = match shape {
                ShapeKind::Rectangle => "Rectangle",
                ShapeKind::Ellipse => "Ellipse",
                ShapeKind::Line => "Line",
                ShapeKind::Vector => "Vector",
            };
            format!("Kind::Shape(ShapeKind::{name})")
        }
        Kind::Text(spec) => {
            let TextSpec {
                text,
                font_family,
                font_size,
                bold,
                italic,
                align: text_align,
                vertical_align,
                line_height,
                color: text_color,
                bindings,
            } = spec;
            let vertical = match vertical_align {
                TextAlignVertical::Top => "TextAlignVertical::Top",
                TextAlignVertical::Center => "TextAlignVertical::Center",
                TextAlignVertical::Bottom => "TextAlignVertical::Bottom",
            };
            let bindings = bindings
                .iter()
                .map(|key| format!("{}.into()", rust_string(key)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Kind::Text(TextSpec {{ text: {}.into(), font_family: {}.into(), font_size: {}, bold: {}, italic: {}, align: {}, vertical_align: {}, line_height: {}, color: {}, bindings: vec![{}] }})",
                rust_string(text),
                rust_string(font_family),
                num(*font_size),
                bold,
                italic,
                align(*text_align),
                vertical,
                num(*line_height),
                color(*text_color),
                bindings
            )
        }
        Kind::Image { asset } => format!("Kind::Image {{ asset: {asset} }}"),
        Kind::Widget(spec) => {
            let WidgetSpec { kind, label, binding, options } = spec;
            let label = label
                .as_ref()
                .map(|value| format!("Some({}.into())", rust_string(value)))
                .unwrap_or_else(|| "None".to_string());
            let binding = binding
                .as_ref()
                .map(|value| format!("Some({}.into())", rust_string(value)))
                .unwrap_or_else(|| "None".to_string());
            let options = options
                .iter()
                .map(|value| format!("{}.into()", rust_string(value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Kind::Widget(WidgetSpec {{ kind: {}, label: {}, binding: {}, options: vec![{}] }})",
                widget_kind(kind),
                label,
                binding,
                options
            )
        }
    }
}

fn node_literal(node: &Node, indent: &str, out: &mut String) {
    out.push_str(indent);
    out.push_str("Node {\n");
    out.push_str(indent);
    out.push_str("    id: ");
    out.push_str(&node.id.to_string());
    out.push_str(",\n");
    out.push_str(indent);
    out.push_str("    name: ");
    out.push_str(&rust_string(&node.name));
    out.push_str(".into(),\n");
    out.push_str(indent);
    out.push_str("    rect: ");
    out.push_str(&rect(node.rect));
    out.push_str(",\n");
    out.push_str(indent);
    out.push_str("    style: ");
    out.push_str(&style(&node.style));
    out.push_str(",\n");
    out.push_str(indent);
    out.push_str("    kind: ");
    out.push_str(&kind_literal(&node.kind));
    out.push_str(",\n");
    out.push_str(indent);
    out.push_str("    visible: ");
    out.push_str(&node.visible.to_string());
    out.push_str(",\n");
    out.push_str(indent);
    out.push_str("    locked: ");
    out.push_str(&node.locked.to_string());
    out.push_str(",\n");
    out.push_str(indent);
    out.push_str("    children: vec![\n");
    for child in &node.children {
        node_literal(child, &format!("{indent}        "), out);
        out.push_str(",\n");
    }
    out.push_str(indent);
    out.push_str("    ],\n");
    out.push_str(indent);
    out.push('}');
}

fn asset_literal(asset: &Asset, out: &mut String) {
    out.push_str("        crate::component::Asset {\n            id: ");
    out.push_str(&asset.id.to_string());
    out.push_str(",\n            name: ");
    out.push_str(&rust_string(&asset.name));
    out.push_str(".into(),\n            mime: ");
    out.push_str(&rust_string(&asset.mime));
    out.push_str(".into(),\n            bytes: vec![\n                ");
    for (index, byte) in asset.bytes.iter().enumerate() {
        if index > 0 && index % 24 == 0 {
            out.push_str("\n                ");
        }
        out.push_str(&byte.to_string());
        out.push_str(", ");
    }
    out.push_str("\n            ],\n        },\n");
}

/// Generate the `@generated` design data module.
fn design_source(design: &Design) -> String {
    let mut out = String::new();
    out.push_str("//! @generated — data statis hasil export. Jangan edit manual.\n");
    out.push_str("//! Generator: ");
    out.push_str(figcraft_ir::GENERATOR_NAME);
    out.push_str(" v");
    out.push_str(figcraft_ir::GENERATOR_VERSION);
    out.push_str(" (IR v");
    out.push_str(&design.version.to_string());
    out.push_str(").\n");
    if !design.warnings.is_empty() {
        out.push_str("//! Peringatan import:\n");
        for warning in &design.warnings {
            out.push_str("//! - [");
            out.push_str(warning.code.as_str());
            out.push_str("] ");
            out.push_str(&warning.message.replace('\n', " "));
            out.push('\n');
        }
    }
    out.push_str("\nuse crate::component::{\n");
    out.push_str("    Align, Axis, Color, Component, FrameSpec, Kind, Node, Padding, Paint, ShapeKind,\n");
    out.push_str("    Style, TextAlignVertical, TextSpec, UnsupportedPaint, WidgetKind, WidgetSpec,\n");
    out.push_str("};\n");
    out.push_str("use crate::core::{Rect, Size};\n\n");
    out.push_str("pub fn build() -> Component {\n");
    out.push_str("    Component {\n        name: ");
    out.push_str(&rust_string(&design.name));
    out.push_str(".into(),\n        design_size: Size::new(");
    out.push_str(&num(design.size.width));
    out.push_str(", ");
    out.push_str(&num(design.size.height));
    out.push_str("),\n        assets: vec![\n");
    for asset in &design.assets {
        asset_literal(asset, &mut out);
    }
    out.push_str("        ],\n        root: vec![\n");
    for root in &design.roots {
        node_literal(root, "            ", &mut out);
        out.push_str(",\n");
    }
    out.push_str("        ],\n    }\n}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use figcraft_ir::{Design, Node, Size, Style, Warning, WarningCode};

    fn sample() -> Design {
        let mut design = Design::new("Contoh".into());
        design.size = Size::new(320.0, 180.0);
        design.warnings.push(Warning {
            code: WarningCode::WidgetGuess,
            message: "layer 'OK' as widget Button".into(),
            node: Some(2),
        });
        design.roots.push(Node {
            id: 1,
            name: "Card".into(),
            rect: Rect::new(0.0, 0.0, 320.0, 180.0),
            style: Style {
                fill: Some(Paint::Solid(Color::WHITE)),
                corner_radius: 12.0,
                ..Style::default()
            },
            kind: Kind::Frame(FrameSpec {
                layout: Axis::Vertical,
                gap: 8.0,
                padding: figcraft_ir::Padding::uniform(16.0),
                align: Align::Start,
            }),
            visible: true,
            locked: false,
            children: vec![Node {
                id: 2,
                name: "OK".into(),
                rect: Rect::new(16.0, 16.0, 80.0, 32.0),
                style: Style::default(),
                kind: Kind::Widget(WidgetSpec {
                    kind: WidgetKind::Button,
                    label: Some("Kirim".into()),
                    binding: Some("submit".into()),
                    options: Vec::new(),
                }),
                visible: true,
                locked: false,
                children: Vec::new(),
            }],
        });
        design
    }

    #[test]
    fn crate_names_are_sanitized() {
        assert_eq!(sanitize_crate_name("My Cool-Design!"), "my_cool_design");
        assert_eq!(sanitize_crate_name("9lives"), "lives");
        assert_eq!(sanitize_crate_name(""), "figcraft_design");
    }

    #[test]
    fn golden_structure_is_complete_and_deterministic() {
        let design = sample();
        let options = EmitOptions::new("figcraft_sample".into(), Backend::Cpu);
        let first = emit(&design, &options).expect("emit succeeds");
        let second = emit(&design, &options).expect("emit succeeds");
        assert_eq!(first, second, "emission must be deterministic");

        let paths: Vec<&str> = first.files.iter().map(|file| file.path.as_str()).collect();
        for expected in [
            "figcraft_sample/Cargo.toml",
            "figcraft_sample/src/main.rs",
            "figcraft_sample/src/component/mod.rs",
            "figcraft_sample/src/caller/layout.rs",
            "figcraft_sample/src/renderer/skia.rs",
            "figcraft_sample/src/renderer/gpu.rs",
            "figcraft_sample/src/app.rs",
            "figcraft_sample/src/design/mod.rs",
            "figcraft_sample/src/user/handlers.rs",
            "figcraft_sample/sync.ps1",
        ] {
            assert!(paths.contains(&expected), "missing {expected}");
        }
        assert_eq!(first.manifest.file_count, first.files.len());
        assert_eq!(
            first
                .files
                .iter()
                .find(|file| file.path.ends_with("src/user/handlers.rs"))
                .map(|file| file.owner),
            Some(FileOwner::User)
        );
        assert!(first.files.iter().all(|file| !file.path.contains("..")));
    }

    #[test]
    fn design_source_contains_binding_and_widget() {
        let source = design_source(&sample());
        assert!(source.contains("WidgetKind::Button"));
        assert!(source.contains("\"submit\".into()"));
        assert!(source.contains("Paint::Solid(Color::new(255, 255, 255, 255))"));
        assert!(source.contains("widget_guess"));
    }

    #[test]
    fn rejects_wrong_ir_version() {
        let mut design = sample();
        design.version = 99;
        assert!(matches!(
            emit(&design, &EmitOptions::new("x".into(), Backend::Cpu)),
            Err(EmitError::VersionMismatch { .. })
        ));
    }
}
