# {{CRATE}}

UI Rust hasil export dari FigCraft oleh **{{GENERATOR}} v{{GENERATOR_VERSION}}**
(IR v{{IR_VERSION}}). Desain: **{{SCENE_NAME}}** ({{NODE_COUNT}} layer).

## Jalankan

```sh
cargo run                         # jendela desktop dengan backend {{DEFAULT_BACKEND}}
cargo run -- --backend cpu        # pilih backend saat runtime
cargo run -- --png                # render satu frame ke {{CRATE}}.png
cargo run -- --frames 3           # smoke test: render 3 frame tanpa jendela
cargo run -- --help
```

Backend yang tersedia: `cpu` (tanpa dependency), `skia` (canvas 2D nyata),
`gpu` (wgpu/offscreen). Jika sebuah backend tidak dikompilasi, pesan error
yang jelas akan ditampilkan.

Build ringan tanpa Skia/wgpu:

```sh
cargo run --no-default-features --features "cpu,ui"
```

Tanpa jendela sama sekali (cocok untuk server/CI):

```sh
cargo run --no-default-features --features cpu -- --png
```

## Peta modul

| Folder | Nama spesifikasi | Tugas |
| --- | --- | --- |
| `src/component/` | `komponen` | data UI, state, event. Tidak tahu cara menggambar. |
| `src/caller/` | `pemanggil` | layout, resolve binding, hit-test, susun `DrawList`. |
| `src/renderer/` | `penggambar` | `DrawList` → `Frame` RGBA8 (CPU/Skia/GPU). |
| `src/core.rs` | — | `Point`, `Size`, `Rect`, `Color`, `Frame` (daun). |
| `src/app.rs` | — | shell winit, input, present, shortcut. |
| `src/design/` | — | **@generated** data desain. Ditimpa saat export ulang. |
| `src/user/` | — | milik Anda. **Tidak pernah ditimpa** saat export ulang. |

## Interaksi

- Klik / hover pada widget mengubah `State`.
- `R` render ulang, `S` simpan PNG, `Esc` keluar.
- Ubah perilaku di `src/user/handlers.rs`.

## Export ulang

File dengan header `@generated` aman ditimpa. `src/user/` tidak pernah
disentuh. Lihat `GENERATED.md` untuk daftarnya.
