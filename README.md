# Resistance Mod Launcher (core + Tauri)

[![CI](https://github.com/reiokr/resistance-mod-launcher/actions/workflows/ci.yml/badge.svg)](https://github.com/reiokr/resistance-mod-launcher/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/reiokr/resistance-mod-launcher)](https://github.com/reiokr/resistance-mod-launcher/releases)

Native Linux core and desktop app for XCOM 2 mod management. This is an
independent Tauri 2 + Svelte 5 implementation that runs natively on Linux and
manages the Windows/Proton install without Wine.

**Status: core + Tauri 2 app (V3).** The core CLI is read-only; the Tauri app
can save `XComModOptions.ini` with a backup and launch WotC. It includes
Advanced and Simple UI modes, profile covers, profile creation from the current
list or a Steam Workshop collection, and one-click profile switching. The UI is
available in Estonian and English. See [`app/README.md`](app/README.md).

Source: <https://github.com/reiokr/resistance-mod-launcher> ·
[Releases](https://github.com/reiokr/resistance-mod-launcher/releases) ·
[Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) ·
[Third-party notices](THIRD_PARTY_NOTICES.md)

## Why this exists

Switching XCOM 2 modpacks normally means clearing the active mod list and
rebuilding it from scratch. Resistance Mod Launcher stores each modpack as a
profile with its own load order, deactivated mods and missing/pending Workshop
IDs. Apply a profile, press **Save**, launch. Profiles can also be exported and
imported on another machine, so a modpack is no longer tied to one installation.

## Before you start

| Requirement | Required? | Why |
|---|---|---|
| XCOM 2: War of the Chosen in Steam | **Yes** | The game and its Proton prefix are required. |
| Steam running | **Yes for launch** | Started automatically by the launcher if needed. |
| Proton prefix for app 268500 | **Yes** | Game configs and mod paths live inside it. |
| AML (Alternative Mod Launcher) | **Recommended** | Full metadata, categories and dependency data. |
| AML `settings.json` | **Optional** | Since v0.2.4 the launcher falls back to its native catalog. |

**AML is not strictly required.** When installed, the launcher imports AML's
`settings.json` for mod names, authors, categories and dependencies. Without
it, mods are discovered from Steam Workshop folders and `.XComMod` files;
profiles, load order, conflict checks, export/import and cleanup still work.
Dependency data may be incomplete without AML.

Close AML before pressing **Save** or **Launch**: the launcher refuses to write
the config or start the game while AML or XCOM 2 is running.

## Download

The latest AppImage is available on the
[Releases page](https://github.com/reiokr/resistance-mod-launcher/releases/latest).

1. Download `Resistance-Mod-Launcher-<version>-x86_64.AppImage`.
2. Make it executable:

   ```bash
   chmod +x Resistance-Mod-Launcher-<version>-x86_64.AppImage
   ```

3. Run it:

   ```bash
   ./Resistance-Mod-Launcher-<version>-x86_64.AppImage
   ```

SHA-256 checksums are attached to every release as `SHA256SUMS`.

## If something is missing

- **No mods appear?** Check that Steam and the correct library are detected,
  then press **Refresh Workshop**.
- **`settings.json` is missing?** This is not fatal since v0.2.4; the native
  catalog and Workshop folders are used instead.
- **Save is blocked?** Close AML or XCOM 2.
- **Dependencies look incomplete?** Install AML or import its `settings.json`
  once.
- **Game crashes right after launch?** Run `--doctor`. It compares the
  `ModRootDirs` drive in `XComEngine.ini` (usually `S:`) with the Workshop
  folder the launcher uses. If they differ, it prints the exact `ln -sfn`
  command that repoints the Proton drive.

### Diagnosing startup problems

Run the AppImage from a terminal:

```bash
./Resistance-Mod-Launcher-*.AppImage --doctor
```

Paste the output into a GitHub issue. The doctor also verifies that the
game's `ModRootDirs` drive points to the same Workshop folder as the launcher,
and suggests the `ln -sfn` fix when a Steam library has moved. `--version`
prints the launcher version.
If the AppImage does not start at all, include the terminal error and try
`APPIMAGE_EXTRACT_AND_RUN=1 ./Resistance-Mod-Launcher-*.AppImage --doctor` to
rule out FUSE.

## Tested on

- CachyOS (Arch-based), KDE Wayland
- Proton-CachyOS 11.0
- XCOM 2: War of the Chosen via Steam
- large modded install (1000+ mods)

Other distributions are untested. The AppImage expects a working WebKitGTK 4.1,
GTK 3 and `xdg-desktop-portal`; missing system libraries are the most common
reason it does not start.

## Known limitations

- Linux only; there is no native Windows build.
- XCOM 2: War of the Chosen via Steam and Proton is the supported target.
- Unsubscribe/resubscribe is manual in Steam by design.
- Cleanup scans the detected Workshop library; multiple Steam libraries are not
  fully handled yet.
- Dependency checks work best with AML installed; without AML, dependency
  metadata may be incomplete.
- AppImage is the only package; Flatpak/Snap/AUR packages are not provided yet.

## What works

The `aml-preview` binary:

- discovers Steam libraries (native, Flatpak, Snap), the XCOM 2 install, the
  Proton prefix and the AML directory;
- imports AML's `settings.json` when present (all 792 mods, active state, load order);
- scans `steamapps/workshop/content/268500` and merges newly subscribed mods
  that are not in AML's settings yet;
- parses `.XComMod` metadata;
- maps Windows paths (`S:\...`) through the prefix's `dosdevices` symlinks;
- resolves the active list and generates a candidate `XComModOptions.ini`;
- compares the generated list with the current game config.

Validated against the current 792-mod install:

```
mods: 792, active: 790, missing: 0
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

The UI has two modes. **Advanced** is the default and exposes the mod list,
load order, profiles, categories, diagnostics and config editing. **Simple**
shows only the profile cards and the Start button for day-to-day modpack
switching; open it with **Simple view** in the top bar or the footer. The mode
is remembered. Once a profile is configured, the Profiles tab offers the simple
view once with a dismissible hint. Before launching, the app checks that the
game's `ModRootDirs` drive resolves to the same Workshop folder as the launcher;
if it does not, the launch is blocked and the exact `ln -sfn` fix is shown. If
Steam is not running, the launcher starts it and waits up to 60 seconds before
continuing.

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
   - profile export/import to a portable JSON file — done (single and all profiles);
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
   - Steam Workshop collection import into profiles — done (installed mods
     matched by Workshop ID, missing mods linked to Steam);
   - profile deletion impact and unused Workshop mod report — done;
   - unused mods can be moved to the system trash, with in-app list/restore — done;
   - unsubscribe/resubscribe stays manual in Steam for now.
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
