# DevMove preview changelog

## Unreleased (after 0.1.1)

- Allow explicit selection of existing .sql/.sql.gz export files for inclusion in checksum-verified ZIP backups. No MySQL connection or credentials are used.
- Show recorded SQL export paths in the read-only restore preview; extract them only into a new staging folder.
- Add input validation for ordinary SQL files, disallow symbolic links and duplicates, and test SQL archive/verification/extraction with disposable fixtures.

Direct database dumping/import, encryption, cloud backup and full migration remain future work.

## 0.1.1 — 2026-11-23 (Developer preview)

- Add explicit selection of project folders for local ZIP backup. Skip common secrets, generated dependencies and symlinks; reject overlapping source/destination trees and display exclusions.
- Add read-only restore preview from verified ZIP archives, including recorded WinGet IDs, manual reinstall list, timestamps, modules and warnings.
- Verify archive integrity with SHA-256, extract to a separate staging folder without overwriting existing settings.
- Provide built-in driver inventory and third-party package export using Windows PnPUtil.
- Continue shipping a Rust/Tauri Windows desktop UI with React and TypeScript.
- Add GitHub Actions checks for frontend lint/build and Rust formatting/check/tests.

This developer preview **is not a complete migration solution**. Database migration, automatic app reinstall, encrypted Google Drive backup, driver updates and full restore remain unimplemented. ZIP archives remain unencrypted, and local developer-built installers are unsigned.

## 0.1.0 — 2026-11-23 (Initial developer preview)

- Scaffold the Rust/Tauri desktop application and modern UI.
- Capture app inventory, select basic configuration files, create local ZIPs and verify recorded file digests.
- Keep the original PowerShell prototype separate and intact.

Do not remove Windows installations or original data based solely on these developer-preview archives.
