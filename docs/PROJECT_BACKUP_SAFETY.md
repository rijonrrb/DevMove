# Selective project folder backup (developer preview)

The **My project folders** module allows the user to choose up to eight project directories to include in a DevMove local ZIP archive.

## What is included

- Ordinary files within explicitly selected project folders, preserving relative paths.
- Directory names and ordinary project source files.
- Source files are recorded in the archive under Projects/01_folder-name, Projects/02_folder-name and so on.
- Per-file SHA-256 digests, archive verification, and manual extraction into a **new** staging folder.

No folders are scanned automatically and no archived file is directly applied to an existing project or Windows configuration during extraction.

## Exclusions and safety

DevMove excludes common generated dependency directories, including node_modules, vendor, .git, target, build, dist, .next, .venv, .cache and selected IDE caches. It does not follow symlinks and skips common credential files including .env and .env.* files, service-account files, private key file extensions and similar known secret names.

These exclusions are **heuristics**. Other filenames or source files may still contain secrets, API keys, personal data or credentials. The archive is not encrypted. Review every export before sharing, and keep the ZIP in a protected destination. Make an independent encrypted backup of excluded environment files only if authorized and necessary.

Limits prevent accidental unbounded copying: at most eight selected project folders, up to 50,000 included project files or 10 GiB of project content per backup; individual project files over 2 GiB are skipped and reported in the ZIP warnings. Large media files and databases require separate, purpose-built migration procedures.

DevMove rejects backups when a selected project folder and the archive destination contain each other, preventing recursive backups. Nested or overlapping selected project folders are also rejected.

## Known limitations

- The project module does not migrate the runtime, installed software, database services, credentials, environment variables or private keys.
- Skipped files and directories are summarized as warnings; always review the backup summary.
- File permissions, timestamps and NTFS extended attributes are not fully preserved by this developer preview.
- Generated directories must be re-created from package manifests after restoring the project.
- ZIP integrity does not prove a full project or system restore is possible.

## Verification

Rust unit tests exercise exclusions, destination overlap protection, ZIP round trips, SHA-256 checks and extracting to a new folder. Windows GitHub Actions runs these tests on every pull request.
