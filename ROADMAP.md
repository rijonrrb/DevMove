# DevMove — Roadmap

## Purpose
Build a native Windows migration and driver-management application, owned and maintained as first-party software. No external driver-updater GUI is required.

## Version-control workflow
- `main`: reviewed stable snapshots.
- `feature/*`: small, scoped features; push frequently.
- Merge feature branches through pull requests after checks.
- Never push user backup archives, personal files, SQL dumps, Chrome data, credentials, SSH private keys, tokens, or runtime logs.

## Starting point
- The current `DevMove.ps1` + `Engine.ps1` + `ScanApps.ps1` represent an **experimental Windows PowerShell/WinForms prototype**, not the completed product.
- Prototype GUI opens and a small Node/PHP inventory backup and restore check previously ran; the complete workflow and newer features still require further testing.

## Planned Rust migration
- **Core:** Rust with clearly separated modules for inventory, backups, restoration, driver diagnostics, and progress/cancellation.
- **Desktop UI:** Tauri + React + TypeScript, with a sober professional visual system and accessible controls.
- **Apps:** searchable installed-app list, per-app selection, explicit list of unsupported/manual installs.
- **Drivers:** read-only device diagnosis; export installed third-party drivers; compare updates only against trustworthy Windows/OEM metadata. Never install drivers automatically without confirmation. If driver updates are later added, create a restore point and check signing/compatibility first.
- **Backups:** selected categories (VS Code, browser bookmarks/history, developer tools, databases, projects), progress feedback, integrity checks, and manual restore paths for risky operations.
- **Safety:** no silent driver changes, no sharing secrets, no assumed recovery of live browser sessions.

## Next engineering milestones
1. Scaffold new Rust/Tauri workspace in a feature branch.
2. Implement and test installed-app discovery and selection.
3. Port backup/restore modules with tests and risk checks.
4. Add driver diagnostics and safe export.
5. Build polished multi-panel desktop UI.
6. Verify sample restores in a disposable Windows environment, then publish a versioned release.
