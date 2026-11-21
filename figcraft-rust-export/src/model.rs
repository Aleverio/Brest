//! The small intermediate representation used between FigCraft's JSON and the
//! generated Rust source. Parsing is deliberately tolerant: FigCraft versions
//! have used both Figma-style `fills`/`absoluteBoundingBox` and shorter keys.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use figcraft_plugin_sdk::Value;

pub const MAX_NODES: usize = 2_000;
pub const MAX_DEPTH: usize = 24;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub fn literal(self) -> String {
        format!("Rgba::new({}, {}, {}, {})", self.r, self.g, self.b, self.a)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn literal(self) -> String {
        format!(
            "Rect {{ x: {}, y: {}, width: {}, height: {} }}",
            num(self.x),
            num(self.y),
            num(self.width),
            num(self.height)
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axis {
    #[default]
    None,
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    pub fill: Option<Rgba>,
    pub stroke: Option<Rgba>,
    pub stroke_width: f32,
    pub radius: f32,
    pub opacity: f32,
}

impl Style {
    pub fn literal(self) -> String {
        let fill = match self.fill {
            Some(c) => format!("Some({})", c.literal()),
            None => "None".to_string(),
        };
        let stroke = match self.stroke {
            Some(c) => format!("Some({})", c.literal()),
            None => "None".to_string(),
        };
        format!(
            "Style {{ fill: {fill}, stroke: {stroke}, stroke_width: {}, radius: {}, opacity: {} }}",
            num(self.stroke_width),
            num(self.radius),
            num(self.opacity)
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Frame { layout: Axis, gap: f32, padding: f32 },
    Rectangle,
    Ellipse,
    Line { x2: f32, y2: f32 },
    Text { text: String, font_size: f32, bold: bool, color: Rgba, align: Align },
    Image { path: String },
    Other(String),
}

impl Kind {
    pub fn literal(&self) -> String {
        match self {
            Self::Frame { layout, gap, padding } => {
                let layout = match layout {
                    Axis::None => "None".to_string(),
                    Axis::Horizontal => "Some(Axis::Horizontal)".to_string(),
                    Axis::Vertical => "Some(Axis::Vertical)".to_string(),
                };
                format!(
                    "Kind::Frame {{ layout: {layout}, gap: {}, padding: {} }}",
                    num(*gap),
                    num(*padding)
                )
            }
            Self::Rectangle => "Kind::Rectangle".to_string(),
            Self::Ellipse => "Kind::Ellipse".to_string(),
            Self::Line { x2, y2 } => format!("Kind::Line {{ x2: {}, y2: {} }}", num(*x2), num(*y2)),
            Self::Text { text, font_size, bold, color, align } => {
                let align = match align {
                    Align::Start => "Align::Start",
                    Align::Center => "Align::Center",
                    Align::End => "Align::End",
                };
                format!(
                    "Kind::Text {{ text: {}.to_string(), font_size: {}, bold: {}, color: {}, align: {} }}",
                    rust_string(text),
                    num(*font_size),
                    bold,
                    color.literal(),
                    align
                )
            }
            Self::Image { path } => format!("Kind::Image {{ path: {}.to_string() }}", rust_string(path)),
            Self::Other(kind) => format!("Kind::Rectangle /* {kind} */"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: u64,
    pub name: String,
    pub rect: Rect,
    pub style: Style,
    pub kind: Kind,
    pub visible: bool,
    pub children: Vec<Node>,
}

impl Node {
    /// Render this node and its subtree as a Rust expression.
    pub fn literal(&self, indent: &str, out: &mut String) {
        out.push_str(indent);
        out.push_str("Node {\n");
        let field = format!("{indent}    ");
        out.push_str(&field);
        out.push_str(&format!("id: {},\n", self.id));
        out.push_str(&field);
        out.push_str(&format!("name: {}.to_string(),\n", rust_string(&self.name)));
        out.push_str(&field);
        out.push_str(&format!("rect: {},\n", self.rect.literal()));
        out.push_str(&field);
        out.push_str(&format!("style: {},\n", self.style.literal()));
        out.push_str(&field);
        out.push_str(&format!("kind: {},\n", self.kind.literal()));
        out.push_str(&field);
        out.push_str(&format!("visible: {},\n", self.visible));
        if self.children.is_empty() {
            out.push_str(&field);
            out.push_str("children: vec![],\n");
        } else {
            out.push_str(&field);
            out.push_str("children: vec![\n");
            for child in &self.children {
                child.literal(&format!("{field}    "), out);
                out.push_str(",\n");
            }
            out.push_str(&field);
            out.push_str("],\n");
        }
        out.push_str(indent);
        out.push('}');
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub name: String,
    pub width: f32,
    pub height: f32,
    pub roots: Vec<Node>,
    pub count: usize,
    pub bindings: Vec<String>,
}

/// Escape a string for a Rust string literal.
pub fn rust_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Print a float as a Rust `f32` literal.
pub fn num(value: f32) -> String {
    if !value.is_finite() {
        return "0.0".to_string();
    }
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded == 0.0 {
        "0.0".to_string()
    } else {
        let text = format!("{rounded}");
        if text.contains('.') { text } else { format!("{text}.0") }
    }
}

fn get<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        let found = value.get(key);
        if found.is_some_and(|v| !v.is_null()) {
            return found;
        }
    }
    None
}

fn number(value: &Value, keys: &[&str]) -> Option<f64> {
    get(value, keys).and_then(Value::as_f64)
}

fn text(value: &Value, keys: &[&str]) -> Option<String> {
    get(value, keys).and_then(Value::as_str).map(ToString::to_string)
}

fn boolean(value: &Value, keys: &[&str]) -> Option<bool> {
    get(value, keys).and_then(Value::as_bool)
}

/// Parse `#RGB`, `#RRGGBB`, `#RRGGBBAA`, `{r,g,b,a}` (0..1 or 0..255) or an
/// array of components.
fn color(value: Option<&Value>) -> Option<Rgba> {
    let value = value?;
    if let Some(hex) = value.as_str() {
        let trimmed = hex.trim().trim_start_matches('#');
        let expanded;
        let h = if trimmed.len() == 3 {
            expanded = format!("{}{}{}{}{}{}", &trimmed[0..1], &trimmed[0..1], &trimmed[1..2], &trimmed[1..2], &trimmed[2..3], &trimmed[2..3]);
            expanded.as_str()
        } else {
            trimmed
        };
        if h.len() != 6 && h.len() != 8 {
            return None;
        }
        let channel = |a: usize, b: usize| u8::from_str_radix(&h[a..b], 16).ok();
        return Some(Rgba {
            r: channel(0, 2)?,
            g: channel(2, 4)?,
            b: channel(4, 6)?,
            a: if h.len() == 8 { channel(6, 8)? } else { 255 },
        });
    }
    if let Some(obj) = value.as_object() {
        let component = |key: &str| -> Option<f64> {
            obj.get(key).and_then(Value::as_f64).or_else(|| obj.get(key).and_then(Value::as_u64).map(|v| v as f64))
        };
        let scale = |v: f64| if v <= 1.0 && v.fract() != 0.0 { v * 255.0 } else { v };
        let r = component("r")?;
        let g = component("g")?;
        let b = component("b")?;
        let a = component("a").unwrap_or(1.0);
        return Some(Rgba {
            r: scale(r).clamp(0.0, 255.0) as u8,
            g: scale(g).clamp(0.0, 255.0) as u8,
            b: scale(b).clamp(0.0, 255.0) as u8,
            a: scale(a).clamp(0.0, 255.0) as u8,
        });
    }
    None
}

fn first_color(value: &Value, keys: &[&str]) -> Option<Rgba> {
    let list = get(value, keys)?.as_array()?;
    for item in list {
        if let Some(found) = color(item.get("color").or(Some(item))) {
            return Some(found);
        }
    }
    None
}

/// Read a node from `node.get` (or a child object from it) into the IR.
pub fn parse_node(value: &Value, depth: usize, counter: &mut usize) -> Node {
    let id = number(value, &["id"]).unwrap_or(0.0) as u64;
    let name = text(value, &["name"]).unwrap_or_else(|| format!("Layer {id}"));
    let kind_name = text(value, &["type", "kind"]).unwrap_or_else(|| "FRAME".to_string());
    let boxed = get(value, &["absoluteBounds", "absoluteBoundingBox", "bounds", "absoluteRenderBounds"]);

    let x = number(value, &["x"]).or_else(|| boxed.and_then(|b| number(b, &["x"]))).unwrap_or(0.0) as f32;
    let y = number(value, &["y"]).or_else(|| boxed.and_then(|b| number(b, &["y"]))).unwrap_or(0.0) as f32;
    let width = number(value, &["width", "w"]).or_else(|| boxed.and_then(|b| number(b, &["width", "w"]))).unwrap_or(0.0) as f32;
    let height = number(value, &["height", "h"]).or_else(|| boxed.and_then(|b| number(b, &["height", "h"]))).unwrap_or(0.0) as f32;

    let fill = first_color(value, &["fills", "fill"]);
    let stroke = first_color(value, &["strokes", "stroke"]);
    let style = Style {
        fill,
        stroke,
        stroke_width: number(value, &["strokeWeight", "strokeWidth"]).unwrap_or(1.0) as f32,
        radius: number(value, &["cornerRadius", "radius", "rectangleCornerRadii"]).unwrap_or(0.0) as f32,
        opacity: number(value, &["opacity"]).unwrap_or(1.0) as f32,
    };

    let kind = match kind_name.to_ascii_uppercase().as_str() {
        "TEXT" => Kind::Text {
            text: text(value, &["characters", "text", "content"]).unwrap_or_default(),
            font_size: number(value, &["fontSize", "size"]).unwrap_or(14.0) as f32,
            bold: text(value, &["fontName", "fontStyle", "fontWeight"])
                .is_some_and(|f| f.to_ascii_lowercase().contains("bold"))
                || boolean(value, &["bold"]).unwrap_or(false),
            color: fill.unwrap_or(Rgba { r: 0, g: 0, b: 0, a: 255 }),
            align: match text(value, &["textAlign", "textAlignHorizontal", "align"]).unwrap_or_default().to_ascii_lowercase().as_str() {
                "center" => Align::Center,
                "right" | "end" => Align::End,
                _ => Align::Start,
            },
        },
        "ELLIPSE" | "CIRCLE" => Kind::Ellipse,
        "LINE" | "VECTOR" => Kind::Line {
            x2: width,
            y2: height,
        },
        "IMAGE" | "RECTANGLE" if value.get("imageHash").is_some() => Kind::Image {
            path: text(value, &["imagePath", "path", "name"]).unwrap_or_else(|| format!("{id}.png")),
        },
        "RECTANGLE" => Kind::Rectangle,
        "FRAME" | "GROUP" | "COMPONENT" | "INSTANCE" | "SECTION" | "CANVAS" | "DOCUMENT" => {
            let layout = match text(value, &["layoutMode", "layout"]).unwrap_or_default().to_ascii_lowercase().as_str() {
                "horizontal" | "row" => Axis::Horizontal,
                "vertical" | "column" => Axis::Vertical,
                _ => Axis::None,
            };
            Kind::Frame {
                layout,
                gap: number(value, &["itemSpacing", "gap", "spacing"]).unwrap_or(0.0) as f32,
                padding: number(value, &["paddingLeft", "padding", "layoutPadding"])
                    .unwrap_or(0.0) as f32,
            }
        }
        other => Kind::Other(other.to_string()),
    };

    *counter += 1;
    let mut children = Vec::new();
    if depth < MAX_DEPTH && *counter < MAX_NODES {
        if let Some(list) = get(value, &["children"]).and_then(Value::as_array) {
            for child in list {
                if *counter >= MAX_NODES {
                    break;
                }
                children.push(parse_node(child, depth + 1, counter));
            }
        }
    }

    Node {
        id,
        name,
        rect: Rect { x, y, width, height },
        style,
        kind,
        visible: boolean(value, &["visible"]).unwrap_or(true),
        children,
    }
}

/// Collect `{{key}}` placeholders from all text nodes.
pub fn bindings(nodes: &[Node], out: &mut Vec<String>) {
    for node in nodes {
        if let Kind::Text { text, .. } = &node.kind {
            let mut rest = text.as_str();
            while let Some(start) = rest.find("{{") {
                let after = &rest[start + 2..];
                match after.find("}}") {
                    Some(end) => {
                        let key = &after[..end];
                        if !key.is_empty() && !out.iter().any(|k| k == key) {
                            out.push(key.to_string());
                        }
                        rest = &after[end + 2..];
                    }
                    None => break,
                }
            }
        }
        bindings(&node.children, out);
    }
}

/// Build the scene JSON passed to the code generator.
pub fn scene(name: String, roots: Vec<Node>) -> Scene {
    let mut count = 0usize;
    let mut found = Vec::new();
    for root in &roots {
        count_nodes(root, &mut count);
    }
    bindings(&roots, &mut found);
    let width = roots.iter().map(|n| n.rect.x + n.rect.width).fold(0.0f32, f32::max);
    let height = roots.iter().map(|n| n.rect.y + n.rect.height).fold(0.0f32, f32::max);
    Scene { name, width: width.max(1.0), height: height.max(1.0), roots, count, bindings: found }
}

fn count_nodes(node: &Node, count: &mut usize) {
    *count += 1;
    for child in &node.children {
        count_nodes(child, count);
    }
}

