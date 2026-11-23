# DevMove Toolkit - Windows Developer Migration

Run by double-clicking **Launch DevMove.bat**. Native Windows GUI uses built-in PowerShell 5.1 and WinForms; no compiler needed.

## BEFORE reinstalling Windows

1. Choose Backup mode and select the modules you want.
2. Choose **D:, E:, or preferably an external drive** as the output directory. Backups on C: may be erased. Repartitioning the *whole disk* can also erase D:!
3. Close Chrome completely for browser history; if using databases, start XAMPP MySQL/MariaDB first.
4. Enter MySQL username/password in the GUI if necessary; password is never written to its config or logs.
5. Press Scan my PC, then Start backup, and review progress plus warnings.
6. Each run creates a DevMove_YYYYMMDD_HHMMSS folder containing a manifest, backup-result.json and backup-log.txt.
7. **Copy this entire DevMove Toolkit folder to the backup drive too**, otherwise the tool itself will be lost when Windows is reinstalled.
8. Verify your SQL and project files before formatting anything. Keep a second copy if possible.

## CONTENT AND RESTORE

| Module | Captures | Restores |
| --- | --- | --- |
| Apps | WinGet apps.json | WinGet installs supported apps, may require manual prompts |
| VS Code | settings, snippets, profiles, extension lists | Settings + reinstall extensions. Also turn on VS Code Settings Sync |
| Chrome | per-profile bookmarks, history, favicons, if accessible | File copy only after closing Chrome; use Google Chrome Sync for passwords, extensions, preferences |
| XAMPP settings | php.ini, Apache and MySQL config | Manual compare/merge; do not overwrite newer config |
| Databases | all-databases.sql using mysqldump | **Manual import only** after checking MySQL/MariaDB version and database contents |
| Node/PHP | versions and global npm package inventory | Reinstall manually, refer to inventory |
| Git | global Git config | Copies files to your new user profile |
| Projects | XAMPP htdocs or additional folder, skipping node_modules, vendor, build caches | Manual relocation; reinstall packages |
| SSH | sensitive .ssh directory | Manual restore and check permissions; disabled by default |

## CHROME AND LOGINS - IMPORTANT

Enable Chrome Sync and Google Password Manager in your OLD Windows account **before wiping it**. A raw Chrome folder backup cannot reliably move passwords, passkeys, saved sessions or 2FA to a new Windows account (Windows DPAPI protection). The app intentionally does not extract cookies, raw passwords, profile encryption keys or authentication tokens. You may need to sign in to websites again. Chrome must be fully closed to copy History successfully; inspect the warnings and the actual file.

VS Code Settings Sync is also recommended. Extension authentication, workspaceStorage, license state and sign-in tokens may not migrate.

## DATABASE SAFETY

The app uses XAMPP mysqldump with --all-databases, --routines, --events, --triggers, --single-transaction. It can include MySQL/MariaDB internal system schemas and may not be compatible across versions. Do not blindly import the entire dump into another version. Manually review/restore user schemas. Check the SQL output is nonempty and test on an isolated database first. Non-transactional tables require extra care.

## RESTORE

1. Install clean Windows, updates, VS Code, Chrome, Git, Node and compatible XAMPP as needed.
2. Copy both the toolkit and your DevMove_... backup folder to the new PC.
3. Launch the toolkit, switch to Restore mode, browse to the backup folder containing manifest.json, choose tasks and click Start restore.
4. Close Chrome and VS Code before restoring their files. Be careful not to overwrite data after sync has started.
5. XAMPP, database SQL, project folders, SSH keys and npm packages are *manual restore* items (the GUI locates them and logs what to do).
6. Sign in where required; test at least one website/project and database before considering migration complete.

## SAFETY / LIMITATIONS

- The backup directory is not automatically zipped. It is safer for large SQL/project files; you can compress it afterward.
- Backup might contain personal browsing history, SQL data, project .env files, SSH keys and confidential information. Use encrypted offline/external storage; never push it to public GitHub.
- No disk formatting, deletion of old Windows or automatic SQL importing is performed.
- Closing the GUI does not cancel a running background backup.
- Detailed job logs and status files live under toolkit/_runtime.
- Full disk imaging and tested restores are still recommended for complete machine recovery. WSL distro, Docker, software license state, certificates, hardware drivers, and all system-wide environment variables are not included.

Version 1.0.
