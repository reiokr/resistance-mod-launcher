# Resistance Mod Launcher (core + Tauri)

Native Linux core and desktop app for XCOM 2 mod management. This is an
independent Tauri 2 + Svelte 5 implementation that runs natively on Linux and
manages the Windows/Proton install without Wine.

**Status: core + Tauri 2 MVP.** The core CLI is read-only; the Tauri app can
save `XComModOptions.ini` with a backup and launch WotC. The UI is available in
Estonian and English. See [`app/README.md`](app/README.md).

Source: <https://github.com/reiokr/resistance-mod-launcher> ·
[Releases](https://github.com/reiokr/resistance-mod-launcher/releases) ·
[Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) ·
[Third-party notices](THIRD_PARTY_NOTICES.md)

## What works

The `aml-preview` binary:

- discovers Steam libraries (native, Flatpak, Snap), the XCOM 2 install, the
  Proton prefix and the AML directory;
- imports AML's `settings.json` (all 791 mods, active state, load order);
- scans `steamapps/workshop/content/268500`;
- parses `.XComMod` metadata;
- maps Windows paths (`S:\...`) through the prefix's `dosdevices` symlinks;
- resolves the active list and generates a candidate `XComModOptions.ini`;
- compares the generated list with the current game config.

Validated against the current 791-mod install:

```
mods: 791, active: 790, missing: 0
current ActiveMods: 790
generated ActiveMods: 790
IniVersion: generated == current
Order identical: yes
```

The generated file is **byte-for-byte identical** to the current
`XComModOptions.ini` (same SHA-256), including CRLF line endings, section order
and the `IniVersion` timestamp.

## Run

```bash
cargo run --bin aml-preview
```

Useful options:

```bash
# machine-readable summary, do not write anything
cargo run --bin aml-preview -- --json --no-write

# vanilla XCOM 2 instead of War of the Chosen
cargo run --bin aml-preview -- --vanilla

# write the preview somewhere else
cargo run --bin aml-preview -- --out /tmp/preview-XComModOptions.ini

# print the INI to stdout
cargo run --bin aml-preview -- --stdout --no-write
```

Default output: `/tmp/xcom2-launcher-linux/XComModOptions.ini`.
The real game config is never touched by `aml-preview`.

## Tauri UI

```bash
cd app
pnpm install
pnpm tauri dev
```

Install the native app into the application menu:

```bash
cd app
./install-native.sh
```

Build a self-contained AppImage and a release directory:

```bash
cd app
./package-appimage.sh
```

Output:

```text
dist/release/Resistance-Mod-Launcher-<version>-x86_64.AppImage
dist/release/SHA256SUMS
dist/release/LICENSE
dist/release/NOTICE
dist/release/PROVENANCE.md
dist/release/THIRD_PARTY_NOTICES.md
```

The AppImage bundles the documentation above under
`usr/lib/Resistance Mod Launcher/`. Pushing a `v*` tag runs the GitHub
release workflow and attaches the AppImage plus checksums and legal files.

Entry: **Resistance Mod Launcher**. The old Windows-AML wrapper is a
separate project with the entry **XCOM 2 AML (Windows/Proton)**.

The app imports the same data, lists all mods, lets you enable/disable and
reorder them, saves with a backup + atomic replace, and launches WotC through a
generated Windows batch (with SteamAppId/SteamGameId) via `protontricks-launch`.
With the current 790 active mods the first save is
`unchanged: true` (byte-identical). The launch command checks that Steam is
running and runs the optional
`~/.config/xcom2-launcher-linux/prelaunch` hook before starting the game.

The UI ships with Estonian and English translations. The `EN`/`ET` button in
the top bar switches languages; the choice is stored locally.

## Safety

- Read-only for the game prefix and AML settings.
- The only write is the preview file in `/tmp` (or `--out`).
- The future write path will use a backup + atomic replace, just like
  `aml-linux`.

## Architecture

- `src/lib.rs` — path discovery, AML settings import, `.XComMod` parsing,
  Windows→Linux path mapping, load order, AML-compatible INI generation,
  diffing.
- `src/bin/aml-preview.rs` — CLI preview.
- `Cargo.toml` — library `xcom2-launcher-core` + binary `aml-preview`.

The library is UI-free so the Tauri backend can call it directly.

## Next milestones

1. **Tauri 2 MVP — done (basic version)**
   - import AML settings;
   - mod list with search/filter;
   - enable/disable + drag load order;
   - save `XComModOptions.ini` with backup/atomic write;
   - launch WotC via `protontricks-launch`.
   - Still to add: vanilla mode, AppImage/AUR packaging.
2. **AML compatibility**
   - import AML's `settings.json` read-only and own the active state — done;
   - profiles/categories — done (native `state.json`);
   - automatic categories from Workshop tags/name/description — done;
   - learning from manual category corrections (Naive Bayes) — done;
   - duplicate ID, class-override and missing-dependency checks — done;
   - mod config editor with encoding/line-ending preservation and backups — done;
   - ReadMe viewer — done;
   - Estonian/English UI — done;
   - public GPL-3.0 repository with CI — done.
3. **Workshop**
   - metadata via Steam Web API — done (cached, read-only);
   - update indicators from `appworkshop_268500.acf` — done;
   - description/tags/details pane and tag filter — done;
   - missing dependency titles/tags from the Workshop API — done;
   - optional native subscribe/unsubscribe via Steamworks — later.
4. **Native Linux build**
   - Feral VFS paths (`~/.local/share/feral-interactive/...`);
   - native `share/data/xcomgame/mods`.
5. **Packaging**
   - AppImage + release docs/checksums — done;
   - AUR PKGBUILD — later (maintainer account needed).

## Requirements

- Rust 1.85+ (tested with 1.100).
- XCOM 2 (Proton) + AML 1.6.x for the preview data.
- No root, no Wine, no Proton calls for the preview.

## License and attribution

GPL-3.0-only. This is an independent Rust/Svelte reimplementation; it does not
contain AML source code or assets. AML is GPL-3.0; its file formats and behavior
were used for interoperability. See `NOTICE` for attribution and the
trademark disclaimer, and `PROVENANCE.md` for the audit notes. AML project:
<https://github.com/X2CommunityCore/xcom2-launcher>.

This project is unofficial and is not affiliated with or endorsed by Firaxis
Games, 2K, Take-Two Interactive, Valve, or the Alternative Mod Launcher (AML)
project. XCOM, XCOM 2 and related marks are trademarks of their respective
owners.
