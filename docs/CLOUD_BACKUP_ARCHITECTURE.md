# DevMove: Local ZIP + Google Drive cloud backup — implementation contract

**Status:** Proposed implementation, not yet shipped. Implement as first-party Rust/Tauri modules. OAuth app registration & user consent will be needed at runtime.

## User journey
1. Select source modules, check individual apps, and select an external/local destination.
2. Optional checkbox: **Also upload an encrypted copy to Google Drive**. If not connected, show **Connect Google Drive**, launch system browser with OAuth consent and PKCE, then resume.
3. Preflight: estimate size, verify destination capacity, check permissions, identify open files and unsupported apps; show manual steps and checkboxes.
4. Create local staging folder and validated manifest. Export app-specific configuration and database SQL safely, respecting user selections.
5. Package into **portable archive** (.zip for modest sizes; split into numbered volumes only when a supported, tested archive format and restore implementation exists). Maintain manifest, source paths, UTC timestamps, error statuses, SHA-256 hashes, schema version and archive index.
6. Verify checksum and archive readability, then atomically move to selected destination. Never delete a previous good backup on failed write.
7. If cloud selected: encrypt with vetted library and user-managed recovery key or passphrase, then upload with Drive resumable protocol. If network fails, local backup stays intact and queued transfer can resume.
8. Check uploaded file size/metadata/hash where possible; mark **Uploaded** separately from **Backup verified** and **Restored in test**. Never claim remote success without an acknowledged API response.
9. History shows local/cloud status separately; restore wizard can choose local files or authorized Drive archive, download, verify, decrypt, dry run, and restore selected categories.

## First-party Rust architecture
- `inventory`: installed apps and Windows paths; WinGet and registry inventory.
- `rules`: curated declarative per-app rules (files, exclusions, version guards, expected prerequisites).
- `backup`: preflight + staging + manifest + archiver + integrity check.
- `restore`: non-destructive inspection, selective conflict plan, transfer and operation logs.
- `cloud/google_drive`: OAuth PKCE, app-owned folder discovery/creation, resumable upload, remote listings/downloads, disconnect.
- `secrets`: OS-protected OAuth tokens; separate encryption/recovery key management. No hard-coded client secret in a desktop binary.
- `jobs`: progress events, cancellation checkpoints, persisted resume state, one active writer per archive.
- `history`: metadata only; never store credentials or content excerpts.
- `ui`: Tauri 2 + React/TypeScript. All privileged operations and filesystem access remain behind narrow Rust commands.

## OAuth specifics
- Use **Google OAuth 2.0 Desktop Application** client with external system browser, PKCE, and 127.0.0.1 loopback redirect. Never embed a password-entry browser view.
- Request the narrow `https://www.googleapis.com/auth/drive.file` scope, creating an app-owned root folder named `Dev Backup` and files within it. Do not request broad access to all Drive files.
- Store refresh tokens only in an OS-protected credential store. Support user-initiated **Disconnect**, credential revocation where possible, and safe reauthorization.
- Show requested permissions clearly and explain that login and backup encryption are separate mechanisms.
- Drive API configuration requires a Google Cloud project, Drive API enabled, and a registered Desktop OAuth client. Production OAuth verification requirements must be confirmed before a public release.

## Resumable transfer
- Use Drive API `uploadType=resumable`; start a resumable session with app-owned folder ID and media type. Stream encrypted archive in chunks.
- Persist only non-secret job metadata and safe resumable state; retry quota/5xx errors with exponential backoff + jitter, and re-authenticate on invalid tokens.
- Distinguish **Pause**, **Cancel**, **Retry**; resume when the remote API still accepts the upload session, otherwise restart transfer without deleting the local backup.
- Do not invent 'pause' UX until the transfer implementation actually supports it.
- Confirm Drive file ID, size and upload completion before success. To validate remote bytes, use server-provided checksum when available or download/test-check a sample; remote metadata alone is not a restore test.
- Allow upload bandwidth limit, chunk size and scheduling options later; 5+ MB backups especially warrant resumable transfer.

## Backup archive manifest proposal
```json
{
  "schemaVersion": 1,
  "tool": "DevMove",
  "backupId": "uuid",
  "createdAtUtc": "ISO8601",
  "machine": "stable nonsecret user-facing label",
  "archiveFormat": "zip",
  "encryption": { "scheme": "vetted AEAD suite; exact implementation TBD" },
  "sourceModules": ["apps", "vscode", "chrome", "database"],
  "files": [
    { "path": "VSCode/User/settings.json", "size": 1234, "sha256": "hex", "status": "ok" }
  ],
  "warnings": [],
  "errors": [],
  "backupState": "local-verified",
  "cloud": { "requested": true, "state": "uploaded", "fileId": "opaque" }
}
```
*Sample schema only.* Final manifest must not contain access tokens, full sensitive file paths or private secrets. Store cloud-specific fields in local history if keeping the archive content provider-agnostic.

## Integrity, privacy and retention
- Compute hashes as data is archived; validate archive structure and hashes before reporting success.
- Use vetted authenticated encryption (not ZIP's obsolete ZipCrypto). Do not implement home-made crypto. Keep decryption instructions available offline.
- With stored credentials, DB dumps, browser history and SSH private keys, default to **encryption required** for Google Drive.
- Backups can consume large amounts of Google storage; display free quota when available and storage estimates before upload.
- Backup retention policies are opt-in; default to never deleting last known-good backup. For remote deletes, show a preview and require explicit consent.
- Consider 3-2-1 backup discipline: source data, local external copy, cloud copy; a single cloud copy is not sufficient.
- Use structured audit logs and warnings, but redact credentials, tokens, and secrets.

## Acceptance criteria
- Local ZIP created and hash-verified from small synthetic sample set; selective restore matches original hashes.
- Offline/network failure leaves a usable local archive and accurate cloud pending status.
- OAuth consent can be completed by user once and securely reused; disconnect prevents further uploads.
- Cloud uploader resumes interrupted transfers without replacing the user's good local archive.
- Restore requires an explicit confirmation before overwriting existing files or running installers.
- Database dump errors, Chrome open-file warnings, credential reauth, quota limits and insufficient space are surfaced plainly.
- Hardware/driver data is never altered by cloud backup or by the scan.
