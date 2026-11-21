# {{CRATE}}

Exported from a FigCraft design with **figcraft-rust-export**. Scene: `{{SCENE_NAME}}`
({{NODE_COUNT}} layers). Default renderer: **{{BACKEND}}**.

## The three modules

| Module | File | What it does |
| --- | --- | --- |
| `komponen` (component) | `src/component.rs` | The UI tree captured from FigCraft: layers, geometry, styles, text and `{{key}}` bindings. Set values with `Component::set`. |
| `pemanggil` (caller) | `src/caller.rs` | Walks the tree, resolves layout and placeholders, and turns each layer into draw calls for the renderer. |
| `penggambar` (renderer) | `src/renderer/` | Draws those calls. `cpu.rs` is a dependency-free software rasteriser, `skia.rs` uses Skia, `gpu.rs` uses wgpu. |

## Run it

```sh
cargo run                 # {{BACKEND}} (the backend chosen at export time)
cargo run --no-default-features --features cpu    # software rasteriser
cargo run --no-default-features --features skia   # Skia canvas
cargo run --no-default-features --features gpu    # wgpu, offscreen
```

The result is written to `{{CRATE}}.png`.

## Insert values

Text layers exported from FigCraft can contain `{{name}}` placeholders. Fill them
before drawing:

```rust
let mut caller = caller::Caller::new(renderer::cpu::CpuRenderer::new());
caller.set("title", "Hello from Rust");
let png = caller.draw(&component::build())?;
```

Only the CPU backend is enabled by default in this export. Skia and wgpu are
optional dependencies: if their APIs have moved on, adjust `src/renderer/skia.rs`
or `src/renderer/gpu.rs` - the component and caller modules need no changes.
