Sebuah plugin untuk FigCraft, yang di buat menggunakna webassembly

Plugin ini bertugas untuk menambahkan sebuah metode export dari desain di FigCraft yang umumnya di
export ke html + css atau sejenisnya, sekarang bisa di export ke rust, yang di bagi menjadi 3 module
yaitu yang pertama penggambar yang bertugas menggambar semua yang di minta ke cpu, atau menggunakan 
skia, yang ke 2 adalah pemanggil, yang mengurus panggilan dari komponen komponen, dan semua nilai yang
di minta, untuk di kasih ke penggambar, yang ke 3 adalah komponen, yaitu yang mengurus bagaimana cara
membuat sebuah UI, mulai dari seperti memasukan nilai di wight, dan sejeninsya

ada 3 metode yang bisa di pakai untuk mengexport desain dari figcraft, yaitu menggunakan rust dan skia
, menggambarnya langsung menggunakan cpu, atau menggunakan gpu

---

## Implementasi

Plugin ini sudah dibuat di folder `figcraft-rust-export/` dengan SDK Rust resmi FigCraft yang di-`vendor`
di folder `sdk/`. Plugin hanya **membaca** dokumen dan menyerahkan hasil export ke dialog Save milik
FigCraft melalui `save_file` (api manifest `4`).

### Bangun plugin

```powershell
./scripts/build.ps1
```

Skrip itu menghasilkan:

- `figcraft-rust-export/figcraft_rust_export.wasm` — module WASM yang dibutuhkan manifest
- `dist/figcraft-rust-export-plugin.zip` — paket berisi `figcraft-plugin.json` + `.wasm` untuk di-import

Build manual:

```sh
cargo build --release --target wasm32-unknown-unknown -p figcraft-rust-export
```

### Pasang di FigCraft

1. Buka **Plugins → Install / Import**.
2. Pilih folder `figcraft-rust-export/` (atau ekstrak dulu `dist/figcraft-rust-export-plugin.zip`).
3. Jalankan perintah **Export to Rust…**. Pilih renderer `Skia`, `CPU`, atau `GPU`, lalu isi nama crate.
4. Simpan file `.zip` yang ditawarkan FigCraft.

Jika tidak ada layer yang dipilih, plugin memakai semua layer tingkat atas di halaman aktif
(`page.list` → `node.get`).

### Hasil export

ZIP berisi satu project Cargo dengan tiga modul sesuai spesifikasi:

| Modul | File | Fungsi |
| --- | --- | --- |
| `komponen` / `component` | `src/component.rs` | Pohon UI hasil FigCraft: layer, geometri, gaya, teks, dan placeholder `{{key}}`. |
| `pemanggil` / `caller` | `src/caller.rs` | Menelusuri pohon, mengisi nilai, lalu menerjemahkannya ke panggilan gambar untuk renderer. |
| `penggambar` / `renderer` | `src/renderer/` | Backend `cpu.rs` (rasterisator tanpa dependency), `skia.rs`, dan `gpu.rs` (wgpu). |

File tambahan `reference/figcraft.css` diambil dari `inspect.code` sebagai perbandingan dengan export
bawaan FigCraft.

Jalankan project hasil export:

```sh
cargo run                                              # backend pilihan saat export
cargo run --no-default-features --features cpu         # rasterisator CPU
cargo run --no-default-features --features skia        # Skia canvas
cargo run --no-default-features --features gpu         # wgpu (offscreen)
```

Hasilnya adalah PNG di direktori project. Nilai runtime bisa disuntikkan lewat
`caller.set("nama", ...)` untuk placeholder `{{nama}}` pada layer teks.

### Verifikasi

```sh
cargo test -p figcraft-rust-export
cargo run -p figcraft-rust-export --example generate_sample
cargo run --manifest-path figcraft-rust-export/dist/sample/figcraft_sample/Cargo.toml
cargo check --manifest-path figcraft-rust-export/dist/sample/figcraft_sample/Cargo.toml --no-default-features --features skia
cargo check --manifest-path figcraft-rust-export/dist/sample/figcraft_sample/Cargo.toml --no-default-features --features gpu
```

### Referensi API FigCraft

- [Plugin docs](https://github.com/storytold/figcraft/blob/main/docs/plugins.md)
- [SDK source](https://github.com/storytold/figcraft/blob/main/plugins/sdk/src/lib.rs)
- [Contoh exporter `export-names`](https://github.com/storytold/figcraft/blob/main/plugins/examples/export-names/src/lib.rs)
- [Control protocol / node fields](https://github.com/storytold/figcraft/blob/main/docs/control-protocol.md)
- [MCP / inspect commands](https://github.com/storytold/figcraft/blob/main/docs/mcp.md)
