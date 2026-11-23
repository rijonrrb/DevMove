# Read-only restore preview

The developer-preview Restore & verify screen accepts a DevMove ZIP archive created by the Rust desktop application. It checks all recorded file SHA-256 hashes before showing a restore plan.

## What is displayed

- Backup creation timestamp, selected modules and file count.
- Recorded uncompressed size and any warnings or exclusions.
- WinGet package IDs present in the archived WinGet export (not guaranteed to be available on the new computer).
- Manually installed or unmatched software that requires human review.

The preview uses information **inside the archive**, not data from an online application catalog. It neither downloads anything nor changes Windows. Any software that was selected in the original app inventory but has no matching WinGet export entry appears in the manual review list.

## Workflow

1. Open Restore & verify in the native Windows application.
2. Choose a previously created DevMove ZIP file.
3. Wait for SHA-256 verification; invalid archives are rejected.
4. Review application IDs, manual reinstalls, included files and warnings.
5. If appropriate, extract verified contents into a **new** staging folder. This does not overwrite current files.

Do not use the ZIP alone as evidence that all settings, software, credentials or databases can be restored. Keep a second independently tested backup before reinstalling Windows.

## Security boundaries

- The restore plan is read-only. No WinGet install command is executed.
- Archive metadata parsing is size-limited to reduce memory exhaustion from unexpected ZIP entries.
- ZIP verification rejects unsafe archive paths or mismatched content digests.
- ZIP files are unencrypted, so private backup contents should never be shared publicly.

## Not yet supported

- Package-by-package installation with confirmation, version pinning, offline cache, progress, cancellation and rollback.
- Applying project settings or browser profiles into their original locations, with conflict resolution.
- Database migration and exact service/runtime reconstruction.
- Google Drive download/decrypt/recovery and persistent restore history.
