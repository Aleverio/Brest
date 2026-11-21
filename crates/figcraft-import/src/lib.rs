//! Parse FigCraft node JSON into the versioned `figcraft-ir`.
//!
//! The importer is the **only** place that knows FigCraft's field names. It is
//! intentionally strict about structure and generous about aliases: a field
//! that cannot be understood produces a `Warning` instead of a silently wrong
//! UI. Host I/O (fetching image bytes, reading the page) is not done here.

use std::borrow::ToOwned;
use std::fmt;
use std::string::{String, ToString};
use std::vec::Vec;

use figcraft_ir::{
    Align, Asset, Axis, Color, Design, FrameSpec, Kind, Node, NodeId, Padding, Paint, Rect,
    ShapeKind, Size, SourceInfo, Style, TextAlignVertical, TextSpec, UnsupportedPaint, Warning,
    WarningCode, WidgetKind, WidgetSpec, IR_VERSION,
};
use serde_json::Value;

/// Everything the importer needs from the host.
#[derive(Clone, Debug, Default)]
pub struct ImportContext {
    pub page_name: Option<String>,
    pub selection: Vec<NodeId>,
    pub code_reference: bool,
    /// Images already fetched by the host. Matched by asset name/image hash.
    pub assets: Vec<Asset>,
    pub limits: ImportLimits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_asset_bytes: usize,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_nodes: 2_000,
            max_depth: 24,
            max_asset_bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImportError {
    InvalidJson(String),
    UnexpectedShape(String),
    LimitReached(&'static str),
    InvalidNodeId(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(f, "invalid FigCraft JSON: {message}"),
            Self::UnexpectedShape(message) => write!(f, "unexpected FigCraft node: {message}"),
            Self::LimitReached(what) => write!(f, "import limit reached: {what}"),
            Self::InvalidNodeId(value) => write!(f, "invalid node id: {value}"),
        }
    }
}

/// Parsed but not yet normalised document.
#[derive(Clone, Debug)]
pub struct RawDocument {
    value: Value,
}

impl RawDocument {
    pub fn as_value(&self) -> &Value {
        &self.value
    }
}

/// Parse JSON only. Kept separate so callers can report a syntax error before
/// spending time on normalisation.
pub fn parse_document(raw: &str) -> Result<RawDocument, ImportError> {
    let value = serde_json::from_str(raw).map_err(|e| ImportError::InvalidJson(e.to_string()))?;
    Ok(RawDocument { value })
}

/// Parse and normalise in one call.
pub fn import(raw: &str, context: &ImportContext) -> Result<Design, ImportError> {
    let document = parse_document(raw)?;
    normalize(&document, context)
}

/// Convert a parsed FigCraft node document into IR v1.
pub fn normalize(document: &RawDocument, context: &ImportContext) -> Result<Design, ImportError> {
    let roots = root_values(document.as_value());
    if roots.is_empty() {
        return Err(ImportError::UnexpectedShape(String::from(
            "no layer object or layer list at the document root",
        )));
    }

    let total_asset_bytes = context.assets.iter().map(|asset| asset.bytes.len()).sum::<usize>();
    if total_asset_bytes > context.limits.max_asset_bytes {
        return Err(ImportError::LimitReached("total asset bytes"));
    }

    let mut state = ImportState {
        context,
        count: 0,
        warnings: Vec::new(),
        bounds: Rect::default(),
        has_bounds: false,
    };

    let mut nodes = Vec::new();
    for value in roots {
        if state.count >= state.context.limits.max_nodes {
            state.warn(WarningCode::LimitReached, String::from("node limit reached; later layers omitted"), None);
            break;
        }
        nodes.push(read_node(value, None, 0, &mut state)?);
    }

    let size = if state.has_bounds {
        state.bounds.size.clamped()
    } else {
        Size::new(1.0, 1.0)
    };

    let mut design = Design::new(
        context
            .page_name
            .clone()
            .unwrap_or_else(|| String::from("FigCraft design")),
    );
    design.version = IR_VERSION;
    design.size = size;
    design.roots = nodes;
    design.assets = context.assets.clone();
    design.warnings = state.warnings;
    design.source = SourceInfo {
        page_name: context.page_name.clone(),
        selection: context.selection.clone(),
        code_reference: context.code_reference,
    };
    Ok(design)
}

struct ImportState<'a> {
    context: &'a ImportContext,
    count: usize,
    warnings: Vec<Warning>,
    bounds: Rect,
    has_bounds: bool,
}

impl ImportState<'_> {
    fn warn(&mut self, code: WarningCode, message: String, node: Option<NodeId>) {
        self.warnings.push(Warning { code, message, node });
    }

    fn note_bounds(&mut self, rect: Rect) {
        if rect.size.width <= 0.0 || rect.size.height <= 0.0 {
            return;
        }
        self.bounds = if self.has_bounds {
            self.bounds.union(rect)
        } else {
            rect
        };
        self.has_bounds = true;
    }
}

fn root_values(value: &Value) -> Vec<&Value> {
    match value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(map) => {
            for key in ["nodes", "layers", "roots"] {
                if let Some(items) = map.get(key).and_then(Value::as_array) {
                    return items.iter().collect();
                }
            }
            // A whole page/document is a container: its children are the roots.
            let kind = map
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_ascii_uppercase();
            if matches!(kind.as_str(), "PAGE" | "DOCUMENT" | "CANVAS")
                && let Some(items) = map.get("children").and_then(Value::as_array)
            {
                return items.iter().collect();
            }
            vec![value]
        }
        _ => Vec::new(),
    }
}

fn read_node(
    value: &Value,
    parent_origin: Option<figcraft_ir::Point>,
    depth: usize,
    state: &mut ImportState<'_>,
) -> Result<Node, ImportError> {
    if depth > state.context.limits.max_depth {
        return Err(ImportError::LimitReached("layer depth"));
    }
    state.count += 1;
    if state.count > state.context.limits.max_nodes {
        return Err(ImportError::LimitReached("node count"));
    }

    let id = read_id(value)?;
    let name = text(value, &["name"]).unwrap_or_else(|| format!("Layer {id}"));
    let type_name = text(value, &["type", "kind"]).unwrap_or_default().to_ascii_uppercase();
    let (relative, absolute) = read_rect(value, parent_origin);
    state.note_bounds(absolute);

    let rotation = number(value, &["rotation"]).unwrap_or(0.0);
    if rotation.abs() > 0.01 {
        state.warn(
            WarningCode::Rotation,
            format!("layer '{name}' is rotated {rotation} deg; rotation is not supported in v1"),
            Some(id),
        );
    }

    let style = read_style(value, id, state);
    let kind = read_kind(value, &name, &type_name, id, &style, state);

    let mut children = Vec::new();
    if let Some(list) = get(value, &["children"]).and_then(Value::as_array) {
        for child in list {
            if state.count >= state.context.limits.max_nodes {
                state.warn(
                    WarningCode::LimitReached,
                    String::from("node limit reached; later children omitted"),
                    Some(id),
                );
                break;
            }
            children.push(read_node(child, Some(absolute.origin), depth + 1, state)?);
        }
    }

    Ok(Node {
        id,
        name,
        rect: relative,
        style,
        kind,
        visible: boolean(value, &["visible"]).unwrap_or(true),
        locked: boolean(value, &["locked"]).unwrap_or(false),
        children,
    })
}

fn read_id(value: &Value) -> Result<NodeId, ImportError> {
    if let Some(id) = number(value, &["id"])
        && id >= 0.0
        && id.fract() == 0.0
    {
        return Ok(id as NodeId);
    }
    if let Some(id) = text(value, &["id"]) {
        let digits: String = id.chars().filter(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            return digits
                .parse::<NodeId>()
                .map_err(|_| ImportError::InvalidNodeId(id));
        }
    }
    Err(ImportError::InvalidNodeId(String::from("missing or non-numeric id")))
}

/// Read FigCraft's absolute bounds and return `(relative to parent, absolute)`.
/// Coordinates in FigCraft are page-absolute; the IR stores parent-relative
/// rectangles so the generated layout solver has one unambiguous rule.
fn read_rect(value: &Value, parent_origin: Option<figcraft_ir::Point>) -> (Rect, Rect) {
    let bounds = get(value, &["absoluteBounds", "absoluteBoundingBox", "absoluteRenderBounds", "bounds"]);
    let x = number(value, &["x"]).or_else(|| bounds.and_then(|b| number(b, &["x"]))).unwrap_or(0.0) as f32;
    let y = number(value, &["y"]).or_else(|| bounds.and_then(|b| number(b, &["y"]))).unwrap_or(0.0) as f32;
    let width = number(value, &["width", "w"]).or_else(|| bounds.and_then(|b| number(b, &["width", "w"]))).unwrap_or(0.0).max(0.0) as f32;
    let height = number(value, &["height", "h"]).or_else(|| bounds.and_then(|b| number(b, &["height", "h"]))).unwrap_or(0.0).max(0.0) as f32;

    let absolute = Rect::new(x, y, width, height);
    let relative = match parent_origin {
        Some(parent) => Rect::new(x - parent.x, y - parent.y, width, height),
        None => absolute,
    };
    (relative, absolute)
}

fn read_style(
    value: &Value,
    id: NodeId,
    state: &mut ImportState<'_>,
) -> Style {
    let fill = first_paint(value, &["fills", "fill"], id, state);
    let stroke = first_paint(value, &["strokes", "stroke"], id, state);
    Style {
        fill,
        stroke,
        stroke_width: number(value, &["strokeWeight", "strokeWidth"]).unwrap_or(1.0).max(0.0) as f32,
        corner_radius: number(value, &["cornerRadius", "radius"])
            .or_else(|| {
                get(value, &["rectangleCornerRadii"])
                    .and_then(Value::as_array)
                    .and_then(|list| list.first())
                    .and_then(Value::as_f64)
            })
            .unwrap_or(0.0)
            .max(0.0) as f32,
        opacity: number(value, &["opacity"]).unwrap_or(1.0).clamp(0.0, 1.0) as f32,
        clips_content: boolean(value, &["clip", "clipsContent"]).unwrap_or(false),
    }
}

fn first_paint(
    value: &Value,
    keys: &[&str],
    id: NodeId,
    state: &mut ImportState<'_>,
) -> Option<Paint> {
    let list = get(value, keys)?;
    let paints = match list {
        Value::Array(items) => items,
        single => core::slice::from_ref(single),
    };
    for paint in paints {
        let paint_type = text(paint, &["type", "kind"]).unwrap_or_else(|| String::from("SOLID"));
        let paint_type = paint_type.to_ascii_uppercase();
        if paint_type.contains("SOLID")
            && let Some(color) = parse_color(paint.get("color").unwrap_or(paint))
        {
            return Some(Paint::Solid(color));
        }
        if paint_type.contains("IMAGE")
            && let Some(hash) = text(paint, &["imageHash", "hash", "name"])
        {
            if let Some(asset) = state.context.assets.iter().find(|item| item.name == hash) {
                return Some(Paint::Image(asset.id));
            }
            state.warn(
                WarningCode::MissingAsset,
                format!("image '{hash}' was not supplied by the host; using a grey fill"),
                Some(id),
            );
        }
        if paint_type != "SOLID" {
            state.warn(
                WarningCode::UnsupportedPaint,
                format!("paint '{paint_type}' is not supported in v1; using its colour"),
                Some(id),
            );
        }
        if let Some(color) = parse_color(paint.get("color").unwrap_or(paint)) {
            return Some(Paint::Unsupported(UnsupportedPaint {
                kind: paint_type,
                fallback: color,
            }));
        }
    }
    None
}

fn read_kind(
    value: &Value,
    name: &str,
    type_name: &str,
    id: NodeId,
    style: &Style,
    state: &mut ImportState<'_>,
) -> Kind {
    let has_interaction = get(value, &["interactions", "prototype"]).is_some_and(|v| {
        v.as_array().is_some_and(|list| !list.is_empty()) || v.as_object().is_some_and(|map| !map.is_empty())
    });
    let is_component = matches!(type_name, "COMPONENT" | "INSTANCE")
        || value.get("componentProperties").is_some()
        || value.get("componentPropertyDefinitions").is_some();
    let widget = if is_component || has_interaction {
        Some(WidgetKind::from_name(name))
    } else {
        match WidgetKind::from_name(name) {
            WidgetKind::Custom(_) => None,
            guessed => Some(guessed),
        }
    };
    if let Some(kind) = &widget {
        state.warn(
            WarningCode::WidgetGuess,
            format!("layer '{name}' was imported as widget {}", kind.as_str()),
            Some(id),
        );
        return Kind::Widget(WidgetSpec {
            kind: kind.clone(),
            label: get(value, &["componentProperties"]).and_then(|props| text(props, &["label"])),
            binding: text(value, &["binding"]),
            options: get(value, &["options"])
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(Value::as_str).map(ToOwned::to_owned).collect())
                .unwrap_or_default(),
        });
    }

    // An image fill is the strongest signal: FigCraft often uses a RECTANGLE
    // with an IMAGE paint instead of a dedicated image node.
    if let Some(Paint::Image(asset)) = style.fill {
        return Kind::Image { asset };
    }

    match type_name {
        "TEXT" => {
            let color = match style.fill {
                Some(Paint::Solid(color)) | Some(Paint::Unsupported(UnsupportedPaint { fallback: color, .. })) => color,
                _ => Color::BLACK,
            };
            let text_value = text(value, &["characters", "text", "content"]).unwrap_or_default();
            let bindings = extract_bindings(&text_value);
            let font = text(value, &["fontFamily", "fontName"]).unwrap_or_else(|| String::from("sans-serif"));
            if !font.eq_ignore_ascii_case("sans-serif") {
                state.warn(
                    WarningCode::FontFallback,
                    format!("font '{font}' is not bundled; the generated UI uses its bitmap font"),
                    Some(id),
                );
            }
            Kind::Text(TextSpec {
                text: text_value,
                font_family: font,
                font_size: number(value, &["fontSize", "size"]).unwrap_or(14.0).max(1.0) as f32,
                bold: text(value, &["fontStyle", "fontWeight"])
                    .is_some_and(|value| value.to_ascii_lowercase().contains("bold"))
                    || boolean(value, &["bold"]).unwrap_or(false),
                italic: text(value, &["fontStyle"])
                    .is_some_and(|value| value.to_ascii_lowercase().contains("italic")),
                align: parse_align(
                    &text(value, &["textAlign", "textAlignHorizontal", "align"]).unwrap_or_default(),
                ),
                vertical_align: match text(value, &["textAlignVertical"]).unwrap_or_default().to_ascii_lowercase().as_str() {
                    "center" | "middle" => TextAlignVertical::Center,
                    "bottom" => TextAlignVertical::Bottom,
                    _ => TextAlignVertical::Top,
                },
                line_height: number(value, &["lineHeightPx", "lineHeight"]).unwrap_or(0.0) as f32,
                color,
                bindings,
            })
        }
        "ELLIPSE" | "CIRCLE" => Kind::Shape(ShapeKind::Ellipse),
        "LINE" => Kind::Shape(ShapeKind::Line),
        "VECTOR" | "BOOLEAN_OPERATION" | "POLYGON" | "STAR" => Kind::Shape(ShapeKind::Vector),
        "IMAGE" => image_kind(value, id, state),
        "RECTANGLE" if value.get("imageHash").is_some() => image_kind(value, id, state),
        "RECTANGLE" => Kind::Shape(ShapeKind::Rectangle),
        "FRAME" | "GROUP" | "SECTION" | "CANVAS" | "DOCUMENT" | "COMPONENT" | "INSTANCE" => {
            if let Some(Paint::Image(asset)) = style.fill {
                state.warn(
                    WarningCode::UnsupportedPaint,
                    String::from("image fills on frames are exported as a flat colour in v1"),
                    Some(id),
                );
                let _ = asset;
            }
            Kind::Frame(FrameSpec {
                layout: match text(value, &["layout", "layoutMode"]).unwrap_or_default().to_ascii_lowercase().as_str() {
                    "horizontal" | "row" => Axis::Horizontal,
                    "vertical" | "column" => Axis::Vertical,
                    _ => Axis::None,
                },
                gap: number(value, &["itemSpacing", "gap", "spacing"]).unwrap_or(0.0).max(0.0) as f32,
                padding: Padding {
                    top: number(value, &["paddingTop"]).unwrap_or(0.0) as f32,
                    right: number(value, &["paddingRight"]).unwrap_or(0.0) as f32,
                    bottom: number(value, &["paddingBottom"]).unwrap_or(0.0) as f32,
                    left: number(value, &["paddingLeft", "padding"]).unwrap_or(0.0) as f32,
                },
                align: parse_align(
                    &text(value, &["primaryAxisAlignItems", "counterAxisAlignItems"]).unwrap_or_default(),
                ),
            })
        }
        other => {
            state.warn(
                WarningCode::UnsupportedNode,
                format!("layer type '{other}' is exported as a rectangle"),
                Some(id),
            );
            Kind::Shape(ShapeKind::Rectangle)
        }
    }
}

fn image_kind(value: &Value, id: NodeId, state: &mut ImportState<'_>) -> Kind {
    let hash = text(value, &["imageHash", "image", "name"])
        .unwrap_or_else(|| format!("image-{id}"));
    match state.context.assets.iter().find(|item| item.name == hash) {
        Some(asset) => Kind::Image { asset: asset.id },
        None => {
            state.warn(
                WarningCode::MissingAsset,
                format!("image '{hash}' was not supplied; exporting a grey rectangle"),
                Some(id),
            );
            Kind::Shape(ShapeKind::Rectangle)
        }
    }
}

fn parse_align(value: &str) -> Align {
    match value.to_ascii_lowercase().as_str() {
        "center" | "middle" => Align::Center,
        "right" | "end" => Align::End,
        "space_between" | "spacebetween" => Align::SpaceBetween,
        _ => Align::Start,
    }
}

/// Extract `{{key}}` bindings from text in one pass.
pub fn extract_bindings(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let key = &after[..end];
                if !key.is_empty() && !found.iter().any(|existing| existing == key) {
                    found.push(key.to_string());
                }
                rest = &after[end + 2..];
            }
            None => break,
        }
    }
    found
}

/// Parse `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA`, `{r,g,b,a}` (0..1 or 0..255)
/// and `[r,g,b,a]` colour representations.
pub fn parse_color(value: &Value) -> Option<Color> {
    match value {
        Value::String(hex) => parse_hex(hex.trim().trim_start_matches('#')),
        Value::Object(map) => {
            let channel = |key: &str| -> Option<f64> {
                map.get(key).and_then(Value::as_f64).or_else(|| {
                    map.get(key).and_then(Value::as_u64).map(|value| value as f64)
                })
            };
            let rgb = [channel("r")?, channel("g")?, channel("b")?];
            let alpha = channel("a");
            Some(colour_from_channels(rgb, alpha))
        }
        Value::Array(items) if items.len() >= 3 => {
            let channel = |index: usize| -> Option<f64> {
                items.get(index).and_then(Value::as_f64).or_else(|| {
                    items.get(index).and_then(Value::as_u64).map(|value| value as f64)
                })
            };
            let rgb = [channel(0)?, channel(1)?, channel(2)?];
            let alpha = items.get(3).and_then(Value::as_f64);
            Some(colour_from_channels(rgb, alpha))
        }
        _ => None,
    }
}

/// If every supplied channel is at most 1.0, the colour is normalised (0..1)
/// and is scaled to 0..255. Otherwise it is already in byte form.
fn colour_from_channels(rgb: [f64; 3], alpha: Option<f64>) -> Color {
    let normalised = rgb.iter().chain(alpha.iter()).all(|value| *value <= 1.0);
    let scale = |value: f64| -> u8 {
        (if normalised { value * 255.0 } else { value }).clamp(0.0, 255.0) as u8
    };
    Color::new(
        scale(rgb[0]),
        scale(rgb[1]),
        scale(rgb[2]),
        scale(alpha.unwrap_or(if normalised { 1.0 } else { 255.0 })),
    )
}

fn parse_hex(hex: &str) -> Option<Color> {
    let expanded;
    let full = match hex.len() {
        3 => {
            expanded = format!(
                "{}{}{}{}{}{}",
                &hex[0..1], &hex[0..1], &hex[1..2], &hex[1..2], &hex[2..3], &hex[2..3]
            );
            expanded.as_str()
        }
        4 => {
            expanded = format!(
                "{}{}{}{}{}{}{}{}",
                &hex[0..1], &hex[0..1], &hex[1..2], &hex[1..2], &hex[2..3], &hex[2..3],
                &hex[3..4], &hex[3..4]
            );
            expanded.as_str()
        }
        6 | 8 => hex,
        _ => return None,
    };
    let channel = |from: usize, to: usize| u8::from_str_radix(&full[from..to], 16).ok();
    Some(Color::new(
        channel(0, 2)?,
        channel(2, 4)?,
        channel(4, 6)?,
        if full.len() == 8 { channel(6, 8)? } else { 255 },
    ))
}

fn get<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .filter(|found| !found.is_null())
}

fn number(value: &Value, keys: &[&str]) -> Option<f64> {
    get(value, keys).and_then(Value::as_f64)
}

fn text(value: &Value, keys: &[&str]) -> Option<String> {
    get(value, keys)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn boolean(value: &Value, keys: &[&str]) -> Option<bool> {
    get(value, keys).and_then(Value::as_bool)
}
