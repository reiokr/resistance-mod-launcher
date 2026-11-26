# Security

## Reporting

Please report security issues privately to the repository maintainer rather
than opening a public issue. Include the version, the affected file, and a
minimal reproduction if possible.

## Scope

Resistance Mod Launcher is a local desktop app. It reads Steam/XCOM 2 files,
writes `XComModOptions.ini` and native state under
`~/.local/share/xcom2-launcher-linux`, and can fetch read-only Workshop
metadata through Valve's public API.

Relevant issues include:

- path traversal or writing outside the intended mod/config directories;
- arbitrary file overwrite through crafted mod metadata;
- injection through mod names, descriptions, or config files rendered by the UI;
- secrets or personal paths accidentally committed to the repository.

## Data handling

- No telemetry or analytics is sent by the app.
- Workshop metadata is cached locally in
  `~/.local/share/xcom2-launcher-linux/workshop_cache.json`.
- Backups are stored under `~/.local/share/xcom2-launcher-linux/backups`.
- Do not attach real user configuration files or Workshop caches to public
  issues; redact paths and IDs first.
