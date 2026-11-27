# Third-party notices

Resistance Mod Launcher is built with open-source software. Each dependency
remains under its own license. The authoritative license metadata is in
`Cargo.lock`, `app/src-tauri/Cargo.lock`, `app/pnpm-lock.yaml`, and the
corresponding upstream packages.

Key dependencies:

| Component | License | Upstream |
|---|---|---|
| Tauri | MIT OR Apache-2.0 | https://github.com/tauri-apps/tauri |
| Tauri Dialog plugin / `rfd` | MIT OR Apache-2.0 / MIT | https://github.com/tauri-apps/plugins-workspace · https://github.com/PolyMeilex/rfd |
| `trash` | MIT | https://github.com/Byron/trash-rs |
| Svelte | MIT | https://github.com/sveltejs/svelte |
| Vite | MIT | https://github.com/vitejs/vite |
| Rust crates (`anyhow`, `serde`, `serde_json`, `ureq`, `walkdir`, `rayon`, `regex`, `encoding_rs`, ...) | mostly MIT / Apache-2.0 | see crates.io and `Cargo.lock` |
| pnpm / esbuild tooling | MIT | https://github.com/pnpm/pnpm · https://github.com/evanw/esbuild |

Before distributing binary release archives, generate a complete third-party
license bundle from the lockfiles (for example with `cargo about` and
`pnpm licenses list`) and ship it with the binary. This file is a summary, not
a replacement for the full upstream license texts.
