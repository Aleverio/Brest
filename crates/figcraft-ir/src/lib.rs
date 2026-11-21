//! Versioned intermediate representation (IR) between FigCraft and the Rust
//! code emitter.
//!
//! This crate has **no dependencies** beyond `alloc`. It is deliberately a
//! dumb data container: parsing, layout and code generation live in separate
//! crates. Every design carries its own IR version and generator version so a
//! generated project can tell which rules produced it.
//!
//! Coordinates are always **relative to the parent node**. The importer is
//! responsible for converting FigCraft's absolute bounds into this form.

use std::string::{String, ToString};
use std::vec::Vec;

/// Bump when the IR layout changes in an incompatible way.
pub const IR_VERSION: u16 = 1;
pub const GENERATOR_NAME: &str = "figcraft-rust-export";
pub const GENERATOR_VERSION: &str = "2.0.0";

pub type NodeId = u64;
pub type AssetId = u32;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// A size that is safe to use as a render target.
    pub fn clamped(self) -> Self {
        Self {
            width: self.width.clamp(1.0, 16_384.0),
            height: self.height.clamp(1.0, 16_384.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub const fn left(self) -> f32 {
        self.origin.x
    }

    pub const fn top(self) -> f32 {
        self.origin.y
    }

    pub const fn right(self) -> f32 {
        self.origin.x + self.size.width
    }

    pub const fn bottom(self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Translate a rectangle by a parent origin.
    pub const fn offset(self, by: Point) -> Self {
        Self {
            origin: Point::new(self.origin.x + by.x, self.origin.y + by.y),
            size: self.size,
        }
    }

    /// Smallest rectangle containing both inputs.
    pub fn union(self, other: Self) -> Self {
        let left = self.left().min(other.left());
        let top = self.top().min(other.top());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(left, top, right - left, bottom - top)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const BLACK: Self = Self::new(0, 0, 0, 255);
    pub const WHITE: Self = Self::new(255, 255, 255, 255);
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);
}

/// A paint is deliberately narrow in v1. Anything else becomes a warning and
/// a nearest-match solid colour instead of silently generating wrong code.
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Solid(Color),
    Image(AssetId),
    Unsupported(UnsupportedPaint),
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnsupportedPaint {
    pub kind: String,
    pub fallback: Color,
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
    SpaceBetween,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub fill: Option<Paint>,
    pub stroke: Option<Paint>,
    pub stroke_width: f32,
    pub corner_radius: f32,
    pub opacity: f32,
    pub clips_content: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
    Line,
    Vector,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlignVertical {
    #[default]
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextSpec {
    /// Raw text including `{{key}}` placeholders.
    pub text: String,
    pub font_family: String,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub align: Align,
    pub vertical_align: TextAlignVertical,
    pub line_height: f32,
    pub color: Color,
    pub bindings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameSpec {
    pub layout: Axis,
    pub gap: f32,
    pub padding: Padding,
    pub align: Align,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Padding {
    pub const fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

/// Widget kinds that the generated UI can drive with `State`.
///
/// Design tools do not have native buttons or text fields; these come from
/// component/instance names, component properties and prototype interactions.
/// The importer records its guess in a warning so the user can correct it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WidgetKind {
    Button,
    TextInput,
    TextArea,
    Checkbox,
    RadioButton,
    Switch,
    Slider,
    ProgressBar,
    Dropdown,
    ListBox,
    Menu,
    MenuItem,
    Tabs,
    TabItem,
    Tooltip,
    Modal,
    Accordion,
    Custom(String),
}

impl WidgetKind {
    /// Guess a widget kind from a FigCraft layer name.
    pub fn from_name(name: &str) -> Self {
        let needle = name.to_ascii_lowercase();
        let has = |word: &str| needle.contains(word);
        if has("textarea") || has("text area") {
            Self::TextArea
        } else if has("input") || has("textfield") || has("text field") {
            Self::TextInput
        } else if has("checkbox") || has("check box") {
            Self::Checkbox
        } else if has("radio") {
            Self::RadioButton
        } else if has("switch") || has("toggle") {
            Self::Switch
        } else if has("slider") {
            Self::Slider
        } else if has("progress") {
            Self::ProgressBar
        } else if has("dropdown") || has("select") || has("combobox") {
            Self::Dropdown
        } else if has("listbox") || has("list item") || has("list") {
            Self::ListBox
        } else if has("menuitem") || has("menu item") {
            Self::MenuItem
        } else if has("menu") {
            Self::Menu
        } else if has("tab item") || has("tabitem") {
            Self::TabItem
        } else if has("tabs") || has("tab bar") {
            Self::Tabs
        } else if has("tooltip") {
            Self::Tooltip
        } else if has("modal") || has("dialog") {
            Self::Modal
        } else if has("accordion") || has("disclosure") {
            Self::Accordion
        } else if has("button") || has("btn") {
            Self::Button
        } else {
            Self::Custom(name.to_string())
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Button => "Button",
            Self::TextInput => "TextInput",
            Self::TextArea => "TextArea",
            Self::Checkbox => "Checkbox",
            Self::RadioButton => "RadioButton",
            Self::Switch => "Switch",
            Self::Slider => "Slider",
            Self::ProgressBar => "ProgressBar",
            Self::Dropdown => "Dropdown",
            Self::ListBox => "ListBox",
            Self::Menu => "Menu",
            Self::MenuItem => "MenuItem",
            Self::Tabs => "Tabs",
            Self::TabItem => "TabItem",
            Self::Tooltip => "Tooltip",
            Self::Modal => "Modal",
            Self::Accordion => "Accordion",
            Self::Custom(_) => "Custom",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WidgetSpec {
    pub kind: WidgetKind,
    pub label: Option<String>,
    pub binding: Option<String>,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Frame(FrameSpec),
    Shape(ShapeKind),
    Text(TextSpec),
    Image { asset: AssetId },
    Widget(WidgetSpec),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    /// Always relative to the parent frame.
    pub rect: Rect,
    pub style: Style,
    pub kind: Kind,
    pub visible: bool,
    pub locked: bool,
    pub children: Vec<Node>,
}

impl Node {
    pub fn walk<'a>(&'a self, visit: &mut impl FnMut(&'a Self)) {
        visit(self);
        for child in &self.children {
            child.walk(visit);
        }
    }

    pub fn count(&self) -> usize {
        let mut total = 0;
        self.walk(&mut |_| total += 1);
        total
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub id: AssetId,
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningCode {
    UnsupportedPaint,
    UnsupportedNode,
    Rotation,
    MissingAsset,
    WidgetGuess,
    FontFallback,
    LimitReached,
}

impl WarningCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedPaint => "unsupported_paint",
            Self::UnsupportedNode => "unsupported_node",
            Self::Rotation => "rotation_ignored",
            Self::MissingAsset => "missing_asset",
            Self::WidgetGuess => "widget_guess",
            Self::FontFallback => "font_fallback",
            Self::LimitReached => "limit_reached",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Warning {
    pub code: WarningCode,
    pub message: String,
    pub node: Option<NodeId>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceInfo {
    pub page_name: Option<String>,
    pub selection: Vec<NodeId>,
    pub code_reference: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Design {
    pub version: u16,
    pub generator: String,
    pub generator_version: String,
    pub name: String,
    pub size: Size,
    pub roots: Vec<Node>,
    pub assets: Vec<Asset>,
    pub warnings: Vec<Warning>,
    pub source: SourceInfo,
}

impl Design {
    pub fn new(name: String) -> Self {
        Self {
            version: IR_VERSION,
            generator: String::from(GENERATOR_NAME),
            generator_version: String::from(GENERATOR_VERSION),
            name,
            size: Size::default(),
            roots: Vec::new(),
            assets: Vec::new(),
            warnings: Vec::new(),
            source: SourceInfo::default(),
        }
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        fn find(nodes: &[Node], id: NodeId) -> Option<&Node> {
            for node in nodes {
                if node.id == id {
                    return Some(node);
                }
                if let Some(found) = find(&node.children, id) {
                    return Some(found);
                }
            }
            None
        }
        find(&self.roots, id)
    }

    pub fn node_count(&self) -> usize {
        self.roots.iter().map(Node::count).sum()
    }

    pub fn bindings(&self) -> Vec<String> {
        let mut found = Vec::new();
        for root in &self.roots {
            root.walk(&mut |node| {
                if let Kind::Text(spec) = &node.kind {
                    for key in &spec.bindings {
                        if !found.iter().any(|existing| existing == key) {
                            found.push(key.clone());
                        }
                    }
                }
                if let Kind::Widget(spec) = &node.kind
                    && let Some(key) = &spec.binding
                    && !found.iter().any(|existing| existing == key)
                {
                    found.push(key.clone());
                }
            });
        }
        found
    }

    /// Structural checks that do not need the original JSON.
    pub fn validate(&self) -> Vec<Warning> {
        let mut problems = Vec::new();
        if self.version != IR_VERSION {
            problems.push(Warning {
                code: WarningCode::UnsupportedNode,
                message: format!(
                    "IR version {} cannot be read by a generator expecting {}",
                    self.version,
                    IR_VERSION
                ),
                node: None,
            });
        }
        if self.roots.is_empty() {
            problems.push(Warning {
                code: WarningCode::UnsupportedNode,
                message: String::from("the design has no visible root layer"),
                node: None,
            });
        }
        let mut seen = Vec::new();
        for root in &self.roots {
            root.walk(&mut |node| {
                if seen.contains(&node.id) {
                    problems.push(Warning {
                        code: WarningCode::UnsupportedNode,
                        message: format!("duplicate node id {}", node.id),
                        node: Some(node.id),
                    });
                } else {
                    seen.push(node.id);
                }
                if let Kind::Image { asset } = node.kind
                    && !self.assets.iter().any(|item| item.id == asset)
                {
                    problems.push(Warning {
                        code: WarningCode::MissingAsset,
                        message: format!("image asset {asset} is missing"),
                        node: Some(node.id),
                    });
                }
            });
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_union_covers_both_boxes() {
        let a = Rect::new(10.0, 20.0, 30.0, 40.0);
        let b = Rect::new(0.0, 0.0, 20.0, 20.0);
        assert_eq!(a.union(b), Rect::new(0.0, 0.0, 40.0, 60.0));
    }

    #[test]
    fn widget_names_are_recognised() {
        assert_eq!(WidgetKind::from_name("Primary Button"), WidgetKind::Button);
        assert_eq!(WidgetKind::from_name("Email input"), WidgetKind::TextInput);
        assert_eq!(WidgetKind::from_name("Remember me checkbox"), WidgetKind::Checkbox);
    }

    #[test]
    fn design_reports_missing_assets() {
        let mut design = Design::new(String::from("test"));
        design.roots.push(Node {
            id: 1,
            name: String::from("image"),
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            style: Style::default(),
            kind: Kind::Image { asset: 99 },
            visible: true,
            locked: false,
            children: Vec::new(),
        });
        let warnings = design.validate();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, WarningCode::MissingAsset);
    }
}
