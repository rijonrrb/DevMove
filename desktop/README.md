# DevMove desktop (developer preview)

This directory contains the Windows desktop application using **Tauri 2 + Rust + React + TypeScript**. It supersedes the PowerShell/WinForms UI but does not yet have equivalent backup or restore coverage.

## Prerequisites

- Windows 10 or Windows 11
- Node.js and npm
- Rust stable toolchain (rustup, cargo, rustc)
- MSVC C++ build tools and the WebView2 runtime required by Tauri

## Local development

~~~powershell
cd desktop
npm ci
npm run dev          # browser preview; native commands disabled
npm run build        # frontend typecheck and production bundle
npm run lint
npx tauri dev        # Windows desktop app with native Rust backend
~~~

The browser preview intentionally does not fabricate app or driver data. Native inventory and backup actions work only when launched through Tauri.

## Implemented in the Rust preview

- Installed applications: WinGet export merged with Windows uninstall inventory, searchable selection and manual-reinstall distinction.
- Driver diagnostics: read-only signed-driver inventory and Windows PnP problem codes, including devices without detected signed drivers.
- Driver export: explicitly selected Windows folder for exporting installed third-party driver packages using built-in PnPUtil. Requires applicable Windows permissions; no drivers are changed.
- Local archive: selected VS Code settings, Chrome bookmarks/history (when accessible), Git config, tool versions, and selected application manifests.
- ZIP verification: SHA-256 digests for each file and readable archive validation.
- Safe ZIP extraction: recover recorded files into a new staging folder without overwriting Windows settings.
- Native destination picker and on-screen status/error feedback.

## Not yet implemented

- Automatically applying extracted settings or installing packages into Windows from an archive.
- XAMPP database dumps and project folder migration in the new Rust application.
- Google Drive OAuth, encryption, cloud transfer, incremental snapshots, and schedules.
- Automatic driver updates, compatibility checks, and restore-point creation. Driver package export is implemented separately and does not install drivers.
- Persisted backup history and durable background jobs.

ZIP archives in this preview are **not encrypted** and may contain sensitive configuration. Use a private, secure destination and retain an independently tested backup before reinstalling Windows.

## Code layout

- src/: React application, types, native IPC wrappers and UI.
- src-tauri/src/migration.rs: Windows application/device inventory and archive routines.
- src-tauri/capabilities/: desktop permissions.

See [Windows build and preview validation](../docs/PREVIEW_BUILD.md), the [repository roadmap](../ROADMAP.md) and [cloud architecture specification](../docs/CLOUD_BACKUP_ARCHITECTURE.md) for future milestones.
