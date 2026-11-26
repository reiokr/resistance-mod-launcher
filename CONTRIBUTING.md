# Contributing

Thanks for helping with Resistance Mod Launcher.

## Ground rules

- This is an independent GPL-3.0-only project. By contributing you agree that
  your changes are licensed under GPL-3.0-only.
- Do not copy source code, UI text, artwork, icons, or other assets from the
  Alternative Mod Launcher (AML) or from XCOM 2/Steam without a compatible
  license and clear attribution.
- Keep AML compatibility at the level of file formats and observable behavior.
- Do not add telemetry, analytics, or network calls that are not needed for a
  feature. Workshop metadata access is read-only and must respect Valve's terms.
- Never commit game files, Workshop mods, Steam content, user `settings.json`,
  local caches, logs, or backups.

## Build and test

```bash
# core
cargo test

# app
cd app
pnpm install
pnpm build
cd src-tauri
cargo test
```

Run the app with `pnpm tauri dev`, or install a release with
`./install-native.sh`.

## Pull requests

- Keep changes focused and explain the user-facing effect.
- Add or update tests for backend behavior.
- Mention any data files or new dependencies.
- If a change touches compatibility behavior, describe how it was verified
  against a real XCOM 2 install.
