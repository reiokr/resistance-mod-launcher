# Resistance Mod Launcher — Tauri MVP

Native Linux UI for the `xcom2-launcher-core` crate.

Status: **V3**, tested against a 792-mod / 790-active War of the Chosen
install.

## Before you start

- **Required:** XCOM 2: War of the Chosen via Steam and an existing Proton
  prefix (app 268500).
- **Recommended:** AML (Alternative Mod Launcher). If present, its
  `settings.json` supplies mod names, authors, categories and dependencies.
- **Optional:** AML's `settings.json` itself. Since v0.2.4 the native catalog
  and Steam Workshop folders are enough for the mod list, profiles, load order,
  conflict checks, export/import and cleanup.
- Close AML or XCOM 2 before saving or launching.

See the [root README](../README.md) for the full requirement table, test notes
and a short troubleshooting checklist.

Tested on CachyOS + Proton-CachyOS 11.0; other distributions are untested.
For bug reports, run the AppImage with `--doctor` and include the output.

## What it does

- Estonian and English UI with an `ET`/`EN` toggle in the top bar.
- Imports AML's `settings.json` when present for mod metadata (name, author,
  category, workshop ID, size, path) and merges new Workshop folders from
  `steamapps/workshop/content/268500` on launch, so a newly subscribed mod
  appears before AML has refreshed its own settings. New mods start inactive
  and in `Unsorted`.
- Treats `XComModOptions.ini` as the source of truth for the active list and
  load order (so the UI and the game agree).
- Lists all mods, with search plus status/category/Workshop-tag filters.
- Fetches read-only Steam Workshop metadata (description, tags, update time)
  via the public GetPublishedFileDetails API and caches it in
  `~/.local/share/xcom2-launcher-linux/workshop_cache.json`. Workshop tags are
  searchable and filterable in the mod list.
- Compares Workshop `time_updated` with the local Steam download time from
  `appworkshop_268500.acf` and marks mods with an **uuendus** badge.
- Enable/disable mods and drag (or arrow) reorder the active list.
- Categories: manager (create/rename/delete), per-mod assignment in the
  details pane, and category filtering. AML group names are imported on first
  run; categories are stored in the native `state.json`. Workshop tags are a
  separate, author-provided Steam classification and do not replace categories.
- Automatic categories: a rule-based classifier uses Workshop tags, mod names
  and descriptions to suggest categories (`Classes`, `Weapons & Items`,
  `Cosmetics`, `UI`, `Maps & Missions`, `Enemies & Aliens`, `Strategy &
  Facilities`, `Gameplay & Balance`, `Fixes & Tools`, `Factions`, `Audio`,
  `Localization`, `Long War`, `Amalgamation`). The UI shows a preview first;
  by default only **Unsorted** mods move. Mod names are normalized
  (CamelCase/underscore), and `WOTC` / `War of the Chosen` is treated as a
  version marker, not a category; the remaining name words decide the category.
  A `state.json` backup is written before applying, and applying only changes
  native categories, never the game config.
- Category learning: manual category changes are stored as labeled feedback and
  train a small Naive Bayes model over Workshop tags, name tokens and
  description tokens. It activates after at least 5 manual examples from two
  categories, overrides the rules only when confident, and never reclassifies
  manually assigned mods. The UI shows how many predictions came from the
  learned model, the rules, or manual pinning; **Unusta õpitud** clears the
  feedback but keeps the categories.
- Profiles: save the current active list/order under a name, apply it later,
  and delete it. Applying a profile is not written to `XComModOptions.ini`
  until you press **Salvesta**. Profiles can also be created from a Steam
  Workshop collection: installed collection mods are matched by Workshop ID
  (no duplicates), selected with one click, and missing mods are listed with
  **Ava Steamis** links. The collection description becomes the profile
  description, and missing IDs stay tracked as pending. A later collection
  view can add newly installed mods one by one.
- Diagnostics: scans active mods for class/screen-listener conflicts, missing
  required Workshop dependencies (including installed-but-inactive ones), and
  duplicate mod IDs. Results are cached in `diagnostics.json` and shown in the
  **Kontroll** tab and per-mod details. Missing dependency names and Workshop
  tags are resolved through the public Workshop API (best effort) and cached in
  `workshop_cache.json`. The scan uses the current UI active list, even before
  saving.
- Mod config editor: lists every `Config/**/*.ini` in a mod and edits it in a
  modal editor. Saves preserve the file's encoding (UTF-8/UTF-8 BOM/UTF-16/
  Windows-1252) and line endings, and write a byte-for-byte backup under
  `~/.local/share/xcom2-launcher-linux/backups/config-*` first. Editing is
  blocked while AML or XCOM 2 is running.
- ReadMe viewer: finds `ReadMe.txt`/`ReadMe.md` case-insensitively in the mod
  root, decodes UTF-8/UTF-16/Windows-1252, hides the default mod-project
  placeholder text, and shows the result in a modal.
- Saves `XComModOptions.ini` with a backup + atomic replace.
- Launches XCOM 2: War of the Chosen via a generated Windows `.bat` under
  `cmd.exe` (through `protontricks-launch`). The batch sets
  `SteamAppId`/`SteamGameId`/`SteamOverlayGameId`, matching the environment
  AML's child game gets. A bare `XCom2.exe` launch lacks those and the big mod
  pack crashes at startup.
- Checks that Steam is running before launch.
- Runs an optional prelaunch hook before launching. The hook is
  `~/.config/xcom2-launcher-linux/prelaunch`; in this setup it points to
  `reapply-xcom2-fixes.sh`, replacing the `xcom2-preflight` part of Steam's
  launch options.
- Refuses to save while AML or XCOM 2 is running.

The first save with the current 790 active mods produces a **byte-identical**
`XComModOptions.ini` (`unchanged: true`), so it is safe to test.

## Run

```bash
cd app
pnpm install
pnpm tauri dev
```

If pnpm complains about ignored build scripts:

```bash
pnpm approve-builds --all
# or
pnpm install --allow-build=esbuild
```

The first Rust/Tauri build takes a couple of minutes; later starts are fast.

## Install to the start menu (native)

```bash
cd app
./install-native.sh
```

This builds a release binary (embedded frontend), installs it to
`~/.local/bin/xcom2-mod-launcher`, and adds:

- `Resistance Mod Launcher` to the application menu
- `~/.local/share/applications/xcom2-mod-launcher.desktop`
- a 256px icon under `~/.local/share/icons/hicolor/256x256/apps/`
- license/provenance documents under
  `~/.local/share/doc/resistance-mod-launcher/`

The old Windows-AML wrapper is a separate project; its start-menu entry is
removed. It remains available through Steam / `Launcher/launcher.exe`, which is
now the font-scaled AML launcher.

The native app is also added to Steam as a non-Steam shortcut
`Resistance Mod Launcher` (target: `~/.local/bin/xcom2-mod-launcher`).
Launching it from Steam works the same as from the menu; its **Käivita XCOM 2**
button uses the batch described above.

Steam's **Launch Options** field is a single command line for the XCOM 2 app; it
cannot contain a second launcher choice. The non-Steam shortcut is the Steam
equivalent: it appears in the library, not in the game's Launch Options
dropdown. Putting the native app into XCOM 2's Launch Options would make Play
open the launcher instead of the game.

## AppImage

```bash
cd app
./package-appimage.sh
```

This runs `pnpm tauri build` and assembles:

```text
dist/release/Resistance-Mod-Launcher-<version>-x86_64.AppImage
dist/release/SHA256SUMS
dist/release/LICENSE
dist/release/NOTICE
dist/release/PROVENANCE.md
dist/release/THIRD_PARTY_NOTICES.md
dist/release/README.md
```

The AppImage contains the same docs under
`usr/lib/Resistance Mod Launcher/`. For systems without FUSE, run it with:

```bash
./Resistance-Mod-Launcher-<version>-x86_64.AppImage --appimage-extract-and-run
```

## Compared to Windows AML

| Feature | Native app | Windows AML |
|---|---|---|
| Mod list, search, enable/disable, load order | yes | yes |
| Save config with backup/atomic write | yes | yes |
| Launch WotC with correct Steam env | yes | yes |
| Categories / groups | basic (manager, assignment, filter; no collapsible headers) | yes |
| Profiles | basic (save/apply/delete; native-only) | yes |
| Workshop description, tags, update indicators | yes (read-only) | yes |
| Workshop subscribe/unsubscribe, downloading updates | no | yes |
| Workshop changelog | no | yes |
| Conflict / dependency checks | basic (missing dependencies, class overrides, duplicate IDs) | yes |
| Mod config editor | yes (raw INI editor, encoding/line-ending preservation, backups) | yes |
| ReadMe viewer | yes (ReadMe.txt/ReadMe.md) | yes |
| Vanilla XCOM 2 / native Linux build | no (WotC/Proton only) | Windows only |

The old AML remains the fallback for the missing features until they are ported.

## Headless backend checks

```bash
cd app/src-tauri
cargo run -- --headless-state
cargo run -- --headless-save
cargo run -- --headless-diagnostics
```

- `--headless-state` prints the summary and paths.
- `--headless-mods` prints `id<TAB>name<TAB>workshop_id<TAB>category` for every
  mod, including newly discovered Workshop subscriptions.
- `--headless-collection <collection_id>` previews a Steam Workshop collection.
- `--headless-import-collection <collection_id> [name]` creates a profile from
  all currently installed collection mods (test helper).
- `--headless-delete-profile <name>` deletes a profile (test helper).
- `--headless-save` runs the same backend save path as the UI, without a window.
  With the current list it returns `unchanged: true` and writes nothing.
- `--headless-diagnostics` scans the saved active list and prints
  `missing_dependency_count`, `missing_dependency_links`, `conflicts` and
  `duplicate_groups`, then refreshes `diagnostics.json`.
- `--headless-config-list <mod_id>` and
  `--headless-config-read <mod_id> <Config/File.ini>` inspect the config editor
  without opening a window (read-only).
- `--headless-readme <mod_id>` prints the decoded ReadMe content (or a
  not-found result).
- `--headless-auto-categories` prints the automatic-category preview (counts and
  samples); `--headless-auto-categories-apply` applies it to `state.json`
  (backup first).

## UI

- **Two UI modes.** Advanced is the default and exposes mod management,
  profiles, load order, categories, diagnostics and config editing. Simple
  shows only profile cards and the Start button; open it with **Simple view**
  in the top bar or the footer. The mode is remembered. Simple Start applies
  and saves the selected profile before launching, checks the Workshop drive
  mapping first, and starts Steam automatically if it is not running. Once a
  profile is configured, the Profiles tab offers the simple view once with a
  dismissible hint.
- **Modid** tab: all 792 mods; checkbox toggles active state; click a row for
  details. Filters: status, native category, and Workshop tag (tags used by at
  least two mods). Search also matches Workshop tags.
- **Laadimisjärjekord** tab: active mods in load order; drag rows or use ↑/↓.
- **Profiilid** tab: create a profile from a Steam Workshop collection
  (default), from the current active list, or empty (advanced). Existing
  collection mods are selected by default (uncheck as needed); missing mods
  stay visible with Steam links. Profile cards show the collection source,
  description, pending count and an optional custom cover image.
  **Muuda modisid** applies the profile and opens the Mods tab with an editing
  banner; **Salvesta profiil** writes the changes back to that profile.
  **Ajalugu** keeps the previous state (up to 20 versions), shows added,
  removed and reordered mods, and restores an older version. **Dubleeri**
  creates a test copy; **Uuenda originaali** syncs the copy back to its parent
  profile.
- **Profile picker** in the top bar: selecting a profile applies its active
  list/order immediately, runs the conflict check, and marks the profile as
  current. A yellow dot means the current list differs from the saved profile.
  To add a mod to the current profile (including a manual non-collection mod),
  enable it in **Modid**, then press **Salvesta profiil** in the top bar. The
  separate **Salvesta** button writes the game config.
- **Kategooriad** tab: category manager plus **Automaatne kategoriseerimine**.
  The preview shows counts and sample mods per category; **Rakenda** writes
  native categories and backs up `state.json` first. The overwrite checkbox
  allows re-classifying mods that already have a category. Manual category
  changes in the details pane train the learner; manual mods are pinned and
  never reclassified automatically.
- **Kontroll** tab: prints missing dependencies, class conflicts and duplicate
  IDs. Missing dependency rows show the resolved Workshop title, tags and an
  **Ava Steamis** button; if the item is deleted/private the numeric ID remains.
  A scan reads active mods' `Config/**/XComEngine.ini` and `Src/**/*.uc`;
  it takes a few seconds with ~790 mods. The last result is cached and reused
  after a restart.
- Details pane: drag its left edge to resize the panel; double-click resets the
  width. The chosen width is remembered. **Muuda configut (INI)**: opens a modal with the selected mod's
  `Config/**/*.ini` files. The editor shows encoding, line endings and modified
  time; **Salvesta**/Ctrl+S writes the file after a backup. **Lae uuesti**
  discards the buffer and re-reads from disk.
- Details pane **Ava ReadMe**: shows the mod's `ReadMe.txt`/`ReadMe.md` or a
  clear "no ReadMe" message.
- **Info** in the footer: version, GPL-3.0-only license, source link, and the
  unofficial/trademark disclaimer.
- **Värskenda Workshopi**: fetches/refreshes Workshop metadata for all mods
  with a Workshop ID (about 590 in the current setup). Network required; the
  result is cached.
- **Salvesta**: backup + atomic write of `XComModOptions.ini`.
- **Käivita XCOM 2**: starts the WotC exe under Proton; Steam must be running.
- Details pane: Workshop description, tags, last update, local download time,
  update badge, and **Ava Steamis**.
- Prelaunch hook: `~/.config/xcom2-launcher-linux/prelaunch` is executed before
  the game. It is optional; this setup symlinks it to the Amalgamation GTS fix.

Backups: `~/.local/share/xcom2-launcher-linux/backups/<timestamp>/`

## What is not here yet

- Collapsible group headers inside the mod table (category filter + manager
  are implemented; the list is still flat).
- Conflict resolution/load-order hints beyond AML-style class override
  detection.
- Native Workshop subscribe/unsubscribe and downloading. Collection import is
  read-only: missing mods open in Steam and must be subscribed there.
- Workshop changelog viewer (the public Workshop API does not return it;
  Steamworks access would be needed).
- Vanilla XCOM 2 mode (WotC is assumed).
- Native Linux XCOM 2 (Feral) mode.
- Release packaging (AppImage/AUR). `bundle.active` is currently false.

## Architecture

```
app/
  src/                  Svelte 5 UI
  src-tauri/            Tauri 2 backend
    src/main.rs         get_state / save_state / launch_game
  package.json
  vite.config.js
```

The backend uses `xcom2-launcher-core` from the parent directory for discovery,
AML import, `.XComMod` parsing, path mapping, INI rendering and safe write.

## Safety

- Save refuses while AML or XCOM 2 is running.
- A backup of the old `XComModOptions.ini` is created before a real change.
- The write is atomic (temp file + rename).
- The app does not touch `XComEngine.ini`; it warns if `ModRootDirs` is missing.
- Launch is a separate explicit button.

## License

GPL-3.0-only. This is an independent Rust/Svelte reimplementation; it does not
contain AML source code or assets. See the repository `NOTICE` for attribution
and the trademark disclaimer.
