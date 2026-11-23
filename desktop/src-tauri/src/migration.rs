//! DevMove's first Windows-native inventory and local-archive backend.
//! No driver installation, secrets extraction, or cloud operations occur here.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
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
    #[serde(default)]
    pub custom_folders: Vec<String>,
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
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn registry_apps() -> Vec<Application> {
    let roots = [
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];
    let mut found = BTreeMap::new();
    for (root, path) in roots {
        let Ok(key) = RegKey::predef(root).open_subkey(path) else {
            continue;
        };
        for entry in key.enum_keys().flatten() {
            let Ok(sub) = key.open_subkey(entry) else {
                continue;
            };
            let Ok(name) = sub.get_value::<String, _>("DisplayName") else {
                continue;
            };
            if name.trim().is_empty() {
                continue;
            }
            let system_component: u32 = sub.get_value("SystemComponent").unwrap_or(0);
            if system_component == 1 {
                continue;
            }
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
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("devmove-winget-{stamp}.json"));
    let output = Command::new("winget.exe")
        .arg("export")
        .arg("-o")
        .arg(&path)
        .args(["--accept-source-agreements", "--disable-interactivity"])
        .output()
        .map_err(|e| format!("WinGet is unavailable: {e}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(&path);
        return Err(format!(
            "WinGet export failed (exit {:?})",
            output.status.code()
        ));
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
                    let source_name = source
                        .pointer("/Details/Name")
                        .and_then(Value::as_str)
                        .unwrap_or("WinGet")
                        .to_owned();
                    let Some(packages) = source.get("Packages").and_then(Value::as_array) else {
                        continue;
                    };
                    for package in packages {
                        let Some(id) = package.get("PackageIdentifier").and_then(Value::as_str)
                        else {
                            continue;
                        };
                        let tail = normalize(id.rsplit('.').next().unwrap_or(""));
                        let matching = if tail.len() >= 5 {
                            apps.iter_mut().find(|a| normalize(&a.name).contains(&tail))
                        } else {
                            None
                        };
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
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    AppScan { apps, warnings }
}

#[tauri::command]
pub async fn scan_apps() -> Result<AppScan, String> {
    tauri::async_runtime::spawn_blocking(scan_apps_inner)
        .await
        .map_err(|e| e.to_string())
}

fn ps_output(script: &str) -> Result<Value, String> {
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| format!("PowerShell unavailable: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Device inventory failed (exit {:?})",
            output.status.code()
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("Driver JSON parsing failed: {e}"))
}

fn text_field(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn scan_drivers_inner() -> Result<DriverScan, String> {
    let command = "[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $ErrorActionPreference='Stop'; $ds=@(Get-CimInstance Win32_PnPSignedDriver | Select-Object DeviceName,Manufacturer,DriverProviderName,DriverVersion,DeviceID,InfName); $bad=@(Get-CimInstance Win32_PnPEntity | Where-Object { $null -ne $_.ConfigManagerErrorCode -and $_.ConfigManagerErrorCode -ne 0 } | Select-Object DeviceID,Name,Manufacturer,ConfigManagerErrorCode); @{devices=$ds;issues=$bad} | ConvertTo-Json -Depth 5 -Compress";
    let root = ps_output(command)?;
    let mut problems = BTreeMap::new();
    if let Some(values) = root.get("issues").and_then(Value::as_array) {
        for v in values {
            if let Some(id) = v.get("DeviceID").and_then(Value::as_str) {
                let code = v
                    .get("ConfigManagerErrorCode")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                problems.insert(
                    id.to_string(),
                    (text_field(v, "Name"), text_field(v, "Manufacturer"), code),
                );
            }
        }
    }
    let mut devices = Vec::new();
    let mut known_device_ids = HashSet::new();
    if let Some(values) = root.get("devices").and_then(Value::as_array) {
        for item in values {
            let id = text_field(item, "DeviceID");
            known_device_ids.insert(id.clone());
            let name = text_field(item, "DeviceName");
            if name.trim().is_empty() {
                continue;
            }
            devices.push(Driver {
                name,
                manufacturer: text_field(item, "Manufacturer"),
                provider: text_field(item, "DriverProviderName"),
                version: text_field(item, "DriverVersion"),
                inf: text_field(item, "InfName"),
                problem_code: problems.get(&id).map(|p| p.2).unwrap_or(0),
                device_id: id,
            });
        }
    }
    // Devices without a signed driver can still be reported as missing by Windows.
    for (id, (name, manufacturer, problem_code)) in &problems {
        if known_device_ids.contains(id) {
            continue;
        }
        devices.push(Driver {
            name: if name.trim().is_empty() {
                "Unknown device".into()
            } else {
                name.clone()
            },
            manufacturer: manufacturer.clone(),
            provider: "Driver not detected".into(),
            version: String::new(),
            device_id: id.clone(),
            inf: String::new(),
            problem_code: *problem_code,
        });
    }
    devices.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(DriverScan {
        issues: problems.len(),
        devices,
    })
}

#[tauri::command]
pub async fn scan_drivers() -> Result<DriverScan, String> {
    tauri::async_runtime::spawn_blocking(scan_drivers_inner)
        .await
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
        warnings.push(format!(
            "Skipped locked or inaccessible file: {archive_path}"
        ));
        return Ok(());
    };
    zip.start_file(
        &archive_path,
        SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
    )
    .map_err(|e| e.to_string())?;
    let mut buf = [0u8; 65536];
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    loop {
        let count = source_file.read(&mut buf).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buf[..count]);
        zip.write_all(&buf[..count]).map_err(|e| e.to_string())?;
        bytes += count as u64;
    }
    files.push(ArchivedFile {
        path: archive_path,
        bytes,
        sha256: format!("{:x}", hash.finalize()),
    });
    Ok(())
}

fn add_tree(
    zip: &mut ZipWriter<File>,
    source: &Path,
    prefix: &str,
    files: &mut Vec<ArchivedFile>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if !source.exists() {
        warnings.push(format!("{prefix} not found"));
        return Ok(());
    }
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|e| e.to_string())?;
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
    zip.start_file(
        archive_path,
        SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
    )
    .map_err(|e| e.to_string())?;
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
    if !destination.is_dir() {
        return Err("Choose an existing backup destination folder".to_owned());
    }
    if request.modules.is_empty() {
        return Err("Select at least one backup category".to_owned());
    }
    let allowed: HashSet<&str> = ["apps", "vscode", "chrome", "git", "node", "projects"]
        .into_iter()
        .collect();
    if request
        .modules
        .iter()
        .any(|item| !allowed.contains(item.as_str()))
    {
        return Err(
            "One or more backup categories are not supported by the Rust preview".to_owned(),
        );
    }
    let project_folders = validate_project_folders(&request, &destination)?;
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
                    if src.is_file() {
                        add_file(
                            &mut zip,
                            &src,
                            format!("VSCode/User/{name}"),
                            &mut files,
                            &mut warnings,
                        )?;
                    }
                }
                for name in ["snippets", "profiles"] {
                    add_tree(
                        &mut zip,
                        &dir.join(name),
                        &format!("VSCode/User/{name}"),
                        &mut files,
                        &mut warnings,
                    )?;
                }
            }
        }
        if modules.contains("chrome") {
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                let base = PathBuf::from(local)
                    .join("Google")
                    .join("Chrome")
                    .join("User Data");
                if let Ok(profiles) = fs::read_dir(&base) {
                    for entry in profiles.flatten() {
                        let profile = entry.file_name().to_string_lossy().to_string();
                        if profile != "Default" && !profile.starts_with("Profile ") {
                            continue;
                        }
                        for name in ["Bookmarks", "Bookmarks.bak", "History", "Favicons"] {
                            let src = entry.path().join(name);
                            if src.is_file() {
                                add_file(
                                    &mut zip,
                                    &src,
                                    format!("Chrome/{profile}/{name}"),
                                    &mut files,
                                    &mut warnings,
                                )?;
                            }
                        }
                    }
                } else {
                    warnings.push("Chrome profile folder not found".into());
                }
                warnings.push("Chrome passwords, passkeys, cookies, and authenticated sessions are intentionally not migrated. Enable Chrome Sync.".into());
            }
        }
        if modules.contains("git") {
            for name in [".gitconfig", ".gitignore_global"] {
                let src = home.join(name);
                if src.is_file() {
                    add_file(
                        &mut zip,
                        &src,
                        format!("Git/{name}"),
                        &mut files,
                        &mut warnings,
                    )?;
                }
            }
        }
        if modules.contains("apps") {
            let selected: HashSet<&str> = request.app_ids.iter().map(String::as_str).collect();
            match winget_export() {
                Ok(mut source) => {
                    if let Some(sources) = source.get_mut("Sources").and_then(Value::as_array_mut) {
                        for item in sources.iter_mut() {
                            if let Some(packages) =
                                item.get_mut("Packages").and_then(Value::as_array_mut)
                            {
                                packages.retain(|p| {
                                    p.get("PackageIdentifier")
                                        .and_then(Value::as_str)
                                        .is_some_and(|id| selected.contains(id))
                                });
                            }
                        }
                        sources.retain(|item| {
                            item.get("Packages")
                                .and_then(Value::as_array)
                                .is_some_and(|packages| !packages.is_empty())
                        });
                    }
                    write_json(&mut zip, "Applications/winget.json", &source, &mut files)?;
                }
                Err(err) => warnings.push(format!(
                    "Selected WinGet package inventory was not exported: {err}"
                )),
            }
            write_json(
                &mut zip,
                "Applications/selected.json",
                &json!(request.app_ids),
                &mut files,
            )?;
        }
        if modules.contains("node") {
            let mut versions = BTreeMap::new();
            for (label, program, args) in [
                ("Node", "node.exe", vec!["--version"]),
                ("npm", "npm.cmd", vec!["--version"]),
                ("PHP", "php.exe", vec!["--version"]),
                ("Git", "git.exe", vec!["--version"]),
            ] {
                if let Ok(output) = Command::new(program).args(args).output() {
                    let out = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                    versions.insert(label, out);
                }
            }
            write_json(
                &mut zip,
                "Development/versions.json",
                &json!(versions),
                &mut files,
            )?;
        }
        if modules.contains("projects") {
            for (index, source) in project_folders.iter().enumerate() {
                add_project_tree(&mut zip, source, index, &mut files, &mut warnings)?;
            }
        }
        if files.is_empty() {
            warnings.push("No source files were found for the selected categories".into());
        }
        let manifest = ArchiveManifest {
            schema_version: 1,
            created_at: Utc::now().to_rfc3339(),
            modules: request.modules.clone(),
            selected_apps: request.app_ids.clone(),
            files,
            warnings: warnings.clone(),
        };
        zip.start_file(
            "manifest.json",
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .map_err(|e| e.to_string())?;
        zip.write_all(&serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        zip.finish().map_err(|e| e.to_string())?;
        Ok((manifest.files.len(), warnings))
    };
    let (count, warnings) = match work() {
        Ok(out) => out,
        Err(err) => {
            let _ = fs::remove_file(&part_path);
            return Err(err);
        }
    };
    let verification = verify_backup_inner(&part_path.to_string_lossy())?;
    if !verification.verified {
        let _ = fs::remove_file(&part_path);
        return Err(format!(
            "Archive did not verify: {}",
            verification.errors.join("; ")
        ));
    }
    fs::rename(&part_path, &archive_path).map_err(|e| e.to_string())?;
    let bytes = fs::metadata(&archive_path)
        .map_err(|e| e.to_string())?
        .len();
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
    let manifest_value = read_archive_json(&mut zip, "manifest.json", 4 * 1024 * 1024)?;
    let manifest: ArchiveManifest =
        serde_json::from_value(manifest_value).map_err(|e| e.to_string())?;
    if manifest.schema_version != 1 {
        return Err("Unsupported archive format version".to_owned());
    }
    let mut errors = Vec::new();
    for record in &manifest.files {
        if record.path.contains("..") || record.path.starts_with('/') || record.path.contains('\\')
        {
            errors.push(format!("Unsafe file path: {}", record.path));
            continue;
        }
        match zip.by_name(&record.path) {
            Ok(mut entry) => {
                let mut hasher = Sha256::new();
                let mut buf = [0u8; 65536];
                let mut total = 0u64;
                loop {
                    match entry.read(&mut buf) {
                        Ok(0) => break,
                        Ok(count) => {
                            hasher.update(&buf[..count]);
                            total += count as u64;
                        }
                        Err(err) => {
                            errors.push(format!("{}: {err}", record.path));
                            break;
                        }
                    }
                }
                if format!("{:x}", hasher.finalize()) != record.sha256 || total != record.bytes {
                    errors.push(format!("Checksum mismatch: {}", record.path));
                }
            }
            Err(err) => errors.push(format!("Missing {}: {err}", record.path)),
        }
    }
    Ok(Verification {
        verified: errors.is_empty(),
        files_checked: manifest.files.len(),
        errors,
    })
}

#[tauri::command]
pub async fn create_backup(request: BackupRequest) -> Result<BackupResult, String> {
    tauri::async_runtime::spawn_blocking(move || create_backup_inner(request))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn verify_backup(path: String) -> Result<Verification, String> {
    tauri::async_runtime::spawn_blocking(move || verify_backup_inner(&path))
        .await
        .map_err(|e| e.to_string())?
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
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("devmove-test-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sample.txt");
        fs::write(&source, b"DevMove archive integrity test").unwrap();
        let archive = dir.join("sample.zip");
        {
            let mut zip = ZipWriter::new(File::create(&archive).unwrap());
            let mut files = Vec::new();
            let mut warnings = Vec::new();
            add_file(
                &mut zip,
                &source,
                "sample.txt".into(),
                &mut files,
                &mut warnings,
            )
            .unwrap();
            assert!(warnings.is_empty());
            let manifest = ArchiveManifest {
                schema_version: 1,
                created_at: "2026-11-23T00:00:00Z".into(),
                modules: vec!["test".into()],
                selected_apps: Vec::new(),
                files,
                warnings,
            };
            zip.start_file("manifest.json", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(&serde_json::to_vec(&manifest).unwrap())
                .unwrap();
            zip.finish().unwrap();
        }
        let check = verify_backup_inner(archive.to_str().unwrap()).unwrap();
        assert!(check.verified, "{:?}", check.errors);
        assert_eq!(check.files_checked, 1);
        let restored =
            extract_archive_inner(archive.to_str().unwrap(), dir.to_str().unwrap()).unwrap();
        assert_eq!(restored.files_extracted, 1);
        assert_eq!(
            fs::read(PathBuf::from(restored.output_directory).join("sample.txt")).unwrap(),
            b"DevMove archive integrity test"
        );
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

fn extract_archive_inner(
    archive_path: &str,
    base_directory: &str,
) -> Result<ExtractResult, String> {
    let verification = verify_backup_inner(archive_path)?;
    if !verification.verified {
        return Err(format!(
            "Archive checksum verification failed: {}",
            verification.errors.join("; ")
        ));
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
            if record.path.starts_with('/')
                || record.path.contains('\\')
                || record
                    .path
                    .split('/')
                    .any(|segment| segment == ".." || segment == ".")
            {
                return Err(format!("Refusing unsafe archive path: {}", record.path));
            }

            let mut entry = zip.by_name(&record.path).map_err(|e| e.to_string())?;
            let safe_path = entry
                .enclosed_name()
                .ok_or_else(|| format!("Refusing unsafe archive entry: {}", record.path))?;
            let target = output.join(safe_path);
            if !target.starts_with(&output) {
                return Err("Archive path escaped destination".into());
            }

            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            // create_new protects any existing file from accidental overwrites.
            let mut file = File::options()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| format!("Cannot create {}: {e}", record.path))?;
            let count = std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
            if count != record.bytes {
                return Err(format!("Incomplete extraction: {}", record.path));
            }
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
        Err(err) => Err(format!(
            "{err}. Partial extraction, if any: {}",
            output.display()
        )),
    }
}

/// Extracts backup contents to a NEW staging directory, without changing OS settings.
#[tauri::command]
pub async fn extract_archive(
    archive_path: String,
    destination: String,
) -> Result<ExtractResult, String> {
    tauri::async_runtime::spawn_blocking(move || extract_archive_inner(&archive_path, &destination))
        .await
        .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverExportResult {
    pub output_directory: String,
    pub packages_exported: usize,
}

fn export_drivers_inner(destination: &str) -> Result<DriverExportResult, String> {
    let parent = PathBuf::from(destination);
    if !parent.is_dir() {
        return Err("Choose an existing destination directory for driver packages".into());
    }
    let stamp = Utc::now().format("%Y%m%d_%H%M%S_%3f").to_string();
    let output = parent.join(format!("DevMove_Drivers_{stamp}"));
    fs::create_dir(&output).map_err(|e| format!("Cannot create driver backup directory: {e}"))?;

    // PnPUtil is a Windows-included driver store tool; export does not install
    // or replace any device driver. Only third-party driver packages are exported.
    let result = Command::new("pnputil.exe")
        .arg("/export-driver")
        .arg("*")
        .arg(&output)
        .output()
        .map_err(|e| format!("Windows PnPUtil is unavailable: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "Windows driver export failed (exit {:?}). Administrator permission may be required. Partial output: {}",
            result.status.code(),
            output.display()
        ));
    }
    let packages = fs::read_dir(&output)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .count();
    if packages == 0 {
        return Err("Windows reported no exported third-party driver packages".into());
    }
    Ok(DriverExportResult {
        output_directory: output.to_string_lossy().to_string(),
        packages_exported: packages,
    })
}

#[tauri::command]
pub async fn export_drivers(destination: String) -> Result<DriverExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || export_drivers_inner(&destination))
        .await
        .map_err(|e| e.to_string())?
}

const MAX_PROJECT_FOLDERS: usize = 8;
const MAX_PROJECT_FILES: usize = 50_000;
const MAX_PROJECT_BYTES: u64 = 10 * 1024 * 1024 * 1024;
const MAX_SINGLE_PROJECT_FILE: u64 = 2 * 1024 * 1024 * 1024;

fn validate_project_folders(
    request: &BackupRequest,
    destination: &Path,
) -> Result<Vec<PathBuf>, String> {
    if !request.modules.iter().any(|module| module == "projects") {
        return Ok(Vec::new());
    }
    if request.custom_folders.is_empty() {
        return Err("Choose at least one project folder, or deselect Projects".into());
    }
    if request.custom_folders.len() > MAX_PROJECT_FOLDERS {
        return Err(format!(
            "Choose no more than {MAX_PROJECT_FOLDERS} project folders per backup"
        ));
    }
    let dest = fs::canonicalize(destination).map_err(|e| format!("Invalid destination: {e}"))?;
    let dest_casefold = PathBuf::from(dest.to_string_lossy().to_lowercase());
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut normalized: Vec<PathBuf> = Vec::new();

    for raw in &request.custom_folders {
        let path = PathBuf::from(raw);
        if fs::symlink_metadata(&path)
            .map_err(|e| format!("Cannot access project folder: {e}"))?
            .file_type()
            .is_symlink()
        {
            return Err("Selecting a symlink as a project root is not supported".into());
        }
        let source =
            fs::canonicalize(&path).map_err(|e| format!("Cannot resolve project folder: {e}"))?;
        if !source.is_dir() || source.parent().is_none() {
            return Err(format!(
                "Select a regular project directory, not a drive root: {}",
                path.display()
            ));
        }
        let folded = PathBuf::from(source.to_string_lossy().to_lowercase());
        if dest_casefold.starts_with(&folded) || folded.starts_with(&dest_casefold) {
            return Err(format!(
                "Backup destination and project folder must not contain each other: {}",
                source.display()
            ));
        }
        if normalized
            .iter()
            .any(|known| known.starts_with(&folded) || folded.starts_with(known))
        {
            return Err(format!(
                "Selected project folders must not overlap: {}",
                source.display()
            ));
        }
        normalized.push(folded);
        roots.push(source);
    }
    Ok(roots)
}

fn project_excluded(path: &Path, is_dir: bool) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if is_dir {
        return matches!(
            name.as_str(),
            ".git"
                | "node_modules"
                | "vendor"
                | "dist"
                | "build"
                | ".next"
                | "target"
                | ".cache"
                | "__pycache__"
                | ".venv"
                | ".idea"
                | ".vs"
                | ".terraform"
                | ".ssh"
                | ".aws"
        );
    }
    let sensitive = name == ".env"
        || name.starts_with(".env.")
        || name.starts_with(".secrets")
        || name == ".npmrc"
        || name == ".pypirc"
        || name == "credentials.json"
        || name == "service-account.json"
        || name == "service_account.json"
        || name == "id_rsa"
        || name == "id_ed25519";
    sensitive
        || [".pem", ".pfx", ".key", ".p12", ".p8", ".kdbx"]
            .iter()
            .any(|ext| name.ends_with(ext))
}

fn add_project_tree(
    zip: &mut ZipWriter<File>,
    source: &Path,
    index: usize,
    files: &mut Vec<ArchivedFile>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let label = source.file_name().unwrap_or_default().to_string_lossy();
    let safe_label: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(50)
        .collect();
    let prefix = format!(
        "Projects/{:02}_{}",
        index + 1,
        if safe_label.is_empty() {
            "project"
        } else {
            &safe_label
        }
    );

    let omitted = std::cell::Cell::new(0usize);
    let mut count = 0usize;
    let mut total_bytes = 0u64;
    let iter = WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            let forbidden = entry.path() != source
                && (entry.file_type().is_symlink()
                    || project_excluded(entry.path(), entry.file_type().is_dir()));
            if forbidden {
                omitted.set(omitted.get() + 1);
            }
            !forbidden
        });

    for entry in iter {
        let entry = entry.map_err(|e| format!("Error reading project tree: {e}"))?;
        if !entry.file_type().is_file() {
            continue;
        }
        if project_excluded(entry.path(), false) {
            omitted.set(omitted.get() + 1);
            continue;
        }
        let len = entry.metadata().map_err(|e| e.to_string())?.len();
        if len > MAX_SINGLE_PROJECT_FILE {
            omitted.set(omitted.get() + 1);
            warnings.push(format!(
                "Skipped oversized project file: {}",
                entry.path().display()
            ));
            continue;
        }
        count += 1;
        if count > MAX_PROJECT_FILES || total_bytes.saturating_add(len) > MAX_PROJECT_BYTES {
            return Err(format!(
                "Project backup safety limit exceeded ({} files or {} GB). Select a smaller project set.",
                MAX_PROJECT_FILES,
                MAX_PROJECT_BYTES / (1024 * 1024 * 1024)
            ));
        }
        total_bytes += len;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|e| e.to_string())?;
        let archive_path = format!("{prefix}/{}", relative.to_string_lossy().replace('\\', "/"));
        add_file(zip, entry.path(), archive_path, files, warnings)?;
    }
    if omitted.get() > 0 {
        warnings.push(format!(
            "{prefix}: skipped {} generated/dependency directories, credential-like files or links",
            omitted.get()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod project_tests {
    use super::*;

    #[test]
    fn project_filter_omits_credential_files_and_generated_dependencies() {
        assert!(project_excluded(Path::new(".env.production"), false));
        assert!(project_excluded(Path::new("private.pem"), false));
        assert!(project_excluded(Path::new("node_modules"), true));
        assert!(!project_excluded(Path::new("package.json"), false));
        assert!(!project_excluded(Path::new("src"), true));
    }

    #[test]
    fn project_archiving_skips_sensitive_files_and_can_verify() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let work = std::env::temp_dir().join(format!("devmove-project-test-{stamp}"));
        let source = work.join("sample_project");
        let dest = work.join("backup");
        fs::create_dir_all(source.join("src")).unwrap();
        fs::create_dir_all(source.join("node_modules")).unwrap();
        fs::create_dir_all(&dest).unwrap();
        fs::write(source.join("src").join("index.ts"), b"export const ok=true").unwrap();
        fs::write(source.join(".env"), b"SECRET=should-not-be-copied").unwrap();
        fs::write(source.join("node_modules").join("index.js"), b"generated").unwrap();
        let request = BackupRequest {
            destination: dest.to_string_lossy().to_string(),
            modules: vec!["projects".into()],
            app_ids: Vec::new(),
            custom_folders: vec![source.to_string_lossy().to_string()],
        };
        let folders = validate_project_folders(&request, &dest).unwrap();
        let archive = dest.join("project.zip");
        let mut zip = ZipWriter::new(File::create(&archive).unwrap());
        let mut files = Vec::new();
        let mut warnings = Vec::new();
        add_project_tree(&mut zip, &folders[0], 0, &mut files, &mut warnings).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "Projects/01_sample_project/src/index.ts");
        assert!(!warnings.is_empty());
        let manifest = ArchiveManifest {
            schema_version: 1,
            created_at: Utc::now().to_rfc3339(),
            modules: request.modules,
            selected_apps: Vec::new(),
            files,
            warnings,
        };
        zip.start_file("manifest.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        zip.finish().unwrap();
        let checked = verify_backup_inner(&archive.to_string_lossy()).unwrap();
        assert!(checked.verified, "{:?}", checked.errors);
        fs::remove_dir_all(work).unwrap();
    }

    #[test]
    fn project_destination_inside_source_is_rejected() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("devmove-overlap-test-{stamp}"));
        let source = base.join("source");
        let destination = source.join("backups");
        fs::create_dir_all(&destination).unwrap();
        let request = BackupRequest {
            destination: destination.to_string_lossy().to_string(),
            modules: vec!["projects".into()],
            app_ids: Vec::new(),
            custom_folders: vec![source.to_string_lossy().to_string()],
        };
        assert!(validate_project_folders(&request, &destination).is_err());
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod project_backup_integration_tests {
    use super::*;

    #[test]
    fn selected_project_backup_verify_and_extract_round_trip() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("devmove-project-flow-{stamp}"));
        let project = base.join("important-project");
        let dest = base.join("archives");
        fs::create_dir_all(project.join("src")).unwrap();
        fs::create_dir_all(&dest).unwrap();
        fs::write(
            project.join("src").join("main.ts"),
            b"console.log('backup roundtrip')",
        )
        .unwrap();
        fs::write(project.join(".env.local"), b"DO_NOT_EXPORT=true").unwrap();
        let request = BackupRequest {
            destination: dest.to_string_lossy().to_string(),
            modules: vec!["projects".into()],
            app_ids: Vec::new(),
            custom_folders: vec![project.to_string_lossy().to_string()],
        };
        let backup = create_backup_inner(request).unwrap();
        assert_eq!(backup.file_count, 1);
        assert!(!backup.warnings.is_empty());
        let validation = verify_backup_inner(&backup.archive_path).unwrap();
        assert!(validation.verified);
        let extracted =
            extract_archive_inner(&backup.archive_path, &base.to_string_lossy()).unwrap();
        let folder = PathBuf::from(extracted.output_directory)
            .join("Projects")
            .join("01_important-project");
        assert_eq!(
            fs::read(folder.join("src").join("main.ts")).unwrap(),
            b"console.log('backup roundtrip')"
        );
        assert!(!folder.join(".env.local").exists());
        fs::remove_dir_all(base).unwrap();
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub created_at: String,
    pub modules: Vec<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub winget_package_ids: Vec<String>,
    pub manual_installations: Vec<String>,
    pub warnings: Vec<String>,
}

/// Read a limited amount from an archive member before JSON parsing.
/// This prevents unexpectedly large metadata from exhausting application memory.
fn read_archive_json(zip: &mut ZipArchive<File>, name: &str, limit: u64) -> Result<Value, String> {
    let member = zip
        .by_name(name)
        .map_err(|e| format!("{name} not found: {e}"))?;
    if member.size() > limit {
        return Err(format!("{name} exceeds the supported metadata size"));
    }
    let mut bytes = Vec::new();
    member
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("{name} exceeds the supported metadata size"));
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid {name}: {e}"))
}

fn restore_preview_inner(archive_path: &str) -> Result<RestorePreview, String> {
    let valid = verify_backup_inner(archive_path)?;
    if !valid.verified {
        return Err(format!(
            "Backup verification failed: {}",
            valid.errors.join("; ")
        ));
    }
    let handle = File::open(archive_path).map_err(|e| e.to_string())?;
    let mut zip = ZipArchive::new(handle).map_err(|e| e.to_string())?;
    let manifest_value = read_archive_json(&mut zip, "manifest.json", 4 * 1024 * 1024)?;
    let manifest: ArchiveManifest =
        serde_json::from_value(manifest_value).map_err(|e| e.to_string())?;

    let mut listed_packages: HashSet<String> = HashSet::new();
    let mut warnings = manifest.warnings.clone();

    if manifest.modules.iter().any(|module| module == "apps") {
        match read_archive_json(&mut zip, "Applications/winget.json", 16 * 1024 * 1024) {
            Ok(winget) => {
                if let Some(sources) = winget.get("Sources").and_then(Value::as_array) {
                    for source in sources {
                        if let Some(packages) = source.get("Packages").and_then(Value::as_array) {
                            for package in packages {
                                if let Some(id) =
                                    package.get("PackageIdentifier").and_then(Value::as_str)
                                {
                                    listed_packages.insert(id.to_lowercase());
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => warnings.push(format!("WinGet metadata unavailable: {err}")),
        }
    }

    let mut winget_packages = Vec::new();
    let mut manual_apps = Vec::new();
    for id in &manifest.selected_apps {
        if listed_packages.contains(&id.to_lowercase()) {
            winget_packages.push(id.clone());
        } else if let Some(manual) = id.strip_prefix("manual:") {
            manual_apps.push(manual.to_owned());
        } else {
            manual_apps.push(id.to_owned());
        }
    }
    winget_packages.sort_by_key(|v| v.to_lowercase());
    manual_apps.sort_by_key(|v| v.to_lowercase());
    let total_bytes = manifest
        .files
        .iter()
        .fold(0u64, |sum, file| sum.saturating_add(file.bytes));

    Ok(RestorePreview {
        created_at: manifest.created_at,
        modules: manifest.modules,
        file_count: manifest.files.len(),
        total_bytes,
        winget_package_ids: winget_packages,
        manual_installations: manual_apps,
        warnings,
    })
}

#[tauri::command]
pub async fn inspect_backup(archive_path: String) -> Result<RestorePreview, String> {
    tauri::async_runtime::spawn_blocking(move || restore_preview_inner(&archive_path))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod restore_preview_tests {
    use super::*;

    #[test]
    fn restore_preview_distinguishes_winget_and_manual_apps() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let folder = std::env::temp_dir().join(format!("devmove-preview-test-{stamp}"));
        fs::create_dir_all(&folder).unwrap();
        let archive = folder.join("backup.zip");
        let mut zip = ZipWriter::new(File::create(&archive).unwrap());
        let mut files = Vec::new();
        write_json(
            &mut zip,
            "Applications/winget.json",
            &json!({
                "Sources": [{ "Packages": [{"PackageIdentifier":"Example.Editor"}] }]
            }),
            &mut files,
        )
        .unwrap();
        let manifest = ArchiveManifest {
            schema_version: 1,
            created_at: "2026-11-23T10:00:00Z".into(),
            modules: vec!["apps".into()],
            selected_apps: vec![
                "Example.Editor".into(),
                "manual:Legacy App".into(),
                "Missing.Package".into(),
            ],
            files,
            warnings: vec!["Preview-only fixture".into()],
        };
        zip.start_file("manifest.json", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        zip.finish().unwrap();

        let plan = restore_preview_inner(&archive.to_string_lossy()).unwrap();
        assert_eq!(plan.file_count, 1);
        assert_eq!(plan.winget_package_ids, vec!["Example.Editor"]);
        assert_eq!(
            plan.manual_installations,
            vec!["Legacy App", "Missing.Package"]
        );
        assert_eq!(plan.warnings.len(), 1);
        fs::remove_dir_all(folder).unwrap();
    }
}
