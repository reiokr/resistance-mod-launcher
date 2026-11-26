# Provenance notes

This repository is an independent Rust/Svelte implementation. It does not
contain source code, binaries, artwork, icons, translations, or other assets
from the Alternative Mod Launcher (AML).

## What was used from AML

Only public, observable interoperability details were used:

- the `settings.json` format and field names;
- the `XComModOptions.ini` format and load-order behavior;
- AML-compatible conflict and dependency-check rules;
- the locations of mod `Config/**/*.ini` and `ReadMe.txt`/`ReadMe.md` files;
- the expected ReadMe placeholder text from the XCOM 2 mod project template.

The AML project is licensed under GPL-3.0:
<https://github.com/X2CommunityCore/xcom2-launcher>.

## What is original here

- all Rust backend code in `src/` and `app/src-tauri/src/`;
- the Svelte UI in `app/src/`;
- the Estonian and English UI strings;
- the automatic category rules and the Naive Bayes learner;
- the application icon source (`app/app-icon.svg`) and generated icon set;
- the build/install scripts and documentation.

The ModClassOverrides regex follows the XCOM 2 configuration syntax itself, not
AML-specific code. The ReadMe placeholder strings are short template phrases
used only to detect an empty mod ReadMe.

## Third-party dependencies

Tauri, Svelte, and the Rust crates used here remain under their respective
licenses. See `Cargo.lock`, `app/pnpm-lock.yaml`, and the upstream projects.

## Reporting a provenance issue

If you believe a file or asset was copied without proper attribution, please
open an issue or contact the maintainer. We will audit and correct it.
