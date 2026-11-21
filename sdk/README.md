# figcraft-plugin-sdk (vendored)

This directory is a verbatim copy of the official Rust plugin SDK from
<https://github.com/storytold/figcraft/tree/main/plugins/sdk>.

It is vendored here so the plugin builds on its own without cloning the whole
FigCraft repository. The upstream crate is dual-licensed `MIT OR Apache-2.0`.

Do not edit `src/lib.rs` when upgrading FigCraft: replace the file with the new
upstream version and check the plugin manifest `api` field against the plugin
docs at <https://github.com/storytold/figcraft/blob/main/docs/plugins.md>.
