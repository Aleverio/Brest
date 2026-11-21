# FigCraft → Rust Export (v2)

Plugin FigCraft (WASM) yang mengekspor desain atau selection menjadi **project Rust
yang sungguhan**, bukan cuma penulis PNG. Project hasil export memakai `winit` untuk
window, dengan tiga modul domain:

| Modul | Folder | Tanggung jawab |
| --- | --- | --- |
| `komponen` | `src/component/` | Pohon layer statis, gaya, teks, widget, aset, dan `State`. |
| `pemanggil` | `src/caller/` | Layout → resolve binding → `DrawList` → hit-test. Tidak memiliki renderer. |
| `penggambar` | `src/renderer/` | Backend CPU, Skia, atau GPU yang menghasilkan `Frame` RGBA8. |

`src/app.rs` adalah shell aplikasi (event, state, window, PNG). `src/user/` adalah
tempat kode Anda sendiri dan **tidak pernah ditimpa** saat export ulang.

Arsitektur lengkap dan alasan keputusannya ada di [`docs/ARSITEKTUR.md`](docs/ARSITEKTUR.md).

---

## Bangun plugin

```powershell
./scripts/build.ps1
```

Skrip ini menghasilkan:

- `figcraft-rust-export/figcraft_rust_export.wasm` — modul yang dibaca manifest.
- `dist/figcraft-rust-export-plugin.zip` — paket berisi `figcraft-plugin.json` + WASM.

Build manual:

```sh
cargo build --release --target wasm32-unknown-unknown -p figcraft-rust-export
```

Verifikasi penuh (workspace + project hasil export + WASM):

```powershell
./scripts/verify.ps1
```

---

## Pasang dan pakai di FigCraft

1. Buka **Plugins → Install / Import**.
2. Pilih folder `figcraft-rust-export/`, atau ekstrak `dist/figcraft-rust-export-plugin.zip`.
3. Jalankan perintah lewat **Ctrl+K → Actions → Export to Rust…** atau menu **Plugins**.
   Command id-nya adalah `export`; ada juga `panel` untuk membuka panel opsi.
4. Pilih renderer **default** (CPU / Skia / GPU), nama crate, dan apakah ingin menyertakan
   referensi CSS dari FigCraft.
5. FigCraft akan menawarkan satu file `.zip` untuk disimpan.

Jika tidak ada layer yang dipilih, plugin memakai semua layer tingkat atas pada halaman
aktif (`page.list` → `node.get`). Plugin hanya **membaca** dokumen; semua perubahan
dilakukan FigCraft saat `save_file` dipanggil.

---

## Isi ZIP hasil export

```
figcraft_design/
├─ Cargo.toml                 # default: cpu,skia,gpu,ui
├─ README.md
├─ GENERATED.md               # daftar file yang aman ditimpa
├─ sync.ps1                   # export ulang tanpa menyentuh src/user/
├─ figcraft-manifest.json     # generator, IR versi, owner tiap file
├─ reference/figcraft.css     # opsional, dari inspect.code
└─ src/
   ├─ main.rs
   ├─ app.rs                  # winit + softbuffer + event
   ├─ core.rs                 # Point, Size, Rect, Color, Frame
   ├─ design/mod.rs           # data desain hasil export (@generated)
   ├─ component/{mod,state,tree}.rs
   ├─ caller/{mod,layout,resolve}.rs
   ├─ renderer/{mod,pixels,font,png,cpu,skia,gpu}.rs
   ├─ tests/{mod,layout,render}.rs
   └─ user/{mod,handlers}.rs  # MILIK ANDA
```

### Export ulang yang aman

```powershell
powershell -ExecutionPolicy Bypass -File .\sync.ps1 .\figcraft_design.zip
```

`sync.ps1` mengekstrak ZIP lalu menyalin semua file **kecuali** `src/user/**`.
Simpan kustomisasi (event handler, state tambahan, modul sendiri) di `src/user/`.

---

## Menjalankan project hasil export

Semua backend dikompilasi secara default dan dipilih saat **runtime**:

```sh
cargo run                                   # window, memakai backend default dari export
cargo run -- --backend cpu                  # rasterisator perangkat lunak
cargo run -- --backend skia                 # Skia canvas
cargo run -- --backend gpu                  # wgpu (offscreen + readback)

cargo run -- --backend cpu --png out.png    # render satu frame ke PNG
cargo run -- --backend cpu --frames 3       # smoke test tanpa window
cargo run -- --help
```

Build ringan:

```sh
cargo run --no-default-features --features "cpu,ui"
cargo run --no-default-features --features cpu            # tanpa window; fallback --png
cargo check --no-default-features --features skia
cargo check --no-default-features --features gpu
```

Jika backend tidak dikompilasi, `--backend ...` menghasilkan pesan yang menyarankan
feature yang perlu diaktifkan, bukan kegagalan diam-diam.

---

## Binding, state, dan widget

Teks seperti `Halo {{name}}` dan binding widget diterjemahkan menjadi `State`.
Handler bawaan di `src/user/handlers.rs` sudah menangani hover, klik, checkbox, radio,
switch, slider, dan input teks. Widget yang dikenal v1:

`Button`, `TextInput`, `TextArea`, `Checkbox`, `RadioButton`, `Switch`, `Slider`,
`ProgressBar`, `Dropdown`, `ListBox`, `Menu`, `MenuItem`, `Tabs`, `TabItem`,
`Tooltip`, `Modal`, `Accordion`, dan `Custom(name)`.

Contoh mengubah state dari handler Anda sendiri:

```rust
state.set("name", Value::Text("Dunia".into()));
```

Frame berikutnya otomatis memakai nilai baru; `Component` tetap statis.

---

## Batasan v1

- Paint gradient/image kompleks menghasilkan `Warning` dan fallback solid/abu.
- Gambar dipakai hanya bila host mengirim byte array (`imageBytes`/`bytes`/`imageData`);
  path filesystem tidak disimpan di IR.
- Font memakai bitmap 5×7 bawaan agar CPU/Skia/GPU identik tanpa file font.
- `Svg`, `Vector` kompleks, dan efek/transform lanjutan belum direproduksi persis.
- Uji manual di aplikasi FigCraft masih perlu dilakukan untuk memastikan command
  `export`/`panel` muncul di Actions dan dialog Save berfungsi di mesin Anda.

---

## Verifikasi manual cepat

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p figcraft-rust-export --example generate_sample
cargo test --manifest-path figcraft-rust-export/dist/sample/figcraft_sample/Cargo.toml
cargo run --manifest-path figcraft-rust-export/dist/sample/figcraft_sample/Cargo.toml -- --backend cpu --frames 3
```

---

## Referensi API FigCraft

- [Plugin docs](https://github.com/storytold/figcraft/blob/main/docs/plugins.md)
- [SDK source](https://github.com/storytold/figcraft/blob/main/plugins/sdk/src/lib.rs)
- [Control protocol / node fields](https://github.com/storytold/figcraft/blob/main/docs/control-protocol.md)
- [MCP / inspect commands](https://github.com/storytold/figcraft/blob/main/docs/mcp.md)
