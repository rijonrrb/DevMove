param(
    [Parameter(Mandatory=$true)][string]$Config,
    [Parameter(Mandatory=$true)][string]$StatusPath,
    [Parameter(Mandatory=$true)][string]$LogPath
)
$ErrorActionPreference='Stop'
$cfg=Get-Content -LiteralPath $Config -Raw | ConvertFrom-Json
$mode=[string]$cfg.mode
$tasks=@($cfg.tasks)
$completed=0
$failed=0
$warnings=0
$output=''
$script:notes=@()
function Log([string]$message) {
    $line=('['+(Get-Date -Format 'HH:mm:ss')+'] '+$message)
    Add-Content -LiteralPath $LogPath -Encoding UTF8 -Value $line
}
function Status([string]$state,[string]$step) {
    $s=[ordered]@{state=$state;step=$step;done=$completed;total=$tasks.Count;failed=$failed;warnings=$warnings;output=$output}
    $tmp=$StatusPath+'.new'
    $s | ConvertTo-Json -Compress | Set-Content -LiteralPath $tmp -Encoding UTF8
    Move-Item -LiteralPath $tmp -Destination $StatusPath -Force
}
function Warn([string]$msg) { $script:warnings++; Log ('WARNING: '+$msg) }
function CopyOne([string]$source,[string]$target) {
    if(-not (Test-Path -LiteralPath $source)) { return }
    try {
        $parent=Split-Path -Parent $target
        if(-not (Test-Path -LiteralPath $parent)){ New-Item -ItemType Directory -Path $parent -Force | Out-Null }
        if((Get-Item -LiteralPath $source).PSIsContainer -and (Test-Path -LiteralPath $target -PathType Container)) {
            foreach($child in (Get-ChildItem -LiteralPath $source -Force)) {
                CopyOne $child.FullName (Join-Path $target $child.Name)
            }
        } else {
            Copy-Item -LiteralPath $source -Destination $target -Force -Recurse -ErrorAction Stop
        }
    } catch { Warn ('Could not copy '+$source+': '+$_.Exception.Message) }
}
function SaveText([string]$path,[string[]]$lines) {
    $parent=Split-Path -Parent $path
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    [IO.File]::WriteAllLines($path,$lines,[Text.UTF8Encoding]::new($false))
}
function TryCommand([string]$command,[string[]]$arguments,[string]$saveTo) {
    $found=Get-Command $command -ErrorAction SilentlyContinue
    if(-not $found) { Warn ($command+' is not installed or is not on PATH'); return }
    try {
        $result=& $command @arguments 2>&1 | Out-String
        SaveText $saveTo @($result)
        if($LASTEXITCODE -ne 0) { Warn ($command+' returned exit code '+$LASTEXITCODE) }
    } catch { Warn ($command+': '+$_.Exception.Message) }
}
function BackupChrome {
    $home=Join-Path $env:LOCALAPPDATA 'Google\Chrome\User Data'
    if(-not (Test-Path $home)){ Warn 'Chrome profile folder not found'; return }
    if(Get-Process chrome -ErrorAction SilentlyContinue) { Warn 'Chrome is open. History may be locked. Close Chrome and run another backup for a full history copy.' }
    $profiles=Get-ChildItem -LiteralPath $home -Directory | Where-Object { $_.Name -eq 'Default' -or $_.Name -match '^Profile [0-9]+$' }
    foreach($p in $profiles) {
        foreach($file in @('Bookmarks','Bookmarks.bak','History','Favicons')) {
            CopyOne (Join-Path $p.FullName $file) (Join-Path $output ('Chrome\'+$p.Name+'\'+$file))
        }
    }
    Log 'Chrome bookmarks/history archived (not cookies, passwords, passkeys, or live sessions).'
}
function BackupVscode {
    $src=Join-Path $env:APPDATA 'Code\User'
    foreach($name in @('settings.json','keybindings.json','snippets','profiles','tasks.json')) {
        CopyOne (Join-Path $src $name) (Join-Path $output ('VSCode\User\'+$name))
    }
    $code=Get-Command code.cmd -ErrorAction SilentlyContinue
    if($code) {
        TryCommand 'code.cmd' @('--list-extensions') (Join-Path $output 'VSCode\extensions.txt')
        TryCommand 'code.cmd' @('--list-extensions','--show-versions') (Join-Path $output 'VSCode\extensions-versions.txt')
    } else { Warn 'VS Code CLI not found: extension list could not be exported' }
    Log 'VS Code settings saved. Enable Settings Sync as an additional safeguard.'
}
function BackupXamppConfig {
    $root='C:\xampp'
    foreach($file in @('php\php.ini','apache\conf\httpd.conf','apache\conf\extra\httpd-vhosts.conf','mysql\bin\my.ini','apache\conf\extra\httpd-ssl.conf')) {
        CopyOne (Join-Path $root $file) (Join-Path $output ('XAMPP\Config\'+$file))
    }
}
function BackupDB {
    $dump='C:\xampp\mysql\bin\mysqldump.exe'
    if(-not(Test-Path $dump)){ $dump='C:\xampp\mysql\bin\mariadb-dump.exe' }
    if(-not(Test-Path $dump)) { throw 'No mysqldump.exe or mariadb-dump.exe found under C:\xampp\mysql\bin' }
    $user=[string]$cfg.mysqlUser
    if([string]::IsNullOrWhiteSpace($user)){ $user='root' }
    if($user -notmatch '^[A-Za-z0-9_\-]+$') { throw 'Database username contains unsupported characters' }
    $dir=Join-Path $output 'Databases'
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $sql=Join-Path $dir 'all-databases.sql'
    $psi=New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName=$dump
    $psi.Arguments=('--host=127.0.0.1 --port=3306 --user='+$user+' --all-databases --routines --events --triggers --single-transaction --default-character-set=utf8mb4')
    $psi.UseShellExecute=$false
    $psi.CreateNoWindow=$true
    $psi.RedirectStandardOutput=$true
    $psi.RedirectStandardError=$true
    $psi.EnvironmentVariables['MYSQL_PWD']=[string]$env:DEVMOVE_DB_PASSWORD
    $p=New-Object System.Diagnostics.Process
    $p.StartInfo=$psi
    try {
        if(-not $p.Start()) { throw 'Database export could not start' }
        $errTask=$p.StandardError.ReadToEndAsync()
        $stream=[IO.File]::Open($sql,[IO.FileMode]::Create,[IO.FileAccess]::Write,[IO.FileShare]::None)
        try { $p.StandardOutput.BaseStream.CopyTo($stream) } finally { $stream.Dispose() }
        $p.WaitForExit()
        $errorText=$errTask.Result
        if($p.ExitCode -ne 0) {
            Remove-Item -LiteralPath $sql -Force -ErrorAction SilentlyContinue
            throw ('Database export failed (exit '+$p.ExitCode+'): '+$errorText)
        }
        if((Get-Item -LiteralPath $sql).Length -eq 0) { throw 'Database dump output is empty' }
        Log 'XAMPP SQL dump created. Import only after checking database server version.'
        if($errorText) { Warn $errorText.Trim() }
    } finally { $p.Dispose(); Remove-Item Env:\DEVMOVE_DB_PASSWORD -ErrorAction SilentlyContinue }
}
function BackupProjectFolder([string]$source,[string]$dest) {
    if(-not (Test-Path -LiteralPath $source)) { Warn ('Project folder missing: '+$source); return }
    New-Item -ItemType Directory -Path $dest -Force | Out-Null
    $args=@($source,$dest,'/E','/XJ','/R:1','/W:1','/XD','node_modules','.next','.nuxt','vendor','dist','build','.cache')
    & robocopy.exe @args | Out-Null
    if($LASTEXITCODE -ge 8) { Warn ('Robocopy reported issues for '+$source+' (exit '+$LASTEXITCODE+')') }
    Log ('Copied '+$source+' without build caches/dependencies')
}
function BackupTask([string]$task) {
    switch($task) {
        'apps' {
            $selected=@($cfg.selectedAppIds | Where-Object { $_ -and $_ -notmatch '^MANUAL:' } | Select-Object -Unique)
            $manual=@($cfg.selectedManualApps | Where-Object { $_ } | Select-Object -Unique)
            if($selected.Count -eq 0 -and $manual.Count -eq 0) { throw 'Choose individual apps using the app picker before backup' }
            if($selected.Count -gt 0) {
                $all=Join-Path $output 'apps-scan.json'
                $report=& winget.exe export -o $all --accept-source-agreements --disable-interactivity 2>&1 | Out-String
                if(-not(Test-Path -LiteralPath $all)) { throw ('WinGet export failed: '+$report) }
                $json=Get-Content -LiteralPath $all -Raw | ConvertFrom-Json
                $matched=@()
                foreach($src in @($json.Sources)) {
                    $subset=@($src.Packages | Where-Object { $_.PackageIdentifier -in $selected })
                    $src.Packages=$subset
                    $matched+=@($subset | ForEach-Object { $_.PackageIdentifier })
                }
                $json.Sources=@($json.Sources | Where-Object { @($_.Packages).Count -gt 0 })
                $json | ConvertTo-Json -Depth 15 | Set-Content -LiteralPath (Join-Path $output 'apps.json') -Encoding UTF8
                foreach($app in $selected) { if($app -notin $matched) { Warn ('Not found in fresh WinGet export: '+$app) } }
                Remove-Item -LiteralPath $all -Force
            }
            if($manual.Count -gt 0) {
                SaveText (Join-Path $output 'apps-manual-reinstall.txt') $manual
                Log ('Saved '+$manual.Count+' apps requiring manual installation')
            }
            Log ('Saved '+$selected.Count+' selected WinGet IDs')
            break
        }
        'vscode' { BackupVscode; break }
        'chrome' { BackupChrome; break }
        'xampp' { BackupXamppConfig; break }
        'database' { BackupDB; break }
        'node' {
            TryCommand 'node.exe' @('--version') (Join-Path $output 'Development\node-version.txt')
            TryCommand 'npm.cmd' @('ls','-g','--depth=0','--json') (Join-Path $output 'Development\npm-global.json')
            TryCommand 'php.exe' @('-v') (Join-Path $output 'Development\php-version.txt')
            TryCommand 'composer.cmd' @('--version') (Join-Path $output 'Development\composer-version.txt')
            break
        }
        'git' {
            CopyOne (Join-Path $env:USERPROFILE '.gitconfig') (Join-Path $output 'Git\.gitconfig')
            CopyOne (Join-Path $env:USERPROFILE '.gitignore_global') (Join-Path $output 'Git\.gitignore_global')
            TryCommand 'git.exe' @('--version') (Join-Path $output 'Git\git-version.txt')
            break
        }
        'ssh' {
            CopyOne (Join-Path $env:USERPROFILE '.ssh') (Join-Path $output 'Sensitive\.ssh')
            Warn 'Private SSH keys may be included. Keep this backup on an encrypted drive; never upload it publicly.'
            break
        }
        'drivers' {
            $drv=Join-Path $output 'Drivers\Exported'
            New-Item -ItemType Directory -Force -Path $drv | Out-Null
            $result=& pnputil.exe /export-driver '*' $drv 2>&1 | Out-String
            Log ('Driver export: '+$result.Trim())
            if($LASTEXITCODE -ne 0) { Warn ('Driver export returned '+$LASTEXITCODE+'. Run elevated if required.') }
            if(@(Get-ChildItem -LiteralPath $drv -Force).Count -eq 0) { Warn 'No third-party drivers were exported' }
            break
        }
        'driverinventory' {
            $dir=Join-Path $output 'Drivers'
            New-Item -ItemType Directory -Force -Path $dir | Out-Null
            Get-CimInstance Win32_PnPSignedDriver | Select-Object DeviceName,Manufacturer,DriverProviderName,DriverVersion,DriverDate,InfName,DeviceID | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $dir 'inventory.json') -Encoding UTF8
            Get-CimInstance Win32_PnPEntity | Where-Object { $_.ConfigManagerErrorCode -ne 0 } | Select-Object Name,DeviceID,ConfigManagerErrorCode | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $dir 'problems.json') -Encoding UTF8
            break
        }
        'terminal' {
            foreach($rel in @('Microsoft.PowerShell_profile.ps1','Microsoft.PowerShellISE_profile.ps1')) {
                CopyOne (Join-Path (Join-Path $env:USERPROFILE 'Documents\WindowsPowerShell') $rel) (Join-Path $output ('Terminal\PowerShell\'+$rel))
            }
            $wp=Join-Path $env:LOCALAPPDATA 'Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json'
            CopyOne $wp (Join-Path $output 'Terminal\WindowsTerminal\settings.json')
            break
        }
        'documents' { BackupProjectFolder (Join-Path $env:USERPROFILE 'Documents') (Join-Path $output 'Personal\Documents'); break }
        'downloads' { BackupProjectFolder (Join-Path $env:USERPROFILE 'Downloads') (Join-Path $output 'Personal\Downloads'); break }
        'pictures' { BackupProjectFolder (Join-Path $env:USERPROFILE 'Pictures') (Join-Path $output 'Personal\Pictures'); break }
        'htdocs' { BackupProjectFolder 'C:\xampp\htdocs' (Join-Path $output 'Projects\xampp-htdocs'); break }
        'custom' {
            $src=[string]$cfg.extraFolder
            if([string]::IsNullOrWhiteSpace($src)){ throw 'No custom folder was selected' }
            if($output.StartsWith($src.TrimEnd('\')+'\',[StringComparison]::OrdinalIgnoreCase)){ throw 'Backup destination cannot be inside the source folder' }
            BackupProjectFolder $src (Join-Path $output 'Projects\custom')
            SaveText (Join-Path $output 'Projects\custom-source.txt') @($src)
            break
        }
        default { throw ('Unrecognized backup task: '+$task) }
    }
}
function RestoreFiles([string]$relative,[string]$destination) {
    $source=Join-Path $output $relative
    if(Test-Path -LiteralPath $source) { CopyOne $source $destination; Log ('Restored '+$relative) }
    else { Warn ('Not present: '+$relative) }
}
function RestoreTask([string]$task) {
    switch($task) {
        'apps' {
            $file=Join-Path $output 'apps.json'
            if(Test-Path $file) { & winget.exe import -i $file --accept-package-agreements --accept-source-agreements 2>&1 | Out-String | ForEach-Object {Log $_}; if($LASTEXITCODE -ne 0){Warn 'WinGet import did not fully succeed'} }
            else { Log 'No selected WinGet packages in this backup; check apps-manual-reinstall.txt for manual apps.' }
            break
        }
        'vscode' {
            foreach($name in @('settings.json','keybindings.json','snippets','profiles','tasks.json')) { RestoreFiles ('VSCode\User\'+$name) (Join-Path $env:APPDATA ('Code\User\'+$name)) }
            $list=Join-Path $output 'VSCode\extensions.txt'
            if(Test-Path $list) { foreach($ext in Get-Content -LiteralPath $list) { if($ext -match '^[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+$') { & code.cmd --install-extension $ext 2>&1 | Out-Null; if($LASTEXITCODE -ne 0){Warn ('VS Code extension could not install: '+$ext)} } } }
            break
        }
        'chrome' {
            if(Get-Process chrome -ErrorAction SilentlyContinue){ throw 'Close ALL Chrome windows, then try Restore again. Chrome must not be open.' }
            $src=Join-Path $output 'Chrome'
            if(Test-Path $src) {
                foreach($p in Get-ChildItem -LiteralPath $src -Directory) {
                    foreach($name in @('Bookmarks','Bookmarks.bak','History','Favicons')) {
                        RestoreFiles ('Chrome\'+$p.Name+'\'+$name) (Join-Path $env:LOCALAPPDATA ('Google\Chrome\User Data\'+$p.Name+'\'+$name))
                    }
                }
            }
            Log 'Chrome passwords/sessions are NOT restored from files; sign in to Chrome Sync.'
            break
        }
        'git' {
            RestoreFiles 'Git\.gitconfig' (Join-Path $env:USERPROFILE '.gitconfig')
            RestoreFiles 'Git\.gitignore_global' (Join-Path $env:USERPROFILE '.gitignore_global')
            break
        }
        'node' { Log 'Install Node/PHP/Composer and review Development/npm-global.json; package versions are for reference.'; break }
        'xampp' { Log 'XAMPP config in XAMPP\Config; review and merge manually after installing matching XAMPP version.'; break }
        'database' { Log 'Databases\all-databases.sql saved. Import manually into compatible MySQL/MariaDB; never blindly overwrite existing databases.'; break }
        'ssh' { Log 'Sensitive\.ssh contains key material. Restore manually with correct file permissions; do not publish this folder.'; break }
        'driverinventory' { Log 'Drivers\inventory.json and problems.json contain the driver inventory at backup time.'; break }
        'drivers' { Log 'Drivers\Exported contains third-party driver packages. Inspect and use Windows Device Manager or pnputil /add-driver only when needed.'; break }
        'terminal' {
            RestoreFiles 'Terminal\WindowsTerminal\settings.json' (Join-Path $env:LOCALAPPDATA 'Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json')
            RestoreFiles 'Terminal\PowerShell\Microsoft.PowerShell_profile.ps1' (Join-Path $env:USERPROFILE 'Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1')
            break
        }
        'documents' { Log 'Personal\Documents is available for manual restore to the new user profile.'; break }
        'downloads' { Log 'Personal\Downloads is available for manual restore to the new user profile.'; break }
        'pictures' { Log 'Personal\Pictures is available for manual restore to the new user profile.'; break }
        'htdocs' { Log 'Projects\xampp-htdocs is available. Copy into new XAMPP htdocs only after checking for conflicts.'; break }
        'custom' { Log 'Projects\custom is available for manual relocation; see Projects\custom-source.txt.'; break }
        default { throw ('Unrecognized restore task: '+$task) }
    }
}
try {
    if($mode -eq 'Backup') {
        $dest=[string]$cfg.backupPath
        if(-not (Test-Path -LiteralPath $dest -PathType Container)){ throw 'Backup destination directory not found' }
        $output=Join-Path $dest ('DevMove_'+(Get-Date -Format 'yyyyMMdd_HHmmss'))
        New-Item -ItemType Directory -Path $output -ErrorAction Stop | Out-Null
        Log ('Backup folder: '+$output)
        $manifest=[ordered]@{tool='DevMove Toolkit';version='1.0';date=(Get-Date).ToString('o');computer=$env:COMPUTERNAME;tasks=$tasks;note='No passwords or browser login sessions are migrated automatically. This may contain sensitive project files/SSH keys.'}
        $manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output 'manifest.json') -Encoding UTF8
    } elseif($mode -eq 'Restore') {
        $output=[string]$cfg.backupPath
        if(-not (Test-Path -LiteralPath (Join-Path $output 'manifest.json'))){ throw 'Select a backup folder containing manifest.json' }
        Log ('Restore source: '+$output)
    } else { throw 'Invalid mode' }
    Status 'Running' 'Starting'
    foreach($task in $tasks) {
        Status 'Running' $task
        Log ('Starting '+$mode+': '+$task)
        try {
            if($mode -eq 'Backup') { BackupTask $task } else { RestoreTask $task }
            Log ('Finished '+$task)
        } catch { $failed++; Log ('ERROR in '+$task+': '+$_.Exception.Message) }
        $completed++
        Status 'Running' $task
    }
    if($mode -eq 'Backup') {
        $summary=[ordered]@{date=(Get-Date).ToString('o');tasks=$tasks;failed=$failed;warnings=$warnings;output=$output}
        $summary | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'backup-result.json') -Encoding UTF8
        Copy-Item -LiteralPath $LogPath -Destination (Join-Path $output 'backup-log.txt') -Force
    }
    if($failed -gt 0 -or $warnings -gt 0){ Status 'CompletedWithWarnings' 'Done'; Log ('Finished with '+$failed+' failed task(s) and '+$warnings+' warning(s)') }
    else { Status 'Completed' 'Done'; Log 'Finished successfully' }
} catch {
    $failed++
    Log ('FATAL: '+$_.Exception.Message)
    Status 'Failed' 'Failed'
    exit 1
}
