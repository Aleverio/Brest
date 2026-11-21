//! FigCraft plugin: export the selected layers (or the whole current page) as a
//! Rust Cargo project with the three modules described in the project README:
//! `component`, `caller` and `renderer` (CPU / Skia / GPU).
//!
//! The plugin only reads the document. The generated archive is handed to
//! FigCraft with `save_file`, so the user picks where it goes.

extern crate alloc;

pub mod codegen;
pub mod model;
pub mod zip;

use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;
use figcraft_plugin_sdk::{Event, Input, Result, Value, call, entry, json, log, notify, save_file, set_setting, setting, show_ui};

use codegen::Options;

fn run(input: Input) -> Result<Value> {
    if let Some(event) = input.event.clone() {
        return on_event(&input, event);
    }
    if input.command == "panel" {
        return panel(&Value::Null);
    }
    let options = Options::from_params(&input.params);
    let _ = set_setting("backend", &json!(options.backend));
    let _ = set_setting("crate", &json!(options.crate_name));
    export(&input.selection, options)
}

fn panel(values: &Value) -> Result<Value> {
    let backend = values
        .get("backend")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| setting("backend").and_then(|v| v.as_str().map(ToString::to_string)))
        .unwrap_or_else(|| "Skia".to_string());
    let crate_name = values
        .get("crate")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| setting("crate").and_then(|v| v.as_str().map(ToString::to_string)))
        .unwrap_or_else(|| "figcraft_design".to_string());
    let options = ["Skia", "CPU", "GPU"];
    show_ui(&json!({"title": "Export to Rust", "controls": [
        {"type": "select", "name": "backend", "label": "Renderer", "options": options, "value": backend},
        {"type": "text", "name": "crate", "label": "Crate name", "value": crate_name},
        {"type": "note", "label": "Exports a Cargo project with component.rs, caller.rs and a renderer for CPU, Skia or GPU."},
        {"type": "button", "name": "export", "label": "Export to Rust"},
        {"type": "button", "name": "close", "label": "Close"}
    ]}))
    .map(|()| Value::Null)
}

fn on_event(input: &Input, event: Event) -> Result<Value> {
    match event.control.as_str() {
        "backend" | "crate" => {
            if let Some(backend) = event.values.get("backend") {
                let _ = set_setting("backend", backend);
            }
            if let Some(name) = event.values.get("crate") {
                let _ = set_setting("crate", name);
            }
            panel(&event.values)
        }
        "export" => {
            let options = Options {
                crate_name: codegen::crate_name(event.values.get("crate").and_then(Value::as_str).unwrap_or("figcraft_design")),
                backend: codegen::backend(event.values.get("backend").and_then(Value::as_str).unwrap_or("Skia")),
            };
            let _ = set_setting("backend", &json!(options.backend));
            let _ = set_setting("crate", &json!(options.crate_name));
            export(&input.selection, options)
        }
        "close" => {
            figcraft_plugin_sdk::close_ui();
            Ok(Value::Null)
        }
        _ => Ok(Value::Null),
    }
}

/// The id of the page currently open, from `page.list` (its item has
/// `"current": true`) with `inspect.designContext` as a fallback.
fn current_page() -> Result<u64> {
    if let Ok(pages) = call("page.list", &json!({})) {
        if let Some(list) = pages.as_array() {
            for page in list {
                if page.get("current").and_then(Value::as_bool).unwrap_or(false) {
                    if let Some(id) = page.get("id").and_then(Value::as_u64) {
                        return Ok(id);
                    }
                }
            }
        }
    }
    if let Ok(context) = call("inspect.designContext", &json!({"scope": "page"})) {
        if let Some(id) = context["document"]["page"]["id"].as_u64() {
            return Ok(id);
        }
    }
    Err("Could not find the current page".into())
}

/// Find the layers to export: the selection, or every top-level layer of the
/// current page when nothing is selected.
fn roots(selection: &[u64]) -> Result<Vec<Value>> {
    let ids: Vec<u64> = if selection.is_empty() {
        let page = call("node.get", &json!({"id": current_page()?}))?;
        let children = page["children"].as_array();
        let found = children
            .map(|list| list.iter().filter_map(|c| c.get("id").and_then(Value::as_u64)).collect::<Vec<_>>())
            .unwrap_or_default();
        if found.is_empty() {
            vec![page["id"].as_u64().ok_or("the page has no id")?]
        } else {
            found
        }
    } else {
        selection.to_vec()
    };
    if ids.is_empty() {
        return Err("Select the layers to export first".into());
    }
    let mut counter = ids.len();
    let mut nodes = Vec::with_capacity(ids.len());
    for id in ids {
        let node = call("node.get", &json!({"id": id}))?;
        nodes.push(expand_node(node, 0, &mut counter)?);
    }
    Ok(nodes)
}

/// `node.get` returns one level of children. Fetch each child again so the
/// exported component keeps the full layer tree (bounded by the plugin limits).
fn expand_node(node: Value, depth: usize, counter: &mut usize) -> Result<Value> {
    let ids = child_ids(&node);
    let reported = node.get("childCount").and_then(Value::as_u64).unwrap_or(0);
    if ids.is_empty() && reported == 0 {
        return Ok(node);
    }
    if depth >= model::MAX_DEPTH || *counter >= model::MAX_NODES {
        return Ok(node);
    }

    let mut children = Vec::with_capacity(ids.len());
    for id in ids {
        if *counter >= model::MAX_NODES {
            break;
        }
        *counter += 1;
        let child = call("node.get", &json!({"id": id}))?;
        children.push(expand_node(child, depth + 1, counter)?);
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

fn export(selection: &[u64], options: Options) -> Result<Value> {
    let nodes = roots(selection)?;
    let name = nodes
        .first()
        .and_then(|n| n.get("name").and_then(Value::as_str))
        .unwrap_or("FigCraft design")
        .to_string();
    let mut counter = 0usize;
    let parsed = nodes.iter().map(|node| model::parse_node(node, 0, &mut counter)).collect::<Vec<_>>();
    let scene = model::scene(name, parsed);
    let mut files = codegen::project(&scene, &options)?;
    // Keep FigCraft's own CSS output beside the Rust project as a reference.
    if let Some(id) = scene.roots.first().map(|node| node.id) {
        if let Ok(reply) = call("inspect.code", &json!({"id": id, "language": "css"})) {
            if let Some(code) = reply.get("code").and_then(Value::as_str) {
                files.push(zip::Entry::new("reference/figcraft.css", code.as_bytes().to_vec()));
            }
        }
    }
    let bytes = zip::archive(&files)?;
    let file_name = format!("{}.zip", options.crate_name);
    save_file(&file_name, "application/zip", &bytes)?;
    notify(&format!("Exporting {} layers to Rust ({})", scene.count, options.backend));
    log(&format!(
        "exported {} layers, {} placeholders, {} files, {} bytes",
        scene.count,
        scene.bindings.len(),
        files.len(),
        bytes.len()
    ));
    Ok(json!({
        "crate": options.crate_name,
        "backend": options.backend,
        "layers": scene.count,
        "bindings": scene.bindings,
        "files": files.len(),
        "bytes": bytes.len(),
        "archive": file_name
    }))
}

entry!(run);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_come_from_params() {
        let options = Options::from_params(&json!({"backend": "CPU", "crate": "My App!"}));
        assert_eq!(options.backend, "cpu");
        assert_eq!(options.crate_name, "my_app");
    }

    #[test]
    fn child_ids_come_from_one_level_children() {
        let node = json!({"id": 1, "children": [{"id": 2}, {"name": "no id"}]});
        assert_eq!(child_ids(&node), vec![2]);
        assert!(child_ids(&json!({"id": 1, "childCount": 4})).is_empty());
    }
}
