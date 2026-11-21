//! FigCraft host adapter for the v2 Rust exporter.
//!
//! This crate intentionally contains no code generation. It is a thin bridge:
//! read the document through the FigCraft SDK → `figcraft-import` → `figcraft-emit`
//! → `figcraft-zip` → `save_file`.

#![cfg_attr(test, allow(clippy::expect_used))]

use std::collections::BTreeSet;

use figcraft_emit::{Backend, EmitOptions, sanitize_crate_name};
use figcraft_import::{ImportContext, ImportLimits};
use figcraft_ir::{Asset, Design};
use figcraft_plugin_sdk::{Event, Input, Result, Value, call, close_ui, entry, json, log, notify, save_file, set_setting, setting, show_ui};
use serde_json::to_string;

pub use figcraft_emit::ProjectManifest;

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub crate_name: String,
    pub backend: Backend,
    pub code_reference: bool,
}

impl Options {
    pub fn new(crate_name: &str, backend: &str) -> core::result::Result<Self, String> {
        let backend = Backend::parse(backend).map_err(|error| error.to_string())?;
        Ok(Self {
            crate_name: sanitize_crate_name(crate_name),
            backend,
            code_reference: true,
        })
    }

    pub fn from_params(value: &Value) -> core::result::Result<Self, String> {
        let crate_name = text(value, &["crate", "crateName", "name"])
            .unwrap_or_else(|| "figcraft_design".into());
        let backend = text(value, &["backend", "renderer"])
            .unwrap_or_else(|| "cpu".into());
        Self::new(&crate_name, &backend)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportResult {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub layers: usize,
    pub bindings: usize,
    pub warnings: usize,
    pub files: usize,
    pub manifest: ProjectManifest,
}

fn run(input: Input) -> Result<Value> {
    if let Some(event) = input.event.clone() {
        return on_event(&input, event);
    }
    if input.command == "panel" {
        return panel(&Value::Null);
    }
    let options = Options::from_params(&input.params)?;
    let _ = set_setting("backend", &json!(options.backend.as_str()));
    let _ = set_setting("crate", &json!(options.crate_name));
    export(&input.selection, &options).map(|result| result_summary(&result))
}

fn result_summary(result: &ExportResult) -> Value {
    json!({
        "crate": result.manifest.crate_name,
        "backend": result.manifest.default_backend,
        "layers": result.layers,
        "bindings": result.bindings,
        "warnings": result.warnings,
        "files": result.files,
        "bytes": result.bytes.len(),
        "archive": result.file_name,
    })
}

fn panel(values: &Value) -> Result<Value> {
    let backend = text(values, &["backend"])
        .or_else(|| setting_text("backend"))
        .unwrap_or_else(|| "cpu".into());
    let backend = Backend::parse(&backend)
        .map(|value| display_backend(value).to_string())
        .unwrap_or_else(|_| "CPU".to_string());
    let crate_name = text(values, &["crate"])
        .or_else(|| setting_text("crate"))
        .unwrap_or_else(|| "figcraft_design".into());
    let code_reference = values
        .get("code_reference")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    show_ui(&json!({
        "title": "Export to Rust (v2)",
        "controls": [
            {"type": "select", "name": "backend", "label": "Default renderer", "options": ["CPU", "Skia", "GPU"], "value": backend},
            {"type": "text", "name": "crate", "label": "Crate name", "value": crate_name},
            {"type": "checkbox", "name": "code_reference", "label": "Include FigCraft CSS reference", "value": code_reference},
            {"type": "note", "label": "All three renderers are compiled. Switch at runtime with --backend cpu|skia|gpu. src/user/ is never overwritten by sync.ps1."},
            {"type": "button", "name": "export", "label": "Export to Rust"},
            {"type": "button", "name": "close", "label": "Close"}
        ]
    }))
    .map(|()| Value::Null)
}

fn on_event(input: &Input, event: Event) -> Result<Value> {
    match event.control.as_str() {
        "backend" | "crate" | "code_reference" => {
            if let Some(backend) = event.values.get("backend") {
                let _ = set_setting("backend", backend);
            }
            if let Some(name) = event.values.get("crate") {
                let _ = set_setting("crate", name);
            }
            panel(&event.values)
        }
        "export" => {
            let options = options_from_event(&event.values)?;
            let _ = set_setting("backend", &json!(options.backend.as_str()));
            let _ = set_setting("crate", &json!(options.crate_name));
            export(&input.selection, &options).map(|result| result_summary(&result))
        }
        "close" => {
            close_ui();
            Ok(Value::Null)
        }
        _ => Ok(Value::Null),
    }
}

fn options_from_event(values: &Value) -> core::result::Result<Options, String> {
    let crate_name = text(values, &["crate"])
        .or_else(|| setting_text("crate"))
        .unwrap_or_else(|| "figcraft_design".into());
    let backend = text(values, &["backend"])
        .or_else(|| setting_text("backend"))
        .unwrap_or_else(|| "cpu".into());
    let mut options = Options::new(&crate_name, &backend)?;
    if let Some(value) = values.get("code_reference").and_then(Value::as_bool) {
        options.code_reference = value;
    }
    Ok(options)
}

fn display_backend(backend: Backend) -> &'static str {
    match backend {
        Backend::Cpu => "CPU",
        Backend::Skia => "Skia",
        Backend::Gpu => "GPU",
    }
}

fn text(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn setting_text(key: &str) -> Option<String> {
    setting(key).and_then(|value| value.as_str().map(ToString::to_string))
}

/// Id of the current page from `page.list`, with `inspect.designContext` as a fallback.
fn current_page() -> Result<u64> {
    if let Ok(pages) = call("page.list", &json!({}))
        && let Some(list) = pages.as_array()
    {
        for page in list {
            if page.get("current").and_then(Value::as_bool).unwrap_or(false)
                && let Some(id) = page.get("id").and_then(Value::as_u64)
            {
                return Ok(id);
            }
        }
    }
    if let Ok(context) = call("inspect.designContext", &json!({"scope": "page"}))
        && let Some(id) = context["document"]["page"]["id"].as_u64()
    {
        return Ok(id);
    }
    Err("Could not find the current page".into())
}

/// Expanded roots and the current page name, when the page is known.
fn roots(selection: &[u64]) -> Result<(Vec<Value>, Option<String>)> {
    let limits = ImportLimits::default();
    let mut page_name = None;
    let pending: Vec<u64> = if selection.is_empty() {
        let page = call("node.get", &json!({"id": current_page()?}))?;
        page_name = page.get("name").and_then(Value::as_str).map(ToString::to_string);
        let found = page
            .get("children")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|child| child.get("id").and_then(Value::as_u64)).collect::<Vec<_>>())
            .unwrap_or_default();
        if found.is_empty() {
            vec![page["id"].as_u64().ok_or("the page has no id")?]
        } else {
            found
        }
    } else {
        selection.to_vec()
    };
    if pending.is_empty() {
        return Err("Select the layers to export first".into());
    }

    let mut counter = pending.len();
    let mut nodes = Vec::with_capacity(pending.len());
    for id in pending {
        let node = call("node.get", &json!({"id": id}))?;
        nodes.push(expand_node(node, 0, &limits, &mut counter)?);
    }
    Ok((nodes, page_name))
}

/// `node.get` returns one level of children. Fetch each child again so the
/// exported component keeps the full layer tree (bounded by the plugin limits).
fn expand_node(node: Value, depth: usize, limits: &ImportLimits, counter: &mut usize) -> Result<Value> {
    let ids = child_ids(&node);
    let reported = node.get("childCount").and_then(Value::as_u64).unwrap_or(0);
    if (ids.is_empty() && reported == 0) || depth >= limits.max_depth || *counter >= limits.max_nodes {
        return Ok(node);
    }

    let mut children = Vec::with_capacity(ids.len());
    for id in ids {
        if *counter >= limits.max_nodes {
            break;
        }
        *counter += 1;
        let child = call("node.get", &json!({"id": id}))?;
        children.push(expand_node(child, depth + 1, limits, counter)?);
    }

    let mut result = node;
    if let Some(object) = result.as_object_mut() {
        object.insert("children".to_string(), Value::Array(children));
    }
    Ok(result)
}

fn child_ids(node: &Value) -> Vec<u64> {
    node.get("children")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(|child| child.get("id").and_then(Value::as_u64)).collect())
        .unwrap_or_default()
}

fn export(selection: &[u64], options: &Options) -> core::result::Result<ExportResult, String> {
    let (nodes, page_name) = roots(selection)?;
    let code_reference = if options.code_reference {
        nodes.first().and_then(|node| node.get("id")).and_then(Value::as_u64).and_then(|id| {
            call("inspect.code", &json!({"id": id, "language": "css"}))
                .ok()
                .and_then(|reply| reply.get("code").and_then(Value::as_str).map(ToString::to_string))
        })
    } else {
        None
    };
    let result = build_archive(&nodes, page_name, selection, options, code_reference.as_deref())?;
    let file_name = result.file_name.clone();
    save_file(&file_name, "application/zip", &result.bytes)?;
    notify(&format!(
        "Exporting {} layers to Rust (default: {})",
        result.layers,
        display_backend(options.backend)
    ));
    log(&format!(
        "v2 export: {} layers, {} bindings, {} warnings, {} files, {} bytes",
        result.layers, result.bindings, result.warnings, result.files, result.bytes.len()
    ));
    Ok(result)
}

/// Pure host-side pipeline, used by `export` and by the sample generator.
pub fn build_archive(
    nodes: &[Value],
    page_name: Option<String>,
    selection: &[u64],
    options: &Options,
    code_reference: Option<&str>,
) -> core::result::Result<ExportResult, String> {
    let raw = to_string(nodes).map_err(|error| format!("could not serialize the FigCraft document: {error}"))?;
    let assets = host_assets(nodes);
    let name = page_name
        .or_else(|| nodes.first().and_then(|node| node.get("name")).and_then(Value::as_str).map(ToString::to_string))
        .unwrap_or_else(|| "FigCraft design".into());
    let context = ImportContext {
        page_name: Some(name),
        selection: selection.to_vec(),
        code_reference: code_reference.is_some(),
        assets,
        limits: ImportLimits::default(),
    };
    let mut design: Design = figcraft_import::import(&raw, &context).map_err(|error| error.to_string())?;
    let validation = design.validate();
    design.warnings.extend(validation);

    let emit_options = EmitOptions {
        crate_name: options.crate_name.clone(),
        default_backend: options.backend,
        include_reference_css: options.code_reference,
    };
    let project = figcraft_emit::emit(&design, &emit_options).map_err(|error| error.to_string())?;
    let root = format!("{}/", options.crate_name);

    let mut entries: Vec<(String, Vec<u8>, &'static str)> = project
        .files
        .iter()
        .map(|file| {
            let owner = if file.owner == figcraft_emit::FileOwner::User { "user" } else { "generated" };
            (file.path.clone(), file.data.clone(), owner)
        })
        .collect();
    if let Some(css) = code_reference {
        entries.push((format!("{root}reference/figcraft.css"), css.as_bytes().to_vec(), "generated"));
    }

    let file_list: Vec<Value> = entries
        .iter()
        .map(|(path, _, owner)| json!({"path": path, "owner": owner}))
        .collect();
    let manifest = json!({
        "generator": project.manifest.generator,
        "generator_version": project.manifest.generator_version,
        "ir_version": project.manifest.ir_version,
        "crate": project.manifest.crate_name,
        "default_backend": project.manifest.default_backend,
        "archive_files": entries.len(),
        "files": file_list,
    });
    entries.push((
        format!("{root}figcraft-manifest.json"),
        to_string(&manifest).map_err(|error| format!("could not serialize the manifest: {error}"))?.into_bytes(),
        "generated",
    ));

    let bytes = figcraft_zip::archive(entries.iter().map(|(path, data, _)| (path.as_str(), data.as_slice())))
        .map_err(|error| error.to_string())?;
    Ok(ExportResult {
        bytes,
        file_name: format!("{}.zip", options.crate_name),
        layers: design.node_count(),
        bindings: design.bindings().len(),
        warnings: design.warnings.len(),
        files: entries.len(),
        manifest: project.manifest,
    })
}

/// Image bytes supplied by FigCraft are kept in IR, never as a filesystem path.
/// The host only understands byte arrays; unknown image encodings are skipped
/// with a warning produced by the importer.
fn host_assets(nodes: &[Value]) -> Vec<Asset> {
    let mut assets: Vec<Asset> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for node in nodes {
        collect_assets(node, &mut assets, &mut seen);
    }
    assets
}

fn collect_assets(value: &Value, assets: &mut Vec<Asset>, seen: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_assets(item, assets, seen);
            }
        }
        Value::Object(map) => {
            let name = text(value, &["imageHash", "image", "assetName"])
                .or_else(|| text(value, &["name"]).filter(|_| value.get("type").and_then(Value::as_str) == Some("IMAGE")));
            let bytes = map
                .get("imageBytes")
                .or_else(|| map.get("bytes"))
                .or_else(|| map.get("imageData"))
                .and_then(byte_array);
            if let (Some(name), Some(bytes)) = (name, bytes)
                && !bytes.is_empty()
                && !seen.contains(&name)
            {
                seen.insert(name.clone());
                assets.push(Asset {
                    id: assets.len() as u32 + 1,
                    name,
                    mime: mime_type(&bytes),
                    bytes,
                });
            }
            for child in map.values() {
                collect_assets(child, assets, seen);
            }
        }
        _ => {}
    }
}

fn byte_array(value: &Value) -> Option<Vec<u8>> {
    let items = value.as_array()?;
    if items.is_empty() || items.len() > 8 * 1024 * 1024 {
        return None;
    }
    let mut bytes = Vec::with_capacity(items.len());
    for item in items {
        bytes.push(u8::try_from(item.as_u64()?).ok()?);
    }
    Some(bytes)
}

fn mime_type(bytes: &[u8]) -> String {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png".into()
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg".into()
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif".into()
    } else {
        "application/octet-stream".into()
    }
}

entry!(run);

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../fixtures/figcraft-page.json");

    fn fixture_nodes() -> Vec<Value> {
        vec![serde_json::from_str(FIXTURE).expect("fixture is valid JSON")]
    }

    #[test]
    fn options_come_from_params() {
        let options = Options::from_params(&json!({"backend": "CPU", "crate": "My App!"}))
            .expect("options parse");
        assert_eq!(options.backend, Backend::Cpu);
        assert_eq!(options.crate_name, "my_app");
        assert!(options.code_reference);
    }

    #[test]
    fn child_ids_come_from_one_level_children() {
        let node = json!({"id": 1, "children": [{"id": 2}, {"name": "no id"}]});
        assert_eq!(child_ids(&node), vec![2]);
        assert!(child_ids(&json!({"id": 1, "childCount": 4})).is_empty());
    }

    #[test]
    fn archive_pipeline_is_complete() {
        let options = Options::new("figcraft_sample", "cpu").expect("options parse");
        let result = build_archive(
            &fixture_nodes(),
            Some("Halaman Contoh".into()),
            &[],
            &options,
            Some("body { color: red; }"),
        )
        .expect("build archive");
        assert_eq!(result.manifest.crate_name, "figcraft_sample");
        assert_eq!(result.manifest.default_backend, "cpu");
        assert!(result.layers >= 5);
        assert!(result.bindings >= 1);
        assert!(result.warnings >= 1, "the fixture must report its unsupported gradient");
        assert!(result.bytes.len() > 4_000);
        assert!(result.file_name.ends_with(".zip"));

        let text = String::from_utf8_lossy(&result.bytes);
        assert!(text.contains("figcraft_sample/src/user/handlers.rs"));
        assert!(text.contains("figcraft_sample/figcraft-manifest.json"));
        assert!(text.contains("figcraft_sample/reference/figcraft.css"));
        assert!(text.contains("\"owner\":\"user\""));
    }

    #[test]
    fn host_assets_accept_byte_arrays() {
        let node = json!({
            "id": "1:1",
            "type": "IMAGE",
            "imageHash": "logo",
            "imageBytes": [137, 80, 78, 71, 13, 10, 26, 10]
        });
        let assets = host_assets(&[node]);
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].name, "logo");
        assert_eq!(assets[0].mime, "image/png");
        assert_eq!(assets[0].bytes.len(), 8);
    }

    #[test]
    fn unknown_backend_is_rejected() {
        assert!(Options::new("x", "metal").is_err());
        let error: figcraft_emit::EmitError = Backend::parse("metal").expect_err("invalid backend");
        assert!(error.to_string().contains("metal"));
    }
}
