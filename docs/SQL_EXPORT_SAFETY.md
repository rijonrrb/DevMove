# SQL export backup (developer preview)

DevMove can include **existing SQL export files** in a local ZIP backup. It does not connect to MySQL or MariaDB, request database credentials, create database dumps, validate SQL statements, or import data into a database.

## How to use

1. Export the database independently using phpMyAdmin or the database server's supported export tool. Prefer a consistent transaction snapshot for databases that support it.
2. Save the resulting file as .sql or .sql.gz in a private folder and verify it can be read.
3. In DevMove > Create backup, click **Add SQL file** and explicitly select up to 12 exports. This automatically enables **Existing SQL database exports**.
4. Choose a safe backup destination on a separate physical drive, create the ZIP, and check warnings.
5. On the target computer, use Restore & verify to verify and extract the ZIP into a **new staging folder**. Database exports appear under Databases/ with a numbered prefix.
6. Inspect the extracted files and import into a deliberately chosen database with your trusted database administration tool after verifying server versions, schema requirements, encoding, user privileges, and applications.

## Safety and limitations

- ZIP backups in this developer preview are **not encrypted**. SQL exports can contain names, emails, financial records, application secrets, and other sensitive data. Do not upload or share ZIP files publicly.
- The SHA-256 archive verification confirms that recorded bytes survived the backup unchanged. It **does not** prove that the original dump is complete, internally valid, or restorable to MySQL.
- Only explicitly selected regular files with a .sql or .sql.gz filename are accepted; symbolic links and unsupported extensions are rejected. No broad disk search occurs.
- There is no password storage, no connection string, and no automatic access to XAMPP directories.
- The output ZIP can hold a compressed SQL file but does not decompress or parse it. Unsupported database versions, table engines, missing functions, collation changes, and other compatibility issues may block actual import.
- Test an export and re-import with a disposable database **before** formatting or reinstalling Windows.

**Planned, not implemented:** direct XAMPP/MariaDB discovery, transaction-safe native database dumps, guided import, restore compatibility validation, encrypted cloud backup, and unattended migration.
