//! DevMove's first Windows-native inventory and local-archive backend.
//! No driver installation, secrets extraction, or cloud operations occur here.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub name: String,
    pub id: String,
    pub version: String,
    pub source: String,
    pub supported: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppScan {
    pub apps: Vec<Application>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Driver {
    pub name: String,
    pub manufacturer: String,
    pub provider: String,
    pub version: String,
    pub device_id: String,
    pub inf: String,
    pub problem_code: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverScan {
    pub devices: Vec<Driver>,
    pub issues: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRequest {
    pub destination: String,
    pub modules: Vec<String>,
    pub app_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveManifest {
    pub schema_version: u32,
    pub created_at: String,
    pub modules: Vec<String>,
    pub selected_apps: Vec<String>,
    pub files: Vec<ArchivedFile>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub archive_path: String,
    pub file_count: usize,
    pub bytes_written: u64,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub verified: bool,
    pub files_checked: usize,
    pub errors: Vec<String>,
}

fn normalize(input: &str) -> String {
    input.chars().filter(|c| c.is_ascii_alphanumeric()).flat_map(char::to_lowercase).collect()
}

fn registry_apps() -> Vec<Application> {
    let roots = [
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    let mut found = BTreeMap::new();
    for (root, path) in roots {
        let Ok(key) = RegKey::predef(root).open_subkey(path) else { continue };
        for entry in key.enum_keys().flatten() {
            let Ok(sub) = key.open_subkey(entry) else { continue };
            let Ok(name) = sub.get_value::<String, _>("DisplayName") else { continue };
            if name.trim().is_empty() { continue }
            let system_component: u32 = sub.get_value("SystemComponent").unwrap_or(0);
            if system_component == 1 { continue }
            let version: String = sub.get_value("DisplayVersion").unwrap_or_default();
            found.entry(normalize(&name)).or_insert(Application {
                name: name.trim().to_owned(),
                id: format!("manual:{}", name.trim()),
                version,
                source: "Manual reinstall".to_owned(),
                supported: false,
            });
        }
    }
    found.into_values().collect()
}


fn winget_export() -> Result<Value, String> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let path = std::env::temp_dir().join(format!("devmove-winget-{stamp}.json"));
    let output = Command::new("winget.exe")
        .arg("export").arg("-o").arg(&path)
        .args(["--accept-source-agreements", "--disable-interactivity"])
        .output().map_err(|e| format!("WinGet is unavailable: {e}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(&path);
        return Err(format!("WinGet export failed (exit {:?})", output.status.code()));
    }
    let result = fs::read(&path)
        .map_err(|e| format!("Cannot read WinGet export: {e}"))
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()));
    let _ = fs::remove_file(&path);
    result
}

fn scan_apps_inner() -> AppScan {
    let mut apps = registry_apps();
    let mut warnings = Vec::new();
    match winget_export() {
        Ok(root) => {
            if let Some(sources) = root.get("Sources").and_then(Value::as_array) {
                for source in sources {
                    let source_name = source.pointer("/Details/Name")
                        .and_then(Value::as_str).unwrap_or("WinGet").to_owned();
                    let Some(packages) = source.get("Packages").and_then(Value::as_array) else { continue };
                    for package in packages {
                        let Some(id) = package.get("PackageIdentifier").and_then(Value::as_str) else { continue };
                        let tail = normalize(id.rsplit('.').next().unwrap_or(""));
                        let matching = if tail.len() >= 5 {
                            apps.iter_mut().find(|a| normalize(&a.name).contains(&tail))
                        } else { None };
                        if let Some(existing) = matching {
                            existing.id = id.to_owned();
                            existing.source = source_name.clone();
                            existing.supported = true;
                        } else {
                            apps.push(Application {
                                name: id.rsplit('.').next().unwrap_or(id).to_owned(),
                                id: id.to_owned(),
                                version: String::new(),
                                source: source_name.clone(),
                                supported: true,
                            });
                        }
                    }
                }
            }
        }
        Err(message) => warnings.push(message),
    }
    apps.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    AppScan { apps, warnings }
}

#[tauri::command]
pub async fn scan_apps() -> Result<AppScan, String> {
    tauri::async_runtime::spawn_blocking(scan_apps_inner).await
        .map_err(|e| e.to_string())
}

fn ps_output(script: &str) -> Result<Value, String> {
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output().map_err(|e| format!("PowerShell unavailable: {e}"))?;
    if !output.status.success() {
        return Err(format!("Device inventory failed (exit {:?})", output.status.code()));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("Driver JSON parsing failed: {e}"))
}

fn text_field(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn scan_drivers_inner() -> Result<DriverScan, String> {
    let command = "$ErrorActionPreference='Stop'; $ds=@(Get-CimInstance Win32_PnPSignedDriver | Select-Object DeviceName,Manufacturer,DriverProviderName,DriverVersion,DeviceID,InfName); $bad=@(Get-CimInstance Win32_PnPEntity | Where-Object { $_.ConfigManagerErrorCode -ne 0 } | Select-Object DeviceID,ConfigManagerErrorCode); @{devices=$ds;issues=$bad} | ConvertTo-Json -Depth 5 -Compress";
    let root = ps_output(command)?;
    let mut problems = BTreeMap::new();
    if let Some(values) = root.get("issues").and_then(Value::as_array) {
        for v in values {
            if let Some(id) = v.get("DeviceID").and_then(Value::as_str) {
                let code = v.get("ConfigManagerErrorCode").and_then(Value::as_i64).unwrap_or(0);
                problems.insert(id.to_string(), code);
            }
        }
    }
    let mut devices = Vec::new();
    if let Some(values) = root.get("devices").and_then(Value::as_array) {
        for item in values {
            let id = text_field(item, "DeviceID");
            let name = text_field(item, "DeviceName");
            if name.trim().is_empty() { continue }
            devices.push(Driver {
                name, manufacturer: text_field(item, "Manufacturer"),
                provider: text_field(item, "DriverProviderName"),
                version: text_field(item, "DriverVersion"),
                inf: text_field(item, "InfName"),
                problem_code: *problems.get(&id).unwrap_or(&0),
                device_id: id,
            });
        }
    }
    devices.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(DriverScan { issues: problems.len(), devices })
}

#[tauri::command]
pub async fn scan_drivers() -> Result<DriverScan, String> {
    tauri::async_runtime::spawn_blocking(scan_drivers_inner).await
        .map_err(|e| e.to_string())?
}

fn add_file(
    zip: &mut ZipWriter<File>,
    source: &Path,
    archive_path: String,
    files: &mut Vec<ArchivedFile>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Ok(mut source_file) = File::open(source) else {
        warnings.push(format!("Skipped locked or inaccessible file: {archive_path}"));
        return Ok(());
    };
    zip.start_file(&archive_path, SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated))
        .map_err(|e| e.to_string())?;
    let mut buf = [0u8; 65536];
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    loop {
        let count = source_file.read(&mut buf).map_err(|e| e.to_string())?;
        if count == 0 { break }
        hash.update(&buf[..count]);
        zip.write_all(&buf[..count]).map_err(|e| e.to_string())?;
        bytes += count as u64;
    }
    files.push(ArchivedFile { path: archive_path, bytes, sha256: format!("{:x}", hash.finalize()) });
    Ok(())
}

fn add_tree(
    zip: &mut ZipWriter<File>,
    source: &Path,
    prefix: &str,
    files: &mut Vec<ArchivedFile>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if !source.exists() { warnings.push(format!("{prefix} not found")); return Ok(()) }
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() { continue }
        let relative = entry.path().strip_prefix(source).map_err(|e| e.to_string())?;
        let archive_path = format!("{prefix}/{}", relative.to_string_lossy().replace('\\', "/"));
        add_file(zip, entry.path(), archive_path, files, warnings)?;
    }
    Ok(())
}

fn write_json(
    zip: &mut ZipWriter<File>,
    archive_path: &str,
    data: &Value,
    files: &mut Vec<ArchivedFile>,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(data).map_err(|e| e.to_string())?;
    zip.start_file(archive_path, SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)).map_err(|e| e.to_string())?;
    zip.write_all(&bytes).map_err(|e| e.to_string())?;
    files.push(ArchivedFile {
        path: archive_path.to_owned(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    });
    Ok(())
}

fn create_backup_inner(request: BackupRequest) -> Result<BackupResult, String> {
    let destination = PathBuf::from(&request.destination);
    if !destination.is_dir() { return Err("Choose an existing backup destination folder".to_owned()) }
    if request.modules.is_empty() { return Err("Select at least one backup category".to_owned()) }
    let allowed: HashSet<&str> = ["apps", "vscode", "chrome", "git", "node"].into_iter().collect();
    if request.modules.iter().any(|item| !allowed.contains(item.as_str())) {
        return Err("One or more backup categories are not supported by the Rust preview".to_owned());
    }
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S_%3f").to_string();
    let filename = format!("DevMove_{timestamp}.zip");
    let archive_path = destination.join(filename);
    let part_path = archive_path.with_extension("zip.partial");
    let work = || -> Result<(usize, Vec<String>), String> {
        let file = File::create(&part_path).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(file);
        let mut files = Vec::new();
        let mut warnings = Vec::new();
        let modules: HashSet<&str> = request.modules.iter().map(String::as_str).collect();
        let home = PathBuf::from(std::env::var("USERPROFILE").map_err(|e| e.to_string())?);

        if modules.contains("vscode") {
            if let Ok(appdata) = std::env::var("APPDATA") {
                let dir = PathBuf::from(appdata).join("Code").join("User");
                for name in ["settings.json", "keybindings.json", "tasks.json"] {
                    let src = dir.join(name);
                    if src.is_file() { add_file(&mut zip, &src, format!("VSCode/User/{name}"), &mut files, &mut warnings)?; }
                }
                for name in ["snippets", "profiles"] {
                    add_tree(&mut zip, &dir.join(name), &format!("VSCode/User/{name}"), &mut files, &mut warnings)?;
                }
            }
        }
        if modules.contains("chrome") {
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                let base = PathBuf::from(local).join("Google").join("Chrome").join("User Data");
                if let Ok(profiles) = fs::read_dir(&base) {
                    for entry in profiles.flatten() {
                        let profile = entry.file_name().to_string_lossy().to_string();
                        if profile != "Default" && !profile.starts_with("Profile ") { continue }
                        for name in ["Bookmarks", "Bookmarks.bak", "History", "Favicons"] {
                            let src = entry.path().join(name);
                            if src.is_file() {
                                add_file(&mut zip, &src, format!("Chrome/{profile}/{name}"), &mut files, &mut warnings)?;
                            }
                        }
                    }
                } else { warnings.push("Chrome profile folder not found".into()); }
                warnings.push("Chrome passwords, passkeys, cookies, and authenticated sessions are intentionally not migrated. Enable Chrome Sync.".into());
            }
        }
        if modules.contains("git") {
            for name in [".gitconfig", ".gitignore_global"] {
                let src = home.join(name);
                if src.is_file() { add_file(&mut zip, &src, format!("Git/{name}"), &mut files, &mut warnings)?; }
            }
        }
        if modules.contains("apps") {
            let selected: HashSet<&str> = request.app_ids.iter().map(String::as_str).collect();
            match winget_export() {
                Ok(mut source) => {
                    if let Some(sources) = source.get_mut("Sources").and_then(Value::as_array_mut) {
                        for item in sources.iter_mut() {
                            if let Some(packages) = item.get_mut("Packages").and_then(Value::as_array_mut) {
                                packages.retain(|p| p.get("PackageIdentifier").and_then(Value::as_str)
                                    .is_some_and(|id| selected.contains(id)));
                            }
                        }
                        sources.retain(|item| item.get("Packages").and_then(Value::as_array).is_some_and(|packages| !packages.is_empty()));
                    }
                    write_json(&mut zip, "Applications/winget.json", &source, &mut files)?;
                }
                Err(err) => warnings.push(format!("Selected WinGet package inventory was not exported: {err}")),
            }
            write_json(&mut zip, "Applications/selected.json", &json!(request.app_ids), &mut files)?;
        }
        if modules.contains("node") {
            let mut versions = BTreeMap::new();
            for (label, program, args) in [
                ("Node", "node.exe", vec!["--version"]),
                ("npm", "npm.cmd", vec!["--version"]),
                ("PHP", "php.exe", vec!["--version"]),
                ("Git", "git.exe", vec!["--version"])
            ] {
                if let Ok(output) = Command::new(program).args(args).output() {
                    let out = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                    versions.insert(label, out);
                }
            }
            write_json(&mut zip, "Development/versions.json", &json!(versions), &mut files)?;
        }
        if files.is_empty() { warnings.push("No source files were found for the selected categories".into()); }
        let manifest = ArchiveManifest {
            schema_version: 1,
            created_at: Utc::now().to_rfc3339(),
            modules: request.modules.clone(),
            selected_apps: request.app_ids.clone(),
            files,
            warnings: warnings.clone(),
        };
        zip.start_file("manifest.json", SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)).map_err(|e| e.to_string())?;
        zip.write_all(&serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        zip.finish().map_err(|e| e.to_string())?;
        Ok((manifest.files.len(), warnings))
    };
    let (count, warnings) = match work() {
        Ok(out) => out,
        Err(err) => { let _ = fs::remove_file(&part_path); return Err(err) }
    };
    let verification = verify_backup_inner(&part_path.to_string_lossy())?;
    if !verification.verified {
        let _ = fs::remove_file(&part_path);
        return Err(format!("Archive did not verify: {}", verification.errors.join("; ")));
    }
    fs::rename(&part_path, &archive_path).map_err(|e| e.to_string())?;
    let bytes = fs::metadata(&archive_path).map_err(|e| e.to_string())?.len();
    Ok(BackupResult {
        archive_path: archive_path.to_string_lossy().to_string(),
        file_count: count,
        bytes_written: bytes,
        warnings,
    })
}

fn verify_backup_inner(path: &str) -> Result<Verification, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open archive: {e}"))?;
    let mut zip = ZipArchive::new(file).map_err(|e| e.to_string())?;
    let manifest: ArchiveManifest = {
        let mut entry = zip.by_name("manifest.json").map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
    };
    if manifest.schema_version != 1 { return Err("Unsupported archive format version".to_owned()) }
    let mut errors = Vec::new();
    for record in &manifest.files {
        if record.path.contains("..") || record.path.starts_with('/') || record.path.contains('\\') {
            errors.push(format!("Unsafe file path: {}", record.path)); continue
        }
        match zip.by_name(&record.path) {
            Ok(mut entry) => {
                let mut hasher = Sha256::new();
                let mut buf = [0u8; 65536];
                let mut total = 0u64;
                loop {
                    match entry.read(&mut buf) {
                        Ok(0) => break,
                        Ok(count) => { hasher.update(&buf[..count]); total += count as u64; }
                        Err(err) => { errors.push(format!("{}: {err}", record.path)); break }
                    }
                }
                if format!("{:x}", hasher.finalize()) != record.sha256 || total != record.bytes {
                    errors.push(format!("Checksum mismatch: {}", record.path));
                }
            }
            Err(err) => errors.push(format!("Missing {}: {err}", record.path)),
        }
    }
    Ok(Verification { verified: errors.is_empty(), files_checked: manifest.files.len(), errors })
}

#[tauri::command]
pub async fn create_backup(request: BackupRequest) -> Result<BackupResult, String> {
    tauri::async_runtime::spawn_blocking(move || create_backup_inner(request))
        .await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn verify_backup(path: String) -> Result<Verification, String> {
    tauri::async_runtime::spawn_blocking(move || verify_backup_inner(&path))
        .await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_matching_normalizes_names() {
        assert_eq!(normalize("Visual Studio Code"), "visualstudiocode");
        assert_eq!(normalize("7-Zip (64-bit)"), "7zip64bit");
    }

    #[test]
    fn zip_round_trip_verifies_file_hashes() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("devmove-test-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.txt");
        fs::write(&source, b"DevMove archive integrity test").unwrap();
        let archive = dir.join("sample.zip");
        {
            let mut zip = ZipWriter::new(File::create(&archive).unwrap());
            let mut files = Vec::new();
            let mut warnings = Vec::new();
            add_file(&mut zip, &source, "sample.txt".into(), &mut files, &mut warnings).unwrap();
            assert!(warnings.is_empty());
            let manifest = ArchiveManifest {
                schema_version: 1,
                created_at: "2026-11-23T00:00:00Z".into(),
                modules: vec!["test".into()],
                selected_apps: Vec::new(),
                files,
                warnings,
            };
            zip.start_file("manifest.json", SimpleFileOptions::default()).unwrap();
            zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
            zip.finish().unwrap();
        }
        let check = verify_backup_inner(archive.to_str().unwrap()).unwrap();
        assert!(check.verified, "{:?}", check.errors);
        assert_eq!(check.files_checked, 1);
        let restored = extract_archive_inner(archive.to_str().unwrap(), dir.to_str().unwrap()).unwrap();
        assert_eq!(restored.files_extracted, 1);
        assert_eq!(fs::read(PathBuf::from(restored.output_directory).join("sample.txt")).unwrap(), b"DevMove archive integrity test");
        fs::remove_dir_all(&dir).unwrap();
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractResult {
    pub output_directory: String,
    pub files_extracted: usize,
    pub bytes_written: u64,
}

fn extract_archive_inner(archive_path: &str, base_directory: &str) -> Result<ExtractResult, String> {
    let verification = verify_backup_inner(archive_path)?;
    if !verification.verified {
        return Err(format!("Archive checksum verification failed: {}", verification.errors.join("; ")));
    }
    let base = PathBuf::from(base_directory);
    if !base.is_dir() {
        return Err("Choose an existing extraction destination folder".into());
    }
    let stamp = Utc::now().format("%Y%m%d_%H%M%S_%3f").to_string();
    let output = base.join(format!("DevMove_Extracted_{stamp}"));
    fs::create_dir(&output).map_err(|e| format!("Cannot create extraction folder: {e}"))?;

    let result = (|| -> Result<(usize, u64), String> {
        let mut zip = ZipArchive::new(File::open(archive_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let manifest: ArchiveManifest = {
            let mut entry = zip.by_name("manifest.json").map_err(|e| e.to_string())?;
            let mut buffer = Vec::new();
            entry.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
            serde_json::from_slice(&buffer).map_err(|e| e.to_string())?
        };

        let mut extracted = 0usize;
        let mut copied = 0u64;
        for record in manifest.files {
            if record.path.starts_with('/') || record.path.contains('\\')
                || record.path.split('/').any(|segment| segment == ".." || segment == ".")
            {
                return Err(format!("Refusing unsafe archive path: {}", record.path));
            }

            let mut entry = zip.by_name(&record.path).map_err(|e| e.to_string())?;
            let safe_path = entry.enclosed_name()
                .ok_or_else(|| format!("Refusing unsafe archive entry: {}", record.path))?;
            let target = output.join(safe_path);
            if !target.starts_with(&output) { return Err("Archive path escaped destination".into()); }

            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            // create_new protects any existing file from accidental overwrites.
            let mut file = File::options().write(true).create_new(true)
                .open(&target).map_err(|e| format!("Cannot create {}: {e}", record.path))?;
            let count = std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
            if count != record.bytes { return Err(format!("Incomplete extraction: {}", record.path)); }
            extracted += 1;
            copied += count;
        }
        Ok((extracted, copied))
    })();
    match result {
        Ok((files_extracted, bytes_written)) => Ok(ExtractResult {
            output_directory: output.to_string_lossy().to_string(),
            files_extracted,
            bytes_written,
        }),
        Err(err) => Err(format!("{err}. Partial extraction, if any: {}", output.display())),
    }
}

/// Extracts backup contents to a NEW staging directory, without changing OS settings.
#[tauri::command]
pub async fn extract_archive(
    archive_path: String,
    destination: String,
) -> Result<ExtractResult, String> {
    tauri::async_runtime::spawn_blocking(move || extract_archive_inner(&archive_path, &destination))
        .await.map_err(|e| e.to_string())?
}
