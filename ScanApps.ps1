param([Parameter(Mandatory=$true)][string]$Output)
$ErrorActionPreference='Stop'
$dir=Split-Path -Parent $Output
New-Item -ItemType Directory -Path $dir -Force | Out-Null
$raw=Join-Path $dir 'winget-current.json'
$errors=@()
$wingetData=@()
try {
    & winget.exe export -o $raw --accept-source-agreements --disable-interactivity 2>&1 | Out-Null
    if(-not(Test-Path -LiteralPath $raw)){throw 'WinGet did not create a package export.'}
    $root=Get-Content -LiteralPath $raw -Raw | ConvertFrom-Json
    foreach($src in @($root.Sources)) {
        foreach($p in @($src.Packages)) {
            if($p.PackageIdentifier) {
                $wingetData+= [pscustomobject]@{name=[string]$p.PackageIdentifier;id=[string]$p.PackageIdentifier;source=[string]$src.Details.Name;version=[string]$p.Version;supported=$true}
            }
        }
    }
} catch { $errors+=('WinGet inventory: '+$_.Exception.Message) }
$uninstall=@('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*','HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*','HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*')
$installed=@()
foreach($path in $uninstall) {
    try {
        $installed+= Get-ItemProperty -Path $path -ErrorAction SilentlyContinue | Where-Object {
            $_.DisplayName -and $_.SystemComponent -ne 1 -and $_.ReleaseType -notmatch 'Update|Hotfix|Security'
        } | ForEach-Object { [pscustomobject]@{name=[string]$_.DisplayName;version=[string]$_.DisplayVersion} }
    } catch { }
}
$installed=@($installed | Sort-Object name -Unique)
$known=@{}
foreach($p in $wingetData){$known[$p.id.ToLowerInvariant()]=$true}
$items=@()
foreach($p in $wingetData) {
    $name=$p.name
    $end=($p.id -split '\.')[-1]
    if($end) {
        $candidate=@($installed | Where-Object { $_.name -replace '[^A-Za-z0-9]','' -like ('*'+($end -replace '[^A-Za-z0-9]','')+'*') } | Select-Object -First 1)
        if($candidate.Count -gt 0 -and $candidate[0].name.Length -lt 100){$name=$candidate[0].name}
    }
    $items+= [pscustomobject]@{name=$name;id=$p.id;source=$p.source;version=$p.version;supported=$true}
}
foreach($p in $installed) {
    $norm=($p.name -replace '[^A-Za-z0-9]','').ToLowerInvariant()
    $match=$false
    foreach($item in $items) {
        $tail=(($item.id -split '\.')[-1] -replace '[^A-Za-z0-9]','').ToLowerInvariant()
        if($tail.Length -ge 5 -and $norm.Contains($tail)){$match=$true;break}
    }
    if(-not $match) {
        $items+= [pscustomobject]@{name=$p.name;id=('MANUAL:'+($p.name -replace '[\r\n]',' '));source='Manual reinstall';version=$p.version;supported=$false}
    }
}
$final=[ordered]@{scanDate=(Get-Date).ToString('o');matched=@($items|Where-Object {$_.supported}).Count;manual=@($items|Where-Object {-not $_.supported}).Count;errors=$errors;apps=@($items|Sort-Object name)}
$final|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $Output -Encoding UTF8
Write-Output ('MATCHED='+$final.matched+' MANUAL='+$final.manual+' ERROR='+($errors -join ' | '))
