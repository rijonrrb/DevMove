# DevMove

**A Windows migration and recovery toolkit for moving essential applications, development environments, and personal settings to a clean installation.**

> **Status: Early development.** The repository currently contains an experimental PowerShell/WinForms prototype. The next application is being developed with Rust, Tauri, React, and TypeScript. Features described as planned are not yet available.

## Overview

Reinstalling Windows should not mean rebuilding a working environment from memory. DevMove aims to make migrations deliberate and verifiable: select what matters, capture configuration and application inventories, store a recoverable backup, and restore with a clear understanding of what will change.

The project is designed primarily for Windows users and developers who rely on customized editors, command-line tools, browser profiles, local projects, and databases.

## Rust desktop preview

The new Windows application is being built in [desktop/](desktop/README.md). The initial development preview provides a searchable software inventory, read-only driver diagnostics, selective local ZIP exports, and per-file SHA-256 archive verification. The ZIP contains a limited set of configuration files and package metadata, not installed program binaries.

**This is an unreleased development preview, not a migration-ready product.** Database export, cloud upload, file restoration, and driver updates have not yet been implemented in Rust.

## Current prototype

The legacy prototype provides an initial graphical workflow for:

- Choosing backup categories, including VS Code settings, Chrome bookmarks/history, Git configuration, XAMPP configuration, and development-tool inventories.
- Exporting application inventories through Windows Package Manager (WinGet).
- Creating MySQL/MariaDB SQL exports and copying selected project files.
- Saving a backup manifest and task logs, with an initial restore workflow for supported settings.

**This is not yet a tested, full-system migration solution.** Some restore operations require manual steps. The prototype does not guarantee preservation of browser sign-ins, software licenses, or application secrets. Database backups require independent verification.

For prototype setup and operating instructions, see [Legacy prototype guide](docs/LEGACY_PROTOTYPE_GUIDE.md).

## Product direction

The Rust desktop application is planned around four capabilities:

**Selective migration.** Discover installed software, distinguish reinstallable packages from manual installations, and apply version-aware backup rules for settings, tools, and projects.

**Reliable recovery.** Create portable archives with integrity verification, inspect backups before restoring, and report conflicts or unsupported operations explicitly.

**Cloud backup.** Optionally upload an encrypted backup to an authorized Google Drive account, with resumable transfers and a path to cloud recovery.

**Driver management.** Inspect device and driver status, preserve third-party driver packages, and evaluate updates with compatibility and recovery safeguards before any installation.

The objective is a focused Windows utility, not a replacement for complete disk imaging or specialized disaster-recovery systems.

## Technology

| Layer | Technology |
| --- | --- |
| Desktop application (in development) | Rust, Tauri 2 |
| Interface (in development) | React, TypeScript |
| Windows integration | Windows APIs, WinGet, driver and device-management facilities |
| Existing prototype | PowerShell 5.1, Windows Forms |

## Development

The current prototype can be explored on Windows by opening `Launch DevMove.bat` from the repository root. It does not require the Rust toolchain.

Development milestones and architecture notes are tracked in the [roadmap](ROADMAP.md), [open-source research](docs/OPEN_SOURCE_RESEARCH.md), and [cloud backup design](docs/CLOUD_BACKUP_ARCHITECTURE.md). Work on the new desktop application is tracked in [GitHub Issues](https://github.com/rijonrrb/DevMove/issues).

There is no stable release yet. Do not rely on this repository as the sole backup of data you cannot afford to lose.

## Safety and privacy

DevMove will not attempt to extract encrypted browser session credentials or bypass sign-in protections. Backups of project files, SSH keys, SQL databases, and browsing history can be sensitive; store them securely and test actual restoration before wiping a drive.

The repository does not include personal backup archives, OAuth tokens, database contents, or other machine-specific secrets.

## Contributions

Feedback, bug reports, and scoped feature proposals are welcome through [GitHub Issues](https://github.com/rijonrrb/DevMove/issues). Please include reproduction steps and expected behavior where applicable. Contributions should not include credentials, user data, or proprietary driver binaries.
