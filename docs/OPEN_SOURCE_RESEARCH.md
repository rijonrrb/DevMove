# DevMove — Open-source backup & migration research
Research date: 2026-11-23. Status: competitor review and engineering recommendations; not yet implemented.

## Goal
DevMove is a first-party Windows migration and backup application built with Rust + Tauri + React/TypeScript. It should not require customers to install another third-party backup GUI. Third-party project ideas are used for design inspiration, not copied source.

## Evaluated projects

| Project / official code | What it does well | What we should learn | Limitations for DevMove |
| --- | --- | --- | --- |
| [Kopia](https://github.com/kopia/kopia) (Apache-2.0) | Windows GUI & CLI, encrypted snapshots, compression, deduplication, scheduling, maintenance, retention, browse/restore | Snapshot model, policy-based retention, verification, destination abstraction | Does not replace WinGet + app-specific migration; direct Google Drive uses experimental rclone backend rather than the intended native OAuth UX |
| [Duplicati](https://github.com/duplicati/duplicati) (MIT) | Cloud-first encrypted incremental backup, Google Drive, scheduler, file filters, bandwidth limits, VSS | One wizard for sources + destination, progress, cloud retries, schedules, default encryption | C# stack and generic file backup rather than curated developer app settings |
| [Restic](https://github.com/restic/restic) | Mature encrypted deduplicated backup, reliable snapshot restore, check, forget & prune, Windows VSS option | Content integrity, non-destructive snapshots, restore validation | CLI-first and no native migration presets; Restic itself is written in Go |
| [Rustic](https://github.com/rustic-rs/rustic) and [rustic_core](https://github.com/rustic-rs/rustic_core) (MIT/Apache-2.0) | Rust native, encrypted/deduplicated snapshots, compatible with restic repositories, Rust libraries, resumable architecture | Evaluate `rustic_core` as an *optional library* after testing Windows compatibility and license obligations; do not invent our own cryptography | Core API states it can change; pin versions and add integration tests |
| [Backrest](https://github.com/garethgeorge/backrest) | Snapshot list, scheduling, check/forget/prune, restore browser, notifications | Restore-center interaction patterns, operations history and maintenance | Web interface wrapping Restic; not a self-contained Windows migration product |
| [UrBackup](https://github.com/uroni/urbackup_backend) | File plus full system-image backup, incremental operation and fast recovery | Separate migration backup from disk-image backup; visible recovery scenarios | Client/server architecture and full-image backup is a different, larger project |
| [Win Apps Migrator](https://github.com/ox1d3x3/winappsmigrate) | Per-app migration rules, ZIP output, WinGet/Chocolatey reinstall, restore reporting | Rule/plugin catalog for apps and compatibility checks | Author labels it beta; not a mature safety reference. Evaluate design only. |
| [CloneApp](https://github.com/builtbybel/CloneApp) | 250-ish Windows app setting presets and portable migration workflow | Concept of per-app backup rules and searchable catalog | **Closed source** per its own README. Do NOT present as an OSS implementation or copy proprietary code. |

## Engineering recommendation

**Best fit is a two-layer architecture.** The first-party Rust app decides *what* to back up or restore (WinGet app selection, VS Code settings/extensions, developer environment, profiles, SQL dumps, Git, SSH and driver export). A separate tested backup engine handles *how* to snapshot safely (manifest, compression, encryption, incremental data, integrity and snapshots).

Do NOT begin by inventing a new block-level deduplication format or custom crypto. Start with a simple, verifiable ZIP-based portable migration archive and design the snapshot/engine interface for incremental backups later; evaluate Rustic Core in an isolated spike when its Windows API compatibility is known. Keep the archive format documented.

## Feature decisions ranked by user value

### P0 — before any actual Windows format
- Searchable installed app selector: matched WinGet IDs vs manual apps, explicit selection and searchable checkboxes.
- Preflight inventory: total estimated size, missing permissions, app-open warnings, database availability, output drive free space.
- Explicit module statuses: backed up / skipped / warning / failed / manual restore required.
- Portable ZIP or multipart ZIP (size configured) **with SHA-256 checksums**, a versioned manifest, and a verification command.
- Application-aware migration rules (start with VS Code, Chrome bookmarks/history, XAMPP SQL, PHP/Node versions, Git, SSH and custom folders).
- Restore preview and dry run: show file conflicts, install count, paths, versions and permissions before writing.
- An independent disaster-recovery checklist; test one VS Code profile and one small database restore.

### P1 — Google Drive and reliable routine backup
- Google OAuth2 desktop app + PKCE + loopback redirect + minimal `drive.file` scope. Tokens stored via OS secret protection, never in backup archive or repo.
- Optional "Upload to Google Drive" checkbox after local backup; automatically create/find app-owned `Dev Backup` folder.
- Resumable uploads, retry with exponential backoff, upload progress (bytes transferred), cancel/resume, checksums and remote file ID.
- Encrypt backups before cloud upload with vetted cryptography and recovery key or passphrase; plaintext personal backup files must not silently be uploaded.
- Auto-backup schedules, completion notifications, history, retention (7 daily / 4 weekly / 3 monthly as configurable example), drive quota checks.
- Cloud restore flow: connect Google, browse owned archives, download, verify/decrypt, preview and select items.

### P2 — longer-term power features
- Content-addressed incremental snapshots/deduplication, browse historical file versions, and clean up unused content safely.
- Win VSS integration for locked files, with error reporting and application-specific consistency handling; SQL uses logical exports.
- Plugin rules for known apps with version-aware path mapping and registry keys; signed community rule catalog with review.
- Restore-point creation *before future driver changes*, and driver export/diagnostic inventory. No unattended driver installation.
- Full-disk image workflow as a **separate** future module, never conflated with per-app migration archives.
- CLI/headless mode using the same Rust core, accessibility, keyboard navigation and automated tests.

## Important product distinctions
- A WinGet package manifest is not the installed program binaries; clean Windows will reinstall packages using supported sources.
- Chrome profile copies are not a way to restore passwords, passkeys, cookies or logged-in sessions; use Chrome Sync and expect some sign-ins.
- Google Drive storing an archive is not a full disk image or bootable recovery.
- Restore points are local Windows recovery objects, not portable replacements for data backups.
- Archive integrity check (hash present) is necessary but insufficient: periodically test **actual restores**.
- Readable logs should never contain SQL passwords, Google OAuth tokens, user keys, private file names unnecessarily, or sensitive file contents.

## UI direction (professional, not generic AI dashboard)
- Restrained neutral palette, consistent 8px spacing grid, Segoe UI/Inter typography, no gratuitous gradients or decorative cards.
- Sidebar sections: Overview, Backup, Restore, Applications, Drivers, Cloud, History, Settings.
- Main Backup screen: source inventory table with search and checkboxes, destination card, estimated output size, dry-run summary, progress/log.
- Restore screen: snapshot list + timestamp + storage destination + verified status + conflict preview.
- Cloud screen: account connected state, Drive folder, storage/transfer status, reconnect/disconnect, encryption reminder.
- Use precise and conservative status labels; never say 'verified' without an integrity/restore test.

## Source links
- Kopia: https://github.com/kopia/kopia
- Duplicati: https://github.com/duplicati/duplicati
- Restic: https://github.com/restic/restic
- Rustic: https://github.com/rustic-rs/rustic
- Rustic Core: https://github.com/rustic-rs/rustic_core
- Backrest: https://garethgeorge.github.io/backrest/
- UrBackup: https://www.urbackup.org/
- Win Apps Migrator: https://github.com/ox1d3x3/winappsmigrate
- CloneApp (closed source): https://github.com/builtbybel/CloneApp
- Google Drive OAuth desktop: https://developers.google.com/identity/protocols/oauth2/native-app
- Google Drive restricted scopes: https://developers.google.com/workspace/drive/api/guides/api-specific-auth
- Google Drive resumable upload: https://developers.google.com/workspace/drive/api/guides/manage-uploads
